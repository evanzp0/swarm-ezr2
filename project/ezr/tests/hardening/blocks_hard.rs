//! 加固测试：`Blocks`（src/model/chunk/blocks.rs 变异幸存体杀灭）。
//! 靶点（放量轮 blocks.rs 幸存 4，全部为 `e - s` → `e + s`）：
//! - `75:48` `downloaded` 的 `(*w).min(e - s)`：written 可直接构造超写值，
//!   mutant 的 min 上界 e+s 抬高使超写字节计入总量；
//! - `89:48` `is_done_block` 的 `*w >= e - s`：mutant 阈值 e+s 恒大于块长 → 恒 false；
//! - `98:24` `mark_done` 的 `*w = e - s`：mutant 写入 e+s（written 可读直接观察）；
//! - `107:33` `mark_progress` 的 `done.min(e - s)`：done 超块长时 mutant 钳到 e+s。
//!
//! 挂载：blocks.rs 引用 `super::{lease, plan}` 与内嵌 tests 引用
//! `crate::model::chunk::plan`，故按真实树形挂载 chunk 三模块；lease.rs 的
//! `crate::model::Connection` 解析到根 `model::Connection` 同构定义。

#![allow(missing_docs)]
// crate::model 由 hardening.rs 根提供（model::chunk::{plan,lease,blocks} 挂载壳）。
use crate::model::chunk::blocks::Blocks;

/// 场景基座：total=100、piece=30 → 4 块（30/30/30/10），末块收口 total。
fn blocks100() -> Blocks {
    Blocks::new(100, 30)
}

/// 靶 `89:48`：满块判定。mutant `w >= e+s` 恒 false（e+s 超块长）。
/// 注意首块 s=0 时 e-s == e+s 巧合（30>=30），必须用 s>0 的块（块 1 [30,60)：
/// 原版 30>=30 true，mutant 30>=90 false）。
#[test]
fn is_done_block_true_at_full_length() {
    let mut b = blocks100();
    b.mark_done(0);
    assert!(
        b.is_done_block(0),
        "首块满块（与 mutant 巧合同值，仅作对照）"
    );
    b.mark_done(1);
    assert!(b.is_done_block(1), "s>0 满块：mutant 阈值 e+s=90 恒 false");
    assert!(!b.is_done_block(2), "未写块必须 false（对照）");
}

/// 靶 `98:24`：mark_done 必须恰好写块长（written pub 可读）。
#[test]
fn mark_done_writes_exact_block_length() {
    let mut b = blocks100();
    b.mark_done(3);
    assert_eq!(b.written[3], 10, "末块块长 10；mutant e+s = 30+90 = 120");
    b.mark_done(0);
    assert_eq!(
        b.written[0], 30,
        "mutant e+s = 30+0 = 30 与块长巧合，用末块区分"
    );
}

/// 靶 `75:48`：downloaded 的钳制上界必须是块长 e-s（超写状态由直接构造给出）。
#[test]
fn downloaded_clamps_to_block_length() {
    let mut b = blocks100();
    b.written[0] = 999; // 首块 s=0：e-s == e+s 巧合同值（30），不构成区分度
    b.written[1] = 999; // 块 1 [30,60)：正确上界 30，mutant 上界 e+s=90
    let got = b.downloaded();
    assert_eq!(
        got,
        30 + 30,
        "块1 钳到 e-s=30；mutant 钳到 e+s=90（总 120）"
    );
}

/// 靶 `107:33`：mark_progress 越界钳制到块长 e-s（非 e+s）。
#[test]
fn mark_progress_clamps_to_block_length() {
    let mut b = blocks100();
    b.mark_progress(2, 500);
    assert_eq!(
        b.written[2], 30,
        "块 2 [60,90) 块长 30；mutant 钳到 60+90=150"
    );
    b.mark_progress(2, 17);
    assert_eq!(b.written[2], 17, "合法值直通（对照）");
}

/// 综合对照：正常进度路径总量守恒（防过度钳制）。
#[test]
fn downloaded_tracks_normal_progress() {
    let mut b = blocks100();
    b.mark_progress(0, 12);
    b.mark_progress(1, 30);
    b.mark_done(2);
    assert_eq!(b.downloaded(), 12 + 30 + 30);
}
