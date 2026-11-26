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

#[cfg(test)]
mod stats_hardening {
    use super::{App, Connection};
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-sh", tag)
    }

    fn conn(id: usize, start: u64, end: u64, done: u64) -> Connection {
        Connection {
            id,
            start,
            end,
            done,
        }
    }

    fn task_with_conns_at(id: u32, name: &str, conns: Vec<Connection>) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.protocol = crate::model::Protocol::Http;
        t.name = name.to_string();
        t.state = TaskState::Downloading;
        t.connections = conns;
        t
    }

    #[tokio::test]
    async fn global_dl_speed_sums_downloading_tasks_only() {
        let mut app = make_app("gdl");
        let mut dl = crate::model::sample_task();
        dl.id = 1;
        dl.state = TaskState::Downloading;
        dl.speed = 100.0;
        let mut paused = crate::model::sample_task();
        paused.id = 2;
        paused.state = TaskState::Paused;
        paused.speed = 50.0;
        app.tasks.push(dl);
        app.tasks.push(paused);
        assert_eq!(
            app.global_dl_speed(),
            100.0,
            "全局下载速度 = 仅下载中任务速度之和"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conns_display_total_counts_active_http_conns() {
        let mut app = make_app("cdt");
        let t1 = task_with_conns_at(
            1,
            "a.bin",
            vec![conn(0, 0, 100, 0), conn(1, 0, 0, 0), conn(2, 100, 200, 0)],
        );
        let t2 = task_with_conns_at(2, "b.bin", vec![conn(0, 0, 50, 0)]);
        app.tasks.push(t1);
        app.tasks.push(t2);
        assert_eq!(
            app.conns_display_total(),
            3,
            "display 总数 = 活跃连接（cap>0 未完成）计数，Closed 不计"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conn_is_active_rejects_closed_phase() {
        let mut app = make_app("cia");
        let t = task_with_conns_at(1, "a.bin", vec![conn(0, 0, 100, 0), conn(1, 0, 0, 0)]);
        app.tasks.push(t);
        let tref = &app.tasks[0];
        let c_active = tref.connections[0].clone();
        let c_closed = tref.connections[1].clone();
        assert!(
            app.conn_is_active(1, &c_active),
            "cap>0 且零进度 → Connecting 活跃"
        );
        assert!(
            !app.conn_is_active(1, &c_closed),
            "cap==0 且零进度 → Closed 非活跃"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn peak_ul_speed_is_hist_max_scaled() {
        let mut app = make_app("pul");
        app.up_hist = vec![1, 5, 3];
        assert_eq!(app.peak_ul_speed(), 5120.0, "峰值上传 = 历史最大 × 1024");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn clear_conn_stats_retains_other_tasks_entries() {
        let mut app = make_app("ccs");
        app.conn_cum.insert((1, 0), 7);
        app.conn_cum.insert((2, 0), 9);
        app.conn_speed
            .insert((1, 0), crate::model::speed::SmoothedSpeed::default());
        app.conn_speed
            .insert((2, 0), crate::model::speed::SmoothedSpeed::default());
        app.conn_windows
            .insert((1, 0), crate::model::speed::SpeedWindow::new());
        app.conn_windows
            .insert((2, 0), crate::model::speed::SpeedWindow::new());
        app.clear_conn_stats(1);
        assert!(!app.conn_cum.contains_key(&(1, 0)), "任务 1 的 cum 被清");
        assert!(app.conn_cum.contains_key(&(2, 0)), "任务 2 的 cum 必须保留");
        assert!(!app.conn_speed.contains_key(&(1, 0)), "任务 1 的速度被清");
        assert!(
            app.conn_speed.contains_key(&(2, 0)),
            "任务 2 的速度必须保留"
        );
        assert!(!app.conn_windows.contains_key(&(1, 0)), "任务 1 的窗口被清");
        assert!(
            app.conn_windows.contains_key(&(2, 0)),
            "任务 2 的窗口必须保留"
        );
        app.shutdown().await;
    }
}
