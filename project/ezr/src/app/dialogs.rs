//! dialogs — 对话框确认流（添加/删除按钮激活、任务创建、文件删除）

use super::{App, Dialog, DialogKind, CHECKSUM_ALGOS};
use crate::engine::Cmd;
use crate::model::sidecar::Sidecar;
use crate::model::{checksum, namegen, unix_now, Checksum, Protocol, Task, TaskState};

impl App {
    pub(super) fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            6 => self.dlg_confirm_add(),
            7 => self.dialog = None,
            _ => {}
        }
    }

    /// 修改任务按钮激活（v1.5/FR-01-87：4=确认 5=取消）
    pub(super) fn dlg_activate_modify(&mut self, btn: usize) {
        match btn {
            4 => self.dlg_confirm_modify(),
            5 => self.dialog = None,
            _ => {}
        }
    }

    /// 确认修改（v1.5/FR-01-87，D19/D20）：并发（空=保持，非法钳制 1–64）、
    /// 校验（空=清除，非法聚焦提示不生效）、代理（下拉选择）；立即生效
    /// = 引擎 Reconfigure（运行中）+ 任务字段更新 + 注册表落盘 + toast
    pub(super) fn dlg_confirm_modify(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let Some(id) = d.task_id else {
            return;
        };
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            self.dialog = None;
            return;
        };
        let conns_raw = d.conns.trim().to_string();
        let ck_raw = d.ck_value.trim().to_string();
        let ck_type = d.ck_type;
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
        let proxy = self.proxy_options[proxy_sel].clone();

        // 并发：空 = 保持当前；非空按 1–64 钳制（口径同 QA-PC-04 裁决）
        let concurrency = if conns_raw.is_empty() {
            self.tasks[idx].concurrency
        } else {
            conns_raw.parse::<usize>().unwrap_or(0).clamp(1, 64)
        };

        // 校验：空 = 清除（03-modify-task-05）；非空同添加对话框合法性口径，
        // 非法 → 不生效 + 聚焦校验码字段（03-modify-task-06）
        let checksum = if ck_raw.is_empty() {
            None
        } else {
            match checksum::validate_value(ck_type, &ck_raw) {
                Ok(v) => Some(Checksum {
                    algo: CHECKSUM_ALGOS[ck_type].0,
                    value: v,
                }),
                Err(e) => {
                    let algo = CHECKSUM_ALGOS[ck_type].0;
                    self.set_toast(format!("⚠ {algo} {e}"));
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = 2;
                    }
                    return;
                }
            }
        };

        let (endpoint, missing) = self.cfg.resolve_proxy(&proxy);
        if missing {
            if let crate::model::ProxyChoice::Named(name) = &proxy {
                self.set_toast(format!("⚠ 引用的代理「{name}」不存在，已按直连"));
            }
        }

        // 应用任务字段（DownloadDone 前更新 ⇒ 完成校验用最新值，D19）
        {
            let t = &mut self.tasks[idx];
            t.concurrency = concurrency;
            t.checksum = checksum;
            t.proxy = proxy;
        }
        self.dialog = None;
        // 运行中任务热调（未知/已停句柄引擎侧静默忽略；排队/暂停任务在
        // 下次 Start 经 make_spec 按新值生效）
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine
                .send(Cmd::Reconfigure {
                    id,
                    concurrency,
                    endpoint,
                })
                .await;
        });
        self.set_toast("✓ 任务参数已更新，立即生效");
        self.save_registry();
    }

    /// 确认添加（FR-01-01/03/04/05/26：URL 校验、目录默认、并发钳制、校验码校验、断点接续）
    pub(super) fn dlg_confirm_add(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let url = d.url.trim().to_string();
        if url.is_empty() {
            self.set_toast("⚠ 请输入下载 URL");
            return;
        }
        // FR-01-05：仅接受 http:// / https://
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.set_toast("⚠ 仅支持 http:// 或 https:// 链接");
            return;
        }
        let dir_raw = d.dir.trim().to_string();
        let conns_raw = d.conns.trim().to_string();
        let ck_raw = d.ck_value.trim().to_string();
        let ck_type = d.ck_type;

        // 校验码（可留空 = 不校验；输入统一小写，位数与算法匹配）
        let checksum = if ck_raw.is_empty() {
            None
        } else {
            match checksum::validate_value(ck_type, &ck_raw) {
                Ok(v) => Some(Checksum {
                    algo: CHECKSUM_ALGOS[ck_type].0,
                    value: v,
                }),
                Err(e) => {
                    let algo = CHECKSUM_ALGOS[ck_type].0;
                    self.set_toast(format!("⚠ {algo} {e}"));
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = 4;
                    }
                    return;
                }
            }
        };

        // 保存目录（FR-01-03）：留空 = 配置 download_dir；缺省 = ~/Downloads；
        // 统一去尾斜杠（trim 幂等，归一一次全函数共用）
        let dir = if dir_raw.is_empty() {
            self.cfg
                .download_dir
                .clone()
                .unwrap_or_else(crate::model::config::default_download_dir)
        } else {
            dir_raw
        };
        let dir = dir.trim_end_matches('/').to_string();

        let protocol = if url.starts_with("https://") {
            Protocol::Https
        } else {
            Protocol::Http
        };
        // 重复任务拒绝（Gherkin 01-add-task-16）：同 URL 同保存目录已在列表 →
        // toast「任务已存在」，不创建新任务（对话框保持打开，与其他校验失败一致）
        if self.tasks.iter().any(|t| t.url == url && t.save_dir == dir) {
            self.set_toast("⚠ 任务已存在");
            return;
        }
        let base_name = namegen::derive_name(None, None, &url);
        let id = self.next_id;
        // FR-01-26 断点自动接续：既有 sidecar 同 URL 同路径 → 接续原名
        let resumed = Sidecar::load(&format!("{dir}/{base_name}.ezr"))
            .is_some_and(|sc| sc.url == url)
            || self.tasks.iter().any(|t| {
                t.url == url
                    && t.save_dir == dir
                    && t.name == base_name
                    && t.state != TaskState::Completed
            });
        let name = if resumed {
            base_name.clone()
        } else {
            // 重名检测：任务表 + 盘上（文件/.downloading/sidecar）→ 自动追加序号
            let tasks = &self.tasks;
            namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir)
                    || namegen::exists_on_disk(&dir, n)
            })
        };
        // 并发数（FR-01-04）：非 1–64 范围（留空/0/65/非数字）一律回退配置默认
        // （Gherkin 01-add-task-06：留空/0/65/abc → 均回退默认 4，非钳制）
        let conns = match conns_raw.parse::<usize>() {
            Ok(v) if (1..=64).contains(&v) => v,
            _ => self.cfg.http_concurrency,
        };
        self.next_id += 1;
        // 校验值来源②（FR-01-50/D14）：未显式提供时查保存目录伴随文件
        let checksum = Self::resolve_checksum(&dir, &name, checksum);
        let ts = unix_now();
        let proxy = self.proxy_options[d.proxy_sel.min(self.proxy_options.len() - 1)].clone();
        let t = Task::new_queued(
            id,
            name.clone(),
            protocol,
            url.clone(),
            dir,
            self.cfg.block_size_http,
            conns,
            self.cfg.max_retries,
            checksum,
            proxy,
            ts,
        );
        self.tasks.push(t);
        self.dialog = None;
        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }
        if resumed {
            self.set_toast(format!(
                "✓ 已添加任务 #{id}: {name}（发现有效断点，将自动接续）"
            ));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}"));
        }
        self.save_registry();
    }

    pub(super) fn dlg_activate_delete(&mut self, btn: usize) {
        let Some(idx) = self.sel_idx() else {
            self.dialog = None;
            return;
        };
        let (id, name) = {
            let t = &self.tasks[idx];
            (t.id, t.name.clone())
        };
        match btn {
            0 => {
                // 仅删除任务：文件与 sidecar 保留（FR-01-25）；引擎任务取消（停写 sidecar）
                self.remove_task(idx, id, false);
                self.set_toast(format!("🗑 已删除任务（保留文件）: {name}"));
                self.dialog = None; // 操作完成即关闭对话框
            }
            1 => {
                // 删除任务和文件：记录即时删除；文件在引擎 Cancelled 确认后删除（防重建竞态）
                self.remove_task(idx, id, true);
                self.set_toast(format!("🗑 已删除任务，正在停止引擎并清理本地文件: {name}"));
                self.dialog = None; // 操作完成即关闭对话框
            }
            _ => {
                self.dialog = None;
            }
        }
        let flen = self.filtered().len();
        if self.selected >= flen {
            self.selected = flen.saturating_sub(1);
        }
    }

    /// 从任务表移除并取消引擎任务（删除对话框两路共用）。
    /// `delete_files` = true 时登记延迟删除：引擎回报 [`Evt::Cancelled`]（supervisor
    /// 已完全退出、不再写盘）后再删目标文件/.downloading/.ezr，避免与引擎在途
    /// 写入竞态导致 sidecar/文件被重新创建。
    pub(super) fn remove_task(&mut self, idx: usize, id: u32, delete_files: bool) {
        let (save_dir, name) = {
            let t = &self.tasks[idx];
            (t.save_dir.clone(), t.name.clone())
        };
        self.tasks.remove(idx);
        self.windows.remove(&id);
        if delete_files {
            self.pending_deletes.insert(id, (save_dir, name));
        }
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine.send(Cmd::Cancel { id }).await;
        });
        self.save_registry();
    }

    /// 删除任务的三类本地文件：目标文件、.downloading、.ezr sidecar（FR-01-25「删除任务和文件」）
    pub(super) fn delete_task_files(save_dir: &str, name: &str) {
        let base = save_dir.trim_end_matches('/');
        for path in [
            format!("{base}/{name}"),
            format!("{base}/{name}.downloading"),
            format!("{base}/{name}.ezr"),
        ] {
            let _ = std::fs::remove_file(&path);
        }
    }

    /// 打开添加对话框（空预填；URL/目录留空由用户输入）
    pub fn open_add_dialog(&mut self) {
        self.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 3,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 3,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
        });
    }

    /// 打开修改任务对话框（v1.5/FR-01-87）：四字段预填当前值；
    /// 已完成任务拒绝（D20，toast 提示）；无选中任务 toast 提示
    pub fn open_modify_dialog(&mut self) {
        // 先取选中任务的 owned 快照（借用期只读任务表；选项表追加在快照后）
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, checksum, proxy) = (
            t.id,
            t.name.clone(),
            t.concurrency,
            t.checksum.clone(),
            t.proxy.clone(),
        );
        let ck_type = CHECKSUM_ALGOS
            .iter()
            .position(|(n, _, _)| checksum.as_ref().is_some_and(|c| c.algo == *n))
            .unwrap_or(3);
        // 选项表未收录时追加兜底项保证回显一致（命名代理被从配置删除的
        // 存量任务；正常流都在表内）
        let proxy_sel = match self.proxy_options.iter().position(|o| o == &proxy) {
            Some(i) => i,
            None => {
                self.proxy_options.push(proxy.clone());
                if let crate::model::ProxyChoice::Named(n) = &proxy {
                    self.proxy_labels.push(n.clone());
                }
                self.proxy_options.len() - 1
            }
        };
        self.dialog = Some(Dialog {
            kind: DialogKind::Modify,
            url: String::new(),
            dir: String::new(),
            conns: concurrency.to_string(),
            conns_edited: false,
            ck_type,
            ck_value: checksum.map(|c| c.value).unwrap_or_default(),
            ck_open: false,
            ck_sel: ck_type,
            proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: Some(id),
        });
    }

    /// 打开删除对话框（三选：仅任务/任务和文件/取消）
    pub fn open_delete_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        self.dialog = Some(Dialog {
            kind: DialogKind::Delete,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 0,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: t.name.clone(),
            task_id: None,
        });
    }

    // -----------------------------------------------------------------------
    // 鼠标交互（沿用 demo：点击选中 / 滚轮 / 对话框按钮与字段）
    // -----------------------------------------------------------------------
}

