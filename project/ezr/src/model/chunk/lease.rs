use super::plan::chunk_total;
use crate::model::Connection;

#[must_use]
pub fn lease_snapshot(total: u64, done_bytes: u64, n: usize, piece: u64) -> (Vec<Connection>, u32) {
    let y = chunk_total(total, piece);
    if y == 0 {
        return (Vec::new(), 0);
    }
    let y64 = u64::from(y);
    let piece_end = |i: u64| if i + 1 == y64 { total } else { (i + 1) * piece };
    let done = done_bytes.min(total);
    let (x, lease_from, active, rem) = if done >= total {
        let a = (n.max(1) as u64).min(y64);
        (y64, y64 - a, a, 0u64)
    } else {
        let x = done / piece;
        let a = (n.max(1) as u64).min(y64 - x);
        (x, x, a, done - x * piece)
    };
    let partials = spread_bytes(rem, active as usize, piece);
    let conns = (0..active as usize)
        .map(|i| {
            let idx = lease_from + i as u64;
            let start = idx * piece;
            let end = piece_end(idx);
            Connection {
                id: i + 1,
                start,
                end,
                done: if done >= total {
                    end - start
                } else {
                    partials[i].min(end - start)
                },
            }
        })
        .collect();
    (conns, x as u32)
}

const CONN_WEIGHTS: [f64; 16] = [
    0.95, 0.45, 0.10, 0.00, 0.75, 0.40, 0.85, 0.15, 0.55, 0.20, 0.55, 0.00, 0.70, 0.25, 0.80, 0.35,
];

#[must_use]
pub fn spread_bytes(amount: u64, n: usize, cap: u64) -> Vec<u64> {
    let mut v = vec![0u64; n];
    if amount == 0 || n == 0 {
        return v;
    }
    let wsum: f64 = (0..n).map(|i| CONN_WEIGHTS[i % 16]).sum();
    let mut assigned = 0u64;
    for (i, item) in v.iter_mut().enumerate() {
        let raw = (amount as f64 * (CONN_WEIGHTS[i % 16] / wsum)).round() as u64;
        let d = raw.min(cap);
        *item = d;
        assigned += d;
    }
    let mut diff = amount as i64 - assigned as i64;
    let mut k = 0;
    while diff != 0 && k < 8 * n {
        let i = k % n;
        if diff > 0 && v[i] < cap {
            let add = (diff as u64).min(cap - v[i]);
            v[i] += add;
            diff -= add as i64;
        } else if diff < 0 && v[i] > 0 {
            let sub = ((-diff) as u64).min(v[i]);
            v[i] -= sub;
            diff += sub as i64;
        }
        k += 1;
    }
    v
}
