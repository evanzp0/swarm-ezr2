use crossterm::event::{KeyCode, KeyModifiers};

use super::{App, Dialog, DialogKind};
use crate::textfield::TextInput;

#[must_use]
pub(crate) fn torrent_path_like(token: &str) -> bool {
    token.ends_with(".torrent")
        && (token.starts_with('/')
            || token.starts_with("./")
            || token.starts_with("../")
            || token.starts_with("~/")
            || token.starts_with('~')
            || token.contains('/'))
}

impl App {
    pub fn open_cmd_window(&mut self) {
        self.cmd_open = true;
        self.cmd.reset();
        self.cmd_focus = 0;
        self.cmd_pos = None;
        self.cmd_drag = None;
        self.cmd_drag_moved = false;
    }

    pub fn close_cmd_window(&mut self) {
        self.cmd_open = false;
        self.cmd_area = None;
        self.cmd_pos = None;
        self.cmd_drag = None;
    }

    pub fn open_error_window(&mut self, msg: &str) {
        self.err_open = true;
        self.err_msg = msg.to_string();
        self.err_pos = None;
        self.err_drag = None;
        self.err_area = None;
        self.err_btn_rect = None;
    }

    pub fn close_error_window(&mut self) {
        self.err_open = false;
        self.err_msg = String::new();
        self.err_pos = None;
        self.err_drag = None;
        self.err_area = None;
        self.err_btn_rect = None;
    }

