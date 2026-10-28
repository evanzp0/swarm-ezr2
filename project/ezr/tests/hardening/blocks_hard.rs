#![allow(missing_docs)]
use crate::model::chunk::blocks::Blocks;

fn blocks100() -> Blocks {
    Blocks::new(100, 30)
}

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

#[test]
fn downloaded_clamps_to_block_length() {
    let mut b = blocks100();
    b.written[0] = 999;
    b.written[1] = 999;
    let got = b.downloaded();
    assert_eq!(
        got,
        30 + 30,
        "块1 钳到 e-s=30；mutant 钳到 e+s=90（总 120）"
    );
}

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

#[test]
fn downloaded_tracks_normal_progress() {
    let mut b = blocks100();
    b.mark_progress(0, 12);
    b.mark_progress(1, 30);
    b.mark_done(2);
    assert_eq!(b.downloaded(), 12 + 30 + 30);
}
