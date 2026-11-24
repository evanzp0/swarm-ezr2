use std::path::Path;

use super::App;
use crate::engine::Cmd;
use crate::model::config::{Config, ProxyChoice};
use crate::model::TaskState;

pub(crate) const TOAST_RELOADED: &str = "✓ 配置已重新加载";
pub(crate) const TOAST_NO_CONFIG: &str = "无配置文件";
pub(crate) const TOAST_FAIL_PREFIX: &str = "⚠ 配置热加载失败：";

impl App {
    /// Ctrl+R 入口（keys CONTROL 分支，全局可达与 Ctrl+C 同级）。
    pub fn reload_config(&mut self) {
        let path = crate::model::config::config_path();
        self.reload_config_from(path.as_deref());
    }

    /// 热加载核心（FR-01-119）：路径与流程臂——
    /// ① config_path 缺失（无主目录环境）→ toast「无配置文件」，配置不变；
    /// ② try_load 失败（D 类原子保护）→ toast `⚠ 配置热加载失败：<明细>`（单行化），
    ///    旧配置原样保留；③ 成功 → apply_reloaded（含缺失文件臂 = 默认配置整体热加载）。
    pub(crate) fn reload_config_from(&mut self, path: Option<&Path>) {
        let Some(path) = path else {
            self.set_toast(TOAST_NO_CONFIG);
            return;
        };
        match Config::try_load(path) {
            Ok(cfg) => self.apply_reloaded(cfg),
            Err(msg) => self.set_toast(format!("{}{}", TOAST_FAIL_PREFIX, single_line(&msg))),
        }
    }

    fn apply_reloaded(&mut self, new_cfg: Config) {
        let old_cfg = std::mem::replace(&mut self.cfg, new_cfg);
        // CLI --max-speed 覆盖贯穿热加载重应用（FR-01-119⑥，CLI > config 文件）。
        if let Some(ms) = self.cli_max_speed_override {
            self.cfg.global_max_download_speed = ms;
        }
        self.max_slots = self.cfg.download_slots;

        // 引用消失判定（FR-01-119⑤/D82）：旧池有、新池无 = 删除或改名统一臂。
        let vanished: Vec<String> = old_cfg
            .proxies
            .iter()
            .filter(|p| !self.cfg.proxies.iter().any(|n| n.name == p.name))
            .map(|p| p.name.clone())
            .collect();

        let mut rewrites = 0usize;
        let mut reconfigs: Vec<Cmd> = Vec::new();
        for t in &mut self.tasks {
            let old_choice = t.proxy.clone();
            if let ProxyChoice::Named(n) = &old_choice {
                if vanished.iter().any(|v| v == n) {
                    t.proxy = ProxyChoice::Direct;
                    rewrites += 1;
                }
            }
            if matches!(t.state, TaskState::Downloading | TaskState::Seeding) {
                let old_ep = old_cfg.resolve_proxy(&old_choice).0;
                let new_ep = self.cfg.resolve_proxy(&t.proxy).0;
                if old_ep != new_ep {
                    reconfigs.push(reconfigure_cmd(t, new_ep));
                }
            }
        }
        if rewrites > 0 {
            self.save_registry();
        }

        // 代理下拉重建（添加与修改对话框共用）：直连 + 新池按配置顺序；
        // 浮层打开时同步重建不关浮层——已选名仍在池保持选中，消失回退直连（D81⑤/D82①）。
        let held: Option<ProxyChoice> = self.dialog.as_ref().map(|d| {
            let i = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
            self.proxy_options[i].clone()
        });
        self.proxy_options = vec![ProxyChoice::Direct];
        self.proxy_labels = vec!["直连".to_string()];
        for p in &self.cfg.proxies {
            self.proxy_options.push(ProxyChoice::Named(p.name.clone()));
            self.proxy_labels
                .push(format!("{}（{}）", p.name, p.kind.label()));
        }
        self.default_proxy_sel = 0;
        if let Some(choice) = held {
            if let Some(d) = self.dialog.as_mut() {
                d.proxy_sel = self
                    .proxy_options
                    .iter()
                    .position(|o| o == &choice)
                    .unwrap_or(0);
            }
        }

        // 引擎下发：全局限速立即重定目标（A 类）+ burst/sleep 原子更新（C 类，
        // 新启动任务生效、运行中桶参数不动）+ 逐任务代理热切（有变化才发命令，
        // 存量连接不打断）。
        let engine = self.engine.clone();
        let globals = Cmd::ApplyGlobals {
            max_download_speed: self.cfg.global_max_download_speed,
            max_upload_speed: self.cfg.global_max_upload_speed,
            throttle_burst_secs: self.cfg.throttle_burst_secs,
            throttle_sleep_ms: self.cfg.throttle_sleep_ms,
        };
        tokio::spawn(async move {
            engine.send(globals).await;
            for c in reconfigs {
                engine.send(c).await;
            }
        });

        // 成功反馈（v1.57/D81②）：恒为单条固定文案，不追加任何可选段。
        self.set_toast(TOAST_RELOADED);
    }
}

