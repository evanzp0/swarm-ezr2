//! property_tests — 属性测试（architect 交付物；与常规单测对账隔离）
//!
//! 依据 notes/rust.md「proptest 作为 Rust 属性测试框架」条款：独立模块、
//! 独立运行命令（`cargo test property_tests::`）、独立报告数量——属性测试
//! 随机输入驱动的「通过」是概率性的，失败不与单测基线对账（单测基线数量
//! 口径以 `project/handoff.md` 对账节为准，随批次递进；属性测试独立计数）。
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
//!
//! architect 第二轮（v1.3 后复核批次）追加——覆盖此前未及的纯逻辑面：
//! - `sidecar` 持久化往返：build → JSON → parse 全字段相等（格式稳定性）、
//!   stamp() 投影口径；
//! - `checksum` 校验码解析：validate_value 完全特征化（纯 hex ∧ 位数相等）、
//!   parse_cli_x 往返 + 大小写归一、algo_index_by_name 规范名查找；
//! - `config::parse_speed`："/s" 后缀不变性、单位乘数十进制口径、大小写/
//!   空白归一、garbage → 0；
//! - `config::from_toml` 输出域不变量：block_size/slots > 0、并发钳制
//!   [1,64]、重试 ≥ 1；
//! - `slots` 槽位调度：allocate 幂等（二遍零授予）+ 容量封顶 + 列表序；
//!   enforce_invariants 幂等（两遍 has_slot 向量一致）+ 状态语义；
//! - `timefmt`：分钟取整不变性、定宽形状、字典序单调；
//! - `consistency`：等同 Valid、MissingStamp 判据、URL/大小差异必失效、确定性；
//! - `registry` 持久化往返：from_tasks → JSON → load 全量保真 + next_id
//!   不变量；快照投影往返 TaskSnapshot → Task → TaskSnapshot 保真。

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

// ---------------------------------------------------------------------
// checksum — 校验码解析（特征化 / 往返 / 归一化）【第二轮追加】
// ---------------------------------------------------------------------

proptest! {
    /// validate_value 完全特征化：Ok(小写) ⟺ trim 后纯 hex ∧ 位数 = 算法期望；
    /// 错误形态二选一（非 hex → NotHex；位数不符 → BadLength）
    #[test]
    fn prop_validate_value_characterization(
        algo_idx in 0usize..7,
        hex_chars in prop::collection::vec(
            prop::sample::select(&['0', '7', 'a', 'f', 'A', 'F', '9', 'c']),
            0..140,
        ),
        pad in prop::bool::ANY,
    ) {
        let (_, need, _) = crate::model::checksum::CHECKSUM_ALGOS[algo_idx];
        let core: String = hex_chars.iter().collect();
        let raw = if pad { format!("  {core}\t") } else { core.clone() };
        let all_hex = core.chars().all(|c| c.is_ascii_hexdigit());
        match crate::model::checksum::validate_value(algo_idx, &raw) {
            Ok(v) => {
                prop_assert!(all_hex && core.chars().count() == need, "Ok 仅当纯 hex 且位数相符");
                prop_assert_eq!(v, core.to_lowercase(), "归一为小写");
            }
            Err(e) => {
                prop_assert!(!(all_hex && hex_chars.len() == need), "纯 hex 且位数相符必 Ok");
                match e {
                    crate::model::checksum::ChecksumError::NotHex => {
                        prop_assert!(!all_hex, "NotHex 仅因含非 hex 字符");
                    }
                    crate::model::checksum::ChecksumError::BadLength { expected, got } => {
                        prop_assert_eq!(expected, need);
                        prop_assert_eq!(got, core.chars().count(), "BadLength 报 trim 后位数");
                        prop_assert!(all_hex, "位数错误前提是字符合法");
                    }
                }
            }
        }
    }

    /// parse_cli_x 往返：`<算法后缀>=<小写hex>` → Ok((原下标, 原值))；
    /// 大写 hex 归一小写；算法名大小写不敏感；无 `=`/未知算法 → Err
    #[test]
    fn prop_parse_cli_x_round_trip(
        algo_idx in 0usize..7,
        upper in prop::bool::ANY,
        upper_algo in prop::bool::ANY,
    ) {
        let (disp, need, suffix) = crate::model::checksum::CHECKSUM_ALGOS[algo_idx];
        let hex: String = (0..need)
            .map(|i| char::from_digit((i % 16) as u32, 16).unwrap_or('0'))
            .collect();
        let value = if upper { hex.to_uppercase() } else { hex.clone() };
        let algo = if upper_algo { suffix.to_uppercase() } else { suffix.to_string() };
        let arg = format!("{algo}={value}");
        let got = crate::model::checksum::parse_cli_x(&arg);
        prop_assert!(got.is_ok(), "合法参数必解析成功");
        let (idx, val) = got.unwrap();
        prop_assert_eq!(idx, algo_idx, "算法下标往返");
        prop_assert_eq!(val, hex, "校验值归一小写后往返");
        prop_assert!(crate::model::checksum::parse_cli_x(value.as_str()).is_err(), "缺 = 报错");
        prop_assert!(
            crate::model::checksum::parse_cli_x(&format!("crc32={value}")).is_err(),
            "未知算法（{disp} 组之外）报错"
        );
    }

    /// algo_index_by_name 规范查找：显示名任意大小写（保连字符）恒命中原下标；
    /// 未知名 → None
    #[test]
    fn prop_algo_index_by_name_canonical(algo_idx in 0usize..7, upper in prop::bool::ANY) {
        let (disp, _, _) = crate::model::checksum::CHECKSUM_ALGOS[algo_idx];
        let name = if upper { disp.to_uppercase() } else { disp.to_string() };
        prop_assert_eq!(crate::model::checksum::algo_index_by_name(&name), Some(algo_idx));
        prop_assert_eq!(crate::model::checksum::algo_index_by_name("crc32"), None);
        prop_assert_eq!(crate::model::checksum::algo_index_by_name(""), None);
    }
}

