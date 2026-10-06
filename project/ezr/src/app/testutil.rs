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
