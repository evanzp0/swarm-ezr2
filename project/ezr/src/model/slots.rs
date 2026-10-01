//! slots — 下载槽位调度（FR-01-31/32：默认 5 可配置，按列表顺序分配）
//!
//! 占用 = 「下载中」∪「校验中」（D12）∪「已失败」且将自动重试（未达上限且
//! 属可自动重试类别）∪ 已获槽位的等待中任务；
//! 释放 = 校验成功/完成转「已完成」、手动暂停、达重试上限或停等失败、删除任务。
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


use super::{Task, TaskState};

/// 槽位不变式维护：各状态任务的 `has_slot` 修正（每 tick 开头执行）。
/// - 下载中/校验中恒持有；
/// - 已失败：将自动重试（`retry_in` 有值）持有，停等释放；
/// - 其余状态（除持有槽位的等待中）恒不持有。
pub fn enforce_invariants(tasks: &mut [Task]) {
    for t in tasks.iter_mut() {
        match t.state {
            TaskState::Downloading | TaskState::Verifying => t.has_slot = true,
            TaskState::Failed => t.has_slot = t.retry_in.is_some(),
            TaskState::Queued => {}
            _ => t.has_slot = false,
        }
    }
}

/// 当前占用槽位数（FR-01-31 口径）
#[must_use]
pub fn used(tasks: &[Task]) -> usize {
    tasks
        .iter()
        .filter(|t| match t.state {
            TaskState::Downloading | TaskState::Verifying => true,
            TaskState::Failed => t.retry_in.is_some(),
            TaskState::Queued => t.has_slot,
            _ => false,
        })
        .count()
}

/// 空闲槽位按列表顺序（从上往下）分配给最靠前的未持有槽位的「等待中」任务
/// （FR-01-32）。返回本次新获得槽位的任务 ID 列表（按分配顺序）。
pub fn allocate(tasks: &mut [Task], max_slots: usize) -> Vec<u32> {
    enforce_invariants(tasks);
    let mut free = max_slots.saturating_sub(used(tasks));
    let mut granted = Vec::new();
    for t in tasks.iter_mut() {
        if free == 0 {
            break;
        }
        if t.state == TaskState::Queued && !t.has_slot {
            t.has_slot = true;
            granted.push(t.id);
            free -= 1;
        }
    }
    granted
}

/// 等待任务在队列中的位次（1 基，按列表顺序；已持有槽位者不算位次）
#[must_use]
pub fn queue_pos(tasks: &[Task], task_id: u32) -> Option<usize> {
    let mut pos = 0;
    for t in tasks {
        if t.state == TaskState::Queued && !t.has_slot {
            pos += 1;
            if t.id == task_id {
                return Some(pos);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Protocol, TaskState};

    fn mk(id: u32, state: TaskState) -> Task {
        Task {
            id,
            name: format!("t{id}"),
            protocol: Protocol::Http,
            url: String::new(),
            final_url: None,
            save_dir: String::new(),
            total: 0,
            downloaded: 0,
            speed: 0.0,
            state,
            resumable: true,
            probed: false,
            connections: vec![],
            chunk_done: 0,
            block_size: 1_048_576,
            concurrency: 4,
            retries: 0,
            max_retries: 5,
            made_progress: false,
            retry_in: None,
            fail_kind: None,
            error: None,
            invalidation_streak: 0,
            checksum: None,
            verify_ok: None,
            etag: None,
            last_modified: None,
            has_slot: false,
            upload_speed: 0.0,
            uploaded: 0,
            seeders: 0,
            peers: 0,
            seed_left: 0.0,
            elapsed: 0.0,
            created: String::new(),
            added_at: 0,
        }
    }

    #[test]
    fn downloading_and_verifying_hold_slots() {
        let mut tasks = vec![mk(1, TaskState::Downloading), mk(2, TaskState::Verifying)];
        enforce_invariants(&mut tasks);
        assert_eq!(used(&tasks), 2);
    }

    #[test]
    fn failed_auto_retry_holds_but_stopped_waits_not() {
        let mut a = mk(1, TaskState::Failed);
        a.retry_in = Some(8.0);
        let mut b = mk(2, TaskState::Failed);
        b.retry_in = None; // 停等
        let mut tasks = vec![a, b];
        enforce_invariants(&mut tasks);
        assert!(tasks[0].has_slot);
        assert!(!tasks[1].has_slot);
        assert_eq!(used(&tasks), 1);
    }

    #[test]
    fn allocate_in_list_order() {
        let mut tasks = vec![
            mk(1, TaskState::Queued),
            mk(2, TaskState::Queued),
            mk(3, TaskState::Queued),
        ];
        let granted = allocate(&mut tasks, 2);
        assert_eq!(granted, vec![1, 2]);
        assert!(tasks[0].has_slot && tasks[1].has_slot && !tasks[2].has_slot);
        assert_eq!(used(&tasks), 2);
    }

    #[test]
    fn queue_position_counts_undecorated_only() {
        let mut tasks = vec![mk(1, TaskState::Queued), mk(2, TaskState::Queued)];
        allocate(&mut tasks, 1);
        // 任务 1 获槽（无位次），任务 2 排队第 1 位
        assert_eq!(queue_pos(&tasks, 1), None);
        assert_eq!(queue_pos(&tasks, 2), Some(1));
    }

    #[test]
    fn released_slot_granted_to_front_waiting_task() {
        let mut tasks = vec![mk(1, TaskState::Queued), mk(2, TaskState::Queued)];
        allocate(&mut tasks, 2);
        tasks[0].state = TaskState::Paused; // 暂停释放
        enforce_invariants(&mut tasks);
        assert_eq!(used(&tasks), 1);
        // 任务 1 重新排队（失败重试路径）：仍在列表最前，按顺序优先获得空闲槽位
        tasks[0].state = TaskState::Queued;
        tasks[0].has_slot = false;
        tasks.push(mk(3, TaskState::Queued));
        let granted = allocate(&mut tasks, 2);
        assert_eq!(granted, vec![1]);
    }
}
