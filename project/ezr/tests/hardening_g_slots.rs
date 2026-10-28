#![allow(missing_docs)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

#[path = "../src/model/mod.rs"]
mod model;

use model::{sample_task, TaskState};

#[test]
fn enforce_invariants_grants_slot_to_downloading_and_verifying() {
    let mut dl = sample_task();
    dl.state = TaskState::Downloading;
    dl.has_slot = false;
    let mut vf = sample_task();
    vf.state = TaskState::Verifying;
    vf.has_slot = false;
    let mut tasks = vec![dl, vf];
    model::slots::enforce_invariants(&mut tasks);
    assert!(tasks[0].has_slot, "下载中恒持有槽位（不变式强制置位）");
    assert!(tasks[1].has_slot, "校验中恒持有槽位（不变式强制置位）");
}

#[test]
fn queue_positions_filters_state_and_slot_in_view_order() {
    let mut tasks = vec![sample_task(), sample_task(), sample_task(), sample_task()];
    tasks[0].state = TaskState::Queued;
    tasks[1].state = TaskState::Paused;
    tasks[2].state = TaskState::Queued;
    tasks[2].has_slot = true;
    tasks[3].state = TaskState::Queued;
    let view = vec![0, 1, 2, 3];
    assert_eq!(
        model::slots::queue_positions(&tasks, &view),
        vec![1, 0, 0, 2],
        "位次 = 视图序中「Queued ∧ 未持有槽位」任务的 1 基计数，其余恒 0"
    );
}

#[test]
fn queue_positions_counts_incrementally_across_view() {
    let mut tasks = vec![sample_task(), sample_task(), sample_task()];
    for t in tasks.iter_mut() {
        t.state = TaskState::Queued;
    }
    let view = vec![2, 0, 1];
    assert_eq!(
        model::slots::queue_positions(&tasks, &view),
        vec![1, 2, 3],
        "位次按视图顺序累加，不受列表下标序影响"
    );
}
