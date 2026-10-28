use super::{Task, TaskState};

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

#[must_use]
pub fn queue_positions(tasks: &[Task], view: &[usize]) -> Vec<usize> {
    let mut waiting = 0usize;
    view.iter()
        .map(|&ti| {
            let t = &tasks[ti];
            if t.state == TaskState::Queued && !t.has_slot {
                waiting += 1;
                waiting
            } else {
                0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{sample_task, TaskState};

    fn mk(id: u32, state: TaskState) -> Task {
        let mut t = sample_task();
        t.id = id;
        t.name = format!("t{id}");
        t.state = state;
        t
    }

    #[test]
    fn paused_holds_no_slot() {
        let mut t = mk(1, TaskState::Paused);
        t.has_slot = true;
        let mut tasks = vec![t];
        enforce_invariants(&mut tasks);
        assert!(!tasks[0].has_slot, "暂停态（含失败挂起）恒不持有槽位");
        assert_eq!(used(&tasks), 0, "暂停态不占槽");
        assert!(allocate(&mut tasks, 5).is_empty(), "暂停态不参与分配");
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
        b.retry_in = None;
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
        assert_eq!(queue_pos(&tasks, 1), None);
        assert_eq!(queue_pos(&tasks, 2), Some(1));
    }

    #[test]
    fn released_slot_granted_to_front_waiting_task() {
        let mut tasks = vec![mk(1, TaskState::Queued), mk(2, TaskState::Queued)];
        allocate(&mut tasks, 2);
        tasks[0].state = TaskState::Paused;
        enforce_invariants(&mut tasks);
        assert_eq!(used(&tasks), 1);
        tasks[0].state = TaskState::Queued;
        tasks[0].has_slot = false;
        tasks.push(mk(3, TaskState::Queued));
        let granted = allocate(&mut tasks, 2);
        assert_eq!(granted, vec![1]);
    }
}
