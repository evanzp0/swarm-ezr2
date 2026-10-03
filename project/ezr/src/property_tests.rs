//! property_tests — 属性测试（architect 交付物；与常规单测对账隔离）
//!
//! 依据 notes/rust.md「proptest 作为 Rust 属性测试框架」条款：独立模块、
//! 独立运行命令（`cargo test property_tests::`）、独立报告数量——属性测试
//! 随机输入驱动的「通过」是概率性的，失败不与单测基线（221）对账。
//!
//! 覆盖目标（SKILL「属性测试」条款的纯逻辑面选取）：不变量、守恒性、
//! 幂等性、单调/排序、边界钳制——集中在 model 层纯函数：
//! - `chunk::plan` 分块数学：块数 = ⌈total/piece⌉、块区间无缝铺满 `[0, total)`、
//!   末块吸收余数、越界 None；
//! - `chunk::lease::spread_bytes` 加权散布：总和守恒 `min(amount, n*cap)`、
//!   逐份钳制 `[0, cap]`；
//! - `chunk::lease::lease_snapshot` 快照不变量：区间有序不重叠、`done` 不越
//!   块界、完成块数口径、完成态全块写满；
//! - `retry` 重试决策：进展重置/无进展累加连续性、at_limit 与上限等价、
//!   退避序列单调封顶、Retry-After 优先；
//! - `namegen` 命名：sanitize 幂等且清除路径分隔符、join_path 尾分隔符幂等、
//!   dedupe 结果不被占用集合命中；
//! - `speed::SmoothedSpeed` EMA：值域有界（非负采样下 ∈ [0, max]）、恒定输入
//!   距离单调收敛、zero() 立即归零。

use proptest::prelude::*;

use crate::model::chunk::{block_range, chunk_total, fmt_block_size, lease_snapshot, spread_bytes};
use crate::model::namegen::{dedupe, join_path, sanitize_name};
use crate::model::retry::{backoff_secs, decide};
use crate::model::speed::SmoothedSpeed;
use crate::model::FailKind;

// ---------------------------------------------------------------------
// chunk::plan — 分块数学（不变量 / 铺满 / 末块余数）
// ---------------------------------------------------------------------

proptest! {
    /// 块数不变量：chunk_total = ⌈total/piece⌉（u32::MAX 封顶口径；0 值口径：
    /// total=0 → 0）
    #[test]
    fn prop_chunk_total_is_ceil_div(
        total in 0u64..(1u64 << 48),
        piece in 1u64..(1u64 << 24),
    ) {
        let expected = total.div_ceil(piece).min(u32::MAX as u64) as u32;
        prop_assert_eq!(chunk_total(total, piece), expected);
        prop_assert_eq!(chunk_total(0, piece), 0);
    }

    /// 块区间铺满不变量：相邻块首尾相接（end_i == start_{i+1}），非末块长度
    /// 恒等于 piece，末块 end == total（末块吸收余数）；越界返回 None。
    /// （assume 排除 u32::MAX 封顶角例：封顶数学上不再保证铺满，属超界域）
    #[test]
    fn prop_block_ranges_tile_total(
        total in 1u64..(1u64 << 48),
        piece in 1u64..(1u64 << 24),
        probe in 0u32..1000,
    ) {
        let y = chunk_total(total, piece);
        prop_assume!(y < u32::MAX - 1, "排除块数封顶角例");
        // 末块与越界
        let last = block_range(total, piece, y - 1);
        prop_assert_eq!(last.map(|(_, end)| end), Some(total), "末块 end = total");
        prop_assert!(block_range(total, piece, y).is_none(), "越界应 None");
        // 采样相邻对：首尾相接 + 非末块定长 + 区间不越总界
        let i = u64::from(probe) % u64::from(y);
        let (s0, e0) = block_range(total, piece, i as u32).unwrap();
        prop_assert!(s0 < e0 && e0 <= total);
        if i + 1 < u64::from(y) {
            let (s1, _) = block_range(total, piece, i as u32 + 1).unwrap();
            prop_assert_eq!(e0, s1, "相邻块首尾相接");
            prop_assert_eq!(e0 - s0, piece, "非末块长度 = piece");
        }
    }

    /// fmt_block_size 展示稳定性：MB 整倍数固定「N MB」、KB 整倍数「N KB」、
    /// 其余「N B」（不 panic、非空）
    #[test]
    fn prop_fmt_block_size_stable(bytes in 0u64..(1u64 << 40)) {
        let s = fmt_block_size(bytes);
        prop_assert!(!s.is_empty());
        let mb = 1024 * 1024;
        let kb = 1024;
        if bytes >= mb && bytes % mb == 0 {
            prop_assert_eq!(s, format!("{} MB", bytes / mb));
        } else if bytes >= kb && bytes % kb == 0 {
            prop_assert_eq!(s, format!("{} KB", bytes / kb));
        } else {
            prop_assert_eq!(s, format!("{bytes} B"));
        }
    }
}

