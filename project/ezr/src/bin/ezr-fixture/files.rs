use std::io::Write;
use std::path::Path;

pub fn write_patterned(mut f: &std::fs::File, size: u64) -> std::io::Result<()> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut remaining = size;
    let mut i: u64 = 0;
    while remaining > 0 {
        let n = buf.len().min(remaining as usize);
        for b in buf[..n].iter_mut() {
            *b = (i % 251) as u8;
            i += 1;
        }
        f.write_all(&buf[..n])?;
        remaining -= n as u64;
    }
    Ok(())
}

pub fn gen_files(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let put = |name: &str, size: u64, sparse: bool| -> std::io::Result<()> {
        let p = root.join(name);
        if p.exists() {
            return Ok(());
        }
        let f = std::fs::File::create(&p)?;
        if sparse {
            f.set_len(size)?;
        } else {
            write_patterned(&f, size)?;
        }
        Ok(())
    };
    put("five-m.bin", 5 * 1024 * 1024, false)?;
    put("three-m.bin", 3 * 1024 * 1024, false)?;
    put("eight-m.bin", 8 * 1024 * 1024, false)?;
    put("twelve-m.bin", 12 * 1024 * 1024, false)?;
    put("two-m.bin", 2 * 1024 * 1024, false)?;
    put("one-m.bin", 1024 * 1024, false)?;
    put("half-m.bin", 512 * 1024, false)?;
    put("ten-m.bin", 10 * 1024 * 1024, false)?;
    put("small.bin", 1024, false)?;
    put("big-100m.bin", 100 * 1024 * 1024, true)?;
    put("streamy.bin", 4 * 1024 * 1024, false)?;
    put("ghost-404.bin", 0, false)?;
    println!("文件集已生成: {}", root.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_patterned_matches_i_mod_251_across_buffers() {
        let dir = crate::testenv::uniq_tmp_dir("ezr-fx-pt");
        let p = dir.join("pat.bin");
        let f = std::fs::File::create(&p).unwrap();
        write_patterned(&f, 200_000).unwrap();
        drop(f);
        let data = std::fs::read(&p).unwrap();
        assert_eq!(data.len(), 200_000);
        for (i, b) in data.iter().enumerate() {
            assert_eq!(*b, (i % 251) as u8, "offset {i}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn gen_files_skips_existing_files() {
        let dir = crate::testenv::uniq_tmp_dir("ezr-fx-skip");
        std::fs::write(dir.join("small.bin"), b"custom").unwrap();
        gen_files(&dir).unwrap();
        assert_eq!(std::fs::read(dir.join("small.bin")).unwrap(), b"custom");
        assert_eq!(dir.join("one-m.bin").metadata().unwrap().len(), 1024 * 1024);
        std::fs::remove_dir_all(&dir).ok();
    }
}