// ---------------------------------------------------------------------
// config::parse_speed / from_toml — 限速解析与配置钳制（不变量 / 归一化）
// 【第二轮追加】
// ---------------------------------------------------------------------

proptest! {
    /// "/s" 后缀不变性：不含 `/` 的输入 s，parse_speed(s) == parse_speed(s + "/s")；
    /// 单位乘数十进制口径：n KB = n×1000、n MB = n×1_000_000（f64 精确域内）
    #[test]
    fn prop_parse_speed_suffix_and_units(
        n in 0u64..1_000_000,
        unit in prop::sample::select(&["", " KB", " MB", " b", " KB ", " MB  "]),
        space in prop::sample::select(&["", " ", "  "]),
    ) {
        let s = format!("{space}{n}{unit}");
        prop_assert_eq!(
            crate::model::config::parse_speed(&s),
            crate::model::config::parse_speed(&format!("{s}/s")),
            "『/s』后缀不影响解析结果"
        );
        prop_assert_eq!(crate::model::config::parse_speed(&format!("{n} KB")), n * 1_000);
        prop_assert_eq!(crate::model::config::parse_speed(&format!("{n} MB")), n * 1_000_000);
    }

    /// parse_speed 大小写/空白归一 + 非法输入归零 + 单调性（MB > KB > b）
    /// （garbage 字母表排除 i：避免把可被 f64 解析的 inf 等混入「非法」面——
    /// parse_speed("inf") 走 f64 饱和转换属既有行为，不入本属性口径）
    #[test]
    fn prop_parse_speed_normalization_and_zero(
        n in 1u64..1_000_000,
        upper in prop::bool::ANY,
        garbage in "[a-hj-z]{1,6}",
    ) {
        let s = format!("{n} MB");
        let variant = if upper { s.to_uppercase() } else { s.to_lowercase() };
        prop_assert_eq!(crate::model::config::parse_speed(&variant), n * 1_000_000, "大小写归一");
        prop_assert_eq!(crate::model::config::parse_speed(&format!("{n}kb")), n * 1_000, "无空格后缀");
        prop_assert_eq!(crate::model::config::parse_speed(garbage.as_str()), 0, "非数字归零（不限速）");
        prop_assert!(
            crate::model::config::parse_speed("1 MB")
                > crate::model::config::parse_speed("1 KB")
                && crate::model::config::parse_speed("1 KB") > crate::model::config::parse_speed("1 b"),
            "单位乘数单调 MB > KB > b"
        );
    }

    /// from_toml 输出域不变量：任意（可解析）单键值下，
    /// block_size_http > 0 ∧ download_slots > 0 ∧ 并发 ∈ [1,64] ∧ max_retries ≥ 1；
    /// 正值透传、0 回退默认；垃圾文本全默认
    /// （v 上界 2^32：TOML 整数超目标类型宽度时整段解析失败回默认，属
    /// 「垃圾输入」面，不进逐键透传断言）
    #[test]
    fn prop_config_from_toml_domain_invariants(
        key in prop::sample::select(&["block_size_http", "download_slots", "default_concurrency", "max_retries"]),
        v in 0u64..(1u64 << 32),
        garbage in prop::bool::ANY,
    ) {
        use crate::model::config::Config;
        let text = if garbage {
            "not [valid toml ===".to_string()
        } else {
            format!("{key} = {v}")
        };
        let c = Config::from_toml(&text);
        prop_assert!(c.block_size_http > 0, "块大小恒正");
        prop_assert!(c.download_slots > 0, "槽位恒正");
        prop_assert!((1..=64).contains(&c.default_concurrency), "并发钳制 [1,64]");
        prop_assert!(c.max_retries >= 1, "重试上限恒 ≥ 1");
        if garbage {
            prop_assert_eq!(c, Config::default(), "垃圾 TOML 全默认");
            return Ok(());
        }
        match key {
            "block_size_http" => {
                let expect = if v == 0 { Config::default().block_size_http } else { v };
                prop_assert_eq!(c.block_size_http, expect, "正值透传/0 回退");
            }
            "download_slots" => {
                let expect = if v == 0 { Config::default().download_slots } else { v as usize };
                prop_assert_eq!(c.download_slots, expect, "正值透传/0 回退");
            }
            "max_retries" => {
                let expect = if v == 0 { Config::default().max_retries } else { u32::try_from(v).unwrap_or(1) };
                prop_assert_eq!(c.max_retries, expect, "正值透传/0 回退");
            }
            _ => {
                let clamped = (v as usize).clamp(1, 64);
                prop_assert_eq!(c.default_concurrency, clamped, "并发 clamp 语义");
            }
        }
    }
}