// ---------------------------------------------------------------------
// chunk::lease — 散布守恒与快照不变量（守恒性 / 钳制 / 有序）
// ---------------------------------------------------------------------

proptest! {
    /// spread_bytes 守恒：总和精确 = min(amount, n*cap)（容量不足时钳到总容量），
    /// 逐份 ∈ [0, cap]；amount=0 全零
    #[test]
    fn prop_spread_bytes_conserves(
        amount in 0u64..(1u64 << 32),
        n in 0usize..24,
        cap in 0u64..(1u64 << 20),
    ) {
        let v = spread_bytes(amount, n, cap);
        prop_assert_eq!(v.len(), n);
        let capacity = u64::try_from(n).unwrap_or(u64::MAX).saturating_mul(cap);
        let sum: u64 = v.iter().sum();
        prop_assert_eq!(sum, amount.min(capacity), "总和守恒（容量钳制口径）");
        for (i, x) in v.iter().enumerate() {
            prop_assert!(*x <= cap, "第 {i} 份超帽");
        }
        if amount == 0 {
            prop_assert!(v.iter().all(|x| *x == 0));
        }
    }

    /// lease_snapshot 快照不变量：区间单调不重叠且不越 [0, total)；每连接
    /// done ∈ [0, 块长]；完成块数口径（未完成 done/piece 向下、完成态 = 总块数）；
    /// 完成态每连接写满整块
    #[test]
    fn prop_lease_snapshot_invariants(
        total in 1u64..(1u64 << 40),
        piece in 1u64..(1u64 << 20),
        n in 1usize..17,
        done_raw in 0u64..(1u64 << 41),
    ) {
        let (conns, x) = lease_snapshot(total, done_raw, n, piece);
        let y = u64::from(chunk_total(total, piece));
        let done = done_raw.min(total);
        // 完成块数口径
        if done >= total {
            prop_assert_eq!(u64::from(x), y, "完成态：x = 总块数");
        } else {
            prop_assert_eq!(u64::from(x), done / piece, "未完成：x = done/piece 向下");
        }
        // 区间几何不变量
        let mut prev_end: Option<u64> = None;
        for c in &conns {
            prop_assert!(c.start < c.end && c.end <= total, "区间在 [0, total) 内");
            prop_assert!(c.done <= c.end - c.start, "done 不越块长");
            if let Some(pe) = prev_end {
                prop_assert_eq!(c.start, pe, "区间首尾相接（无重叠无缝隙）");
            }
            prev_end = Some(c.end);
            if done >= total {
                prop_assert_eq!(c.done, c.end - c.start, "完成态写满整块");
            }
        }
    }
}

// ---------------------------------------------------------------------
// retry — 重试决策（连续性 / 等价性 / 单调封顶 / 优先级）
// ---------------------------------------------------------------------

proptest! {
    /// decide 连续性规则与上限等价：有进展 → 计数重置 1；无进展 → +1；
    /// Transient 下 at_limit ⟺ 新计数 ≥ max_retries；Fatal/Verify 永不自动
    #[test]
    fn prop_retry_decide_rules(
        made_progress in prop::bool::ANY,
        retries in 0u32..20,
        max_retries in 1u32..10,
        retry_after in proptest::option::of(0.01f64..1e6),
        auto_retry in prop::bool::ANY,
        kind in prop::sample::select(&[FailKind::Transient, FailKind::Fatal, FailKind::Verify]),
    ) {
        let (new_retries, d) = decide(kind, made_progress, retries, max_retries, retry_after, auto_retry);
        prop_assert_eq!(new_retries, if made_progress { 1 } else { retries + 1 }, "连续性规则");
        if kind == FailKind::Transient {
            prop_assert_eq!(d.at_limit, new_retries >= max_retries, "at_limit ⟺ 达上限");
            if d.auto {
                // 自动重试时延来源：Retry-After 优先，否则退避表
                prop_assert_eq!(
                    d.delay_secs,
                    retry_after.unwrap_or_else(|| backoff_secs(new_retries))
                );
            }
        } else {
            prop_assert!(!d.auto, "Fatal/Verify 不自动重试");
            prop_assert!(!d.at_limit);
        }
    }

    /// 退避序列（计数 ≥ 1）：非降且封顶 60（8 → 16 → 32 → 60 → 60 …）
    #[test]
    fn prop_backoff_monotone_capped(a in 1u32..40, b in 1u32..40) {
        let (fa, fb) = (backoff_secs(a), backoff_secs(b));
        if a <= b {
            prop_assert!(fa <= fb, "退避随计数非降");
        }
        prop_assert!(fa <= 60.0 && fb <= 60.0, "封顶 60s");
        prop_assert!([8.0, 16.0, 32.0, 60.0].contains(&fa), "取值域固定四档");
    }
}

