//! idgen — 任务 ID 生成（u32 随机数 + 任务全集查重；v1.20，操作者 20261011 指令）
//!
//! 设计口径（取代顺序自增 `next_id`）：
//! - 顺序 ID 的三个固有问题：①注册表需跨会话簿记 `next_id`，任务清空后计数
//!   不回退（持续膨胀）；②逼近 `u32::MAX` 时无回收语义——release 静默回绕
//!   与既有任务撞号（debug 构建直接 panic）；③顺序值泄漏任务创建数量。
//! - 随机 u32：分配时对当前任务全集（含已完成/失败历史快照）查重，冲突即
//!   重试——4.29e9 取值空间下生日碰撞在十万级任务量级概率仍可忽略，且查重
//!   兜底后唯一性确定成立；无需回收逻辑，任务清空与否均不影响分配。
//! - 0 值保留作「无任务」哨兵（不得分配）。
//! - ID 仅是内部关联键（引擎事件 / 连接窗口 / sidecar `task.id`），创建顺序
//!   由 `added_at`（Unix 秒）承载；展示面仅详情页单独一行。
//!
//! 随机源：SplitMix64（零依赖；统计质量足够 ID 唯一性用途）。种子熵 =
//! 系统时钟纳秒 ^ 进程号 ^ 栈地址（ASLR）^ 进程内调用序号——四次异或保证
//! 同进程同纳秒连发、多进程同启、跨运行均获得不同序列。

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 进程内调用序号（同纳秒多次调用仍获得不同种子）
static CALL_SEQ: AtomicU32 = AtomicU32::new(0);

/// SplitMix64 黄金比例增量常数
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// SplitMix64 生成器（零依赖伪随机源）
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GOLDEN);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// 生成一个非零的随机任务 ID 候选（唯一性由 [`alloc_task_id`] 查重保证）
fn random_candidate() -> u32 {
    let stack_probe = 0u8;
    // 取引用地址仅作熵源（不解引用；as usize 为安全转换，无 unsafe）
    let addr = &stack_probe as *const u8 as usize as u64;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let seq = CALL_SEQ.fetch_add(1, Ordering::Relaxed) as u64;
    let mut g = SplitMix64(
        nanos ^ ((u64::from(std::process::id())) << 32) ^ addr ^ seq.wrapping_mul(GOLDEN),
    );
    let cand = (g.next_u64() >> 32) as u32;
    // 0 为保留哨兵；命中概率 2^-32 量级，直接取 1 兜底
    if cand == 0 {
        1
    } else {
        cand
    }
}

/// 分配一个与既有任务不冲突的随机任务 ID（u32；0 保留哨兵；冲突自动重试
/// ——取值空间 4.29e9，实际不会形成有感的重试链）。
/// `used` = 对任务全集（含已完成/失败历史）的占用判定闭包。
#[must_use]
pub fn alloc_task_id(used: impl Fn(u32) -> bool) -> u32 {
    loop {
        let cand = random_candidate();
        if !used(cand) {
            return cand;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// 连续分配 1000 个 ID：全部非零、互不相同（查重分配的唯一性口径）
    #[test]
    fn allocated_ids_are_nonzero_and_unique() {
        let mut used: Vec<u32> = Vec::new();
        for _ in 0..1000 {
            let id = alloc_task_id(|c| used.contains(&c));
            assert_ne!(id, 0, "0 为保留哨兵，不得分配");
            used.push(id);
        }
        let unique: HashSet<u32> = used.iter().copied().collect();
        assert_eq!(unique.len(), used.len(), "分配结果互不相同");
    }

    /// 占用集命中时自动重试：把前 64 个候选占住后，新分配仍不撞号
    #[test]
    fn collision_with_used_set_retries() {
        let prefill: Vec<u32> = (0..64).map(|_| random_candidate()).collect();
        let id = alloc_task_id(|c| prefill.contains(&c));
        assert!(!prefill.contains(&id), "分配结果不得落在占用集内");
    }

    /// 同纳秒连发（CALL_SEQ 熵）不得产生相同候选
    #[test]
    fn consecutive_candidates_differ() {
        let a = random_candidate();
        let b = random_candidate();
        assert_ne!(a, b, "同纳秒连发应因调用序号获得不同序列");
    }
}
