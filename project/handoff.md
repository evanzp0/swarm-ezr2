# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder 终态　会话：v124-code-batch5（任务 v124-code-batch5）
> 本轮（v1.18 实现）：specifier v124-spec-batch5 交接的修订 A/B TDD 实现 + 测试 3 条。
> 全量 **2506P/0F**（Δ=+3）、clippy 0、fmt 0、arch 12 规则过。操作者缺陷闭环：
> 「已下载 100% 的文件清码 retry 进度回退重传」→ 文件完整性直判零重传收尾。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.1，分期模式）+ `project/mission/phase-01.md`（**v1.18**）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120-code-two-batch） | FR-01-102 连接速度同管线 + FR-01-103 仅添加/立即下载三钮 | 已交付 | 2438P/0F、clippy 0 |
| coder（v121-code-batch3） | FR-01-104 1MB 块下限 + FR-01-105 实时字节口径 + D30 下调即时收缩 | 已交付 | 2470P/0F、clippy 0 |
| coder（v122-code-batch4） | FR-01-06 v1.17 双对话框粘贴 + FR-01-51 v1.17 清码三分支 + mock hold 门控根治 flake | 已交付 | 2503P/0F、clippy 0 |
| specifier（v124-spec-batch5） | FR-01-51 v1.18 直判规格 + D34 v1.18 注记 + 场景 12 注/QA-IC-12 v1.18 | 已交付 | parser + dry-check 过 |
| coder（v124-code-batch5，本轮） | 修订 A 终态 sidecar 即时落盘 + 修订 B 清码 retry 文件完整性直判 + 测试 3 条 | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过 |

### 遗留受限项（当前有效）
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e（见二）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 本轮无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾，
  v1.18 实现精化见二）。修订 B fallback（文件不完整 → 作废断点从头重下）为 FR-01-22
  作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（coder v124-code-batch5）

### 本会话产物清单
- `src/engine/supervisor.rs`（修订 A）：多块完成臂「有校验值」时完成前即时落盘**全块完成
  口径**的终态 sidecar——原仅 `SIDECAR_FLUSH`（2s）周期落盘，完成点与最后落盘间的尾部
  字节不在台账（快速下载从未落盘）；「校验中」退出应用重启的续传入口同根修复。单流路径
  不动（侧车 = non_resumable，重开即 truncate 全量重流）。
- `src/app/tasks.rs`（修订 B）：`requeue_failed` verify_fail 臂分支③（清码，checksum=None）
  改**文件完整性直判**——①`.downloading`/目标任一存在且 `len == total` → 不经引擎直接
  收尾 Completed（命中 `.downloading` 则 rename 目标、命中目标免改名、`Sidecar::remove`、
  `verify_ok = None`、速度归零、清连接/速度窗、toast「✓ 文件已完整，直接完成（无校验）:
  {名}」、`save_registry()`）；②缺失/大小不符 → `Sidecar::remove` 作废断点 + `downloaded = 0`、
  `chunk_done = 0`、清连接、清速度窗、`session_seen.insert(id, 0)`（比照 Invalidated 臂）+
  toast「↻ {手动重试|重新排队}（本地文件不完整，断点已作废，将从头下载）: {名}」、保持
  Queued。分支①②（伴随/显式值重校验）与公共尾段不动。
- `tests/hardening_v115.rs`：+1 端到端（修订 A+B 双直击：2×1MB 多块 + 错误 SHA-256 →
  Failed(Verify) 时断言盘上终态 sidecar `downloaded == total` → 清码 → R → 断言**立即**
  Completed + toast「无校验」+ 目标文件在 + `.downloading`/sidecar 删）。
- `tests/hardening_g_appcore.rs`：+2（`…_completes_directly_when_file_intact` 直判臂 /
  `…_discards_checkpoint_when_file_short` fallback 臂，纯 App 构造无服务器）。
- TDD 证据：三红（sidecar 未落盘 / 直判臂 Queued≠Completed / fallback 臂 downloaded 1024≠0）
  → 修订 A 落码后失败点前移（修订 B 断言红）→ 修订 B 落码全绿。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单测 | 2506P / 0F（13 目标） | `cargo test --workspace` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| fmt | 0 偏差 | `cargo fmt --check` |
| 架构边界 | 12 规则全过 | `bash scripts/arch_check.sh` |
| 抖动排查 | 受影响双目标 3 轮复跑全绿（284P/328P × 3） | `cargo test --test hardening_v115 --test hardening_g_appcore` ×3 |

### 对账口径（基线 → 终态）
- 基线 2503P/0F（v122-code-batch4 终态；本轮开工全新环境实测复现一致）→ 终态 **2506P/0F**。
- Δ = +3 = 3 条新测试函数（v115 +1、appcore +2），无挂载树倍增（新测试在 tests/ 顶层
  测试文件，非 `#[path]` 收编源内联测试）。分项：373/30/11/124/304/**284**/277/2/12/
  255/173/333/**328**（粗体为本轮 +1/+2 目标），和 = 2506 自洽。
- 兼容性实证：既有 `retry_verify_failed_cleared_checksum_requeues_without_verify`（夹具
  无盘上文件 → 走 fallback → Queued）与 `retry_verify_failed_with_companion_reverifies…`
  （分支①）均保持绿。

### QA 会话须知（e2e 前必读）
- 套件同步清单（累计，不变）：六套件（见一·遗留受限项）先同步再跑 e2e。
- 本轮 e2e 直击点：QA-IC-12（清码 → R → **立即**完成「无校验」+ 大小不符变体）。
  v1.18 判据：R 后不经等待/下载过渡帧直接转「已完成」、fixture 断言无新请求（不探测
  不重传）、`.downloading` 已改名目标 + sidecar 已删；大小不符变体断言从头重下
  （sidecar 删、进度清零）。

### 待办与待批（未决项）
- 无新增裁决待批：本轮为 D34 钦定语义（「不重传直接收尾」）的实现层缺陷修复，mission
  v1.18 已落账（裁决项维持关闭）。
- fallback（文件不完整 → 作废断点从头重下）为 FR-01-22 作废语义同源的实现定义值，
  操作者可改判（如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，不建议）。

### 移交建议
- 下一角色：six-pack/cleaner（本轮改动面小：supervisor.rs 完成臂 + tasks.rs 分支③ +
  测试 3 条），之后 architect → hardender → QA。
