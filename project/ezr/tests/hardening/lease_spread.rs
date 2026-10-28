#![allow(missing_docs)]
#[path = "../../src/model/chunk/lease.rs"]
mod lease;
#[path = "../../src/model/chunk/plan.rs"]
mod plan;

use lease::{lease_snapshot, spread_bytes};

#[test]
fn spread_shape_exact_weights() {
    assert_eq!(spread_bytes(1000, 3, 1000), vec![633, 300, 67]);
    assert_eq!(spread_bytes(7, 3, 100), vec![5, 2, 0]);
}

#[test]
fn spread_topup_fires_on_rounding_deficit() {
    let v = spread_bytes(7, 3, u64::MAX);
    assert_eq!(
        v.iter().sum::<u64>(),
        7,
        "舍入亏空必须回补（mutant == 0 不回补）"
    );
}

#[test]
fn spread_conservation_scan_covers_negative_diff_branch() {
    let mut neg_diff_seen = 0usize;
    for a in 1..=20000u64 {
        let v = spread_bytes(a, 16, u64::MAX);
        let s: u64 = v.iter().sum();
        assert_eq!(s, a, "amount={a} 守恒破坏 v={v:?}");
        let wsum = 7.05f64;
        let raws: u64 = (0..16)
            .map(|i| {
                (a as f64
                    * ([
                        0.95, 0.45, 0.10, 0.00, 0.75, 0.40, 0.85, 0.15, 0.55, 0.20, 0.55, 0.00,
                        0.70, 0.25, 0.80, 0.35,
                    ][i] / wsum))
                    .round() as u64
            })
            .sum();
        if raws > a {
            neg_diff_seen += 1;
        }
    }
    assert!(
        neg_diff_seen > 0,
        "扫描区间必须含负 diff 输入，否则该测试失去杀灭力"
    );
}

#[test]
fn spread_negative_diff_exact_distribution() {
    assert_eq!(
        spread_bytes(24, 16, u64::MAX),
        vec![2, 2, 0, 0, 3, 1, 3, 1, 2, 1, 2, 0, 2, 1, 3, 1]
    );
    assert_eq!(spread_bytes(9, 3, u64::MAX), vec![5, 3, 1]);
    assert_eq!(spread_bytes(12, 3, u64::MAX), vec![7, 4, 1]);
}

#[test]
fn spread_clamped_topup_stays_consistent() {
    let v = spread_bytes(10_000, 16, 640);
    assert_eq!(v.iter().sum::<u64>(), 10_000);
    assert!(v.iter().all(|&x| x <= 640));
}

#[test]
fn spread_zero_amount_or_zero_n_short_circuit() {
    assert_eq!(spread_bytes(0, 5, 100), vec![0; 5]);
    assert!(spread_bytes(5, 0, 100).is_empty());
}

#[test]
fn lease_snapshot_done_state_tail_blocks() {
    let (conns, x) = lease_snapshot(100, 100, 2, 30);
    assert_eq!(x, 4);
    assert_eq!(conns.len(), 2);
    assert_eq!(conns[0].id, 1, "id 1 基：mutant i*1 变体给出 0");
    assert_eq!(conns[1].id, 2);
    assert_eq!(conns[0].start, 60);
    assert_eq!(conns[0].end, 90);
    assert_eq!(conns[0].done, 30);
    assert_eq!(conns[1].start, 90);
    assert_eq!(conns[1].end, 100);
    assert_eq!(conns[1].done, 10);
}

#[test]
fn lease_snapshot_active_partial_distribution() {
    let (conns, x) = lease_snapshot(100, 50, 3, 30);
    assert_eq!(x, 1);
    assert_eq!(conns.len(), 3);
    assert_eq!(conns[0].start, 30);
    assert_eq!(conns[0].end, 60);
    assert_eq!(conns[0].done, 13);
    assert_eq!(conns[1].start, 60);
    assert_eq!(conns[1].end, 90);
    assert_eq!(conns[1].done, 6);
    assert_eq!(conns[2].start, 90);
    assert_eq!(conns[2].end, 100, "末块收口到 total");
    assert_eq!(conns[2].done, 1);
}