// ---------------------------------------------------------------------
// slots — 槽位调度（幂等性 / 容量封顶 / 状态语义）【第二轮追加】
// ---------------------------------------------------------------------

fn slot_task(
    id: u32,
    state: crate::model::TaskState,
    has_slot: bool,
    retry_in: Option<f64>,
) -> crate::model::Task {
    let mut t = crate::model::sample_task();
    t.id = id;
    t.state = state;
    t.has_slot = has_slot;
    t.retry_in = retry_in;
    t
}

proptest! {
    /// allocate 幂等与封顶（全等待中队列）：一遍授予 = min(空闲, 未持槽数)、
    /// 授予后 used ≤ max_slots、二遍零授予、授予按列表序
    #[test]
    fn prop_slots_allocate_idempotent_capped(
        n in 0usize..12,
        max_slots in 0usize..10,
        initial in prop::collection::vec(prop::bool::ANY, 12),
    ) {
        use crate::model::slots;
        use crate::model::TaskState;
        let mut tasks: Vec<_> = (0..n)
            .map(|i| slot_task(u32::try_from(i).unwrap_or(0) + 1, TaskState::Queued, initial[i], None))
            .collect();
        let used_before = slots::used(&tasks);
        let candidates = n - used_before; // 全等待中：未持槽者即可候选
        let free_before = max_slots.saturating_sub(used_before);
        let granted = slots::allocate(&mut tasks, max_slots);
        prop_assert_eq!(
            granted.len(),
            free_before.min(candidates),
            "授予数 = min(空闲, 未持槽数)"
        );
        prop_assert!(slots::used(&tasks) <= max_slots.max(used_before), "容量封顶（既有超额不回收）");
        for id in &granted {
            let t = tasks.iter().find(|t| t.id == *id);
            prop_assert!(t.is_some_and(|t| t.state == TaskState::Queued && t.has_slot), "授予者等待中且持槽");
        }
        // 列表序：granted 内 id 的列表位置严格递增
        for w in granted.windows(2) {
            let pa = tasks.iter().position(|t| t.id == w[0]);
            let pb = tasks.iter().position(|t| t.id == w[1]);
            prop_assert!(pa.is_some_and(|a| pb.is_some_and(|b| a < b)), "授予按列表序");
        }
        // 幂等：二遍零授予
        let second = slots::allocate(&mut tasks, max_slots);
        prop_assert!(second.is_empty(), "二遍 allocate 零授予");
    }

    /// enforce_invariants 幂等与状态语义：两遍执行 has_slot 向量一致；
    /// 下载中/校验中恒持槽；失败 = 将自动重试；其余恒不持（等待中除外，保持原值）
    #[test]
    fn prop_slots_enforce_idempotent_semantics(
        n in 0usize..12,
        states in prop::collection::vec(
            prop::sample::select(&[
                crate::model::TaskState::Queued,
                crate::model::TaskState::Downloading,
                crate::model::TaskState::Paused,
                crate::model::TaskState::Verifying,
                crate::model::TaskState::Failed,
                crate::model::TaskState::Completed,
            ]),
            12,
        ),
        slots0 in prop::collection::vec(prop::bool::ANY, 12),
        retry_in in prop::collection::vec(proptest::option::of(1.0f64..600.0), 12),
    ) {
        use crate::model::slots;
        use crate::model::TaskState;
        let mut tasks: Vec<_> = (0..n)
            .map(|i| slot_task(u32::try_from(i).unwrap_or(0) + 1, states[i], slots0[i], retry_in[i]))
            .collect();
        slots::enforce_invariants(&mut tasks);
        let after_once: Vec<bool> = tasks.iter().map(|t| t.has_slot).collect();
        // 语义断言
        for t in &tasks {
            match t.state {
                TaskState::Downloading | TaskState::Verifying => prop_assert!(t.has_slot, "下载/校验恒持槽"),
                TaskState::Failed => prop_assert_eq!(t.has_slot, t.retry_in.is_some(), "失败=将自动重试"),
                TaskState::Queued => {}
                _ => prop_assert!(!t.has_slot, "其余态恒不持槽"),
            }
        }
        slots::enforce_invariants(&mut tasks);
        let after_twice: Vec<bool> = tasks.iter().map(|t| t.has_slot).collect();
        prop_assert_eq!(after_twice, after_once, "enforce 幂等");
    }
}

