use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crossterm::event::KeyCode;
use proptest::prelude::*;

use crate::app::DialogKind;
use crate::engine::{merge_answers, parse_retry_after_header, HostsFile};
use crate::model::checksum::{
    algo_index_exact_or_default, parse_companion_content, CHECKSUM_ALGOS,
};
use crate::model::chunk::{
    block_range, chunk_total, fmt_block_size, lease_snapshot, spread_bytes, Blocks,
};
use crate::model::config::ProxyChoice;
use crate::model::idgen::alloc_task_id;
use crate::model::namegen::{
    dedupe, derive_name, from_content_disposition, from_url_path, join_path, sanitize_name,
    sidecar_path_of, DOWNLOADING_EXT, SIDECAR_EXT,
};
use crate::model::retry::{backoff_secs, classify_http_status, decide};
use crate::model::slots::{queue_pos, queue_positions};
use crate::model::speed::SmoothedSpeed;
use crate::model::{is_http_url, protocol_of_url, FailKind, Protocol, Task, TaskState};

proptest! {
    #[test]
    fn prop_chunk_total_is_ceil_div(
        total in 0u64..(1u64 << 48),
        piece in 1u64..(1u64 << 24),
    ) {
        let expected = total.div_ceil(piece).min(u32::MAX as u64) as u32;
        prop_assert_eq!(chunk_total(total, piece), expected);
        prop_assert_eq!(chunk_total(0, piece), 0);
    }

    #[test]
    fn prop_block_ranges_tile_total(
        total in 1u64..(1u64 << 48),
        piece in 1u64..(1u64 << 24),
        probe in 0u32..1000,
    ) {
        let y = chunk_total(total, piece);
        prop_assume!(y < u32::MAX - 1, "排除块数封顶角例");
        let last = block_range(total, piece, y - 1);
        prop_assert_eq!(last.map(|(_, end)| end), Some(total), "末块 end = total");
        prop_assert!(block_range(total, piece, y).is_none(), "越界应 None");
        let i = u64::from(probe) % u64::from(y);
        let (s0, e0) = block_range(total, piece, i as u32).unwrap();
        prop_assert!(s0 < e0 && e0 <= total);
        if i + 1 < u64::from(y) {
            let (s1, _) = block_range(total, piece, i as u32 + 1).unwrap();
            prop_assert_eq!(e0, s1, "相邻块首尾相接");
            prop_assert_eq!(e0 - s0, piece, "非末块长度 = piece");
        }
    }

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

proptest! {
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

    #[test]
    fn prop_lease_snapshot_invariants(
        (piece, total) in (1u64..(1u64 << 20)).prop_flat_map(|piece| {
            (Just(piece), 1u64..((1u64 << 40).min(piece << 32)))
        }),
        n in 1usize..17,
        done_raw in 0u64..(1u64 << 41),
    ) {
        let (conns, x) = lease_snapshot(total, done_raw, n, piece);
        let y = u64::from(chunk_total(total, piece));
        let done = done_raw.min(total);
        if done >= total {
            prop_assert_eq!(u64::from(x), y, "完成态：x = 总块数");
        } else {
            prop_assert_eq!(u64::from(x), done / piece, "未完成：x = done/piece 向下");
        }
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

proptest! {
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

proptest! {
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

proptest! {
    #[test]
    fn prop_ema_bounded(samples in prop::collection::vec(0.0f64..1e6, 1..40)) {
        let mut s = SmoothedSpeed::default();
        let max_sample = samples.iter().cloned().fold(0.0f64, f64::max);
        for x in &samples {
            s.push(*x);
            let v = s.value();
            prop_assert!((0.0..=max_sample).contains(&v), "值域 [0, max]（EMA 凸组合）");
        }
        s.zero();
        prop_assert_eq!(s.value(), 0.0);
    }

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

proptest! {
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

    #[test]
    fn prop_algo_index_by_name_canonical(algo_idx in 0usize..7, upper in prop::bool::ANY) {
        let (disp, _, _) = crate::model::checksum::CHECKSUM_ALGOS[algo_idx];
        let name = if upper { disp.to_uppercase() } else { disp.to_string() };
        prop_assert_eq!(crate::model::checksum::algo_index_by_name(&name), Some(algo_idx));
        prop_assert_eq!(crate::model::checksum::algo_index_by_name("crc32"), None);
        prop_assert_eq!(crate::model::checksum::algo_index_by_name(""), None);
    }
}

proptest! {
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

    #[test]
    fn prop_config_from_toml_domain_invariants(
        key in prop::sample::select(&["block_size_http", "download_slots", "http_concurrency", "max_retries"]),
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
        prop_assert!((1..=64).contains(&c.http_concurrency), "并发钳制 [1,64]");
        prop_assert!(c.max_retries >= 1, "重试上限恒 ≥ 1");
        if garbage {
            prop_assert_eq!(c, Config::default(), "垃圾 TOML 全默认");
            return Ok(());
        }
        match key {
            "block_size_http" => {
                let expect = if v == 0 {
                    Config::default().block_size_http
                } else {
                    v.max(crate::model::chunk::MIN_HTTP_BLOCK_SIZE)
                };
                prop_assert_eq!(c.block_size_http, expect, "正值钳下限透传/0 回退");
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
                prop_assert_eq!(c.http_concurrency, clamped, "并发 clamp 语义");
            }
        }
    }
}

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

fn arb_task_states(
    n: usize,
) -> impl proptest::strategy::Strategy<Value = Vec<crate::model::TaskState>> {
    static STATES: &[crate::model::TaskState] = &[
        crate::model::TaskState::Queued,
        crate::model::TaskState::Downloading,
        crate::model::TaskState::Paused,
        crate::model::TaskState::Verifying,
        crate::model::TaskState::Failed,
        crate::model::TaskState::Completed,
    ];
    prop::collection::vec(prop::sample::select(STATES), n)
}

fn slot_task_vec(
    n: usize,
    states: &[crate::model::TaskState],
    flags: &[bool],
    retry_in: &[Option<f64>],
) -> Vec<crate::model::Task> {
    (0..n)
        .map(|i| {
            slot_task(
                u32::try_from(i).unwrap_or(0) + 1,
                states[i],
                flags[i],
                retry_in[i],
            )
        })
        .collect()
}

proptest! {
    #[test]
    fn prop_slots_allocate_idempotent_capped(
        n in 0usize..12,
        max_slots in 0usize..10,
        initial in prop::collection::vec(prop::bool::ANY, 12),
    ) {
        use crate::model::slots;
        use crate::model::TaskState;
        let mut tasks: Vec<_> =
            slot_task_vec(n, &[TaskState::Queued; 12], &initial, &[None; 12]);
        let used_before = slots::used(&tasks);
        let candidates = n - used_before;
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
        for w in granted.windows(2) {
            let pa = tasks.iter().position(|t| t.id == w[0]);
            let pb = tasks.iter().position(|t| t.id == w[1]);
            prop_assert!(pa.is_some_and(|a| pb.is_some_and(|b| a < b)), "授予按列表序");
        }
        let second = slots::allocate(&mut tasks, max_slots);
        prop_assert!(second.is_empty(), "二遍 allocate 零授予");
    }

    #[test]
    fn prop_slots_enforce_idempotent_semantics(
        n in 0usize..12,
        states in arb_task_states(12),
        slots0 in prop::collection::vec(prop::bool::ANY, 12),
        retry_in in prop::collection::vec(proptest::option::of(1.0f64..600.0), 12),
    ) {
        use crate::model::slots;
        use crate::model::TaskState;
        let mut tasks: Vec<_> = slot_task_vec(n, &states, &slots0, &retry_in);
        slots::enforce_invariants(&mut tasks);
        let after_once: Vec<bool> = tasks.iter().map(|t| t.has_slot).collect();
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

proptest! {
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

proptest! {
    #[test]
    fn prop_consistency_decision_rules(
        etag in "[a-z0-9]{1,6}",
        size in 0u64..(1u64 << 40),
        etag_probe_missing in prop::bool::ANY,
        etag_stored_missing in prop::bool::ANY,
    ) {
        use crate::model::consistency::{check, Consistency, ServerStamp};
        let stored = ServerStamp {
            etag: if etag_stored_missing { None } else { Some(etag.clone()) },
            last_modified: Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string()),
            size: Some(size),
        };
        prop_assert_eq!(check(&stored, &stored), Consistency::Valid, "等同快照可续传");
        prop_assert_eq!(check(&stored, &stored), check(&stored, &stored));
        let probed_size = ServerStamp { size: Some(size.wrapping_add(1)), ..stored.clone() };
        prop_assert_eq!(check(&stored, &probed_size), Consistency::Invalidated);
        let probed_no_etag = ServerStamp { etag: None, ..stored.clone() };
        if stored.etag.is_some() {
            prop_assert_eq!(check(&stored, &probed_no_etag), Consistency::Invalidated, "服务器不再返回 ETag → 失效");
        } else {
            prop_assert_eq!(check(&stored, &probed_no_etag), Consistency::Valid, "stored 未记录 → 宽容");
        }
        let mut incomplete = stored.clone();
        incomplete.size = None;
        prop_assert_eq!(check(&incomplete, &stored), Consistency::MissingStamp);
        if !etag_stored_missing && !etag_probe_missing {
            let probed_diff = ServerStamp { etag: Some(format!("{etag}x")), ..stored.clone() };
            prop_assert_eq!(check(&stored, &probed_diff), Consistency::Invalidated);
        }
    }
}

proptest! {
    #[test]
    fn prop_sidecar_json_round_trip(
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
        let sc = Sidecar::build(&stamp, size, block_size, &blocks, non_resumable, expected.as_ref(), task);
        prop_assert_eq!(sc.version, crate::model::sidecar::SIDECAR_VERSION, "build 恒当前版本");
        let text = serde_json::to_string(&sc).unwrap();
        prop_assert!(!text.contains("final_url"), "v1.28 序列化不得含 final_url 键: {text}");
        let loaded: Sidecar = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(loaded, sc.clone(), "JSON 往返全字段保真");
        let st = sc.stamp();
        prop_assert_eq!(st.etag, etag, "stamp 投影 etag");
        prop_assert_eq!(st.size, Some(size), "stamp 投影 size");
    }

    #[test]
    fn prop_registry_round_trip_and_projection(
        k in 0usize..6,
        states in arb_task_states(6),
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
        let reg = Registry::from_tasks(&tasks);
        for (i, snap) in reg.tasks.iter().enumerate() {
            prop_assert_eq!(snap.id, tasks[i].id, "任务 ID 随快照保留");
        }
        let text = serde_json::to_string(&reg).unwrap();
        let loaded = serde_json::from_str::<Registry>(&text).unwrap();
        prop_assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            text,
            "注册表 JSON 往返保真"
        );
        for snap in &reg.tasks {
            let back = crate::model::registry::TaskSnapshot::from(&crate::model::Task::from(snap));
            prop_assert_eq!(
                serde_json::to_string(&back).unwrap(),
                serde_json::to_string(snap).unwrap(),
                "快照投影往返保真"
            );
        }
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
    .prop_filter(
        "排除零宽字符（w() 按 0 计，宽度下界不论及）",
        |s: &String| {
            !s.chars().any(|c| {
                let u = c as u32;
                (0xFE00..=0xFE0F).contains(&u)
                    || (0x200B..=0x200F).contains(&u)
                    || u == 0x2060
                    || u == 0xFEFF
            })
        },
    )
}

fn zero_width_text() -> impl proptest::strategy::Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            proptest::char::range('\u{FE00}', '\u{FE0F}'),
            proptest::char::range('\u{200B}', '\u{200F}'),
            proptest::char::range('\u{2060}', '\u{2060}'),
            proptest::char::range('\u{FEFF}', '\u{FEFF}'),
        ],
        0..48,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {

    #[test]
    fn prop_text_width_within_bounds(s in text_with_wide()) {
        let n = s.chars().count();
        let width = crate::ui::w(&s);
        prop_assert!(width >= n, "宽度下界（每字符至少 1 列）");
        prop_assert!(width <= 2 * n, "宽度上界（每字符至多 2 列）");
    }

    #[test]
    fn prop_text_zero_width_is_free(s in zero_width_text()) {
        prop_assert_eq!(crate::ui::w(&s), 0, "纯零宽串显示宽为 0");
    }

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

fn dialog_key_seq() -> impl Strategy<Value = Vec<KeyCode>> {
    let nav_keys = prop::sample::select(vec![
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Backspace,
    ])
    .boxed();
    let char_keys = any::<char>().prop_map(KeyCode::Char).boxed();
    prop::collection::vec(
        prop::strategy::Union::new_weighted(vec![(8, nav_keys), (2, char_keys)]),
        0..60,
    )
}

proptest! {
    #[test]
    fn prop_text_char_caps_and_consumption(
        kind in prop::sample::select(vec![DialogKind::Add, DialogKind::Modify]),
        focus in 0usize..8,
        chars in prop::collection::vec(any::<char>(), 0..320),
    ) {
        let mut d = crate::app::testutil::dialog(kind, focus);
        let is_text = matches!(
            (kind, focus),
            (DialogKind::Add, 0)
                | (DialogKind::Add, 1)
                | (DialogKind::Add, 2)
                | (DialogKind::Modify, 0)
        );
        for c in &chars {
            let consumed = crate::app::text_char(kind, &mut d, focus, *c);
            prop_assert_eq!(consumed, is_text, "消费口径与文本字段矩阵一致 (focus={})", focus);
            prop_assert!(d.url.len_chars() <= 300, "URL ≤ 300");
            prop_assert!(d.dir.len_chars() <= 300, "目录 ≤ 300");
            prop_assert!(d.conns.len_chars() <= 2, "并发 ≤ 2 位");
            prop_assert!(d.conns.value.chars().all(|x| x.is_ascii_digit()), "并发仅数字");
            if !d.conns.is_empty() {
                prop_assert!(d.conns_edited, "并发非空时 edited 必已置位");
            }
            let fields = [
                (crate::app::TextField::Url, d.url.len_chars(), d.url.cursor),
                (crate::app::TextField::Dir, d.dir.len_chars(), d.dir.cursor),
                (crate::app::TextField::Conns, d.conns.len_chars(), d.conns.cursor),
                (crate::app::TextField::CkValue, d.ck_value.len_chars(), d.ck_value.cursor),
            ];
            for (f, len, cur) in fields {
                prop_assert!(cur <= len, "{f:?} 光标 ≤ 字段长 (focus={})", focus);
            }
        }
    }

    #[test]
    fn prop_text_backspace_pops_at_most_one(
        kind in prop::sample::select(vec![DialogKind::Add, DialogKind::Modify]),
        focus in 0usize..8,
        seed in prop::collection::vec(any::<char>(), 0..8),
    ) {
        let mut d = crate::app::testutil::dialog(kind, focus);
        let seed_s: String = seed.into_iter().collect();
        d.url.set(seed_s.clone());
        d.dir.set(seed_s.clone());
        d.conns.set("42");
        d.ck_value.set(seed_s.clone());
        let is_bs_field = matches!(
            (kind, focus),
            (DialogKind::Add, 0)
                | (DialogKind::Add, 1)
                | (DialogKind::Add, 2)
                | (DialogKind::Add, 4)
                | (DialogKind::Modify, 0)
                | (DialogKind::Modify, 2)
        );
        let (url0, dir0, conns0, ck0) = (
            d.url.value.clone(),
            d.dir.value.clone(),
            d.conns.value.clone(),
            d.ck_value.value.clone(),
        );
        let cursor0 = (
            d.url.cursor,
            d.dir.cursor,
            d.conns.cursor,
            d.ck_value.cursor,
        );
        let consumed = crate::app::text_backspace(kind, &mut d, focus);
        prop_assert_eq!(consumed, is_bs_field, "退格消费口径与字段矩阵一致 (focus={})", focus);
        let expect_pop = |before: &str, after: &str| {
            let mut expect = before.to_string();
            expect.pop();
            assert_eq!(after, expect);
        };
        let follow = |before: usize, len: usize, after: usize| {
            assert_eq!(after, before.saturating_sub(1).min(len), "退格后光标随迁");
        };
        match (kind, focus) {
            (DialogKind::Add, 0) => {
                expect_pop(&url0, &d.url.value);
                follow(cursor0.0, d.url.len_chars(), d.url.cursor);
            }
            (DialogKind::Add, 1) => {
                expect_pop(&dir0, &d.dir.value);
                follow(cursor0.1, d.dir.len_chars(), d.dir.cursor);
            }
            (DialogKind::Add, 4) => {
                expect_pop(&ck0, &d.ck_value.value);
                follow(cursor0.3, d.ck_value.len_chars(), d.ck_value.cursor);
            }
            (DialogKind::Modify, 2) => {
                expect_pop(&ck0, &d.ck_value.value);
                follow(cursor0.3, d.ck_value.len_chars(), d.ck_value.cursor);
            }
            (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
                expect_pop(&conns0, &d.conns.value);
                prop_assert!(d.conns_edited, "并发退格置 edited");
                follow(cursor0.2, d.conns.len_chars(), d.conns.cursor);
            }
            _ => {
                prop_assert_eq!(&d.url.value, &url0, "非退格字段零变化");
                prop_assert_eq!(&d.dir.value, &dir0, "非退格字段零变化");
                prop_assert_eq!(&d.conns.value, &conns0, "非退格字段零变化");
                prop_assert_eq!(&d.ck_value.value, &ck0, "非退格字段零变化");
            }
        }
    }

    #[test]
    fn prop_push_hex_capped_cap_and_charset(
        chars in prop::collection::vec(any::<char>(), 0..200),
    ) {
        let mut d = crate::app::testutil::dialog(DialogKind::Add, 4);
        for c in &chars {
            crate::app::push_hex_capped(&mut d, *c);
            prop_assert!(d.ck_value.len_chars() <= 128, "校验码 ≤ 128");
            prop_assert!(
                d.ck_value.value.chars().all(|x| x.is_ascii_hexdigit()),
                "仅十六进制字符"
            );
        }
    }

    #[test]
    fn prop_dropdown_open_key_sync_and_bounds(
        sel0 in 0usize..=CHECKSUM_ALGOS.len(),
        keys in dialog_key_seq(),
    ) {
        let mut d = crate::app::testutil::dialog(DialogKind::Add, 3);
        d.ck_open = true;
        d.ck_sel = sel0;
        d.ck_type = sel0;
        for k in &keys {
            if !d.ck_open {
                break;
            }
            let was_open = d.ck_open;
            let navigated = matches!(k, KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End);
            crate::app::dropdown_open_key(&mut d, *k);
            prop_assert!(
                d.ck_sel <= CHECKSUM_ALGOS.len(),
                "ck_sel 恒在下拉 8 项界内（NONE + 7 算法）"
            );
            prop_assert_eq!(d.ck_type, d.ck_sel, "ck_type 与 ck_sel 同步");
            if was_open && navigated {
                prop_assert!(d.ck_open, "选择键保持展开");
            }
            if d.ck_type == crate::model::checksum::CK_NONE {
                prop_assert!(d.ck_value.is_empty(), "选中 NONE 时校验码实时清空");
            }
            if *k == KeyCode::Tab {
                prop_assert_eq!(d.focus, 4, "Tab 关闭并跳焦 4");
            }
            if *k == KeyCode::BackTab {
                prop_assert_eq!(d.focus, 2, "BackTab 关闭并跳焦 2");
            }
        }
    }

    #[test]
    fn prop_proxy_dropdown_open_key_bounds_and_guard(
        n in 0usize..12,
        sel0 in 0usize..12,
        kind in prop::sample::select(vec![DialogKind::Add, DialogKind::Modify]),
        keys in dialog_key_seq(),
    ) {
        let mut d = crate::app::testutil::dialog(kind, 5);
        d.proxy_open = true;
        d.proxy_sel = sel0 % n.max(1);
        for k in &keys {
            if !d.proxy_open {
                break;
            }
            crate::app::proxy_dropdown_open_key(&mut d, n, *k);
            prop_assert!(d.proxy_sel < n.max(1), "proxy_sel 恒在界内（n=0 守卫后 n'=1）");
            let expect_focus = match (kind, k) {
                (DialogKind::Add, KeyCode::Tab) => Some(6),
                (DialogKind::Add, KeyCode::BackTab) => Some(4),
                (DialogKind::Modify, KeyCode::Tab) => Some(4),
                (DialogKind::Modify, KeyCode::BackTab) => Some(2),
                _ => None,
            };
            if let Some(f) = expect_focus {
                prop_assert_eq!(d.focus, f, "关闭并按布局跳焦");
            }
        }
    }
}

proptest! {
    #[test]
    fn prop_blocks_op_sequence_invariants(
        piece in 1u64..(1u64 << 20),
        n_blocks in 1u64..=4096u64,
        ops in prop::collection::vec((0u8..2, any::<u32>(), any::<u64>()), 0..48),
    ) {
        let total = (n_blocks - 1) * piece + 1;
        let mut b = Blocks::new(total, piece);
        let y = b.count();
        prop_assert_eq!(u64::from(y), n_blocks, "块数构造口径");
        for (op, i, amt) in ops {
            let idx = i % y;
            match op {
                0 => b.mark_done(idx),
                _ => b.mark_progress(idx, amt),
            }
            prop_assert!(b.downloaded() <= total, "downloaded ≤ total");
            for (bi, w) in b.written.iter().enumerate() {
                if let Some((s, e)) = block_range(total, piece, bi as u32) {
                    prop_assert!(*w <= e - s, "块 {bi} 写入不越块长");
                }
            }
        }
        for i in 0..y {
            b.mark_done(i);
        }
        prop_assert_eq!(b.downloaded(), total, "全块完成后 downloaded == total");
        prop_assert_eq!(b.completed(), b.count(), "全块完成后 x == y");
    }

    #[test]
    fn prop_save_json_atomic_idempotent(
        value0 in prop::collection::vec(any::<u64>(), 0..20),
        value1 in prop::collection::vec(any::<u64>(), 0..20),
    ) {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-prop-atomic");
        let path = dir.join("snap.json").to_string_lossy().to_string();
        crate::model::save_json_atomic(&value0, &path).unwrap();
        let bytes1 = std::fs::read(&path).unwrap();
        crate::model::save_json_atomic(&value0, &path).unwrap();
        let bytes2 = std::fs::read(&path).unwrap();
        prop_assert_eq!(bytes1, bytes2, "同内容二遍保存字节一致（幂等短路）");
        prop_assert!(
            !dir.join("snap.json.tmp").exists(),
            "幂等跳过不得残留 tmp 文件"
        );
        crate::model::save_json_atomic(&value1, &path).unwrap();
        let parsed: Vec<u64> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        prop_assert_eq!(parsed, value1, "内容漂移后落盘且可解析还原");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prop_from_url_path_total_and_shape(
        scheme in "[a-zA-Z]{1,6}",
        host in "[a-z0-9.]{0,12}",
        seg in "[a-zA-Z0-9_%.~-]{0,24}",
    ) {
        let url = format!("{scheme}://{host}/{seg}");
        let _ = from_url_path(&url);
        for (suffix, expect_suffix) in [("%", "%"), ("%2", "%2"), ("%.bin", "%.bin")] {
            let u = format!("http://x/p{suffix}");
            if let Some(name) = from_url_path(&u) {
                prop_assert!(name.ends_with(expect_suffix), "残缺尾 {suffix} 保留原文");
            }
        }
        if let Some(name) = from_url_path("http://x/50%25.bin") {
            prop_assert_eq!(name, "50%.bin");
        }
        let _ = seg;
    }

    #[test]
    fn prop_from_content_disposition_shape(
        disp in "(attachment|inline)?;? ?",
        name in "[a-zA-Z0-9_.\\-]{1,20}",
    ) {
        let cd = format!("{disp}filename={name}");
        match from_content_disposition(&cd) {
            Some(n) => {
                prop_assert!(!n.is_empty());
                prop_assert!(!n.contains('/') && !n.contains('\\') && !n.contains('\0'));
            }
            None => prop_assert!(name.is_empty(), "非空裸 token 应可提取"),
        }
    }

    #[test]
    fn prop_parse_companion_content_stability(
        algo_idx in 0usize..CHECKSUM_ALGOS.len(),
        token in prop::collection::vec("[0-9a-fA-F]", 0..132),
        noise in prop::option::of("[g-zG-Z]{1,4}"),
        tail in prop::option::of("[^\n]{0,20}"),
    ) {
        let (_, need, _) = CHECKSUM_ALGOS[algo_idx];
        let mut tok: String = token.into_iter().collect();
        if let Some(n) = &noise {
            tok = format!("{tok}{n}");
        }
        let content = match &tail {
            Some(t) => format!("{tok} {t}\n"),
            None => format!("{tok}\n"),
        };
        let v = tok.to_lowercase();
        let expect_some = !v.is_empty()
            && v.chars().all(|c| c.is_ascii_hexdigit())
            && v.chars().count() == need
            && noise.is_none();
        match parse_companion_content(algo_idx, &content) {
            Some(sc) => {
                prop_assert!(expect_some, "有效输入才得 Some");
                prop_assert_eq!(sc.algo_idx, algo_idx);
                prop_assert_eq!(sc.value, v);
            }
            None => prop_assert!(!expect_some, "无效输入必须 None"),
        }
    }

    #[test]
    fn prop_parse_retry_after_header(
        secs in prop::option::of(any::<f64>().prop_filter("finite", |v| v.is_finite())),
        junk in "[a-zA-Z]{0,6}",
    ) {
        if let Some(s) = secs {
            let raw = format!("{s}");
            let parsed = parse_retry_after_header(Some(&raw));
            if s >= 0.0 {
                prop_assert_eq!(parsed, Some(s), "非负有限秒数原值返回");
            } else {
                prop_assert_eq!(parsed, None, "负数拒绝");
            }
        }
        prop_assert_eq!(parse_retry_after_header(Some("NaN")), None, "NaN 拒绝");
        if !junk.parse::<f64>().is_ok() {
            prop_assert_eq!(parse_retry_after_header(Some(&junk)), None, "垃圾拒绝");
        }
        prop_assert_eq!(parse_retry_after_header(None), None, "缺头 None");
        prop_assert_eq!(parse_retry_after_header(Some("inf")), Some(f64::INFINITY));
    }

    #[test]
    fn prop_classify_http_status_exhaustive(status in any::<u16>()) {
        let transient =
            status == 408 || status == 429 || (500..=599).contains(&status);
        let kind = classify_http_status(status);
        prop_assert_eq!(matches!(kind, FailKind::Transient), transient);
        if !transient {
            prop_assert!(matches!(kind, FailKind::Fatal));
        }
    }

    #[test]
    fn prop_queue_positions_prefix_count_oracle(
        n in 1usize..14,
        seed in any::<u64>(),
    ) {
        let mut tasks = Vec::new();
        for i in 0..n {
            let mut t = crate::model::sample_task();
            t.id = i as u32 + 1;
            t.state = match (i + seed as usize) % 3 {
                0 => TaskState::Queued,
                1 => TaskState::Paused,
                _ => TaskState::Downloading,
            };
            t.has_slot = (i * 7 + seed as usize).is_multiple_of(2);
            tasks.push(t);
        }
        let mut view: Vec<usize> = (0..n).collect();
        view.sort_by_key(|&i| {
            (i.wrapping_mul(2654435761).wrapping_add(seed as usize)) % 9973
        });
        let positions = queue_positions(&tasks, &view);
        prop_assert_eq!(positions.len(), n);
        for (vi, &ti) in view.iter().enumerate() {
            let waiting = tasks[ti].state == TaskState::Queued && !tasks[ti].has_slot;
            let expect = if waiting {
                view[..=vi]
                    .iter()
                    .filter(|&&pj| {
                        tasks[pj].state == TaskState::Queued && !tasks[pj].has_slot
                    })
                    .count()
            } else {
                0
            };
            prop_assert_eq!(positions[vi], expect, "位次 = 视图前缀等待计数");
        }
        let identity: Vec<usize> = (0..n).collect();
        let positions = queue_positions(&tasks, &identity);
        for (ti, &p) in positions.iter().enumerate() {
            let expect = queue_pos(&tasks, tasks[ti].id).unwrap_or(0);
            prop_assert_eq!(p, expect, "恒等视图下与 queue_pos 数值等价");
        }
    }

    #[test]
    fn prop_algo_index_exact_or_default(variant in "[a-zA-Z0-9_ ]{0,12}") {
        for (i, (disp, _, _)) in CHECKSUM_ALGOS.iter().enumerate() {
            prop_assert_eq!(
                algo_index_exact_or_default(disp),
                i,
                "规范名回查 {}",
                disp
            );
        }
        let expected = CHECKSUM_ALGOS
            .iter()
            .position(|(disp, _, _)| *disp == variant)
            .unwrap_or(3);
        prop_assert_eq!(algo_index_exact_or_default(&variant), expected);
        prop_assert_eq!(algo_index_exact_or_default("sha1"), 3);
    }

    #[test]
    fn prop_url_scheme_helpers(
        scheme in prop::sample::select(vec!["http", "https", "HTTP", "HTTPS", "ftp", "Https", ""]),
        rest in "[^\n]{0,20}",
    ) {
        let url = format!("{scheme}://{rest}");
        let expect_http = url.starts_with("http://") || url.starts_with("https://");
        prop_assert_eq!(is_http_url(&url), expect_http);
        prop_assert_eq!(
            protocol_of_url(&url) == Protocol::Https,
            url.starts_with("https://")
        );
    }

    #[test]
    fn prop_path_suffix_conventions(
        dir in "/[a-z]{0,10}(/)?",
        name in "[a-z0-9]{1,12}",
    ) {
        let t = Task::new_queued(
            1,
            name.clone(),
            Protocol::Http,
            "http://x/f.bin".to_string(),
            dir.clone(),
            1 << 20,
            4,
            5,
            None,
            ProxyChoice::Direct,
            0,
        );
        prop_assert_eq!(t.sidecar_path(), sidecar_path_of(&dir, &name), "Task 方法 ≡ 散点助手");
        prop_assert_eq!(
            t.sidecar_path(),
            format!("{}{SIDECAR_EXT}", t.target_path()),
            "sidecar ≡ target + .ezr"
        );
        prop_assert_eq!(
            t.downloading_path(),
            format!("{}{DOWNLOADING_EXT}", t.target_path()),
            "downloading ≡ target + .downloading"
        );
    }

    #[test]
    fn prop_task_projection_bounds(
        total in 0u64..(1u64 << 40),
        downloaded in 0u64..(1u64 << 40),
        speed in 0.0f64..1e9,
        state in prop::sample::select(vec![
            TaskState::Queued,
            TaskState::Downloading,
            TaskState::Paused,
            TaskState::Completed,
            TaskState::Failed,
        ]),
        chunk_done in 0u32..1000,
    ) {
        let mut t = Task::new_queued(
            1,
            "f.bin".to_string(),
            Protocol::Http,
            "http://x/f.bin".to_string(),
            "/dl".to_string(),
            1 << 20,
            4,
            5,
            None,
            ProxyChoice::Direct,
            0,
        );
        t.total = total;
        t.downloaded = downloaded;
        t.speed = speed;
        t.state = state;
        t.chunk_done = chunk_done;
        let p = t.progress();
        prop_assert!((0.0..=1.0).contains(&p), "progress ∈ [0,1]");
        if total == 0 {
            prop_assert_eq!(p, 0.0, "未知大小 progress = 0");
        }
        if state != TaskState::Downloading || speed <= 0.0 {
            prop_assert!(t.eta_secs().is_none(), "非下载态或零速无 ETA");
        }
        let (x, y, n) = t.chunk_info();
        prop_assert!(u64::from(x) <= y, "已完成块数 ≤ 总块数");
        prop_assert_eq!(n, 1u64 << 20, "块大小透传");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_digest_bytes_output_domain(
        data in prop::collection::vec(any::<u8>(), 0..300),
        algo_idx in 0usize..7,
    ) {
        let digest = crate::model::checksum::digest_bytes(&data, algo_idx);
        let (_, need, _) = CHECKSUM_ALGOS[algo_idx];
        prop_assert_eq!(digest.len(), need, "位数恰为算法表期望值");
        prop_assert!(
            digest.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "小写十六进制"
        );
        prop_assert_eq!(
            crate::model::checksum::validate_value(algo_idx, &digest),
            Ok(digest.clone()),
            "计算输出被校验器原样接受"
        );
    }

    #[test]
    fn prop_digest_file_matches_bytes(
        data in prop::collection::vec(any::<u8>(), 0..(1 << 20)),
        algo_idx in 0usize..7,
    ) {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-prop-digest");
        let path = dir.join("payload.bin");
        std::fs::write(&path, &data).unwrap();
        let streamed =
            crate::model::checksum::digest_file(&path.to_string_lossy(), algo_idx).unwrap();
        let inmem = crate::model::checksum::digest_bytes(&data, algo_idx);
        prop_assert_eq!(streamed, inmem, "流式 == 内存 同内容同摘要");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prop_endpoint_url_bracket_iff_colon(
        ip in "[0-9a-fA-F:]{2,50}",
    ) {
        let cfg = crate::model::config::ProxyConfig {
            name: "p".to_string(),
            kind: crate::model::config::ProxyKind::Http,
            ip: ip.clone(),
            port: 1,
            username: None,
            password: None,
        };
        let url = cfg.endpoint_url();
        if ip.contains(':') {
            prop_assert!(url.contains(&format!("[{ip}]:1")), "含冒号必带方括号: {url}");
        } else {
            prop_assert!(!url.contains('['), "无冒号不加括号: {url}");
        }
    }

    #[test]
    fn prop_endpoint_url_ipv6_parseable(
        segs in prop::collection::vec("[0-9a-fA-F]{1,4}", 8..=8),
        port in 1u16..=65535,
    ) {
        let ip = segs.join(":");
        let cfg = crate::model::config::ProxyConfig {
            name: "p".to_string(),
            kind: crate::model::config::ProxyKind::Http,
            ip: ip.clone(),
            port,
            username: None,
            password: None,
        };
        let url = cfg.endpoint_url();
        prop_assert!(url.contains(&format!("[{ip}]:{port}")), "IPv6 必带方括号: {url}");
        let parsed = url::Url::parse(&url);
        prop_assert!(parsed.is_ok(), "全 8 段方括号形态可被 url 解析: {url}");
        if let Ok(parsed) = parsed {
            prop_assert_eq!(parsed.port_or_known_default(), Some(port), "端口保真");
        }
    }

    #[test]
    fn prop_endpoint_url_plain_ip_parseable(
        ip in "[0-9]{1,3}(\\.[0-9]{1,3}){3}",
        port in 1u16..=65535,
    ) {
        let cfg = crate::model::config::ProxyConfig {
            name: "p".to_string(),
            kind: crate::model::config::ProxyKind::Socks5,
            ip,
            port,
            username: None,
            password: None,
        };
        let url = cfg.endpoint_url();
        prop_assert!(!url.contains('['), "无冒号 ip 不加方括号: {url}");
        prop_assert!(url::Url::parse(&url).is_ok(), "点分形态可解析: {url}");
    }
}

proptest! {
    #[test]
    fn prop_task_progress_bounded(total in 0u64..(1 << 40), downloaded in 0u64..(1 << 40)) {
        let mut t = crate::model::sample_task();
        t.total = total;
        t.downloaded = downloaded;
        let p = t.progress();
        prop_assert!((0.0..=1.0).contains(&p), "progress 越界: {p}");
        if total == 0 {
            prop_assert_eq!(p, 0.0);
        }
    }

    #[test]
    fn prop_task_eta_none_gates(
        state in prop::sample::select(&[
            TaskState::Queued, TaskState::Paused, TaskState::Downloading,
            TaskState::Verifying, TaskState::Completed, TaskState::Failed,
        ]),
        speed in -1.0f64..1e9,
        total in 0u64..(1 << 40),
        downloaded in 0u64..(1 << 40),
    ) {
        let mut t = crate::model::sample_task();
        t.state = state;
        t.speed = speed;
        t.total = total;
        t.downloaded = downloaded.min(total);
        let eta = t.eta_secs();
        if state != TaskState::Downloading || speed <= 0.0 {
            prop_assert_eq!(eta, None, "非下载态/零速恒 None");
        } else {
            prop_assert!(eta.is_some(), "下载中正速必有 ETA");
            if let Some(secs) = eta {
                prop_assert!(secs < u64::MAX, "非饱和有限值");
            }
        }
    }

    #[test]
    fn prop_task_chunk_info_consistent(
        total in 1u64..(1 << 32),
        block_size in 1u64..(1 << 20),
        chunk_done in 0u32..6000,
    ) {
        let mut t = crate::model::sample_task();
        t.total = total;
        t.block_size = block_size;
        t.chunk_done = chunk_done;
        let (x, y, n) = t.chunk_info();
        prop_assert_eq!(y, u64::from(crate::model::chunk::chunk_total(total, block_size)));
        prop_assert_eq!(n, block_size);
        prop_assert!(u64::from(x) <= y, "完成块数不得越界: x={x} y={y}");
    }

    #[test]
    fn prop_task_paths_suffix_invariants(
        dir in "[a-zA-Z0-9_./-]{0,40}",
        name in "[a-zA-Z0-9_-]{1,30}",
    ) {
        let mut t = crate::model::sample_task();
        t.save_dir = dir;
        t.name = name.clone();
        let target = t.target_path();
        prop_assert_eq!(t.downloading_path(), format!("{target}{}", crate::model::namegen::DOWNLOADING_EXT));
        prop_assert_eq!(t.sidecar_path(), format!("{target}{}", crate::model::namegen::SIDECAR_EXT));
        prop_assert!(target.ends_with(&name), "目标路径以文件名收尾");
    }
}

proptest! {
    #[test]
    fn prop_speed_window_rate_bounded(
        n in 1usize..40,
        base_ms in 0u64..600_000,
        cums in prop::collection::vec(any::<u64>(), 1..40),
    ) {
        let t0 = std::time::Instant::now();
        let mut w = crate::model::speed::SpeedWindow::new();
        let mut last = None;
        for i in 0..n {
            let at = t0 + std::time::Duration::from_millis(base_ms + i as u64 * 120);
            let c = cums[i % cums.len()];
            w.push(at, c);
            last = Some(at);
        }
        let rate = w.rate();
        prop_assert!(rate >= 0.0, "速率恒非负: {rate}");
        let (cmin, cmax) = cums.iter().fold((u64::MAX, 0u64), |(lo, hi), &c| {
            (lo.min(c), hi.max(c))
        });
        let bound = (cmax.saturating_sub(cmin)) as f64 / 0.05;
        prop_assert!(rate <= bound + 1e-9, "速率超极差上界: rate={rate} bound={bound}");
        prop_assert_eq!(w.last_push(), last, "last_push = 最后推进时刻");
    }
}

proptest! {
    #[test]
    fn prop_throttle_bucket_step_invariants(
        rate in 1.0f64..1e9,
        tokens in 0.0f64..1e9,
        elapsed in 0.0f64..1e4,
        remaining in 0.0f64..1e12,
    ) {
        let cap = rate * crate::engine::BURST_SECS;
        let available = (tokens + elapsed * rate).min(cap);
        let (tokens_after, taken, wait) =
            crate::engine::bucket_step(rate, tokens, elapsed, remaining);
        prop_assert_eq!(taken, remaining.min(available), "取走 = min(remaining, 可用)");
        prop_assert_eq!(taken + tokens_after, available, "预算守恒");
        prop_assert!(tokens_after >= 0.0 && tokens_after <= cap, "桶余钳制 [0, cap]");
        if remaining <= available {
            prop_assert_eq!(wait, 0, "取足收束");
        } else {
            let deficit_ms = (remaining - available) / rate * 1000.0;
            prop_assert_eq!(
                wait,
                deficit_ms.max(crate::engine::SLEEP_MS as f64) as u64,
                "等待 = max(deficit 换算, SLEEP_MS)"
            );
        }
    }
}

proptest! {
    #[test]
    fn prop_apply_chunk_snapshot_monotone(
        n_blocks in 1usize..=4096,
        piece in 1u64..(1 << 16),
        extra in 0u64..4096,
        done_bytes in 0u64..(4096u64 * (1 << 16)),
        old_done in 0u32..6000,
    ) {
        let total = (n_blocks as u64 - 1) * piece + extra.min(piece) + 1;
        let mut t = crate::model::sample_task();
        t.total = total;
        t.block_size = piece;
        t.concurrency = 4;
        t.apply_chunk_snapshot(done_bytes, old_done);
        let (expect_conns, snap) =
            crate::model::chunk::lease_snapshot(total, done_bytes, 4, piece);
        prop_assert_eq!(&t.connections, &expect_conns, "连接视图 = lease_snapshot 同参投影");
        let y_count = crate::model::chunk::chunk_total(total, piece);
        if t.resumable && y_count > 1 {
            prop_assert_eq!(t.chunk_done, old_done, "多块路径 = 事件口径（前缀幻影不得盖过）");
        } else {
            prop_assert_eq!(t.chunk_done, snap.max(old_done), "单流/单块 = max(快照, 事件口径)");
        }
        prop_assert!(t.chunk_done >= old_done, "不越事件口径");
        let y = u64::from(y_count);
        let (x, y_view, _) = t.chunk_info();
        prop_assert_eq!(y_view, y, "展示块数 = 分块数学");
        prop_assert!(u64::from(x) <= y, "读取侧钳制：展示完成数不越总块数（即便存储越界）");
    }

    #[test]
    fn prop_thread_count_data_plane(
        conns in prop::collection::vec(
            (0u64..(1 << 20), 0u64..(1 << 20), 0u64..(1 << 20)),
            0..12,
        ),
        downloading in prop::bool::ANY,
    ) {
        let mut t = crate::model::sample_task();
        t.state = if downloading {
            TaskState::Downloading
        } else {
            TaskState::Paused
        };
        t.connections = conns
            .iter()
            .map(|&(s, len, done)| {
                let end = s.saturating_add(len);
                crate::model::Connection {
                    id: 1,
                    start: s,
                    end,
                    done: done.min(end - s),
                }
            })
            .collect();
        let expected = t
            .connections
            .iter()
            .filter(|c| {
                let cap = c.end.saturating_sub(c.start);
                if downloading {
                    cap > 0 && c.done < cap
                } else {
                    cap > 0
                }
            })
            .count();
        prop_assert_eq!(t.thread_count(), expected, "thread_count 数据面口径");
    }
}

proptest! {
    #[test]
    fn prop_derive_name_fallback_chain(
        cd in prop::option::of(prop::collection::vec(any::<char>(), 0..24)),
        seg_a in "[a-zA-Z0-9._\\-]{0,12}",
        seg_b in "[a-zA-Z0-9._\\-]{0,12}",
        with_final in prop::bool::ANY,
    ) {
        let cd: Option<String> = cd.map(|v| v.into_iter().collect());
        let final_url = with_final.then(|| format!("http://f.io/{seg_a}"));
        let url = format!("http://o.io/{seg_b}");
        let name = derive_name(cd.as_deref(), final_url.as_deref(), &url);

        prop_assert!(!name.is_empty(), "derive_name 恒产出非空名");
        prop_assert_eq!(name.clone(), sanitize_name(&name), "输出不含路径分隔符/控制字符");

        let cd_pick = cd.as_deref().map(str::trim).filter(|s| !s.is_empty());
        if let Some(n) = cd_pick {
            prop_assert_eq!(name, sanitize_name(n), "CD 非空时最高优先");
        } else if let Some(f) = final_url.as_deref().and_then(from_url_path) {
            prop_assert_eq!(name, sanitize_name(&f), "落点路径段次优先");
        } else if let Some(o) = from_url_path(&url) {
            prop_assert_eq!(name, sanitize_name(&o), "原始 URL 路径段再次之");
        } else {
            prop_assert!(name.starts_with("download-"), "全空时时间戳兜底");
        }
    }

    #[test]
    fn prop_alloc_task_id_fresh_nonzero_unique(
        seed in prop::collection::vec(any::<u32>(), 0..4096),
        rounds in 1usize..8,
    ) {
        let mut used: std::collections::HashSet<u32> = seed.into_iter().collect();
        used.insert(0);
        let mut seen = Vec::new();
        for _ in 0..rounds {
            let id = alloc_task_id(|c| used.contains(&c));
            prop_assert_ne!(id, 0, "0 为保留哨兵，不得分配");
            prop_assert!(!used.contains(&id), "分配结果不得与占用集命中");
            prop_assert!(!seen.contains(&id), "同轮连续分配互不相同");
            seen.push(id);
            used.insert(id);
        }
    }
}

fn arb_ipv4() -> impl Strategy<Value = IpAddr> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(a, b, c, d)| IpAddr::V4(Ipv4Addr::new(a, b, c, d)))
}

fn arb_ipv6_ula() -> impl Strategy<Value = IpAddr> {
    (any::<u16>(), any::<u16>())
        .prop_map(|(a, b)| IpAddr::V6(Ipv6Addr::new(0xfd00, 0, a, 0, 0, 0, 0, b)))
}

proptest! {
    #[test]
    fn prop_hosts_parse_contract(
        lines in prop::collection::vec(
            (
                prop_oneof![arb_ipv4(), arb_ipv6_ula()],
                prop::collection::vec("[a-z][a-z0-9]{0,7}", 0..4),
                prop::collection::vec("[a-z0-9]{1,6}[!?*#%/]{0,2}", 0..3),
                any::<bool>(),
            ),
            0..10,
        ),
    ) {
        let mut text = String::new();
        let mut text_commented = String::new();
        let mut declared: std::collections::BTreeMap<String, Vec<IpAddr>> = Default::default();
        for (ip, aliases, garbage, comment) in &lines {
            let mut toks: Vec<String> = vec![ip.to_string()];
            for a in aliases {
                toks.push(a.clone());
            }
            let line = toks.join(" ");
            text.push_str(&line);
            text_commented.push_str(&line);
            if *comment {
                text_commented.push_str(" # v146 probe");
            }
            text.push('\n');
            text_commented.push('\n');
            for g in garbage {
                let gl = format!("## {g} {g}=");
                text.push_str(&gl);
                text.push('\n');
                text_commented.push_str(&gl);
                text_commented.push('\n');
            }
            for a in aliases {
                let key = a.to_ascii_lowercase();
                let ips = declared.entry(key).or_default();
                if !ips.contains(ip) {
                    ips.push(*ip);
                }
            }
        }

        let hosts = HostsFile::from_lines(&text);
        let hosts_c = HostsFile::from_lines(&text_commented);

        for s in ["-probe9", "-PROBE9"] {
            prop_assert_eq!(hosts.lookup(s), None, "未声明探针应 None");
        }

        for (alias, expected) in &declared {
            let got = hosts.lookup(alias);
            let want = Some(expected.clone());
            prop_assert_eq!(&got, &want, "命中列表 = 首见序去重声明序列");
            prop_assert_eq!(
                hosts.lookup(&alias.to_ascii_uppercase()),
                got.clone(),
                "大写变体等价"
            );
            prop_assert_eq!(hosts_c.lookup(alias), got.clone(), "行尾注释免疫");
        }
    }

    #[test]
    fn prop_merge_answers_ipv4_first_dedup(
        v4 in prop::collection::vec(arb_ipv4(), 0..12),
        v6 in prop::collection::vec(arb_ipv6_ula(), 0..12),
    ) {
        let merged = merge_answers(v4.clone(), v6.clone());
        prop_assert_eq!(
            merged.iter().collect::<std::collections::BTreeSet<_>>().len(),
            merged.len(),
            "输出零重复"
        );
        let last_v4 = merged.iter().rposition(|ip| ip.is_ipv4());
        let first_v6 = merged.iter().position(|ip| ip.is_ipv6());
        if let (Some(l4), Some(f6)) = (last_v4, first_v6) {
            prop_assert!(l4 < f6, "v4 先于 v6（last_v4={l4}, first_v6={f6}）");
        }
        let mut oracle: Vec<IpAddr> = Vec::with_capacity(v4.len() + v6.len());
        for ip in v4.iter().chain(v6.iter()) {
            if !oracle.contains(ip) {
                oracle.push(*ip);
            }
        }
        prop_assert_eq!(merged, oracle, "全局去重保序 oracle");
    }
}

fn field_text_full() -> impl proptest::strategy::Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            5 => proptest::char::range('\u{20}', '\u{7E}'),
            2 => proptest::char::range('\u{4E00}', '\u{9FA5}'),
            1 => Just('\u{FE0E}'),
        ],
        1..40,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn field_window_of(
    s: &str,
    cursor: usize,
    budget: usize,
    focused: bool,
) -> crate::textfield::FieldDisplay {
    let mut t = crate::textfield::TextInput::with_value(s.to_string());
    t.set_cursor(cursor);
    t.field_window("", budget, focused)
}

proptest! {
    #[test]
    fn prop_field_window_slice_identity(
        s in field_text_full(),
        cursor in 0usize..48,
        budget in 1usize..=30,
        focused in proptest::bool::ANY,
    ) {
        let fd = field_window_of(&s, cursor, budget, focused);
        let pre_core = fd.pre.strip_prefix('…').unwrap_or(&fd.pre);
        let post_core = fd.post.strip_suffix('…').unwrap_or(&fd.post);
        let expect: String = s
            .chars()
            .skip(fd.win.win_lo)
            .take(fd.win.win_hi - fd.win.win_lo)
            .collect();
        prop_assert_eq!(format!("{pre_core}{post_core}"), expect, "窗口内容 = 原值切片");
        let n = s.chars().count();
        prop_assert!(fd.win.win_lo <= fd.win.win_hi, "窗界有序");
        prop_assert!(fd.win.win_hi <= n, "窗尾不越值域");
        if focused {
            let anchor = cursor.min(n);
            prop_assert!(
                fd.win.win_lo <= anchor && anchor <= fd.win.win_hi,
                "聚焦锚（光标）在窗口内"
            );
        }
    }

    #[test]
    fn prop_field_window_budget_conserved(
        s in field_text_full(),
        cursor in 0usize..48,
        budget in 3usize..=30,
        focused in proptest::bool::ANY,
    ) {
        let fd = field_window_of(&s, cursor, budget, focused);
        let shown = crate::textfield::w(&fd.pre) + crate::textfield::w(&fd.post);
        prop_assert!(shown <= budget, "显示宽 ≤ 预算（含指示符）");
        let narrow_only = s.chars().all(|c| (c as u32) < 0x80 || c == '\u{FE0E}');
        if narrow_only && crate::textfield::w(&s) > budget {
            prop_assert_eq!(shown, budget, "窄字符超宽域内容恰满预算（不动点口径）");
        }
    }

    #[test]
    fn prop_field_window_fits_shows_all(
        s in field_text_full(),
        cursor in 0usize..48,
        slack in 0usize..8,
        focused in proptest::bool::ANY,
    ) {
        let budget = crate::textfield::w(&s) + slack;
        let fd = field_window_of(&s, cursor, budget, focused);
        let n = s.chars().count();
        prop_assert_eq!(fd.win.win_lo, 0, "整值可见：窗头 = 0");
        prop_assert_eq!(fd.win.win_hi, n, "整值可见：窗尾 = 字符数");
        prop_assert_eq!(fd.win.cols.len(), n + 1, "cols = 字符数 + 1");
        prop_assert_eq!(format!("{}{}", fd.pre, fd.post), s, "pre+post = 原值");
    }

    #[test]
    fn prop_click_offset_within_window(
        s in field_text_full(),
        cursor in 0usize..48,
        slack in 0usize..8,
        col in 0u16..200u16,
    ) {
        let budget = crate::textfield::w(&s) + slack;
        let mut fd = field_window_of(&s, cursor, budget, true);
        fd.win.value_x = 40;
        let off = crate::textfield::TextInput::click_offset(&fd.win, col);
        prop_assert!(
            fd.win.win_lo <= off && off <= fd.win.win_hi,
            "点击映射封闭在 [win_lo, win_hi]"
        );
    }

    #[test]
    fn prop_marquee_constant_width_and_periodic(width in 1usize..=60, anim in 0u64..600u64) {
        let color = ratatui::style::Color::Green;
        let block = (width / 4).clamp(4, 8);
        let period = (width + block) as u64;
        let total: usize = crate::ui::marquee_bar(width, anim, color)
            .iter()
            .flat_map(|s| s.content.chars())
            .filter(|c| *c == '█' || *c == '░')
            .count();
        prop_assert_eq!(total, width, "总格数恒定 = width");
        let a: String = crate::ui::marquee_bar(width, anim, color)
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        let b: String = crate::ui::marquee_bar(width, anim + period, color)
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        prop_assert_eq!(a, b, "帧画面按周期循环");
    }

    #[test]
    fn prop_wrap_text_lossless_and_bounded(s in field_text_full(), width in 1usize..=40usize) {
        let lines = crate::ui::wrap_text(&s, width);
        let joined: String = lines.iter().map(|l| l.as_str()).collect::<Vec<_>>().concat();
        prop_assert_eq!(&joined, &s, "折行零损耗");
        for l in &lines {
            let lw = crate::textfield::w(l);
            prop_assert!(
                lw <= width || l.chars().count() == 1,
                "行宽 ≤ 折行宽（单宽字豁免）: {l:?} w={lw} width={width}"
            );
        }
    }
}
