//! paste — bracketed paste 输入路由（FR-01-06）：纯函数便于单测

use super::{App, Dialog, DialogKind};

impl App {
    /// bracketed paste 事件入口（FR-01-06）：仅 Add 对话框的文本字段接收粘贴；
    /// 下拉框展开、删除对话框、无对话框时忽略（避免粘贴触发按钮/导航）。
    pub fn on_paste(&mut self, text: &str) {
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        if !paste_accepts(d.kind, d.ck_open) {
            return;
        }
        let focus = d.focus;
        dlg_apply_paste(d, focus, text);
    }
}

/// 判定粘贴事件是否可作用于当前对话框状态（FR-01-06）
fn paste_accepts(kind: DialogKind, ck_open: bool) -> bool {
    kind == DialogKind::Add && !ck_open
}

/// 粘贴文本净化：剔除控制字符（剪贴板可能携带换行/制表等）
fn paste_sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// 截断式追加：目标字符串不超过 `cap` 个字符（按 char 计数，不切分多字节字符）
fn push_capped(dst: &mut String, src: &str, cap: usize) -> bool {
    let room = cap.saturating_sub(dst.chars().count());
    let taken: String = src.chars().take(room).collect();
    let inserted = !taken.is_empty();
    dst.push_str(&taken);
    inserted
}

/// 将粘贴文本按当前焦点路由到 Add 对话框字段（FR-01-06）。
/// 过滤规则与逐键输入一致：URL/目录 ≤300 字符；并发仅数字 ≤2 位；
/// 校验码仅十六进制 ≤128 位；按钮焦点（5=确认 6=取消）不接收。
fn dlg_apply_paste(d: &mut Dialog, focus: usize, text: &str) -> bool {
    let clean = paste_sanitize(text);
    match focus {
        0 => push_capped(&mut d.url, &clean, 300),
        1 => push_capped(&mut d.dir, &clean, 300),
        2 => {
            let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
            let inserted = push_capped(&mut d.conns, &digits, 2);
            d.conns_edited |= inserted;
            inserted
        }
        4 => {
            let hex: String = clean.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            push_capped(&mut d.ck_value, &hex, 128)
        }
        _ => false,
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    fn add_dialog(focus: usize) -> Dialog {
        crate::app::testutil::dialog(DialogKind::Add, focus)
    }

    /// 01-add-task-17：粘贴 200 字符长 URL，字段完整接收
    #[test]
    fn paste_long_url_fully_inserted() {
        let mut d = add_dialog(0);
        let url = format!("http://example.com/{}", "a".repeat(180));
        assert_eq!(url.chars().count(), 199);
        assert!(dlg_apply_paste(&mut d, 0, &url));
        assert_eq!(d.url, url);
    }

    /// 粘贴携带换行/制表的剪贴板内容：控制字符被剔除
    #[test]
    fn paste_strips_control_chars() {
        let mut d = add_dialog(0);
        assert!(dlg_apply_paste(&mut d, 0, "http://example.com/f.bin\r\n"));
        assert_eq!(d.url, "http://example.com/f.bin");
    }

    /// URL 字段 300 字符上限与逐键输入一致
    #[test]
    fn paste_url_caps_at_300() {
        let mut d = add_dialog(0);
        let blob = "x".repeat(400);
        assert!(dlg_apply_paste(&mut d, 0, &blob));
        assert_eq!(d.url.chars().count(), 300);
    }

    /// 校验码字段：仅十六进制被接收，最多 128 位（SHA-512）
    #[test]
    fn paste_checksum_filters_hex_and_caps() {
        let mut d = add_dialog(4);
        assert!(dlg_apply_paste(&mut d, 4, "  zzFF88ff00!dead-beef  "));
        assert_eq!(d.ck_value, "FF88ff00deadbeef");
        let blob = "f".repeat(200);
        assert!(dlg_apply_paste(&mut d, 4, &blob));
        assert_eq!(d.ck_value.chars().count(), 128);
    }

    /// 并发数字段：仅数字，最多 2 位；edited 标记置位
    #[test]
    fn paste_conns_two_digits_only() {
        let mut d = add_dialog(2);
        assert!(dlg_apply_paste(&mut d, 2, "1a2b3"));
        assert_eq!(d.conns, "12");
        assert!(d.conns_edited);
        d.conns = "9".into();
        assert!(dlg_apply_paste(&mut d, 2, "42"));
        assert_eq!(d.conns, "94");
    }

    /// 并发字段满 2 位后再粘贴：无变化
    #[test]
    fn paste_conns_full_rejects() {
        let mut d = add_dialog(2);
        d.conns = "64".into();
        assert!(!dlg_apply_paste(&mut d, 2, "7"));
        assert_eq!(d.conns, "64");
    }

    /// 纯控制字符粘贴：无效果
    #[test]
    fn paste_all_control_is_noop() {
        let mut d = add_dialog(0);
        assert!(!dlg_apply_paste(&mut d, 0, "\r\n\t"));
        assert!(d.url.is_empty());
    }

    /// 按钮焦点（5=确认 6=取消）不接收粘贴
    #[test]
    fn paste_buttons_ignore() {
        let mut d = add_dialog(5);
        assert!(!dlg_apply_paste(&mut d, 5, "http://example.com/"));
        assert!(d.url.is_empty());
    }

    /// 粘贴仅作用于 Add 对话框文本字段：下拉展开或 Delete 对话框时忽略
    #[test]
    fn paste_accepts_only_add_text_fields() {
        assert!(paste_accepts(DialogKind::Add, false));
        assert!(!paste_accepts(DialogKind::Add, true));
        assert!(!paste_accepts(DialogKind::Delete, false));
    }

    /// 截断式追加按字符计数，不切分多字节字符
    #[test]
    fn push_capped_respects_char_boundary() {
        let mut s = String::from("下载");
        assert!(push_capped(&mut s, "器abc", 3));
        assert_eq!(s, "下载器");
        assert!(!push_capped(&mut s, "x", 3));
        assert_eq!(s, "下载器");
    }

    fn make_app(tag: &str) -> crate::app::App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-paste-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        crate::app::App::new(crate::model::config::Config::default(), reg)
    }

    /// App::on_paste 入口路由（FR-01-06）：Add 对话框文本字段接收粘贴；
    /// 无对话框 / 算法下拉展开 / Delete 对话框一律忽略
    #[tokio::test]
    async fn on_paste_routes_by_dialog_state() {
        let mut app = make_app("route");

        // 无对话框 → 忽略（不 panic、无副作用）
        app.on_paste("http://example.com/x.bin");

        // Add 对话框 focus 0 → 粘贴进 URL
        app.dialog = Some(add_dialog(0));
        app.on_paste("http://example.com/p.bin\r\n");
        assert_eq!(
            app.dialog.as_ref().unwrap().url,
            "http://example.com/p.bin",
            "粘贴净化后进入 URL 字段"
        );

        // 算法下拉展开 → 忽略
        app.dialog.as_mut().unwrap().ck_open = true;
        app.on_paste("http://example.com/y.bin");
        assert_eq!(
            app.dialog.as_ref().unwrap().url,
            "http://example.com/p.bin",
            "下拉展开时不接收粘贴"
        );

        // Delete 对话框 → 忽略（避免粘贴触发按钮/导航）
        let mut del = add_dialog(0);
        del.kind = DialogKind::Delete;
        app.dialog = Some(del);
        app.on_paste("http://example.com/z.bin");
        assert!(
            app.dialog.as_ref().unwrap().url.is_empty(),
            "Delete 忽略粘贴"
        );
        app.shutdown().await;
    }
}
