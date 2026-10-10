# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner 终态　会话：v125-clean（任务 v125-clean）
> 本轮（v1.18 清理）：对 coder v124-code-batch5 交付做保持行为清理与四度量（覆盖率/CRAP/
> DRY/变异点 scan）复核；全量 **2506P/0F**（Δ=0）、clippy 0、fmt 0、arch 12 规则过。
> 行为保持实证：全部清理为提取/改名级机械等价变换，测试集零增删。

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
| coder（v124-code-batch5） | 修订 A 终态 sidecar 即时落盘 + 修订 B 清码 retry 文件完整性直判 + 测试 3 条 | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过 |
| cleaner（v125-clean，本轮） | 保持行为清理 8 处 + 四度量口径修复与达标复核（见二） | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过、CPD tiles 58→55 |

### 遗留受限项（当前有效）
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e（见二）。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，近似口径偏差方向见 `packs/_common/notes/rust.md`）：
  存量 **72 条近似 CRAP>6** 全部为既有分发核心/UI 绘制/解析函数（on_evt、draw、block_worker、
  from_toml 等，comp≈分支数属「拆不动残余高 comp 点」登记口径），非本轮引入；本轮改动面
  函数经源码复核 comp 全部 ≤6。
- **CPD 剩余 55 tiles**（PMD 7.28，-l rust --minimum-tokens 50）：18 个 supervisor 测试区
  mock 样板 + 37 个分发臂/UI 绘制/既有夹具形态，均为历轮登记既有项；本轮消解 3 处
  （66tok/52tok/127tok）后无新增。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 本轮无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾，
  v1.18 实现精化见 coder v124-code-batch5 轮）。修订 B fallback（文件不完整 → 作废断点
  从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（cleaner v125-clean）

### 本会话产物清单
- `src/app/tasks.rs`：`requeue_failed` 保持行为拆分——verify_fail 臂三分支持续化收口为
  `requeue_verify_failed`（①②③分派）/ `start_reverify`（①②校验指令）/ `direct_finalize_or_discard`
  （③直判/fallback）三个私有助手；`label` 提升至函数顶部单次计算（原同函数 2 处 4 行重复）；
  消除 `name` 冗余重克隆 2 处（原值仍存活的失效 shadow）。
- `src/engine/supervisor.rs`：三处同构收口为共用助手——`flush_sidecar()`（修订 A 终态落盘臂
  与周期落盘臂的 lock→build→save 4 行同构）、`failed_flow()`（监督循环失败臂与大小校验
  失败臂的 blocks 快照→sidecar→Flow::Failed 尾段，PMD 66tok）、`block_span()`（block_worker
  重领/新领两臂的块区间+已写量视图，PMD 52tok）。
- `src/app/testutil.rs` + `src/app/dialog_keys/mod.rs` + `src/app/dialogs.rs`：对话测试的
  sidecar 预置夹具收敛为 `testutil::preset_resume_sidecar()` 单一构造点（PMD 127tok，
  25 行 × 2 份 → 各 1 行调用）。
- `packs/_common/engineering.md` / `packs/_common/notes/rust.md`：本轮沉淀 3 条可复用经验
  （复合命令尾随 `&` 后台化陷阱；cargo-llvm-cov 导出缺挂载上下文与手动 export 合并口径；
  lcov 具名函数跨距归属口径）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单测 | 2506P / 0F（13 目标，Δ=0） | `cargo test --workspace` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| fmt | 0 偏差 | `cargo fmt --check` |
| 架构边界 | 12 规则全过 | `bash scripts/arch_check.sh` |
| 覆盖率 | 改动面 cov：requeue_failed 1.00 / start_reverify 1.00 / requeue_verify_failed 1.00 / direct_finalize_or_discard 0.97 / flush_sidecar 覆盖在位；无缺口、未补测 | llvm-cov 导出口径见 `packs/_common/notes/rust.md`（全部 20 个测试二进制作 object 直导 + normpath 合并） |
| CRAP（受限近似） | 改动面源码 comp 全 ≤6：requeue_failed=5、requeue_verify_failed=3、start_reverify=1、direct_finalize_or_discard=5（BRDA 11 为 \|\| 链上界假阳性，未硬拆）、flush_sidecar=2 | `.work/tmp/crap_v4.py`（会话级脚本，口径注记在脚本头） |
| DRY | PMD CPD tiles 58→55（消解 3 处，均验证回归） | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src` |
| 变异点 scan | supervisor.rs=86、tasks.rs=30，均 <100 门槛（未跑变异测试，scan/count 模式） | `cargo mutants --list --line-col=true` |

### 对账口径（基线 → 终态）
- 基线 2506P/0F（coder v124-code-batch5 终态，本轮开工全新环境实测复现一致）→ 终态
  **2506P/0F**，**Δ = 0**：本轮纯保持行为重构，测试集零增删（SKILL「测试补齐为纯追加」
  本轮无追加需求，覆盖率复核未暴露改动面缺口）。
- 行为保持的机械等价性依据：全部改动为函数提取/调用替换/重复字面量收敛，无控制流、
  无状态写序、无 toast/持久化语义变化；2506 测试全绿 + clippy/pedantic 0 为回归实证。

### QA 会话须知（e2e 前必读）
- 套件同步清单（累计，不变）：六套件（见一·遗留受限项）先同步再跑 e2e。
- 本轮 e2e 直击点不变（QA-IC-12 清码 → R → 立即完成「无校验」+ 大小不符变体）；本轮
  清理不改变任何可观测行为，套件断言无需调整。

### 待办与待批（未决项）
- 无新增裁决待批。沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为
  实现定义值，操作者可改判（如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，
  不建议）。

### 移交建议
- 下一角色：six-pack/architect（清理批次已收口：模块边界未动、依赖方向 12 规则全过；
  architect 可在现基线上做架构评审与属性测试覆盖盘点，再交 hardender 变异加固）。