// ---------------------------------------------------------------------
// timefmt — 时间显示（取整不变 / 定宽形状 / 字典序单调）【第二轮追加】
// ---------------------------------------------------------------------

proptest! {
    /// fmt_created 分钟取整不变性（CST 偏移为 60 的倍数）+ 定宽形状
    /// `YYYY-MM-DD HH:MM`（16 字符）+ 字典序单调（t1 ≤ t2 ⇒ 串 ≤）
    #[test]
    fn prop_fmt_created_shape_monotone(t1 in 0u64..4_102_444_800, t2 in 0u64..4_102_444_800) {
        use crate::model::fmt_created;
        let s = fmt_created(t1);
        prop_assert_eq!(s.len(), 16, "定宽 16 字符");
        let b = s.as_bytes();
        prop_assert_eq!(b[4], b'-');
        prop_assert_eq!(b[7], b'-');
        prop_assert_eq!(b[10], b' ');
        prop_assert_eq!(b[13], b':');
        prop_assert!(b.iter().enumerate().all(|(i, c)| matches!(i, 4 | 7 | 10 | 13) || c.is_ascii_digit()), "其余全数字");
        prop_assert_eq!(fmt_created(t1 - t1 % 60), s.clone(), "秒位截断不影响分钟显示");
        if t1 <= t2 {
            prop_assert!(s <= fmt_created(t2), "字典序随时间单调");
        }
    }
}

// ---------------------------------------------------------------------
// consistency — 续传一致性（判据 / 确定性）【第二轮追加】
// ---------------------------------------------------------------------

