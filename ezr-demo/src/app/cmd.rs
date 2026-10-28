use crossterm::event::{KeyCode, KeyModifiers};

use super::*;

/// 命令窗口：多行文本框 + 确定按钮。
///
/// - 输入 http(s) URL / 本地 torrent 文件路径 / 磁力链接；
/// - Enter 确认分流（= 触发左侧第一个按钮 [确定] 的 click）；
/// - Shift+Enter（kitty 协议 CSI 13;2u 可区分上报）或 Alt+Enter（ESC 前缀，
///   普通终端降级可用）在文本框内换行；
/// - Ctrl+Enter（kitty 协议 CSI 13;5u）或 Ctrl+J（\n 字节，任何终端）确认，
///   [确定] 按钮（点击 / Tab 后 Enter）同样确认；
/// - Shift+Tab（BackTab）对光标前的本地路径 token 做 readline 式补全；
/// - 确认后按输入类型分流：「添加任务：HTTP(S)」/「添加任务：BT」；
/// - 多行编辑/视口/滚动语义见 [`crate::multiline::MultiLineBuf`]（可复用
///   编辑模型，与 BT 对话框 trackers 输入共用）。
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

    /// Shift+Enter / Alt+Enter：在当前光标处换行（光标后内容移到新行）
    fn cmd_newline(&mut self) {
        if !self.cmd.newline() {
            self.set_toast(format!("⚠ 命令窗口最多 {} 行", crate::multiline::MULTI_MAX_LINES));
        }
    }

    /// 确认：取第一个非空行，按类型分流打开对应添加对话框
    pub(super) fn cmd_confirm(&mut self) {
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
        } else {
            // 本地 torrent 路径 / 磁力链接 → 添加任务：BT
            self.open_add_bt_dialog_with(&input);
        }
    }

    /// Shift+Tab：readline 式本地路径补全（对光标前的 whitespace 分隔 token）。
    ///
    /// - 仅对「像本地路径」的 token 生效（/ 开头、./ ../、~/、或含 /）；
    /// - 支持 ~ 展开；多候选补到最长公共前缀 + toast 列出候选；
    /// - 唯一候选且为目录时补尾随 /。
    pub(super) fn cmd_complete_path(&mut self) {
        let line = self.cmd.line().value.clone();
        let col = self.cmd.line().cursor;
        let before: String = line.chars().take(col).collect();
        let after: String = line.chars().skip(col).collect();

        // start = 光标前最后一个空白后的字节偏移（char_indices 产出字节下标）
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

        // token 内的目录前缀（原样保留，含 ~）与文件名前缀
        let (tok_dir, prefix) = match token.rsplit_once('/') {
            Some((d, p)) => (format!("{d}/"), p.to_string()),
            None => (String::new(), token.to_string()),
        };

        // 扫描目录：~ 展开后 + 目录前缀
        let expanded = if let Some(rest) = token.strip_prefix('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
            format!("{home}{rest}")
        } else {
            token.to_string()
        };
        let scan_dir = match expanded.rsplit_once('/') {
            Some((d, _)) if d.is_empty() => "/".to_string(),
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

        // 最长公共前缀（按字符）
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

        let new_before = format!("{before_left}{tok_dir}{file_part}", before_left = &before[..start]);
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

    pub(super) fn cmd_paste(&mut self, text: &str) {
        self.cmd.insert_str(text);
    }

    pub(super) fn on_cmd_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        // 任何键入（光标移动/编辑）都重新拉回视口跟随；滚动条/滚轮滚动解除
        // （滚动路径 scroll_by/scroll_to 内部置 false，优先级高于此默认值）
        self.cmd.follow = true;
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        // Ctrl+Enter / Ctrl+J = 确认：kitty 协议终端 Ctrl+Enter 上报为
        // Enter+CONTROL（CSI 13;5u），其余终端 Ctrl+Enter 发送 \n 字节，
        // 由 crossterm 解析为 Char('j')+CONTROL——两条路径在此汇合。
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
            // Shift+Enter（kitty 协议 CSI 13;2u 可区分上报）/ Alt+Enter
            // （ESC 前缀，普通终端降级可用）= 文本框内换行
            KeyCode::Enter if mods.contains(KeyModifiers::SHIFT) => self.cmd_newline(),
            KeyCode::Enter if mods.contains(KeyModifiers::ALT) => self.cmd_newline(),
            // Enter = 触发左侧第一个按钮（[确定]）的 click：确认分流
            // （文本框焦点与按钮焦点同义；[确定] 焦点下 Tab 切回文本框）
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