fn reconfigure_cmd(
    t: &crate::model::Task,
    endpoint: Option<crate::model::config::ProxyEndpoint>,
) -> Cmd {
    if t.protocol.is_bt() {
        let bt = t.bt.as_ref();
        Cmd::ReconfigureBt {
            id: t.id,
            concurrency: t.concurrency,
            files: bt.map(|b| b.files.clone()).unwrap_or_default(),
            proxy_endpoint: endpoint,
            max_download_speed: t.max_download_speed,
            max_upload_speed: t.max_upload_speed,
            sequential: bt.is_some_and(|b| b.sequential),
        }
    } else {
        Cmd::Reconfigure {
            id: t.id,
            concurrency: t.concurrency,
            endpoint,
            max_download_speed: t.max_download_speed,
        }
    }
}

fn single_line(msg: &str) -> String {
    msg.split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("；")
}

#[cfg(test)]
mod reload_tests {
    use std::path::PathBuf;

    use super::*;
    use crate::engine::EngineHandle;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-reload", tag)
    }

    fn write_cfg(dir: &std::path::Path, text: &str) -> PathBuf {
        let p = dir.join("config.toml");
        std::fs::write(&p, text).unwrap();
        p
    }

    fn with_proxies(names: &[(&str, u16)]) -> String {
        let mut s = String::from("[common]\ndownload_slots = 5\n\n");
        for (n, port) in names {
            s.push_str(&format!("[[common.proxy]]\nname = \"{n}\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = {port}\n\n"));
        }
        s
    }

    async fn recv_all(rx: &mut tokio::sync::mpsc::Receiver<Cmd>, n: usize) -> Vec<Cmd> {
        let mut out = Vec::new();
        for _ in 0..n {
            let c = tokio::time::timeout(std::time::Duration::from_millis(800), rx.recv())
                .await
                .expect("引擎命令应在超时前到达")
                .expect("发送端存活");
            out.push(c);
        }
        out
    }

    #[tokio::test]
    async fn no_config_path_toasts_no_config_file_and_keeps_cfg() {
        let mut app = make_app("none");
        app.reload_config_from(None);
        assert_eq!(app.toast.as_deref(), Some("无配置文件"));
        assert_eq!(
            app.cfg.download_slots,
            crate::model::config::DEFAULT_DOWNLOAD_SLOTS
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn success_toast_is_fixed_text_and_slots_apply() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-ok");
        let p = write_cfg(&dir, "[common]\ndownload_slots = 3\n");
        let mut app = make_app("ok");
        app.max_slots = 5;
        app.reload_config_from(Some(&p));
        assert_eq!(app.toast.as_deref(), Some("✓ 配置已重新加载"));
        assert_eq!(app.max_slots, 3, "download_slots 即时生效（队列栏 0/3）");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn invalid_key_rejects_atomically_and_single_line_toast() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-bad");
        let p = write_cfg(&dir, "[common]\ndownload_slots = \"abc\"\n");
        let mut app = make_app("bad");
        let cfg_before = app.cfg.clone();
        app.reload_config_from(Some(&p));
        let toast = app.toast.clone().unwrap();
        assert!(
            toast.starts_with("⚠ 配置热加载失败："),
            "失败 toast 前缀：{toast}"
        );
        assert!(
            toast.contains("键 common.download_slots"),
            "复用 try_load 逐键报错文案：{toast}"
        );
        assert!(!toast.contains('\n'), "单行显示无换行：{toast}");
        assert_eq!(app.cfg, cfg_before, "旧配置原样保留（无半新半旧）");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn parse_error_single_line() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-parse");
        let p = write_cfg(&dir, "not [ toml");
        let mut app = make_app("parse");
        app.reload_config_from(Some(&p));
        let toast = app.toast.clone().unwrap();
        assert!(
            toast.starts_with("⚠ 配置热加载失败：配置文件解析失败："),
            "{toast}"
        );
        assert!(!toast.contains('\n'), "解析失败两段拼接也单行化：{toast}");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn missing_file_applies_defaults_without_regenerating_template() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-miss");
        let p = dir.join("config.toml");
        std::fs::write(&p, "[common]\ndownload_slots = 2\n").unwrap();
        let mut app = make_app("miss");
        app.max_slots = 2;
        std::fs::remove_file(&p).unwrap();
        app.reload_config_from(Some(&p));
        assert_eq!(app.toast.as_deref(), Some("✓ 配置已重新加载"));
        assert_eq!(
            app.max_slots,
            crate::model::config::DEFAULT_DOWNLOAD_SLOTS,
            "默认配置整体热加载（download_slots 回默认）"
        );
        assert!(
            !p.exists(),
            "热加载不重生成模板（模板生成属启动逻辑 FR-01-85）"
        );
        assert!(app.proxy_options.len() == 1, "默认配置代理池空，下拉仅直连");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_override_survives_reload() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-cli");
        let p = write_cfg(&dir, "[common]\nglobal_max_download_speed = \"1 MiB/s\"\n");
        let mut app = make_app("cli");
        app.set_cli_max_speed_override(262_144);
        app.reload_config_from(Some(&p));
        assert_eq!(
            app.cfg.global_max_download_speed, 262_144,
            "CLI 覆盖贯穿热加载重应用，不被文件值冲掉"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn vanished_proxy_rewrites_task_to_direct_and_persists() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-vanish");
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081), ("p2", 18082)]));
        let mut app = App::new(Config::try_load(&p).unwrap(), reg.clone());
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());
        app.tasks.push(crate::app::testutil::dl_task(2, "b.bin"));
        app.tasks[1].proxy = ProxyChoice::Named("p2".into());

        let p2 = write_cfg(&dir, &with_proxies(&[("p2", 18082)]));
        app.reload_config_from(Some(&p2));

        assert_eq!(
            app.toast.as_deref(),
            Some("✓ 配置已重新加载"),
            "引用消失不追加专属提示段"
        );
        assert_eq!(
            app.tasks[0].proxy,
            ProxyChoice::Direct,
            "引用 p1 的任务自动转直连"
        );
        assert_eq!(
            app.tasks[1].proxy,
            ProxyChoice::Named("p2".into()),
            "引用 p2 的任务不变"
        );
        assert_eq!(app.proxy_options.len(), 2, "下拉重建 = 直连 + p2");
        assert_eq!(app.proxy_labels[1], "p2（http）");
        let saved = std::fs::read_to_string(&reg).unwrap();
        assert!(
            saved.contains("\"proxy\": \"direct\"") && !saved.contains("named:p1"),
            "任务级代理选择随注册表落库（重启不复活旧名）：{saved}"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn rename_arm_new_name_enters_pool_without_migrating_refs() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-rename");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081), ("p2", 18082)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());

        let p2 = write_cfg(&dir, &with_proxies(&[("p1x", 18081), ("p2", 18082)]));
        app.reload_config_from(Some(&p2));
        assert_eq!(app.toast.as_deref(), Some("✓ 配置已重新加载"));
        assert_eq!(
            app.tasks[0].proxy,
            ProxyChoice::Direct,
            "旧名消失任务转直连"
        );
        assert_eq!(app.proxy_labels.len(), 3, "直连 + p1x + p2（配置顺序）");
        assert_eq!(app.proxy_labels[1], "p1x（http）");
        assert!(
            !app.proxy_labels.iter().any(|l| l.starts_with("p1（")),
            "旧名不再出现"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn readd_same_name_does_not_rehook_rewritten_task() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-readd");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());

        let p2 = write_cfg(&dir, "[common]\ndownload_slots = 5\n");
        app.reload_config_from(Some(&p2));
        assert_eq!(app.tasks[0].proxy, ProxyChoice::Direct);

        let p3 = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        app.reload_config_from(Some(&p3));
        assert_eq!(
            app.tasks[0].proxy,
            ProxyChoice::Direct,
            "重加同名不复挂（选择已落库直连，不自动迁移）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn open_add_dialog_dropdown_rebuilds_in_place() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-dlg");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081), ("p2", 18082)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.open_add_dialog();
        {
            let d = app.dialog.as_mut().unwrap();
            d.proxy_sel = 2;
        }

        let p2 = write_cfg(&dir, &with_proxies(&[("p2", 18082)]));
        app.reload_config_from(Some(&p2));
        assert!(app.dialog.is_some(), "浮层不因热加载关闭");
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.proxy_sel, 1, "已选 p2 仍在池 → 保持选中（重建后下标 1）");
        assert_eq!(app.proxy_options.len(), 2);

        let p3 = write_cfg(&dir, &with_proxies(&[("p3", 18083)]));
        app.reload_config_from(Some(&p3));
        assert_eq!(
            app.dialog.as_ref().unwrap().proxy_sel,
            0,
            "已选 p2 消失 → 回退直连选中"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn open_modify_dialog_remaps_selection_and_drops_vanished_name() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-mdlg");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());
        app.tasks[0].state = TaskState::Paused;
        app.selected = 0;
        app.open_modify_dialog();
        assert_eq!(app.dialog.as_ref().unwrap().proxy_sel, 1);

        let p2 = write_cfg(&dir, &with_proxies(&[("p2", 18082)]));
        app.reload_config_from(Some(&p2));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.proxy_sel, 0, "旧名消失 → 预填直连（不被重新注入，D82①）");
        assert_eq!(app.proxy_options.len(), 2, "选项 = 直连 + p2");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn endpoint_change_on_running_http_task_sends_reconfigure() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-ep");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());
        app.tasks[0].concurrency = 7;
        app.tasks[0].max_download_speed = 1234;

        let (spy_tx, mut spy_rx) = tokio::sync::mpsc::channel::<Cmd>(16);
        app.engine = EngineHandle::from_sender(spy_tx);

        let p2 = write_cfg(&dir, &with_proxies(&[("p1", 18082)]));
        app.reload_config_from(Some(&p2));
        let cmds = recv_all(&mut spy_rx, 2).await;
        match &cmds[0] {
            Cmd::ApplyGlobals { .. } => {}
            other => panic!("首个命令应为 ApplyGlobals：{other:?}"),
        }
        match &cmds[1] {
            Cmd::Reconfigure {
                id,
                concurrency,
                endpoint,
                max_download_speed,
            } => {
                assert_eq!(*id, 1);
                assert_eq!(*concurrency, 7, "并发原值不变");
                assert_eq!(*max_download_speed, 1234, "限速原值不变");
                let ep = endpoint.as_ref().expect("新端点应下发");
                assert_eq!(ep.url, "http://127.0.0.1:18082");
            }
            other => panic!("应为 Reconfigure：{other:?}"),
        }
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn unchanged_endpoint_sends_no_reconfigure_and_toast_still_fixed() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-noep");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());
        app.tasks[0].state = TaskState::Downloading;

        let (spy_tx, mut spy_rx) = tokio::sync::mpsc::channel::<Cmd>(16);
        app.engine = EngineHandle::from_sender(spy_tx);

        let p2 = write_cfg(&dir, "[common]\ndownload_slots = 5\n\n[[common.proxy]]\nname = \"p1\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 18081\n");
        app.reload_config_from(Some(&p2));
        let cmds = recv_all(&mut spy_rx, 1).await;
        assert!(
            matches!(cmds[0], Cmd::ApplyGlobals { .. }),
            "端点无变化仅发全局命令，不发 Reconfigure"
        );
        tokio::time::timeout(std::time::Duration::from_millis(150), spy_rx.recv())
            .await
            .expect_err("不应有第二条命令");
        assert_eq!(app.toast.as_deref(), Some("✓ 配置已重新加载"));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn paused_task_with_changed_endpoint_gets_no_command_until_start() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-paused");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks.push(crate::app::testutil::dl_task(1, "a.bin"));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());
        app.tasks[0].state = TaskState::Paused;

        let (spy_tx, mut spy_rx) = tokio::sync::mpsc::channel::<Cmd>(16);
        app.engine = EngineHandle::from_sender(spy_tx);

        let p2 = write_cfg(&dir, &with_proxies(&[("p1", 18082)]));
        app.reload_config_from(Some(&p2));
        let cmds = recv_all(&mut spy_rx, 1).await;
        assert!(
            matches!(cmds[0], Cmd::ApplyGlobals { .. }),
            "仅运行中任务逐个 re-resolve 发命令"
        );
        tokio::time::timeout(std::time::Duration::from_millis(150), spy_rx.recv())
            .await
            .expect_err("已暂停任务不发切换命令（下次启动生效）");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn running_bt_vanished_proxy_reconfigures_to_direct() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-bt");
        let p = write_cfg(&dir, &with_proxies(&[("p1", 18081)]));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(Config::try_load(&p).unwrap(), reg);
        app.tasks
            .push(crate::app::testutil::bt_task(1, TaskState::Seeding));
        app.tasks[0].proxy = ProxyChoice::Named("p1".into());

        let (spy_tx, mut spy_rx) = tokio::sync::mpsc::channel::<Cmd>(16);
        app.engine = EngineHandle::from_sender(spy_tx);

        let p2 = write_cfg(&dir, "[common]\ndownload_slots = 5\n");
        app.reload_config_from(Some(&p2));
        assert_eq!(app.tasks[0].proxy, ProxyChoice::Direct);
        let cmds = recv_all(&mut spy_rx, 2).await;
        match &cmds[1] {
            Cmd::ReconfigureBt {
                id,
                proxy_endpoint,
                sequential,
                ..
            } => {
                assert_eq!(*id, 1);
                assert!(proxy_endpoint.is_none(), "直连端点");
                assert!(!*sequential, "sequential 原值不变");
            }
            other => panic!("BT 任务应发 ReconfigureBt：{other:?}"),
        }
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn apply_globals_carries_new_throttle_params() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reload-glob");
        let p = write_cfg(
            &dir,
            "[common]\nglobal_max_download_speed = \"512 KiB/s\"\nglobal_max_upload_speed = \"128 KiB/s\"\n\n[http]\nthrottle_burst_secs = 1.5\nthrottle_sleep_ms = 40\n",
        );
        let mut app = make_app("glob");
        let (spy_tx, mut spy_rx) = tokio::sync::mpsc::channel::<Cmd>(16);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.reload_config_from(Some(&p));
        let cmds = recv_all(&mut spy_rx, 1).await;
        match &cmds[0] {
            Cmd::ApplyGlobals {
                max_download_speed,
                max_upload_speed,
                throttle_burst_secs,
                throttle_sleep_ms,
            } => {
                assert_eq!(*max_download_speed, 512 * 1024);
                assert_eq!(*max_upload_speed, 128 * 1024);
                assert!((*throttle_burst_secs - 1.5).abs() < f64::EPSILON);
                assert_eq!(*throttle_sleep_ms, 40);
            }
            other => panic!("{other:?}"),
        }
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