    pub(crate) fn on_err_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.close_error_window()
            }
            _ => {}
        }
    }

    pub(crate) fn open_add_dialog_with(&mut self, prefill: &str) {
        let proto = crate::model::protocol_of_url(prefill);
        let def_conns = if proto.is_bt() {
            self.cfg.bt_concurrency
        } else {
            self.cfg.http_concurrency
        };
        self.set_dialog(Dialog {
            kind: DialogKind::Add,
            url: TextInput::with_value(prefill),
            dir: TextInput::new(),
            conns: TextInput::with_value(def_conns.to_string()),
            conns_edited: false,
            ck_type: crate::model::checksum::CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: crate::model::checksum::CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
    }

    fn cmd_newline(&mut self) {
        if !self.cmd.newline() {
            self.set_toast(format!(
                "⚠ 命令窗口最多 {} 行",
                crate::multiline::MULTI_MAX_LINES
            ));
        }
    }

    pub(crate) fn cmd_confirm(&mut self) {
        let lines = self.cmd.trim_lines();
        let Some(input) = lines.first().cloned() else {
            self.close_cmd_window();
            return;
        };
        if lines.len() > 1 {
            self.set_toast(format!(
                "⚠ 仅处理第一行输入，其余 {} 行已忽略",
                lines.len() - 1
            ));
        }
        self.close_cmd_window();
        if input.starts_with("http://") || input.starts_with("https://") {
            self.open_add_dialog_with(&input);
        } else if torrent_path_like(&input) {
            match std::fs::read(crate::model::config::expand_tilde(&input)) {
                Ok(bytes) => match crate::bt::metainfo::TorrentMeta::parse(&bytes) {
                    Ok(meta) => self.open_add_bt_dialog_with(&input, &meta),
                    Err(_) => self.open_error_window("无法读取种子文件"),
                },
                Err(_) => self.open_error_window("无法读取种子文件"),
            }
        } else {
            self.open_error_window("暂不支持该类型的链接");
        }
    }

    pub(crate) fn cmd_complete_path(&mut self) {
        let line = self.cmd.line().value.clone();
        let col = self.cmd.line().cursor;
        let before: String = line.chars().take(col).collect();
        let after: String = line.chars().skip(col).collect();

        let start = before
            .char_indices()
            .rev()
            .find(|&(_, c)| c.is_whitespace())
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let token = &before[start..];
        if token.is_empty() {
            return;
        }
        let path_like = token.starts_with('/')
            || token.starts_with("./")
            || token.starts_with("../")
            || token.starts_with('~')
            || token.contains('/');
        if !path_like {
            return;
        }

        let (tok_dir, prefix) = match token.rsplit_once('/') {
            Some((d, p)) => (format!("{d}/"), p.to_string()),
            None => (String::new(), token.to_string()),
        };

        let expanded = if let Some(rest) = token.strip_prefix('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
            format!("{home}{rest}")
        } else {
            token.to_string()
        };
        let scan_dir = match expanded.rsplit_once('/') {
            Some(("", _)) => "/".to_string(),
            Some((d, _)) => d.to_string(),
            None => ".".to_string(),
        };

        let show_hidden = prefix.starts_with('.');
        let mut cands: Vec<(String, bool)> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&scan_dir) {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                if name.starts_with(prefix.as_str()) {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    cands.push((name, is_dir));
                }
            }
        }
        cands.sort_by(|a, b| a.0.cmp(&b.0));
        if cands.is_empty() {
            self.set_toast(format!("无匹配：{token}"));
            return;
        }

        let lcp = cands
            .iter()
            .skip(1)
            .fold(cands[0].0.clone(), |acc, (n, _)| {
                let mut l = 0usize;
                for (a, b) in acc.chars().zip(n.chars()) {
                    if a == b {
                        l += 1;
                    } else {
                        break;
                    }
                }
                acc.chars().take(l).collect()
            });

        let mut file_part = lcp;
        let unique = cands.len() == 1;
        if unique && cands[0].1 {
            file_part.push('/');
        }

        let new_before = format!(
            "{before_left}{tok_dir}{file_part}",
            before_left = &before[..start]
        );
        let new_line = format!("{new_before}{after}");

        let cur = self.cmd.row;
        self.cmd.lines[cur].set(new_line);
        self.cmd.lines[cur].set_cursor(new_before.chars().count());
        self.cmd.follow = true;

        if unique {
            if cands[0].1 {
                self.set_toast(format!("补全目录：{tok_dir}{file_part}"));
            }
        } else {
            let names: Vec<String> = cands.iter().take(5).map(|(n, _)| n.clone()).collect();
            let more = cands.len() - names.len();
            let suffix = if more > 0 {
                format!(" 等 {} 项", cands.len())
            } else {
                String::new()
            };
            self.set_toast(format!("{}{}", names.join("  "), suffix));
        }
    }

    pub(crate) fn cmd_paste(&mut self, text: &str) {
        self.cmd.insert_str(text);
    }

    pub(crate) fn on_cmd_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        self.cmd.follow = true;
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if mods.contains(KeyModifiers::CONTROL)
            && (code == KeyCode::Char('j') || code == KeyCode::Enter)
        {
            self.cmd_confirm();
            return;
        }
        match code {
            KeyCode::Esc => self.close_cmd_window(),
            KeyCode::Char('q') | KeyCode::Char('Q') if self.cmd_focus != 0 => {
                self.close_cmd_window();
            }
            KeyCode::Enter if mods.contains(KeyModifiers::SHIFT) => self.cmd_newline(),
            KeyCode::Enter if mods.contains(KeyModifiers::ALT) => self.cmd_newline(),
            KeyCode::Enter => self.cmd_confirm(),
            KeyCode::Tab => {
                self.cmd_focus = (self.cmd_focus + 1) % 2;
                if self.cmd_focus == 0 {
                    self.cmd.line_mut().cursor_end();
                }
            }
            KeyCode::BackTab => {
                if self.cmd_focus != 0 {
                    self.cmd_focus = 0;
                } else {
                    self.cmd_complete_path();
                }
            }
            KeyCode::Left => self.cmd.left(),
            KeyCode::Right => self.cmd.right(),
            KeyCode::Up => self.cmd.up(),
            KeyCode::Down => self.cmd.down(),
            KeyCode::Home => self.cmd.to_first(),
            KeyCode::End => self.cmd.to_last(),
            KeyCode::Backspace => self.cmd.backspace(),
            KeyCode::Delete => self.cmd.delete(),
            KeyCode::Char(c) if self.cmd_focus == 0 && !c.is_control() => {
                self.cmd.insert_char(c);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod cmd_confirm_tests {

    use sha1::{Digest, Sha1};

    use super::*;
    use crate::bt::metainfo::TorrentMeta;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-cmd", tag)
    }

    fn write_mini_torrent(dir: &std::path::Path, name: &str) -> String {
        let data = b"0123456789";
        let mut pieces = Vec::new();
        for chunk in data.chunks(4) {
            let mut h = Sha1::new();
            h.update(chunk);
            pieces.extend_from_slice(&h.finalize());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"d8:announce4:urls");
        out.extend_from_slice(
            format!(
                "4:infod6:lengthi10e4:name4:{}12:piece lengthi4e6:pieces{}:",
                &name[..4.min(name.len())],
                pieces.len()
            )
            .as_bytes(),
        );
        out.extend_from_slice(&pieces);
        out.push(b'e');
        out.push(b'e');
        let path = dir.join(name);
        std::fs::write(&path, &out).unwrap();
        path.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn confirm_empty_closes_without_action() {
        let mut app = make_app("empty");
        app.open_cmd_window();
        app.cmd_confirm();
        assert!(!app.cmd_open, "空输入仅关窗");
        assert!(app.dialog.is_none() && !app.err_open, "无任何浮层动作");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn confirm_http_prefills_add_dialog() {
        let mut app = make_app("http");
        app.open_cmd_window();
        app.cmd.set_text("https://example.com/f.bin");
        app.cmd_confirm();
        assert!(!app.cmd_open, "先关命令窗口（D44）");
        assert!(!app.err_open);
        let d = app.dialog.as_ref().expect("HTTP 分流开添加对话框");
        assert_eq!(d.kind, DialogKind::Add);
        assert_eq!(d.url.value, "https://example.com/f.bin");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn confirm_torrent_parses_and_opens_bt_dialog() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cmd-bt");
        let path = write_mini_torrent(&dir, "ok.torrent");
        let mut app = make_app("bt");
        app.open_cmd_window();
        app.cmd.set_text(&path);
        app.cmd_confirm();
        assert!(!app.cmd_open && !app.err_open, "解析成功不开错误窗口");
        let d = app.dialog.as_ref().expect("开 BT 对话框");
        assert_eq!(d.kind, DialogKind::AddBt);
        assert_eq!(d.torrent.value, path);
        assert!(!d.bt_tree.is_empty(), "元数据文件树已预填");
        let (sel, total) = d.bt_counts();
        assert_eq!((sel, total), (1, 1));
        assert_eq!(d.task_name, "ok.t", "task_name = info.name（D50）");
        assert_eq!(d.conns.value, "20", "BT 并发预填 20（D48 并发口径）");
        assert_eq!(
            d.dir.value,
            crate::model::config::default_download_dir(),
            "保存到预填配置默认目录（D48，非 demo /srv/downloads）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_tilde_torrent_path_expands_home() {
        let Some(home) = crate::model::config::home_dir() else {
            return;
        };
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir = home.join(format!(".ezr-tilde-{}-{ms}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_mini_torrent(&dir, "ok.torrent");
        assert!(std::path::Path::new(&path).is_file());
        let dirname = dir.file_name().unwrap().to_string_lossy().to_string();
        let input = format!("~/{dirname}/ok.torrent");
        let mut app = make_app("tilde");
        app.open_cmd_window();
        app.cmd.set_text(&input);
        app.cmd_confirm();
        assert!(!app.cmd_open && !app.err_open, "~/ 路径展开后可读取并解析");
        let d = app.dialog.as_ref().expect("~/ 种子路径开 BT 对话框");
        assert_eq!(d.kind, DialogKind::AddBt);
        assert_eq!(d.torrent.value, input, "对话框回显输入原文");
        assert!(!d.bt_tree.is_empty(), "元数据文件树已预填");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_broken_torrent_opens_error_window() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cmd-bad");
        let path = dir.join("bad.torrent");
        std::fs::write(&path, b"garbage-not-bencode").unwrap();
        let mut app = make_app("bad");
        app.open_cmd_window();
        app.cmd.set_text(path.to_string_lossy().as_ref());
        app.cmd_confirm();
        assert!(app.dialog.is_none(), "解析失败不再打开 BT 对话框（D55）");
        assert!(app.err_open, "开错误窗口（D56）");
        assert_eq!(app.err_msg, "无法读取种子文件");
        app.close_error_window();
        assert!(!app.err_open && app.err_msg.is_empty(), "位置关闭即清");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_other_inputs_open_unsupported_error() {
        let mut app = make_app("other");
        for input in [
            "magnet:?xt=urn:btih:deadbeef",
            "ftp://example.com/f.bin",
            "随便写点什么",
            "plain.torrent",
        ] {
            app.open_cmd_window();
            app.cmd.set_text(input);
            app.cmd_confirm();
            assert!(app.err_open, "{input} → 错误窗口");
            assert_eq!(app.err_msg, "暂不支持该类型的链接", "D53 钦定文案");
            assert!(app.dialog.is_none());
            app.close_error_window();
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn confirm_multiline_takes_first_and_toasts_rest() {
        let mut app = make_app("multi");
        app.open_cmd_window();
        app.cmd.set_text("magnet:?x=1\nhttp://example.com/a.bin");
        app.cmd_confirm();
        assert!(app.err_open, "第一行磁力链 → 错误窗口");
        assert_eq!(app.err_msg, "暂不支持该类型的链接");
        app.close_error_window();
        assert_eq!(
            app.toast.as_deref(),
            Some("⚠ 仅处理第一行输入，其余 1 行已忽略"),
            "多行忽略提示（错误窗口文案承载在 err_msg，不占 toast 槽位）"
        );
        app.shutdown().await;
    }

    #[test]
    fn torrent_path_like_predicate() {
        assert!(torrent_path_like("/srv/t/a.torrent"));
        assert!(torrent_path_like("./a.torrent"));
        assert!(torrent_path_like("../a.torrent"));
        assert!(torrent_path_like("~/a.torrent"));
        assert!(torrent_path_like("dir/a.torrent"));
        assert!(!torrent_path_like("plain.torrent"));
        assert!(!torrent_path_like("/srv/t/a.zip"));
        assert!(!torrent_path_like("magnet:?xt=urn:btih:x"));
    }

    #[test]
    fn parse_rejects_garbage_meta_path() {
        assert!(TorrentMeta::parse(b"garbage").is_err());
    }
}
