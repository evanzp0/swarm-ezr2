/// 连接级账本键（任务 id, 连接 id）与观测对（块号, 块内已写字节）
type ConnKey = (u32, usize);
type ConnObs = (u32, u64);

/// 连接级账本推进（FR-01-99 ②累计 + FR-01-102 数据面，App 消费侧计量）：
/// 按 (任务, 连接) 跟踪 Progress 事件的块内 done 增量——同块 = 差值；
/// 换块/首次观测 = 新块内已写字节（本轮下载内的落盘量）；累计量随增量累加。
/// 在传连接（cap > 0 且未写满）的 1s 滑窗以连接级累计字节读数推进（每次事件 push，
/// 任务级 SpeedWindow 同构，FR-01-102 ①）；展示面采样（每秒一次 + EMA）在 tick ⑤。
/// 待命/空连接（cap = 0 或块已完成）不推进窗口（展示面零值速断，FR-01-102 ③）；
/// 窗口端点老化（停传 > 1s：暂停恢复、块间隙）先清窗再推，速率只测最近 1s 增量。
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
            // 换块（领新块）或首次观测：增量 = 新块内已写字节（均为本轮下载内落盘）
            _ => c.done,
        };
        conn_prev.insert(key, (c.block, c.done));
        let cap = c.end.saturating_sub(c.start);
        let active = cap > 0 && c.done < cap;
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
