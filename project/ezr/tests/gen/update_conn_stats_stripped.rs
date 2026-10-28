type ConnKey = (u32, usize);
type ConnObs = (u32, u64);

fn update_conn_stats(
    conn_prev: &mut HashMap<ConnKey, ConnObs>,
    conn_cum: &mut HashMap<ConnKey, u64>,
    conn_windows: &mut HashMap<ConnKey, SpeedWindow>,
    task_id: u32,
    conns: &[crate::engine::ConnView],
) {
    let now = Instant::now();
    for c in conns {
        let key = (task_id, c.id);
        let delta = match conn_prev.get(&key) {
            Some(&(pb, pd)) if pb == c.block => c.done.saturating_sub(pd),
            _ => c.done,
        };
        conn_prev.insert(key, (c.block, c.done));
        let cap = c.end.saturating_sub(c.start);
        let active = crate::model::conn_transferring(cap, c.done);
        if delta > 0 {
            *conn_cum.entry(key).or_insert(0) += delta;
        }
        if active {
            let w = conn_windows.entry(key).or_default();
            if w.last_push()
                .is_some_and(|t| now.duration_since(t) > WINDOW)
            {
                *w = SpeedWindow::new();
            }
            w.push(now, conn_cum.get(&key).copied().unwrap_or(0));
        }
    }
}
