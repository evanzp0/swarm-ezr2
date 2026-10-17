//! 加固测试（six-pack/hardender v137 轮）：model 层 slots 存活体杀灭。
//! 靶点（.work/tmp/mutants-out-slots/mutants.out missed.txt，phase-1 口径
//! `--bin ezr` 单测）：
//! - `enforce_invariants` 17:13 delete match arm Downloading|Verifying——
//!   「下载中/校验中恒持有槽位」不变式的置位方向此前无直接断言（既有测试
//!   只经 `used()` 间接对账，`used` 对两态无条件计 true，杀不灭删臂）；
//! - `queue_positions` 84:24（==→!=）/ 84:45（&&→||）/ 84:48（delete !）/
//!   85:25（+=→*=）——该函数唯一直接覆盖是属性测试
//!   （prop_queue_positions_prefix_count_oracle），按 engineering.md 属性
//!   测试不并入变异被测命令 → 以精确单测杀灭。
//!
//! 布局：#[path] 挂载 src/model/mod.rs（slots 的 `super::{Task, TaskState}`
//! 与内嵌单测的 `crate::model::sample_task` 由此解析）。
//!
//! 入口命令：`cargo test --test hardening_g_slots`
//!
//! lint 姿态与既有加固套件对齐：#[path] 收编的产品源在本测试 crate 内沿用
//! 产品面的豁免口径（产品面 clippy 0 的同一合同）。
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
// 测试挂载机制固有豁免：产品源经 #[path] 部分收编进独立测试 crate，部分产物项
// 在本 crate 上下文无消费者（产品全量编译下均非死代码）。
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

#[path = "../src/model/mod.rs"]
mod model;

use model::{sample_task, TaskState};

/// 靶 enforce_invariants 17:13：下载中/校验中恒持有槽位。
/// 先置 `has_slot = false`（违反不变式态），验证每 tick 开头的不变式修正
/// 强制置位——删除 Downloading|Verifying 臂后两断言即红。
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

/// 靶 queue_positions 84:24（==→!=）/ 84:45（&&→||）/ 84:48（delete !）：
/// 位次只计「等待中且未持有槽位」的任务——非等待状态与已获槽位者恒 0，
/// 等待任务按视图出现顺序得 1 基连续位次。
#[test]
fn queue_positions_filters_state_and_slot_in_view_order() {
    let mut tasks = vec![
        sample_task(), // [0] 等待中未持有 → 位次 1
        sample_task(), // [1] 已暂停 → 0（==→!= 与 &&→|| 会误计）
        sample_task(), // [2] 等待中已持有槽位 → 0（delete ! 会误计）
        sample_task(), // [3] 等待中未持有 → 位次 2
    ];
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

/// 靶 queue_positions 85:25（+=→*=）：位次按视图顺序逐个累加（*= 恒乘 0/1
/// 使第二个及以后等待任务全部塌缩为 0/1）。
#[test]
fn queue_positions_counts_incrementally_across_view() {
    let mut tasks = vec![sample_task(), sample_task(), sample_task()];
    for t in tasks.iter_mut() {
        t.state = TaskState::Queued;
    }
    // 视图序与列表序刻意不同：位次跟随给定视图顺序累计（1、2、3）
    let view = vec![2, 0, 1];
    assert_eq!(
        model::slots::queue_positions(&tasks, &view),
        vec![1, 2, 3],
        "位次按视图顺序累加，不受列表下标序影响"
    );
}
