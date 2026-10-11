//! 测试夹具共享模块（仅 `cfg(test)` 编译）：Dialog 构造的单一事实来源。
//!
//! PMD CPD 登记：Dialog 字段多（14 个），Add/Modify 默认夹具字面量曾在
//! `dialog_keys`/`paste`/`mouse` 各测试模块重复 4 份——收敛为本模块单一
//! 构造点；默认值对齐既有 Add/Modify 夹具（算法下标 3 = SHA-256、校验码
//! 空、下拉收起、代理直连选中）。Delete 等特殊形态由调用方按差异字段
//! 覆盖，不再整段复制字面量。

use super::{Dialog, DialogKind};

/// Add/Modify 对话框夹具：焦点 `focus`，其余字段为共享默认
pub(crate) fn dialog(kind: DialogKind, focus: usize) -> Dialog {
    Dialog {
        kind,
        url: String::new(),
        dir: String::new(),
        conns: String::new(),
        conns_edited: false,
        ck_type: 3,
        ck_value: String::new(),
        ck_open: false,
        ck_sel: 3,
        proxy_sel: 0,
        proxy_open: false,
        focus,
        task_name: String::new(),
        task_id: None,
    }
}

/// 预置同 URL sidecar 夹具（断点接续视图：模拟既有下载残留）——
/// dialog_keys「确认添加接续既有 sidecar」与 dialogs「仅添加接续落暂停」
/// 两个测试共享的单一构造点（PMD CPD 127tok 同构收敛）。
/// URL 固定 `http://example.com/r.bin`、size=1000、块 1 MB、单块已完成、
/// sidecar 落 `dir/r.bin.ezr`。
pub(crate) fn preset_resume_sidecar(dir: &std::path::Path) {
    let sc = crate::model::sidecar::Sidecar::build(
        "http://example.com/r.bin",
        &crate::model::consistency::ServerStamp {
            final_url: None,
            etag: None,
            last_modified: None,
            size: Some(1000),
        },
        1000,
        1024 * 1024,
        &[0],
        false,
        None,
        crate::model::sidecar::SidecarTask {
            id: 99,
            added_at: 0,
            save_dir: dir.to_string_lossy().into_owned(),
            concurrency: 4,
            protocol: crate::model::Protocol::Http,
        },
    );
    sc.save(&dir.join("r.bin.ezr").to_string_lossy()).ok();
}
