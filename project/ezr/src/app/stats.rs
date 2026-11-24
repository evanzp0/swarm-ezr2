use super::App;
use crate::model::speed::SmoothedSpeed;
use crate::model::{Connection, Task};

impl App {
    pub(crate) fn conn_speed_of(&self, task_id: u32, conn_id: usize) -> f64 {
        self.conn_speed
            .get(&(task_id, conn_id))
            .map_or(0.0, SmoothedSpeed::value)
    }

    pub(crate) fn bt_conn_speeds_of(&self, task_id: u32, conn_id: usize) -> (f64, f64) {
        self.bt_conn_stats
            .get(&(task_id, conn_id))
            .map_or((0.0, 0.0), |s| (s.down_ema.value(), s.up_ema.value()))
    }

    pub(crate) fn conn_cum_of(&self, task_id: u32, conn_id: usize) -> u64 {
        self.conn_cum.get(&(task_id, conn_id)).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn global_dl_speed(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.state == crate::model::TaskState::Downloading)
            .map(|t| t.speed)
            .sum()
    }

    #[must_use]
    pub fn global_ul_speed(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.upload_speed > 0.0)
            .map(|t| t.upload_speed)
            .sum()
    }

    #[must_use]
    pub fn conns_display_total(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state.shows_conns())
            .map(|t| self.conns_actual_of(t))
            .sum()
    }

    /// 单任务的「实际并发连接数」（并发面板行数同口径）：
    /// BT = 引擎回报的已建立 peer 连接（握手完成；connections 恒空），
    /// HTTP = 当前活跃的分块连接。
    #[must_use]
    pub fn conns_actual_of(&self, t: &Task) -> usize {
        if !t.state.shows_conns() {
            return 0;
        }
        if t.protocol.is_bt() {
            // BT 的真实并发 = 引擎回报的 peer 连接，
            // 不用 HTTP lease 快照（BT 任务 connections 恒空）。
            self.bt_conns.get(&t.id).map_or(0, Vec::len)
        } else {
            t.connections
                .iter()
                .filter(|c| self.conn_is_active(t.id, c))
                .count()
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn active_conn_count(&self, t: &Task) -> usize {
        if !t.state.shows_conns() {
            return 0;
        }
        t.connections
            .iter()
            .filter(|c| self.conn_is_active(t.id, c))
            .count()
    }

    pub(crate) fn conn_is_active(&self, task_id: u32, c: &Connection) -> bool {
        let cum = self.conn_cum.get(&(task_id, c.id)).copied().unwrap_or(0);
        c.conn_phase(cum).is_active()
    }

    pub(crate) fn active_conn_ids(&self) -> Vec<usize> {
        self.sel_task()
            .filter(|t| t.state.shows_conns())
            .map(|t| {
                let mut ids: Vec<usize> = t
                    .connections
                    .iter()
                    .filter(|c| self.conn_is_active(t.id, c))
                    .map(|c| c.id)
                    .collect();
                ids.sort_unstable();
                ids
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn tab_counts(&self) -> [usize; 2] {
        let doing = self.tasks.iter().filter(|t| !t.state.is_done()).count();
        [doing, self.tasks.len() - doing]
    }

    #[must_use]
    pub fn peak_dl_speed(&self) -> f64 {
        self.speed_hist.iter().copied().max().unwrap_or(0) as f64 * 1024.0
    }

    #[must_use]
    pub fn peak_ul_speed(&self) -> f64 {
        self.up_hist.iter().copied().max().unwrap_or(0) as f64 * 1024.0
    }

    pub(crate) fn clear_conn_stats(&mut self, task_id: u32) {
        self.conn_prev.retain(|(tid, _), _| *tid != task_id);
        self.conn_cum.retain(|(tid, _), _| *tid != task_id);
        self.conn_speed.retain(|(tid, _), _| *tid != task_id);
        self.conn_windows.retain(|(tid, _), _| *tid != task_id);
    }

    pub(crate) fn clear_bt_conn_stats(&mut self, task_id: u32) {
        self.bt_conn_stats.retain(|(tid, _), _| *tid != task_id);
    }
}
