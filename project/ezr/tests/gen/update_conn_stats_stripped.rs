/// 连接级账本键（任务 id, 连接 id）与观测三元组（块号, 块内已写, 时刻）
type ConnKey = (u32, usize);
type ConnObs = (u32, u64, Instant);

/// 连接级账本推进（FR-01-99，App 消费侧计量）：按 (任务, 连接) 跟踪 Progress
/// 事件的块内 done 增量——同块 = 差值；换块/首次观测 = 新块内已写字节（本轮
/// 下载内的落盘量）；增量/时间差经 EMA 得展示速度，累计量随增量累加。
/// 待命/空连接（cap = 0 或块已完成）零值速断（`zero()`，无拖尾），
/// 在传连接即使瞬时零增量也持续平滑（避免 `-` 闪烁，FR-01-99 ①）。
fn update_conn_stats(
    conn_prev: &mut HashMap<ConnKey, ConnObs>,
    conn_cum: &mut HashMap<ConnKey, u64>,
    conn_speed: &mut HashMap<ConnKey, SmoothedSpeed>,
    task_id: u32,
    conns: &[crate::engine::ConnView],
) {
    let now = Instant::now();
    for c in conns {
        let key = (task_id, c.id);
        let (delta, dt) = match conn_prev.get(&key) {
            Some(&(pb, pd, pt)) if pb == c.block => (
                c.done.saturating_sub(pd),
                now.duration_since(pt).as_secs_f64(),
            ),
            // 换块（领新块）或首次观测：增量 = 新块内已写字节（均为本轮下载内落盘）
            _ => (c.done, 0.0),
        };
        conn_prev.insert(key, (c.block, c.done, now));
        let cap = c.end.saturating_sub(c.start);
        let active = cap > 0 && c.done < cap;
        let d = conn_speed.entry(key).or_default();
        if !active {
            d.zero();
        } else if dt > 0.0 {
            // 首次观测/换块帧无时间差，只累计不复算速度（EMA 下一帧收敛）
            d.push(delta as f64 / dt);
        }
        if delta > 0 {
            *conn_cum.entry(key).or_insert(0) += delta;
        }
    }
}
