use super::App;
use crate::engine::Cmd;

impl App {
    pub(crate) fn conn_count(&self) -> usize {
        if let Some(t) = self.sel_task().filter(|t| t.protocol.is_bt()) {
            return self.bt_conns.get(&t.id).map_or(0, Vec::len);
        }
        self.active_conn_ids().len()
    }

    pub(crate) fn move_conn_sel(&mut self, delta: i32) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无并发明细（仅下载中/做种中可选）");
            return;
        }
        let n = self.conn_count();
        if n == 0 {
            self.set_toast("当前任务没有并发连接明细");
            return;
        }
        let new = (self.conns_sel as i32 + delta).clamp(0, n as i32 - 1) as usize;
        self.conns_sel = new;
        let vis = self.visible_conns_rows.max(1);
        if new < self.conns_scroll {
            self.conns_scroll = new;
        } else if new >= self.conns_scroll + vis {
            self.conns_scroll = new + 1 - vis;
        }
    }

    pub(crate) fn disconnect_conn(&mut self) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无可断开的并发连接（仅下载中/做种中）");
            return;
        }
        if !self.tasks[idx].protocol.is_bt() {
            self.set_toast("仅 BT 任务支持断开并发连接（B）");
            return;
        }
        let id = self.tasks[idx].id;
        let rows = self.bt_conns.get(&id).cloned().unwrap_or_default();
        if self.conns_sel >= rows.len() {
            self.set_toast("当前任务没有可断开的并发连接");
            return;
        }
        self.conns_sel = self.conns_sel.min(rows.len() - 1);
        let conn = rows[self.conns_sel].id;
        let ip = rows[self.conns_sel]
            .addr
            .rsplit_once(':')
            .map_or(rows[self.conns_sel].addr.as_str(), |(ip, _)| ip);
        let left = rows.len() - 1;
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine.send(Cmd::BtDisconnect { id, conn }).await;
        });
        self.set_toast(format!("✂ 已断开 {ip} 并加入黑名单（剩余 {left} 条）"));
    }
}