proptest! {
    /// check 特征化不变量：等同 → Valid；stored 缺 URL 或大小 → MissingStamp；
    /// URL 或大小任一不同 → Invalidated；ETag/Last-Modified 缺失方向不对称按
    /// 口径（stored 有而 probe 缺 → 失效；stored 无 → 宽容）；确定性（两遍同判）
    #[test]
    fn prop_consistency_decision_rules(
        url_a in "[a-z]{1,6}/[a-z]{1,6}",
        etag in "[a-z0-9]{1,6}",
        size in 0u64..(1u64 << 40),
        etag_probe_missing in prop::bool::ANY,
        etag_stored_missing in prop::bool::ANY,
    ) {
        use crate::model::consistency::{check, Consistency, ServerStamp};
        let stored = ServerStamp {
            final_url: Some(url_a.clone()),
            etag: if etag_stored_missing { None } else { Some(etag.clone()) },
            last_modified: Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string()),
            size: Some(size),
        };
        // 等同 → Valid（stored 完整时）
        prop_assert_eq!(check(&stored, &stored), Consistency::Valid, "等同快照可续传");
        // 确定性
        prop_assert_eq!(check(&stored, &stored), check(&stored, &stored));
        // URL 变化 → 失效
        let probed_url = ServerStamp { final_url: Some(format!("{url_a}x")), ..stored.clone() };
        prop_assert_eq!(check(&stored, &probed_url), Consistency::Invalidated);
        // 大小变化 → 失效
        let probed_size = ServerStamp { size: Some(size.wrapping_add(1)), ..stored.clone() };
        prop_assert_eq!(check(&stored, &probed_size), Consistency::Invalidated);
        // stored 有 ETag 而 probe 缺失 → 失效；stored 无 ETag → 以 URL/大小为准
        let probed_no_etag = ServerStamp { etag: None, ..stored.clone() };
        if stored.etag.is_some() {
            prop_assert_eq!(check(&stored, &probed_no_etag), Consistency::Invalidated, "服务器不再返回 ETag → 失效");
        } else {
            prop_assert_eq!(check(&stored, &probed_no_etag), Consistency::Valid, "stored 未记录 → 宽容");
        }
        // MissingStamp 判据：仅 stored 不完整时
        let mut incomplete = stored.clone();
        incomplete.size = None;
        prop_assert_eq!(check(&incomplete, &stored), Consistency::MissingStamp);
        let mut no_url = stored.clone();
        no_url.final_url = None;
        prop_assert_eq!(check(&no_url, &stored), Consistency::MissingStamp);
        // ETag 双侧存在但不同 → 失效（构造探针走 etag_probe_missing 面冗余覆盖）
        if !etag_stored_missing && !etag_probe_missing {
            let probed_diff = ServerStamp { etag: Some(format!("{etag}x")), ..stored.clone() };
            prop_assert_eq!(check(&stored, &probed_diff), Consistency::Invalidated);
        }
    }
}

// ---------------------------------------------------------------------
// sidecar / registry — 持久化往返（格式稳定性 / 投影保真）【第二轮追加】
// ---------------------------------------------------------------------

