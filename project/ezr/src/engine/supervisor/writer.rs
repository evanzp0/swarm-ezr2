use std::collections::VecDeque;
use std::fs::File as StdFile;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(super) const BATCH: usize = 256 * 1024;
pub(super) const WRITE_MAX: usize = 4 * 1024 * 1024;
pub(super) const SPARE_MAX: usize = WRITE_MAX / BATCH;
pub(super) const HOLD_MAX: u64 = 256 * 1024 * 1024;

struct Batch {
    at: u64,
    data: Vec<u8>,
}

struct WriteJob {
    intervals: Vec<(u64, u64)>,
    bytes: u64,
    handle: tokio::task::JoinHandle<io::Result<Vec<Vec<u8>>>>,
}

struct WriterInner {
    file: StdFile,
    queue: VecDeque<Batch>,
    buf: Option<Batch>,
    writing: Option<WriteJob>,
    spare: Vec<Vec<u8>>,
    written: Vec<(u64, u64)>,
    flushed_prefix: u64,
    total_flushed: u64,
    total_pushed: u64,
}

impl WriterInner {
    fn pending(&self) -> u64 {
        let queued: u64 = self.queue.iter().map(|b| b.data.len() as u64).sum();
        let buf = self.buf.as_ref().map_or(0, |b| b.data.len() as u64);
        let writing = self.writing.as_ref().map_or(0, |j| j.bytes);
        queued + buf + writing
    }

    fn absorb(&mut self, intervals: &[(u64, u64)]) {
        for (a, b) in intervals {
            insert_merged(&mut self.written, *a, *b);
            self.total_flushed += b - a;
        }
        loop {
            let advance = match self.written.first() {
                Some(&(a, b)) if a <= self.flushed_prefix => b.max(self.flushed_prefix),
                _ => break,
            };
            self.flushed_prefix = advance;
            self.written.remove(0);
        }
    }

    fn recycle(&mut self, bufs: Vec<Vec<u8>>) {
        for b in bufs {
            if self.spare.len() < SPARE_MAX {
                self.spare.push(b);
            }
        }
    }

    fn dispatch(&mut self) {
        if self.writing.is_some() {
            return;
        }
        if let Some(b) = self.buf.take() {
            self.queue.push_back(b);
        }
        if self.queue.is_empty() {
            return;
        }
        let mut batches: Vec<(u64, Vec<u8>)> = Vec::new();
        let mut intervals = Vec::new();
        let mut bytes = 0u64;
        while let Some(front) = self.queue.front() {
            if bytes + front.data.len() as u64 > WRITE_MAX as u64 && !batches.is_empty() {
                break;
            }
            let b = self.queue.pop_front().unwrap();
            bytes += b.data.len() as u64;
            intervals.push((b.at, b.at + b.data.len() as u64));
            batches.push((b.at, b.data));
        }
        let Ok(f) = self.file.try_clone() else {
            return;
        };
        let handle = tokio::task::spawn_blocking(move || {
            for (at, data) in &batches {
                write_all_at(&f, data, *at)?;
            }
            Ok(batches.into_iter().map(|(_, d)| d).collect())
        });
        self.writing = Some(WriteJob {
            intervals,
            bytes,
            handle,
        });
    }
}

#[cfg(unix)]
fn write_all_at(f: &StdFile, data: &[u8], at: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    f.write_all_at(data, at)
}

#[cfg(windows)]
fn write_all_at(f: &StdFile, data: &[u8], at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    let mut off = at;
    let mut rest = data;
    while !rest.is_empty() {
        let n = f.seek_write(rest, off)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "seek_write 写入 0 字节",
            ));
        }
        off += n as u64;
        rest = &rest[n..];
    }
    Ok(())
}

fn insert_merged(v: &mut Vec<(u64, u64)>, a: u64, b: u64) {
    let pos = v.partition_point(|&(s, _)| s < a);
    v.insert(pos, (a, b));
    let i = pos;
    while i + 1 < v.len() && v[i].1 >= v[i + 1].0 {
        let hi = v[i].1.max(v[i + 1].1);
        v[i].1 = hi;
        v.remove(i + 1);
    }
    if i > 0 && v[i - 1].1 >= v[i].0 {
        let hi = v[i - 1].1.max(v[i].1);
        v[i - 1].1 = hi;
        v.remove(i);
    }
}

pub(super) struct Writer {
    inner: Arc<Mutex<WriterInner>>,
}

impl Writer {
    pub(super) fn open(path: &Path) -> io::Result<Writer> {
        let file = StdFile::options().write(true).open(path)?;
        Ok(Self::from_file(file))
    }

    pub(super) fn from_file(file: StdFile) -> Writer {
        Writer {
            inner: Arc::new(Mutex::new(WriterInner {
                file,
                queue: VecDeque::new(),
                buf: None,
                writing: None,
                spare: Vec::new(),
                written: Vec::new(),
                flushed_prefix: 0,
                total_flushed: 0,
                total_pushed: 0,
            })),
        }
    }

