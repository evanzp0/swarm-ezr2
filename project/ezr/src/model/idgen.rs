use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static CALL_SEQ: AtomicU32 = AtomicU32::new(0);

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

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

fn random_candidate() -> u32 {
    let stack_probe = 0u8;
    let addr = &stack_probe as *const u8 as usize as u64;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let seq = CALL_SEQ.fetch_add(1, Ordering::Relaxed) as u64;
    let mut g = SplitMix64(
        nanos ^ ((u64::from(std::process::id())) << 32) ^ addr ^ seq.wrapping_mul(GOLDEN),
    );
    let cand = (g.next_u64() >> 32) as u32;
    if cand == 0 {
        1
    } else {
        cand
    }
}

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

    #[test]
    fn collision_with_used_set_retries() {
        let prefill: Vec<u32> = (0..64).map(|_| random_candidate()).collect();
        let id = alloc_task_id(|c| prefill.contains(&c));
        assert!(!prefill.contains(&id), "分配结果不得落在占用集内");
    }

    #[test]
    fn consecutive_candidates_differ() {
        let a = random_candidate();
        let b = random_candidate();
        assert_ne!(a, b, "同纳秒连发应因调用序号获得不同序列");
    }
}
