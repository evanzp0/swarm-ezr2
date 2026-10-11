//! 加固测试：`spread_bytes` / `lease_snapshot`（src/model/chunk/lease.rs 变异幸存体杀灭）。
//! 靶点（放量轮 lease.rs 幸存 17 中可杀 13 + 超时 2）：
//! - `60:51 / 63:52` 权重索引 `i % 16` → `i / 16`（形状破坏）；
//! - `63:34` `amount * (w/wsum)` → `+` / `/`；`63:58` `/wsum` → `%` / `*`；
//! - `70:21` 回补条件 `diff != 0` → `== 0`（原以大 n 超时假死，小输入直接观察不回补）；
//! - `72:21 / 76:24 / 76:36 / 77:25 / 78:18 / 79:18` 回补减法支路算术与守卫（需负 diff 场景，
//!   n=3 时 amount=9 即出现 raw 舍入和 > amount）；
//! - `32:23` `lease_from + i` → `*`；`38:41` `end - start` → `+`。
//!
//! 等价幸存体（57:20 哨兵早退、70:30 循环预算边界空转、72:17/72:29/76:24/76:36 守卫
//! 边界空转）不在本套件，等价论证记 handoff。

#![allow(missing_docs)]
// lease.rs 依赖 super::plan::chunk_total 与 crate::model::Connection：
// plan 挂为本模块下供 super::plan 解析；crate::model 由 hardening.rs 根提供。
#[path = "../../src/model/chunk/lease.rs"]
mod lease;
#[path = "../../src/model/chunk/plan.rs"]
mod plan;

use lease::{lease_snapshot, spread_bytes};

/// 靶 `60:51 / 63:52`（`i % 16` → `i / 16`）、`63:34`（`*`→`+`/`/`）、`63:58`（`/`→`%`/`*`）：
/// 权重比例分配的精确形状。w=[0.95,0.45,0.10]/1.5，amount=1000 → [633,300,67]。
#[test]
fn spread_shape_exact_weights() {
    assert_eq!(spread_bytes(1000, 3, 1000), vec![633, 300, 67]);
    // 含回补：raw=[4,2,0] 和为 6，diff=+1 回补到首槽 → [5,2,0]
    assert_eq!(spread_bytes(7, 3, 100), vec![5, 2, 0]);
}

/// 靶 `70:21`（`diff != 0` → `== 0`）：amount=7 raw 和为 6 必须回补 1 字节；
/// mutant 条件翻转后永不回补，和滞留 6。
#[test]
fn spread_topup_fires_on_rounding_deficit() {
    let v = spread_bytes(7, 3, u64::MAX);
    assert_eq!(
        v.iter().sum::<u64>(),
        7,
        "舍入亏空必须回补（mutant == 0 不回补）"
    );
}

/// 靶 `72:21 / 76:24(<→==) / 76:36(>→==,>→<) / 77:25 / 78:18(×2) / 79:18(×2)`：
/// 负 diff（舍入盈余）支路。n=3、amount=9：raw=[6,3,1] 和 10 > 9，正确行为减回
/// [5,3,1]；扫描 1..=20000 覆盖全部负 diff 输入（n=3 在 amount=9 即触发）。
#[test]
fn spread_conservation_scan_covers_negative_diff_branch() {
    let mut neg_diff_seen = 0usize;
    for a in 1..=20000u64 {
        let v = spread_bytes(a, 16, u64::MAX);
        let s: u64 = v.iter().sum();
        assert_eq!(s, a, "amount={a} 守恒破坏 v={v:?}");
        // 独立统计负 diff 输入确实存在（raw 舍入和 > amount）
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

/// 负 diff 支路的分布锚（靶 `77:25 delete -` 与 `81:11 +=→*=`）：
/// `77:25` 清槽-补偿后总量仍守恒但分布漂移（v[0] 归零、v[1] 吃满差值），
/// 守恒扫描杀不掉，须精确断言分布；`81:11` 的 `diff *= sub` 使回补跳变，
/// 同被分布锚暴露。期望值经独立实现（Python half-away-from-zero）锚定。
#[test]
fn spread_negative_diff_exact_distribution() {
    // a=24：raw 舍入和 25 > 24（diff=-1），回补从首槽减 1
    assert_eq!(
        spread_bytes(24, 16, u64::MAX),
        vec![2, 2, 0, 0, 3, 1, 3, 1, 2, 1, 2, 0, 2, 1, 3, 1]
    );
    // a=9 / a=12：小槽位负 diff 的分布锚（mutant 77 在 a=12 恰好绕回同分布，
    // 与 a=24 组成双锚防巧合）
    assert_eq!(spread_bytes(9, 3, u64::MAX), vec![5, 3, 1]);
    assert_eq!(spread_bytes(12, 3, u64::MAX), vec![7, 4, 1]);
}

/// 靶 `70:30` 反向护栏（等价体监控）：cap 钳制下回补在预算内完成时总量守恒；
/// 若未来实现改为更大预算消耗（如 8n → n），此断言将失败并触发等价论证重审。
#[test]
fn spread_clamped_topup_stays_consistent() {
    let v = spread_bytes(10_000, 16, 640);
    assert_eq!(v.iter().sum::<u64>(), 10_000);
    assert!(v.iter().all(|&x| x <= 640));
}

/// 哨兵行为（57:20 等价体的行为锚）：零量与零连接短路返回全零/空。
#[test]
fn spread_zero_amount_or_zero_n_short_circuit() {
    assert_eq!(spread_bytes(0, 5, 100), vec![0; 5]);
    assert!(spread_bytes(5, 0, 100).is_empty());
}

/// 靶 `32:23`（`id: i + 1` → `i * 1`）：连接序号必须 1 基。完成态展示最后 min(n, y)
/// 个满块。total=100、piece=30 → y=4；done=100、n=2 → 领块 idx=2,3：
/// 连接 [60,90) done=30 与 [90,100) done=10；x=4。
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

/// 领块态（active 部分）：total=100、piece=30、done=50、n=3 → x=1、rem=20、
/// partials=spread_bytes(20,3,30)=[13,6,1]，连接领块 idx=1,2,3。
/// （38:41 的 `end - start` → `+` 被 min 钳制掩盖：partial ≤ rem < 块长恒成立，
/// 属等价体，记 handoff；本测试为领块分布行为锚。）
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