// ---------------------------------------------------------------------
// namegen — 命名卫生与去重（幂等性 / 唯一性）
// ---------------------------------------------------------------------

proptest! {
    /// sanitize_name 幂等：二遍 sanitize == 一遍；且输出不含路径分隔符/冒号/
    /// NUL/控制字符（越目录写入防御）
    #[test]
    fn prop_sanitize_idempotent(s in prop::collection::vec(any::<char>(), 0..40)) {
        let s = s.into_iter().collect::<String>();
        let once = sanitize_name(&s);
        let twice = sanitize_name(&once);
        prop_assert_eq!(twice.clone(), once.clone(), "sanitize 幂等");
        prop_assert!(!once.contains('/') && !once.contains('\\') && !once.contains(':'));
        prop_assert!(!once.contains('\0'));
        prop_assert!(once.chars().all(|c| (c as u32) >= 0x20), "控制字符已替换");
    }

    /// join_path 尾分隔符幂等：任意尾斜杠形态拼接结果一致
    #[test]
    fn prop_join_path_trailing_slash_idempotent(
        dir in "[a-z]{0,8}",
        slashes in "[/\\\\]{0,4}",
        name in "[a-z0-9]{1,8}",
    ) {
        let with = join_path(&format!("{dir}{slashes}"), &name);
        let without = join_path(dir.trim_end_matches(['/', '\\']), &name);
        prop_assert_eq!(with, without, "尾分隔符不影响拼接");
    }

    /// dedupe 唯一性：返回名不被占用集合命中；base 空闲时原样返回
    #[test]
    fn prop_dedupe_returns_free_name(
        base in "[a-z]{1,6}\\.bin",
        taken_count in 0usize..6,
    ) {
        let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
        if taken_count > 0 {
            taken.insert(base.clone());
            for k in 1..taken_count as u32 {
                taken.insert(format!("{base}.{k}"));
            }
        }
        let got = dedupe(&base, |n| taken.contains(n));
        if taken_count == 0 {
            prop_assert_eq!(got.clone(), base, "base 空闲 → 原样返回");
        }
        prop_assert!(!taken.contains(&got), "结果必须未被占用");
    }
}

// ---------------------------------------------------------------------
// speed::SmoothedSpeed — EMA 平滑（有界性 / 收敛 / 归零）
// ---------------------------------------------------------------------

proptest! {
    /// EMA 有界性：非负采样下值 ∈ [0, 已见最大采样]（凸组合不变量）
    #[test]
    fn prop_ema_bounded(samples in prop::collection::vec(0.0f64..1e6, 1..40)) {
        let mut s = SmoothedSpeed::default();
        let max_sample = samples.iter().cloned().fold(0.0f64, f64::max);
        for x in &samples {
            s.push(*x);
            let v = s.value();
            prop_assert!((0.0..=max_sample).contains(&v), "值域 [0, max]（EMA 凸组合）");
        }
        // zero() 立即归零（零值速断，无拖尾）
        s.zero();
        prop_assert_eq!(s.value(), 0.0);
    }

    /// EMA 收敛性：恒定输入下与目标的距离单调不增（α=1/5 → 0.8 几何收缩）
    #[test]
    fn prop_ema_converges_constant(target in 0.0f64..1e6, steps in 1usize..40) {
        let mut s = SmoothedSpeed::default();
        let mut prev = f64::INFINITY;
        for _ in 0..steps {
            s.push(target);
            let dist = (s.value() - target).abs();
            prop_assert!(dist <= prev, "恒定输入下距离单调不增");
            prev = dist;
        }
    }
}
