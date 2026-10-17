
use super::App;

impl App {
    /// 将文本复制到剪贴板（FR-01-101④；详情面板「校验」「URL」字段名点击触发）：
    /// 系统剪贴板（arboard）优先，失败或无图形会话回退 OSC 52；
    /// 同时记录 [`App::last_copied`] 供诊断与测试断言复制内容。
    pub(crate) fn copy_to_clipboard(&mut self, text: &str) {
        self.last_copied = Some(text.to_string());
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(cb) = self.clipboard.as_mut() {
            if cb.set_text(text.to_string()).is_ok() {
                return; // 系统剪贴板写入成功
            }
            self.clipboard = None; // 实例失效：下次点击重建，本次回退
        }
        copy_osc52(text);
    }
}

/// OSC 52 终端转义序列回退：终端原生方案，无需 X11/Wayland 会话与外部依赖；
/// 写入失败时静默忽略（alternate screen 下终端仍解析该序列并代写剪贴板）。
fn copy_osc52(text: &str) {
    use std::io::Write;
    let seq = osc52_sequence(text);
    let mut out = std::io::stdout();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

/// OSC 52 序列构造（纯函数，复制内容断言用）：`ESC ]52;c;<base64> BEL`
pub(crate) fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", b64_encode(text.as_bytes()))
}

/// 极简 base64 编码（标准字母表 + '=' 填充），仅供 OSC 52 使用，避免引入外部 crate
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

    /// OSC 52 序列 = ESC ]52;c;<base64> BEL；base64 标准字母表 + '=' 填充
    #[test]
    fn osc52_sequence_encodes_base64_payload() {
        assert_eq!(
            osc52_sequence("hello"),
            "\x1b]52;c;aGVsbG8=\x07",
            "RFC4648 已知向量（hello）"
        );
        assert_eq!(osc52_sequence(""), "\x1b]52;c;\x07", "空载荷");
        // 3 字节整除边界：无填充
        assert_eq!(
            osc52_sequence("abc"),
            "\x1b]52;c;YWJj\x07",
            "3 字节整除无填充"
        );
        // 2 字节：一个 '=' 填充
        assert_eq!(osc52_sequence("ab"), "\x1b]52;c;YWI=\x07", "2 字节单填充");
        // 中文（多字节 UTF-8）逐字节编码
        assert_eq!(osc52_sequence("已"), "\x1b]52;c;5bey\x07", "UTF-8 多字节");
    }
}
