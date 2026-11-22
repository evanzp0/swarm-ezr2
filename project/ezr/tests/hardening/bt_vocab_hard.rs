#![allow(missing_docs)]

use crate::model::bt::{delete_deselected_files, BtFile};
use crate::model::persist::remove_missing_ok;
use crate::model::testenv::uniq_tmp_dir;

fn rm_root(p: &std::path::Path) {
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn delete_deselected_removes_previously_selected_file() {
    let root = uniq_tmp_dir("ezr-hard-del-sel");
    let victim = root.join("victim.bin");
    std::fs::write(&victim, b"x").unwrap();
    let old = vec![BtFile {
        path: "victim.bin".to_string(),
        size: 1,
        selected: true,
    }];
    let new = vec![BtFile {
        path: "victim.bin".to_string(),
        size: 1,
        selected: false,
    }];
    let deleted = delete_deselected_files(root.to_str().unwrap(), &old, &new);
    assert_eq!(deleted, 1);
    assert!(!victim.exists());
    rm_root(&root);
}

#[test]
fn delete_deselected_spares_file_that_was_never_selected() {
    let root = uniq_tmp_dir("ezr-hard-del-keep");
    let survivor = root.join("survivor.bin");
    let spare = root.join("spare.bin");
    std::fs::write(&survivor, b"a").unwrap();
    std::fs::write(&spare, b"b").unwrap();
    let old = vec![
        BtFile {
            path: "survivor.bin".to_string(),
            size: 1,
            selected: true,
        },
        BtFile {
            path: "spare.bin".to_string(),
            size: 1,
            selected: false,
        },
    ];
    let new = vec![
        BtFile {
            path: "survivor.bin".to_string(),
            size: 1,
            selected: true,
        },
        BtFile {
            path: "spare.bin".to_string(),
            size: 1,
            selected: false,
        },
    ];
    let deleted = delete_deselected_files(root.to_str().unwrap(), &old, &new);
    assert_eq!(deleted, 0);
    assert!(survivor.exists());
    assert!(spare.exists());
    rm_root(&root);
}

#[test]
fn remove_missing_ok_propagates_non_notfound_error() {
    let root = uniq_tmp_dir("ezr-hard-rmo");
    let missing = root.join("absent.bin");
    assert!(remove_missing_ok(missing.to_str().unwrap()).is_ok());
    let dir_path = root.join("nested");
    std::fs::create_dir(&dir_path).unwrap();
    let err = remove_missing_ok(dir_path.to_str().unwrap()).unwrap_err();
    assert_ne!(err.kind(), std::io::ErrorKind::NotFound);
    rm_root(&root);
}