proptest! {
    /// Sidecar JSON 往返：build → JSON → parse 全字段相等（含版本门与
    /// downloaded 钳制口径不漂移）；stamp() 投影与服务端快照一致
    #[test]
    fn prop_sidecar_json_round_trip(
        url in "http://[a-z]{1,8}/[a-z0-9]{1,8}",
        final_url in proptest::option::of("http://[a-z]{1,8}/r"),
        etag in proptest::option::of("[a-z0-9]{1,8}"),
        last_modified in proptest::option::of("[A-Z][a-z]{2}, 01 Jan 2026 00:00:00 GMT"),
        size in 0u64..(1u64 << 40),
        block_size in 1u64..(1u64 << 20),
        blocks in prop::collection::vec(0u64..(1u64 << 20), 0..64),
        non_resumable in prop::bool::ANY,
        algo in proptest::option::of(prop::sample::select(&["MD5", "SHA-1", "SHA-256", "Adler-32"])),
        proto in prop::sample::select(&[crate::model::Protocol::Http, crate::model::Protocol::Https]),
    ) {
        use crate::model::sidecar::{Sidecar, SidecarTask};
        use crate::model::consistency::ServerStamp;
        use crate::model::Checksum;
        let stamp = ServerStamp {
            final_url: final_url.clone(),
            etag: etag.clone(),
            last_modified: last_modified.clone(),
            size: Some(size),
        };
        let expected = algo.map(|a| Checksum { algo: a, value: "0123abcd".to_string() });
        let task = SidecarTask {
            id: 7,
            added_at: 1_770_000_000,
            save_dir: "/dl".to_string(),
            concurrency: 4,
            protocol: proto,
        };
        let sc = Sidecar::build(&url, &stamp, size, block_size, &blocks, non_resumable, expected.as_ref(), task);
        prop_assert_eq!(sc.version, crate::model::sidecar::SIDECAR_VERSION, "build 恒当前版本");
        let text = serde_json::to_string(&sc).unwrap();
        let loaded: Sidecar = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(loaded, sc.clone(), "JSON 往返全字段保真");
        let st = sc.stamp();
        prop_assert_eq!(st.final_url, final_url, "stamp 投影 final_url");
        prop_assert_eq!(st.etag, etag, "stamp 投影 etag");
        prop_assert_eq!(st.size, Some(size), "stamp 投影 size");
    }

    /// Registry 持久化往返 + 快照投影保真：from_tasks → JSON → parse
    /// 全量保真（JSON 串相等）、next_id ≥ max(id)+1；快照 → Task → 快照
    /// 往返不丢字段（崩溃恢复投影的数据面基础）
    #[test]
    fn prop_registry_round_trip_and_projection(
        k in 0usize..6,
        next_id in 1u32..1000,
        states in prop::collection::vec(
            prop::sample::select(&[
                crate::model::TaskState::Queued,
                crate::model::TaskState::Downloading,
                crate::model::TaskState::Paused,
                crate::model::TaskState::Verifying,
                crate::model::TaskState::Failed,
                crate::model::TaskState::Completed,
            ]),
            6,
        ),
        totals in prop::collection::vec(0u64..(1u64 << 40), 6),
        downloaded_frac in prop::collection::vec(0u64..101, 6),
        retries in prop::collection::vec(0u32..12, 6),
        with_checksum in prop::collection::vec(prop::bool::ANY, 6),
        algo in prop::sample::select(&["MD5", "SHA-256", "Adler-32"]),
    ) {
        use crate::model::registry::Registry;
        use crate::model::sidecar::ChecksumCompat;
        let mut tasks = Vec::new();
        for i in 0..k {
            let mut t = crate::model::sample_task();
            t.id = u32::try_from(i).unwrap_or(0) + 1;
            t.state = states[i];
            t.total = totals[i];
            t.downloaded = totals[i] * downloaded_frac[i] / 100;
            t.retries = retries[i];
            if with_checksum[i] {
                t.checksum = Some(crate::model::Checksum {
                    algo,
                    value: "0123abcd0123abcd".to_string(),
                });
            }
            tasks.push(t);
        }
        let reg = Registry::from_tasks(&tasks, next_id);
        prop_assert!(
            reg.next_id > tasks.iter().map(|t| t.id).max().unwrap_or(0),
            "next_id 恒大于最大任务 id"
        );
        let text = serde_json::to_string(&reg).unwrap();
        let loaded = serde_json::from_str::<Registry>(&text).unwrap();
        prop_assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            text,
            "注册表 JSON 往返保真"
        );
        // 快照投影往返：TaskSnapshot → Task → TaskSnapshot 不丢持久化字段
        for snap in &reg.tasks {
            let back = crate::model::registry::TaskSnapshot::from(&crate::model::Task::from(snap));
            prop_assert_eq!(
                serde_json::to_string(&back).unwrap(),
                serde_json::to_string(snap).unwrap(),
                "快照投影往返保真"
            );
        }
        // 校验期望随投影保留（算法显示名可解析时）
        for (i, snap) in reg.tasks.iter().enumerate() {
            if with_checksum[i] {
                prop_assert!(
                    snap.checksum.as_ref().is_some_and(|c: &ChecksumCompat| c.algo == algo),
                    "校验期望随投影保留"
                );
            }
        }
    }
}

