use crossterm::event::KeyCode;

use super::{App, SbKey};

impl App {
    pub(crate) fn on_val_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.close_val_popup(),
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let label = self.val_field.copy_label();
                let text = self.val_text.clone();
                self.copy_to_clipboard(&text);
                self.set_toast(format!("已复制 {label}"));
            }
            KeyCode::Up => self.sb_scroll(SbKey::Val, -1),
            KeyCode::Down => self.sb_scroll(SbKey::Val, 1),
            KeyCode::PageUp => self.sb_scroll(SbKey::Val, -8),
            KeyCode::PageDown => self.sb_scroll(SbKey::Val, 8),
            KeyCode::Home => self.sb_set(SbKey::Val, 0),
            KeyCode::End => self.sb_set(SbKey::Val, usize::MAX),
            _ => {}
        }
    }

    pub(crate) fn on_about_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc
            | KeyCode::Char('o')
            | KeyCode::Char('O')
            | KeyCode::Char('q')
            | KeyCode::Char('Q') => self.close_about(),
            _ => {}
        }
    }

    pub(crate) fn on_pieces_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc
            | KeyCode::Char('p')
            | KeyCode::Char('P')
            | KeyCode::Char('q')
            | KeyCode::Char('Q') => self.close_pieces(),
            KeyCode::Up => self.pieces_scroll_by(-1),
            KeyCode::Down => self.pieces_scroll_by(1),
            KeyCode::PageUp => self.pieces_scroll_by(-8),
            KeyCode::PageDown => self.pieces_scroll_by(8),
            KeyCode::Home => self.pieces_scroll = 0,
            KeyCode::End => self.pieces_scroll = usize::MAX,
            _ => {}
        }
    }

    pub(crate) fn on_trackers_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc
            | KeyCode::Char('t')
            | KeyCode::Char('T')
            | KeyCode::Char('q')
            | KeyCode::Char('Q') => self.close_trackers_popup(),
            KeyCode::Up => self.trackers_scroll_by(-1),
            KeyCode::Down => self.trackers_scroll_by(1),
            KeyCode::PageUp => self.trackers_scroll_by(-8),
            KeyCode::PageDown => self.trackers_scroll_by(8),
            KeyCode::Home => self.trackers_scroll = 0,
            KeyCode::End => self.trackers_scroll = usize::MAX,
            _ => {}
        }
    }

    pub(crate) fn on_help_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc
            | KeyCode::Char('h')
            | KeyCode::Char('H')
            | KeyCode::Char('q')
            | KeyCode::Char('Q') => self.help_open = false,
            KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),
            KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
            KeyCode::Down => self.help_scroll += 1,
            KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(8),
            KeyCode::PageDown => self.help_scroll += 8,
            KeyCode::Home => self.help_scroll = 0,
            KeyCode::End => self.help_scroll = usize::MAX,
            _ => {}
        }
    }
}
