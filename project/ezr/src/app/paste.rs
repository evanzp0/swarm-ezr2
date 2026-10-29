use super::dialog_keys::{text_field_at, TextField, CAP_CK_HEX, CAP_CONNS_DIGITS, CAP_TEXT};
use super::{App, Dialog, DialogKind};
use crate::model::checksum::CK_NONE;

impl App {
    pub fn on_paste(&mut self, text: &str) {
        if self.cmd_open {
            if self.cmd_focus == 0 {
                self.cmd_paste(text);
            }
            return;
        }
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        if !paste_accepts(d.kind, d.ck_open, d.proxy_open) {
            return;
        }
        let focus = d.focus;
        let kind = d.kind;
        if d.trackers_focus() == Some(focus) {
            d.trackers.insert_str(text);
            return;
        }
        dlg_apply_paste(kind, d, focus, text);
        if kind == DialogKind::Add && focus == 0 {
            self.sync_default_conns_after_url_change();
        }
    }
}

fn paste_accepts(kind: DialogKind, ck_open: bool, proxy_open: bool) -> bool {
    matches!(
        kind,
        DialogKind::Add | DialogKind::Modify | DialogKind::AddBt | DialogKind::ModifyBt
    ) && !ck_open
        && !proxy_open
}

fn paste_sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