/// 混合 CJK/全角/任意字符的文本策略（覆盖 w() 的宽度表三个分支域）
fn text_with_wide() -> impl proptest::strategy::Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            3 => any::<char>(),
            2 => proptest::char::range('\u{4E00}', '\u{9FA5}'),
            1 => proptest::char::range('\u{3000}', '\u{303F}'),
            1 => proptest::char::range('\u{FF00}', '\u{FF60}'),
        ],
        0..48,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    // ---- ui/text 纯文本工具（architect 第三轮补充）----
    // 覆盖维度：显示宽度不变量、截断/填充幂等、时长格式化往返、
    // 大小/速度格式化的单位后缀封闭域与「单位绑定总量」口径。

    /// w() 显示宽度界：n ≤ w(s) ≤ 2n（CJK/全角按 2 计，其余按 1 计）
    #[test]
    fn prop_text_width_within_bounds(s in text_with_wide()) {
        let n = s.chars().count();
        let width = crate::ui::w(&s);
        prop_assert!(width >= n, "宽度下界（每字符至少 1 列）");
        prop_assert!(width <= 2 * n, "宽度上界（每字符至多 2 列）");
    }

    /// truncate 宽度界 + 幂等：w(truncate(s, max)) ≤ max（max ≥ 1；max=0 时
    /// 单字符省略号超出属既有契约口径，不在本属性域）；二次截断恒等
    #[test]
    fn prop_truncate_bounded_and_idempotent(
        s in text_with_wide(),
        max in 1usize..=80,
    ) {
        let once = crate::ui::truncate(&s, max);
        prop_assert!(crate::ui::w(&once) <= max, "截断后显示宽度不超预算");
        let twice = crate::ui::truncate(&once, max);
        prop_assert_eq!(&twice, &once, "截断幂等");
    }

    /// pad_right 精确宽度 + 幂等：w(pad(s, width)) == max(w(s), width)；二次填充恒等
    #[test]
    fn prop_pad_right_exact_and_idempotent(
        s in text_with_wide(),
        width in 0usize..=60,
    ) {
        let once = crate::ui::pad_right(&s, width);
        prop_assert_eq!(
            crate::ui::w(&once),
            std::cmp::max(crate::ui::w(&s), width),
            "填充后显示宽度恰为 max(原宽, 目标宽)"
        );
        let twice = crate::ui::pad_right(&once, width);
        prop_assert_eq!(&twice, &once, "填充幂等");
    }

    /// fmt_dur 往返：格式化 → 解析回原秒数（h:mm:ss / mm:ss 两形态全遍历）；
    /// fmt_eta(Some(s)) 与 fmt_dur(s) 一致（eta 是 dur 的 Option 包装）
    #[test]
    fn prop_fmt_dur_round_trip(secs in 0u64..1_000_000) {
        fn parse_back(text: &str) -> Option<u64> {
            let parts: Vec<&str> = text.split(':').collect();
            match parts.as_slice() {
                [h, m, sec] => Some(
                    h.parse::<u64>().ok()? * 3600
                        + m.parse::<u64>().ok()? * 60
                        + sec.parse::<u64>().ok()?,
                ),
                [m, sec] => Some(m.parse::<u64>().ok()? * 60 + sec.parse::<u64>().ok()?),
                _ => None,
            }
        }
        let text = crate::ui::fmt_dur(secs);
        prop_assert_eq!(
            parse_back(&text),
            Some(secs),
            "时长格式化往返保真"
        );
        prop_assert_eq!(
            crate::ui::fmt_eta(Some(secs)),
            text,
            "eta(Some) 与 dur 同口径"
        );
        prop_assert_eq!(crate::ui::fmt_eta(None), "--:--", "eta(None) 恒占位符");
    }

    /// fmt_size / fmt_size_pair 单位后缀封闭域：后缀只取 B/KB/MB/GB；
    /// fmt_size_pair 的单位只由总量 y 决定（x 同单位展示）且恒含 '/'
    #[test]
    fn prop_fmt_size_suffix_domain(
        bytes in 0u64..u64::MAX,
        x in 0u64..(1u64 << 50),
        y in 0u64..(1u64 << 50),
    ) {
        let unit_of = |total: u64| {
            if total >= 1_000_000_000 { "GB" } else if total >= 1_000_000 { "MB" } else if total >= 1_000 { "KB" } else { "B" }
        };
        let single = crate::ui::fmt_size(bytes);
        let expected = unit_of(bytes);
        prop_assert!(
            single.ends_with(&format!(" {expected}")),
            "fmt_size 后缀封闭域与阈值口径"
        );
        let pair = crate::ui::fmt_size_pair(x, y);
        let expected_pair = unit_of(y);
        prop_assert!(
            pair.ends_with(&format!(" {expected_pair}")),
            "fmt_size_pair 单位绑定总量"
        );
        prop_assert!(pair.contains('/'), "配对格式恒含分隔符");
    }

    /// fmt_speed 稳定域：负零归一（-0.0 与 0.0 同输出）；后缀封闭域
    /// B/s、KB/s、MB/s 且数值前缀可解析
    #[test]
    fn prop_fmt_speed_suffix_domain(bps in 0.0f64..1e15) {
        prop_assert_eq!(
            crate::ui::fmt_speed(-0.0),
            crate::ui::fmt_speed(0.0),
            "负零归一"
        );
        let text = crate::ui::fmt_speed(bps);
        let expected = if bps >= 1_000_000.0 {
            "MB/s"
        } else if bps >= 1_000.0 {
            "KB/s"
        } else {
            "B/s"
        };
        prop_assert!(text.ends_with(expected), "速度后缀封闭域与阈值口径");
        let prefix = text.strip_suffix(expected).and_then(|p| p.trim().parse::<f64>().ok());
        prop_assert!(prefix.is_some(), "数值前缀可解析");
    }
}