#[cfg(test)]
mod add_dialog_flow_tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::model::config::Config;

    /// 添加对话框全流程：输入 URL/目录 → Enter 确认 → 任务入列；
    /// 保存目录尾斜杠归一（FR-01-03，目录归一单点化的行为锚定）
    #[tokio::test]
    async fn add_dialog_confirm_creates_task_with_normalized_dir() {
        let dir = std::env::temp_dir().join(format!("ezr-app-dlg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);

        // A 打开添加对话框
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.dialog.is_some());
        // 输入 URL（focus 0）
        for c in "http://example.com/f.bin".chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        // Tab 到目录字段，输入带尾斜杠的目录
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        for c in format!("{}/", dir.to_string_lossy()).chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        // Enter（非算法/取消焦点）= 确认添加
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.dialog.is_none());
        assert_eq!(app.tasks.len(), 1);
        let t = &app.tasks[0];
        assert_eq!(t.name, "f.bin");
        assert_eq!(t.save_dir, dir.to_string_lossy());
        assert_eq!(t.state, TaskState::Queued);
        assert_eq!(
            t.concurrency,
            crate::model::config::DEFAULT_HTTP_CONCURRENCY
        );
        assert!(app
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("已添加任务")));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.6/FR-01-86/90（D18 两态）：添加对话框代理下拉 = 直连 + 命名条目
    ///（含类型标注，v1.9 三值 http/https/socks5），默认选中恒直连；确认后写入任务
    #[tokio::test]
    async fn add_dialog_proxy_options_two_states() {
        let dir = std::env::temp_dir().join(format!("ezr-dlg-pdef-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut cfg = Config::from_toml(
            // v1.9/FR-01-86/89：夹具按 ip/port/type 字段形态（url 键退役）；
            // v1.8/FR-01-93：socks5 型凭证必填，夹具补齐 username/password 保持
            // 「合法条目」意图（a/b/c 显示为带类型标注的命名代理）
            "[[proxies]]\nname = \"a\"\ntype = \"socks5\"\nip = \"a\"\nport = 2\nusername = \"u\"\npassword = \"p\"\n[[proxies]]\nname = \"b\"\ntype = \"http\"\nip = \"b\"\nport = 3\n[[proxies]]\nname = \"c\"\ntype = \"https\"\nip = \"c\"\nport = 4\n",
        );
        cfg.warnings.clear();
        let mut app = App::new(cfg, reg);
        assert_eq!(
            app.proxy_options.len(),
            4,
            "直连 + 命名 a + 命名 b + 命名 c"
        );
        assert_eq!(app.proxy_labels[0], "直连");
        assert_eq!(
            app.proxy_labels[1], "a（socks5）",
            "命名条目显示名带类型标注（FR-01-89）"
        );
        assert_eq!(app.proxy_labels[2], "b（http）");
        assert_eq!(
            app.proxy_labels[3], "c（https）",
            "https 类型标注（v1.9 三值）"
        );
        app.open_add_dialog();
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.proxy_sel, 0, "默认选中恒「直连」（v1.6 两态）");
        app.dialog.as_mut().unwrap().url = "http://example.com/p.bin".to_string();
        app.dlg_confirm_add();
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(
            app.tasks[0].proxy,
            crate::model::ProxyChoice::Direct,
            "默认直连写入任务"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();

        // 空配置 → 下拉仅「直连」
        let reg2 = dir.join("registry2.json").to_string_lossy().to_string();
        let mut app2 = App::new(Config::default(), reg2);
        app2.open_add_dialog();
        assert_eq!(app2.proxy_options.len(), 1, "仅直连");
        assert_eq!(app2.dialog.as_ref().unwrap().proxy_sel, 0);
        app2.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.5/FR-01-87：m 修改对话框——预填当前值、确定后立即生效（并发/
    /// 校验/代理写入任务）+ toast + 注册表落盘；已完成任务拒绝（D20）
    #[tokio::test]
    async fn modify_dialog_prefills_applies_and_rejects_completed() {
        let dir = std::env::temp_dir().join(format!("ezr-dlg-mod-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut cfg = Config::from_toml(
            // v1.9/FR-01-86/89：夹具按 ip/port/type 字段形态（url 键退役）
            "[[proxies]]\nname = \"a\"\ntype = \"http\"\nip = \"a\"\nport = 2\n[[proxies]]\nname = \"b\"\ntype = \"http\"\nip = \"b\"\nport = 3\n",
        );
        cfg.warnings.clear();
        let mut app = App::new(cfg, reg);
        app.tasks.push(crate::model::Task::new_queued(
            1,
            "m.bin".to_string(),
            Protocol::Http,
            "http://example.com/m.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            2,
            5,
            Some(Checksum {
                algo: "SHA-256",
                value: "a".repeat(64),
            }),
            crate::model::ProxyChoice::Direct,
            0,
        ));
        app.selected = 0;
        // 按 m 打开：预填并发 2 / SHA-256 / 校验码 / 直连
        app.on_key(KeyCode::Char('m'), KeyModifiers::NONE);
        let d = app.dialog.as_ref().expect("m 打开修改对话框");
        assert_eq!(d.kind, DialogKind::Modify);
        assert_eq!(d.conns, "2");
        assert_eq!(d.ck_type, 3, "SHA-256 下标");
        assert_eq!(d.ck_value, "a".repeat(64));
        assert_eq!(d.proxy_sel, 0, "任务为 Direct → 预选直连");
        // 改并发 6 + 清空校验码 + 代理切命名 a → 确定
        let d = app.dialog.as_mut().unwrap();
        d.conns = "6".to_string();
        d.ck_value.clear();
        d.proxy_sel = 1;
        app.dlg_confirm_modify();
        assert!(app.dialog.is_none());
        let t = &app.tasks[0];
        assert_eq!(t.concurrency, 6, "并发立即生效");
        assert!(t.checksum.is_none(), "清空校验码 = 清除校验");
        assert_eq!(t.proxy, crate::model::ProxyChoice::Named("a".into()));
        assert!(app
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("任务参数已更新")));
        assert!(dir.join("registry.json").exists(), "注册表已落盘");
        app.shutdown().await;

        // 已完成任务按 m 拒绝（D20）
        let reg2 = dir.join("registry2.json").to_string_lossy().to_string();
        let mut app2 = App::new(Config::default(), reg2);
        app2.tasks.push(crate::model::Task::new_queued(
            1,
            "c.bin".to_string(),
            Protocol::Http,
            "http://example.com/c.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            5,
            None,
            crate::model::ProxyChoice::Direct,
            0,
        ));
        app2.tasks[0].state = TaskState::Completed;
        app2.selected = 0;
        app2.filter = 1; // 「已完成」页签（D20：已完成任务按 m 拒绝的场景入口）
        app2.on_key(KeyCode::Char('m'), KeyModifiers::NONE);
        assert!(app2.dialog.is_none(), "已完成任务不弹对话框");
        assert!(app2
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("不可修改")));
        app2.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