fn dlg_apply_paste(kind: DialogKind, d: &mut Dialog, focus: usize, text: &str) -> bool {
    let clean = paste_sanitize(text);
    match text_field_at(kind, focus) {
        Some(TextField::Url) => d.url.insert(&clean, CAP_TEXT) > 0,
        Some(TextField::Dir) => d.dir.insert(&clean, CAP_TEXT) > 0,
        Some(TextField::Conns) => {
            let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
            let inserted = d.conns.insert(&digits, CAP_CONNS_DIGITS);
            d.conns_edited |= inserted > 0;
            inserted > 0
        }
        Some(TextField::CkValue) => {
            if d.ck_type == CK_NONE {
                return false;
            }
            let hex: String = clean.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            d.ck_value.insert(&hex, CAP_CK_HEX) > 0
        }
        None => match (kind, focus) {
            (DialogKind::AddBt, 0) => d.torrent.insert(&clean, CAP_TEXT) > 0,
            (DialogKind::AddBt, 1) => d.dir.insert(&clean, CAP_TEXT) > 0,
            (DialogKind::AddBt, 3) | (DialogKind::ModifyBt, 1) => {
                let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
                let inserted = d.conns.insert(&digits, CAP_CONNS_DIGITS);
                d.conns_edited |= inserted > 0;
                inserted > 0
            }
            _ => false,
        },
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    fn add_dialog(focus: usize) -> Dialog {
        crate::app::testutil::dialog(DialogKind::Add, focus)
    }

    fn modify_dialog(focus: usize) -> Dialog {
        crate::app::testutil::dialog(DialogKind::Modify, focus)
    }

    #[test]
    fn paste_long_url_fully_inserted() {
        let mut d = add_dialog(0);
        let url = format!("http://example.com/{}", "a".repeat(180));
        assert_eq!(url.chars().count(), 199);
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 0, &url));
        assert_eq!(d.url.value, url);
    }

    #[test]
    fn paste_strips_control_chars() {
        let mut d = add_dialog(0);
        assert!(dlg_apply_paste(
            DialogKind::Add,
            &mut d,
            0,
            "http://example.com/f.bin\r\n"
        ));
        assert_eq!(d.url.value, "http://example.com/f.bin");
    }

    #[test]
    fn paste_url_caps_at_300() {
        let mut d = add_dialog(0);
        let blob = "x".repeat(400);
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 0, &blob));
        assert_eq!(d.url.len_chars(), 300);
    }

    #[test]
    fn paste_checksum_filters_hex_and_caps() {
        let mut d = add_dialog(4);
        assert!(dlg_apply_paste(
            DialogKind::Add,
            &mut d,
            4,
            "  zzFF88ff00!dead-beef  "
        ));
        assert_eq!(d.ck_value.value, "FF88ff00deadbeef");
        let blob = "f".repeat(200);
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 4, &blob));
        assert_eq!(d.ck_value.len_chars(), 128);
    }

    #[test]
    fn paste_modify_checksum_field_accepts_hex() {
        let mut d = modify_dialog(2);
        assert!(
            dlg_apply_paste(DialogKind::Modify, &mut d, 2, "  zzFF88ff00!dead-beef  "),
            "旧实现门条件仅放行 Add，Modify 粘贴被整门拒绝（红路径）"
        );
        assert_eq!(d.ck_value.value, "FF88ff00deadbeef");
        let blob = "f".repeat(200);
        assert!(dlg_apply_paste(DialogKind::Modify, &mut d, 2, &blob));
        assert_eq!(d.ck_value.len_chars(), 128);
    }

    #[test]
    fn paste_modify_conns_digits_and_cap() {
        let mut d = modify_dialog(0);
        assert!(dlg_apply_paste(DialogKind::Modify, &mut d, 0, "1a2b3"));
        assert_eq!(d.conns.value, "12");
        assert!(d.conns_edited);
        d.conns.set("64");
        assert!(!dlg_apply_paste(DialogKind::Modify, &mut d, 0, "7"));
        assert_eq!(d.conns.value, "64");
    }

    #[test]
    fn paste_modify_dropdown_rows_and_buttons_ignore() {
        let mut d = modify_dialog(1);
        assert!(!dlg_apply_paste(DialogKind::Modify, &mut d, 1, "deadbeef"));
        assert!(!dlg_apply_paste(DialogKind::Modify, &mut d, 3, "x"));
        assert!(!dlg_apply_paste(DialogKind::Modify, &mut d, 4, "x"));
        assert!(!dlg_apply_paste(DialogKind::Modify, &mut d, 5, "x"));
        assert!(d.ck_value.is_empty());
    }

    #[test]
    fn paste_conns_two_digits_only() {
        let mut d = add_dialog(2);
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 2, "1a2b3"));
        assert_eq!(d.conns.value, "12");
        assert!(d.conns_edited);
        d.conns.set("9");
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 2, "42"));
        assert_eq!(d.conns.value, "94");
    }

    #[test]
    fn paste_conns_full_rejects() {
        let mut d = add_dialog(2);
        d.conns.set("64");
        assert!(!dlg_apply_paste(DialogKind::Add, &mut d, 2, "7"));
        assert_eq!(d.conns.value, "64");
    }

    #[test]
    fn paste_all_control_is_noop() {
        let mut d = add_dialog(0);
        assert!(!dlg_apply_paste(DialogKind::Add, &mut d, 0, "\r\n\t"));
        assert!(d.url.is_empty());
    }

    #[test]
    fn paste_buttons_ignore() {
        let mut d = add_dialog(5);
        assert!(!dlg_apply_paste(
            DialogKind::Add,
            &mut d,
            5,
            "http://example.com/"
        ));
        assert!(d.url.is_empty());
    }

    #[test]
    fn paste_accepts_add_modify_text_dialogs() {
        assert!(paste_accepts(DialogKind::Add, false, false));
        assert!(paste_accepts(DialogKind::Modify, false, false));
        assert!(paste_accepts(DialogKind::AddBt, false, false));
        assert!(paste_accepts(DialogKind::ModifyBt, false, false));
        assert!(!paste_accepts(DialogKind::Add, true, false));
        assert!(!paste_accepts(DialogKind::Add, false, true));
        assert!(!paste_accepts(DialogKind::Modify, true, false));
        assert!(!paste_accepts(DialogKind::Modify, false, true));
        assert!(!paste_accepts(DialogKind::Delete, false, false));
    }

    #[tokio::test]
    async fn on_paste_bt_dialog_trackers_multiline_and_fields() {
        let mut app = make_app("btpaste");

        app.dialog = Some(crate::app::testutil::dialog(DialogKind::AddBt, 5));
        app.on_paste("udp://a/ann\nudp://b/ann\r\nhttp://c/ann");
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://a/ann", "udp://b/ann", "http://c/ann"],
                "trackers 粘贴保留换行分段（回归）"
            );
        }

        app.dialog = Some(crate::app::testutil::dialog(DialogKind::AddBt, 0));
        app.on_paste("/tmp/x.torrent");
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.torrent.value, "/tmp/x.torrent", "种子行粘贴");
        }

        app.dialog = Some(crate::app::testutil::dialog(DialogKind::AddBt, 3));
        app.on_paste("2x0");
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.conns.value, "20", "BT 并发粘贴仅数字");
            assert!(d.conns_edited);
        }

        app.dialog = Some(crate::app::testutil::dialog(DialogKind::ModifyBt, 3));
        app.on_paste("udp://m/ann");
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://m/ann"],
                "ModifyBt trackers 粘贴"
            );
        }
        app.shutdown().await;
    }

    #[test]
    fn field_insert_respects_char_boundary() {
        let mut d = add_dialog(4);
        d.ck_value.set("下载");
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 4, "器abc"));
        assert_eq!(d.ck_value.value, "下载abc", "hex 过滤剔除非十六进制字符");
        assert_eq!(d.ck_value.cursor, 5, "光标随迁插入文本之后");
        let mut d2 = modify_dialog(0);
        d2.conns.set("下载");
        assert!(
            !dlg_apply_paste(DialogKind::Modify, &mut d2, 0, "器"),
            "并发字段数字过滤"
        );
    }

    #[test]
    fn paste_inserts_at_cursor() {
        let mut d = add_dialog(0);
        d.url.set("http://a.bin");
        d.url.set_cursor(7);
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 0, "X"));
        assert_eq!(d.url.value, "http://Xa.bin", "粘贴落在光标列");
        assert_eq!(d.url.cursor, 8, "光标随迁至插入文本之后");
        d.url.set_cursor(0);
        assert!(dlg_apply_paste(DialogKind::Add, &mut d, 0, "S"));
        assert_eq!(d.url.value, "Shttp://Xa.bin");
    }

    fn make_app(tag: &str) -> crate::app::App {
        crate::app::testutil::make_app_in("ezr-paste", tag)
    }

    #[tokio::test]
    async fn on_paste_routes_by_dialog_state() {
        let mut app = make_app("route");

        app.on_paste("http://example.com/x.bin");

        app.dialog = Some(add_dialog(0));
        app.on_paste("http://example.com/p.bin\r\n");
        assert_eq!(
            app.dialog.as_ref().unwrap().url.value,
            "http://example.com/p.bin",
            "粘贴净化后进入 URL 字段"
        );

        app.dialog.as_mut().unwrap().ck_open = true;
        app.on_paste("http://example.com/y.bin");
        assert_eq!(
            app.dialog.as_ref().unwrap().url.value,
            "http://example.com/p.bin",
            "下拉展开时不接收粘贴"
        );

        app.dialog = Some(modify_dialog(2));
        app.on_paste("FF00dead\r\n");
        assert_eq!(
            app.dialog.as_ref().unwrap().ck_value.value,
            "FF00dead",
            "修改对话框校验码字段接收粘贴（v1.17）"
        );

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
