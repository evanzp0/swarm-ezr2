/// 连接级账本键（任务 id, 连接 id）与观测对（块号, 块内已写字节）
type ConnKey = (u32, usize);
type ConnObs = (u32, u64);

/// 连接级账本推进（FR-01-99 ②累计 + FR-01-102 数据面，App 消费侧计量）：
/// 按 (任务, 连接) 跟踪 Progress 事件的块内 done 增量——同块 = 差值；
/// 换块/首次观测 = 新块内已写字节（本轮下载内的落盘量）；累计量随增量累加。
/// 在传连接（持块未写满，或流式单连接 done > 0）的 1s 滑窗以连接级累计字节读数
/// 推进（每次事件 push，任务级 SpeedWindow 同构，FR-01-102 ①）；展示面采样
/// （每秒一次 + EMA）在 tick ⑤。待命/空连接（cap = 0 且 done = 0，或块已完成）
/// 不推进窗口（展示面零值速断，FR-01-102 ③）；窗口端点老化（停传 > 1s：暂停
/// 恢复、块间隙）先清窗再推，速率只测最近 1s 增量。
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
        // 在传判定（单源 [`crate::model::conn_transferring`]）：持块连接 =
        // 未写满（done < cap）；流式单连接（FR-01-12 降级路径，无 Content-Length
        // → cap = 0）= done > 0——速度窗口正常推进，并发面板明细/头部并发随
        // ConnPhase 活跃口径同步显示。展示面采样（tick ⑤）同门消费
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