    pub(super) fn push(&self, at: u64, data: &[u8]) -> io::Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.total_pushed += data.len() as u64;
        let mut at = at;
        let mut rest = data;
        while !rest.is_empty() {
            let seal = match &g.buf {
                None => true,
                Some(b) => b.at + b.data.len() as u64 != at || b.data.len() >= BATCH,
            };
            if seal {
                if let Some(b) = g.buf.take() {
                    g.queue.push_back(b);
                }
                let mut v = g
                    .spare
                    .pop()
                    .unwrap_or_else(|| Vec::with_capacity(BATCH));
                v.clear();
                g.buf = Some(Batch { at, data: v });
            }
            let b = g.buf.as_mut().unwrap();
            let room = BATCH - b.data.len();
            let n = rest.len().min(room);
            b.data.extend_from_slice(&rest[..n]);
            if b.data.len() >= BATCH {
                if let Some(b) = g.buf.take() {
                    g.queue.push_back(b);
                }
            }
            at += n as u64;
            rest = &rest[n..];
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn pending_bytes(&self) -> u64 {
        self.inner.lock().unwrap().pending()
    }

    pub(super) fn total_pushed(&self) -> u64 {
        self.inner.lock().unwrap().total_pushed
    }

    pub(super) fn total_flushed(&self) -> u64 {
        self.inner.lock().unwrap().total_flushed
    }

    pub(super) fn global_flushed_prefix(&self) -> u64 {
        self.inner.lock().unwrap().flushed_prefix
    }

    pub(super) fn flushed_extent_from(&self, start: u64) -> u64 {
        let g = self.inner.lock().unwrap();
        let mut c = g.flushed_prefix.max(start);
        if start > g.flushed_prefix {
            c = start;
        }
        for &(a, b) in &g.written {
            if a <= c && b > c {
                c = b;
            }
            if a > c {
                break;
            }
        }
        c
    }

    async fn reclaim(shared: &Arc<Mutex<WriterInner>>) -> io::Result<()> {
        loop {
            let job = {
                let mut g = shared.lock().unwrap();
                match g.writing.as_ref() {
                    Some(j) if j.handle.is_finished() => g.writing.take().unwrap(),
                    _ => return Ok(()),
                }
            };
            let bufs = job
                .handle
                .await
                .map_err(|e| io::Error::other(e.to_string()))??;
            let mut g = shared.lock().unwrap();
            g.absorb(&job.intervals);
            g.recycle(bufs);
        }
    }

    pub(super) async fn step(&self, hold_cap: u64) -> io::Result<()> {
        let shared = Arc::clone(&self.inner);
        Self::reclaim(&shared).await?;
        let job = {
            let mut g = shared.lock().unwrap();
            if g.writing.is_none() {
                g.dispatch();
            }
            if g.pending() >= hold_cap {
                g.writing.take()
            } else {
                None
            }
        };
        if let Some(job) = job {
            let bufs = job
                .handle
                .await
                .map_err(|e| io::Error::other(e.to_string()))??;
            let mut g = shared.lock().unwrap();
            g.absorb(&job.intervals);
            g.recycle(bufs);
        }
        Ok(())
    }

    pub(super) async fn finish(&self) -> io::Result<()> {
        let shared = Arc::clone(&self.inner);
        loop {
            Self::reclaim(&shared).await?;
            let job = {
                let mut g = shared.lock().unwrap();
                if g.writing.is_none() {
                    g.dispatch();
                }
                match g.pending() {
                    0 => None,
                    _ => g.writing.take(),
                }
            };
            let Some(job) = job else { break };
            let bufs = job
                .handle
                .await
                .map_err(|e| io::Error::other(e.to_string()))??;
            let mut g = shared.lock().unwrap();
            g.absorb(&job.intervals);
            g.recycle(bufs);
        }
        Ok(())
    }

    pub(super) async fn sync_data(&self) -> io::Result<()> {
        let f = self.inner.lock().unwrap().file.try_clone()?;
        tokio::task::spawn_blocking(move || f.sync_data())
            .await
            .map_err(|e| io::Error::other(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-writer");
        (dir.join(name), dir)
    }

    fn new_writer(name: &str) -> (Writer, std::path::PathBuf, std::path::PathBuf) {
        let (p, dir) = tmp_path(name);
        std::fs::File::create(&p).unwrap();
        let w = Writer::open(&p).unwrap();
        (w, p, dir)
    }

    async fn drain(w: &Writer) -> io::Result<()> {
        w.finish().await
    }

    #[tokio::test]
    async fn push_seals_batches_and_writes_positions() {
        let (w, p, dir) = new_writer("a.bin");
        w.push(0, &[1u8; BATCH]).unwrap();
        w.push(BATCH as u64, &[2u8; 100]).unwrap();
        assert_eq!(w.pending_bytes(), (BATCH + 100) as u64, "队列满 batch + 在填 batch");
        drain(&w).await.unwrap();
        assert_eq!(w.total_flushed(), BATCH as u64 + 100);
        assert_eq!(w.global_flushed_prefix(), BATCH as u64 + 100);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), BATCH as u64 + 100);
        let data = std::fs::read(&p).unwrap();
        assert!(data[..BATCH].iter().all(|&b| b == 1));
        assert!(data[BATCH..].iter().all(|&b| b == 2));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn push_discontinuous_starts_new_batch() {
        let (w, p, dir) = new_writer("b.bin");
        w.push(0, &[7u8; 100]).unwrap();
        w.push(1_000, &[9u8; 50]).unwrap();
        drain(&w).await.unwrap();
        let data = std::fs::read(&p).unwrap();
        assert_eq!(data.len(), 1_050);
        assert!(data[..100].iter().all(|&b| b == 7));
        assert!(data[100..1_000].iter().all(|&b| b == 0), "空洞保持零");
        assert!(data[1_000..].iter().all(|&b| b == 9));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn step_dispatches_without_waiting_below_runway() {
        let (w, p, dir) = new_writer("c.bin");
        w.push(0, &[1u8; BATCH * 2]).unwrap();
        let hold_cap = (BATCH * 8) as u64;
        w.step(hold_cap).await.unwrap();
        assert_eq!(
            w.pending_bytes(),
            (BATCH * 2) as u64,
            "跑道未满仅派发不等待（在途计入 pending）"
        );
        drain(&w).await.unwrap();
        assert_eq!(w.total_flushed(), (BATCH * 2) as u64);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), (BATCH * 2) as u64);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn step_awaits_when_runway_exhausted() {
        let (w, p, dir) = new_writer("c2.bin");
        for i in 0..8usize {
            w.push((i * BATCH) as u64, &[i as u8; BATCH]).unwrap();
        }
        let hold_cap = (BATCH * 2) as u64;
        w.step(hold_cap).await.unwrap();
        assert!(
            w.total_flushed() > 0,
            "跑道满触发等待回收，至少一单落盘"
        );
        drain(&w).await.unwrap();
        assert_eq!(w.total_flushed(), (BATCH * 8) as u64);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn flushed_extent_tracks_sparse_completion() {
        let (w, p, dir) = new_writer("d.bin");
        w.push(0, &[1u8; 100]).unwrap();
        w.push(10_000, &[2u8; 100]).unwrap();
        assert_eq!(w.global_flushed_prefix(), 0, "前沿未推进");
        drain(&w).await.unwrap();
        assert_eq!(w.flushed_extent_from(0), 100, "[0,100) 已写");
        assert_eq!(w.flushed_extent_from(10_000), 10_100);
        assert_eq!(w.flushed_extent_from(5_000), 5_000, "空洞区无写入");
        assert_eq!(w.flushed_extent_from(99_999), 99_999);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn big_stream_coalesces_into_bounded_writes() {
        let (w, p, dir) = new_writer("e.bin");
        let total = BATCH * 10;
        for i in 0..10usize {
            w.push((i * BATCH) as u64, &[i as u8; BATCH]).unwrap();
        }
        drain(&w).await.unwrap();
        assert_eq!(w.total_flushed(), total as u64);
        let data = std::fs::read(&p).unwrap();
        assert_eq!(data.len(), total);
        for i in 0..10usize {
            assert!(
                data[i * BATCH..(i + 1) * BATCH]
                    .iter()
                    .all(|&b| b == i as u8),
                "第 {i} 段内容一致"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn spare_pool_recycles_buffers() {
        let (w, p, dir) = new_writer("f.bin");
        for i in 0..(SPARE_MAX + 4) {
            w.push((i * BATCH) as u64, &[i as u8; BATCH]).unwrap();
        }
        drain(&w).await.unwrap();
        assert_eq!(w.total_flushed(), ((SPARE_MAX + 4) * BATCH) as u64);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), ((SPARE_MAX + 4) * BATCH) as u64);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn zero_length_push_is_noop() {
        let (w, p, dir) = new_writer("g.bin");
        w.push(0, &[]).unwrap();
        drain(&w).await.unwrap();
        assert_eq!(w.total_pushed(), 0);
        assert_eq!(w.total_flushed(), 0);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn sync_data_completes_without_error() {
        let (w, p, dir) = new_writer("h.bin");
        w.push(0, &[1u8; 1_000]).unwrap();
        drain(&w).await.unwrap();
        w.sync_data().await.unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().len(), 1_000);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn insert_merged_keeps_sorted_disjoint() {
        let mut v: Vec<(u64, u64)> = Vec::new();
        insert_merged(&mut v, 100, 200);
        insert_merged(&mut v, 0, 50);
        insert_merged(&mut v, 200, 300);
        insert_merged(&mut v, 50, 100);
        assert_eq!(v, vec![(0, 300)], "相邻区间合并");
        insert_merged(&mut v, 400, 500);
        insert_merged(&mut v, 350, 400);
        assert_eq!(v, vec![(0, 300), (350, 500)]);
    }
}
