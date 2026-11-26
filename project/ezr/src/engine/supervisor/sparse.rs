use std::fs::File as StdFile;
use std::io;
use std::path::Path;

pub(super) fn preallocate(file: &StdFile, path: &Path, size: u64) -> io::Result<()> {
    if mark_sparse(file, path).is_err() {
        let _ = fallocate_keep_size(file, size);
    }
    file.set_len(size)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn mark_sparse(file: &StdFile, _path: &Path) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    const FSCTL_SET_SPARSE: u32 = 0x0009_0080;
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        handle: isize,
    }
    extern "system" {
        fn DeviceIoControl(
            handle: isize,
            code: u32,
            in_buf: *const u8,
            in_len: u32,
            out_buf: *mut u8,
            out_len: u32,
            returned: *mut u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as isize,
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn mark_sparse(_file: &StdFile, path: &Path) -> io::Result<()> {
    const NTFS_ATTRIB_SPARSE: u32 = 0x0008_0000;
    const XATTR_NAME: &str = "system.ntfs_attrib";
    let existing = match xattr::get(path, XATTR_NAME) {
        Ok(Some(v)) if v.len() == 4 => u32::from_le_bytes([v[0], v[1], v[2], v[3]]),
        _ => 0,
    };
    let next = (existing | NTFS_ATTRIB_SPARSE).to_le_bytes();
    xattr::set(path, XATTR_NAME, &next)?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "linux")))]
fn mark_sparse(_file: &StdFile, _path: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "平台无稀疏标记"))
}

fn fallocate_keep_size(file: &StdFile, size: u64) -> io::Result<()> {
    fs2::FileExt::allocate(file, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preallocate_falls_back_silently_and_sets_size() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-sparse");
        let p = dir.join("f.bin");
        let f = StdFile::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&p)
            .unwrap();
        preallocate(&f, &p, 1_000_000).unwrap();
        let meta = std::fs::metadata(&p).unwrap();
        assert_eq!(meta.len(), 1_000_000, "预分配后逻辑大小到位");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mark_sparse_on_ext4_is_rejected_gracefully() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-sparse-mark");
        let p = dir.join("g.bin");
        let f = StdFile::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&p)
            .unwrap();
        let r = mark_sparse(&f, &p);
        match r {
            Ok(()) => {}
            Err(e) => {
                assert!(
                    e.raw_os_error().is_some()
                        || matches!(
                            e.kind(),
                            io::ErrorKind::Unsupported
                                | io::ErrorKind::PermissionDenied
                                | io::ErrorKind::Other
                                | io::ErrorKind::InvalidInput
                                | io::ErrorKind::NotFound
                        ),
                    "ext4 等非 ntfs3 文件系统应报 OS 级错误而非 panic: {e}"
                );
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preallocate_zero_size_noop() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-sparse-zero");
        let p = dir.join("z.bin");
        let f = StdFile::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&p)
            .unwrap();
        preallocate(&f, &p, 0).unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
