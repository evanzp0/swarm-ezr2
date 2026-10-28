use super::App;

impl App {
    pub(crate) fn copy_to_clipboard(&mut self, text: &str) {
        self.last_copied = Some(text.to_string());
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(cb) = self.clipboard.as_mut() {
            if cb.set_text(text.to_string()).is_ok() {
                return;
            }
            self.clipboard = None;
        }
        copy_osc52(text);
    }
}

fn copy_osc52(text: &str) {
    use std::io::Write;
    let seq = osc52_sequence(text);
    let mut out = std::io::stdout();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

pub(crate) fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", b64_encode(text.as_bytes()))
}

fn b64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = ((*chunk.first().unwrap_or(&0) as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod clipboard_tests {
    use super::*;

    #[test]
    fn osc52_sequence_encodes_base64_payload() {
        assert_eq!(
            osc52_sequence("hello"),
            "\x1b]52;c;aGVsbG8=\x07",
            "RFC4648 已知向量（hello）"
        );
        assert_eq!(osc52_sequence(""), "\x1b]52;c;\x07", "空载荷");
        assert_eq!(
            osc52_sequence("abc"),
            "\x1b]52;c;YWJj\x07",
            "3 字节整除无填充"
        );
        assert_eq!(osc52_sequence("ab"), "\x1b]52;c;YWI=\x07", "2 字节单填充");
        assert_eq!(osc52_sequence("已"), "\x1b]52;c;5bey\x07", "UTF-8 多字节");
    }
}
