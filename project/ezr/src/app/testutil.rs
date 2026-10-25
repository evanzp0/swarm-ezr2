//! 测试夹具共享模块（仅 `cfg(test)` 编译）：Dialog 构造的单一事实来源。
//!
//! PMD CPD 登记：Dialog 字段多（14 个），Add/Modify 默认夹具字面量曾在
//! `dialog_keys`/`paste`/`mouse` 各测试模块重复 4 份——收敛为本模块单一
//! 构造点；默认值对齐既有 Add/Modify 夹具（算法下标 4 = SHA-256——v1.32
//! 起下拉 8 项含首行 NONE，算法表下标 + 1；校验码空、下拉收起、代理直连
//! 选中）。Delete 等特殊形态由调用方按差异字段覆盖，不再整段复制字面量。
//!
//! architect v153 追加（app 树 App 夹具族收口，对齐 `ui::testfx` 既有先例）：
//! `make_app` 曾在 app 树 14 处测试模块同构克隆、`dl_task` 5 处、滚动条播种
//! 循环 4 处（v152-clean 移交的 CPD tile 漂移主体）——收敛为本模块单一构
//! 造点，各调用方原地一行委托（差异点 = prefix/block_size 参数化），临时
//! 目录命名与委托前逐字一致（`<prefix>-<tag>-<pid>-…`）。

use super::{App, Dialog, DialogKind};
use crate::textfield::TextInput;

/// 唯一临时目录 + 默认配置构造 App（prefix 区分调用方测试组；与
/// `ui::testfx::make_app_in` 同型——app 树测试夹具单源）。目录名 =
/// `<prefix>-<tag>-<pid>-<序号>-<毫秒>`（testenv 唯一化语义不变）。
pub(crate) fn make_app_in(prefix: &str, tag: &str) -> App {
    let dir = crate::model::testenv::uniq_tmp_dir(&format!("{prefix}-{tag}"));
    let reg = dir.join("registry.json").to_string_lossy().into_owned();
    App::new(crate::model::config::Config::default(), reg)
}

/// 下载中基准任务（滚动条/交互族）：`sample_task` 基底改 state=Downloading、
/// 已探测、total 5 MB（block_size 沿用 sample 默认 1 MiB）。
pub(crate) fn dl_task(id: u32, name: &str) -> crate::model::Task {
    let mut t = crate::model::sample_task();
    t.id = id;
    t.name = name.to_string();
    t.state = crate::model::TaskState::Downloading;
    t.probed = true;
    t.total = 5_000_000;
    t
}

/// 同 [`dl_task`]，block_size = 1 MB（10^6；分块弹窗数学族口径：
/// 5 MB 总量 5 块整除）——keys/mouse/pieces 各测试模块的
/// `dl_task`/`openable` 收敛单源。
pub(crate) fn dl_task_mb(id: u32, name: &str) -> crate::model::Task {
    let mut t = dl_task(id, name);
    t.block_size = 1_000_000;
    t
}

/// 滚动条交互族播种：n 个排队任务（同 id 同名 dl_task 基底改 Queued）——
/// 配 `visible_rows = 5` 与 30 任务 → 列表滚动上限 25（既有四测脚手架
/// 单源，种子分布不变）。
pub(crate) fn seed_queued(app: &mut App, n: usize) {
    for _ in 0..n {
        let mut t = dl_task(1, "t.bin");
        t.state = crate::model::TaskState::Queued;
        app.tasks.push(t);
    }
}

/// Add/Modify 对话框夹具：焦点 `focus`，其余字段为共享默认
pub(crate) fn dialog(kind: DialogKind, focus: usize) -> Dialog {
    Dialog {
        kind,
        // 输入框组件夹具：字段空 = 光标 0；需光标语义的测试按场景
        // set_cursor 显式置位
        url: TextInput::new(),
        dir: TextInput::new(),
        conns: TextInput::new(),
        conns_edited: false,
        ck_type: 4,
        ck_value: TextInput::new(),
        ck_open: false,
        ck_sel: 4,
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
        &crate::model::consistency::ServerStamp {
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

/// 带活跃连接的任务夹具（并发面板交互测试共享构造点）：`n` 条连接按
/// 1KB 段首尾排布（cap > 0 = 活跃相），配 `Downloading`/`Seeding` 等
/// shows_conns 状态即可入并发面板口径。
pub(crate) fn task_with_conns(
    id: u32,
    name: &str,
    state: crate::model::TaskState,
    n: usize,
) -> crate::model::Task {
    let mut t = crate::model::sample_task();
    t.id = id;
    t.name = name.to_string();
    t.state = state;
    t.connections = (1..=n)
        .map(|i| crate::model::Connection {
            id: i,
            start: (i as u64 - 1) * 1000,
            end: i as u64 * 1000,
            done: 100,
        })
        .collect();
    t
}
