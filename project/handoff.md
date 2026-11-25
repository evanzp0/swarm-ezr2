# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/QA（最终独立验证 + 12-reload-config 可执行化轮）——已完成，QA 为链末角色**
> 路径：…six-pack/specifier → coder → cleaner → architect → (hardender) → **six-pack/QA（本轮：QA-RC-01–14 端到端可执行化 + CRAP/DRY 对账 + 最终验证序列全链，完成后 complete 归档）**
> 会话：six-pack-QA-reload-e2e（上游交接 = 上轮 handoff）
> 环境：nightly 1.102.0-nightly（739adecd4 2026-11-24，rust-toolchain.toml channel=nightly；rsproxy dist 前台装链，DIST_SERVER 指顶层域）+ cargo registry rsproxy sparse + PMD 7.29.0（.tools，gh-proxy.com 直链）+ llvm-tools-preview 随链（llvm-profdata/llvm-cov 直采管线）+ python 依赖一次装齐（pyte/wcwidth/libtorrent 2.1.3 wheel）。
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭——本轮无决策闸门，不受影响；②CRAP/CPD/>100 变异点存量三大沿袭受限项**全部原样沿袭**（见一节，数量与本轮实测恒等），非本轮新增；③nightly 为移动目标（1.102.0-nightly c7d5ba59d 2026-11-20 → 739adecd4 2026-11-24 漂移），全套件以实测全绿为准，行覆盖口径随 nightly 微版本漂移不可跨构建直比。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.61**）；phase-01/02 为现行详述（03/04 计划登记稿）。ezr 不做旧版本兼容沿袭（操作者钦定沿 v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；历轮已同构同步（v1.62）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径） |
| hardender 变异加固轮（20261125） | 307 变异点收敛 + 加固测试 +31 + 软 Gherkin + CRAP + CPD | 已交付 | 上轮账：bin 1165P/0F + 集成 8992P/0F + demo 61P/0F + 变异非等价存活 0 + CRAP 100 + CPD 222 |
| **QA 最终验证轮（20261125，本轮）** | **QA-RC-01–14 可执行化 `suite_reload_config.py`（14 用例）** + harness 扩展（env-unset / 跨套件代理助手 / 页签感知选择）+ **产品修复 ×1**（CLI --max-speed 启动种子化，main.rs 最小改动） | 已交付 | 见二节逐项：reload 套件 14/14 P + CRAP/CPD 基线恒等 + 最终验证序列全绿 |

### 遗留受限项（当前有效）
- **全仓 >100 变异点存量**：mouse.rs=223、textfield.rs=226、scrollbar.rs=223、multiline.rs=204、bt/engine/pieceio.rs=209、bt/peer_id.rs=181、ui/bt_dialog.rs=179、model/config.rs=161、bt/tracker.rs=159、app/dialog_keys/routing.rs=145、app/bt_tree.rs=111、app/dialogs.rs=101——历轮登记沿袭，本轮零触及（未跑变异、零产品改动面）。
- **CRAP 残余沿袭**：ACTIONABLE_COUNT=100（本轮实测**精确恒等** 100；双锚恒等 mouse.rs:178=77.09、trackpool.rs:413=186.96）；行覆盖口径随 nightly 微版本漂移不可跨构建直比（本轮未重采行覆盖）。
- **CPD 残留沿袭**：全仓 222 组（min-tokens 70，PMD 7.29.0；产品侧 18 组集中在历轮旧账文件）——本轮实测**精确恒等** 222 组零漂移。
- 上述三项均为历轮登记的存量口径，QA 轮未新增任何受限项；如需收敛由后续 architect 轮裁决。

### 实现定义值登记
- **本轮零新增实现定义值**：唯一产品改动 = main.rs CLI --max-speed 修复，属对既有规格 FR-01-60/FR-01-119⑥（CLI > config 贯穿）的实现归位，无新口径。
- 历轮登记（D81①–⑤/D82①–③ v1.57 落定版、运行端点判定口径、ApplyGlobals 命令面、CLI 覆盖登记、进度条行尾恒空）全部沿袭有效。

## 二、当前产出情况

### 本会话产物清单（QA 最终验证轮）
1. **`project/qa/runners/suite_reload_config.py`（新建，QA-RC-01–14 全量可执行化）**：14 用例 P/F/S 口径全绿（P=14 F=0 S=0，两轮全量复验）；PTY 端到端、UI 层唯一入口（Ctrl+R = \x12 全局分派）+ config.toml 运行期外部改写（无 IPC）+ fixture/代理/tracker 日志对账；成功 toast 恒定文案断言（v1.57/D81②）、失败臂逐键文案、原子拒绝、引用消失落库、浮层内热加载下拉重建、BT listen_port 启动绑定、无主目录臂（env 卸载 HOME/USERPROFILE/EZR_HOME）、帮助窗条目与滚动、CLI 覆盖贯穿热加载实测限速带宽断言。
2. **harness.py 扩展（QA 基建，最小增量）**：①EzrApp `env_extra` 支持 None = 子进程环境真卸载（QA-RC-12 无主目录启动；fork 后显式 pop，空串 ≠ var_os None）；②跨套件共享代理操作上提（proxies_toml / proxy_dropdown_options / add_task_via_proxy / select_task_by_needle（两页签回退）/ registry_tasks）；suite_named_proxy 本地同构副本保留，收敛归后续 cleaner 轮。
3. **产品修复 ×1（QA 职责内最小修复，复现先行）**：main.rs——`cli.max_speed` 在 `App::new`（引擎桶构造）**之前**种子化 `cfg.global_max_download_speed`，`set_cli_max_speed_override` 保留（热加载重应用口径不变）。修复前实测：CLI 256 KiB/s 启动后引擎桶仍按 config 1 MiB/s 运行（首次 reload 前覆盖不生效），违反 FR-01-60/TP-03 验收口径；修复后 RC-10 全绿。
4. **CRAP/DRY 对账（复现上轮口径并核对基线）**：CRAP 直采管线复现（插桩构建 → 直跑 bin 测试 250.9s 采 profraw → merge/export → 段级过滤 → crap_calc.py）→ ACTIONABLE_COUNT=100 精确恒等、双锚恒等；DRY（PMD 7.29.0 CPD min-tokens 70）→ 222 组精确恒等。本轮零新增问题（修复产品 bug 不涉复杂度/克隆面变化）。
5. **notes 沉淀**：`project/notes/testing.md`（reload 套件轮六坑：toast 单槽竞态/长周期×块周转双约束/disconnect 确定性/修改框时序/窄终端 toast 截断/announce 落账窗）+ `project/notes/environment.md`（rsproxy 独通与 DIST_SERVER 基址坑/nightly cargo 新布局/磁盘时序实例/PMD 与 libtorrent 装机预算）+ INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd 见行注） |
|---|---|---|
| **reload 套件全量（两轮复验）** | **14/14 P/0F/0S** | `python3 suite_reload_config.py`（cwd=project/qa/runners，QA_ONLY 可点跑） |
| arch_check | 19/19 全过 | `bash scripts/arch_check.sh` |
| fmt / clippy（ezr + demo） | 双项目 fmt --check 干净 + clippy --all-targets 零警告 | `cargo fmt --check` / `cargo clippy --all-targets`（demo 用 `+nightly`） |
| bin 全量（无插桩） | 1168P/0F（196.48s） | `cargo test --bin ezr` |
| 集成 13 套件 | 9016P/0F：g_appcore 930/g_slots 254/g_proxy 16/g_engine 927/g_supervisor 900/g_throttle_err 549/g_main 2/g_v147 914/hardening 262/engine_backfill 1050/ui_backfill 1080/v115 1073/mutation 1059 | `cargo test --test hardening_<名>`（串行前台，日志 .work/tmp/f-*.log） |
| demo | 61P/0F | `cargo test`（cwd=ezr-demo） |
| 属性独立 | 92P/0F（1076 filtered） | `cargo test --bin ezr property_tests::` |
| 加固套件独立入口 | 31P/0F（1137 filtered） | `cargo test --bin ezr -- -- hardening_restore:: stats_hardening:: overlay_hardening:: keys_hardening:: hardening_app_core::` |
| CRAP | ACTIONABLE_COUNT=100 恒等；mouse.rs:178=77.09 恒等；trackpool.rs:413=186.96 恒等 | 插桩管线复现（handoff 四要素②），报告 .work/tmp/crap-report.txt |
| CPD | 222 组恒等（min-tokens 70，EXIT=4 属正常） | `.tools/pmd-bin-7.29.0/bin/pmd cpd --dir src --language rust --minimum-tokens 70`，报告 .work/tmp/cpd-report.txt |

### 对账口径
- **bin 侧**：1165 → 1168（+3）。归因 = 仓库 HEAD 提交（`hardender:reload config fix`，上轮 handoff 写定后入库的收尾提交）自带 3 个 reload 相关测试——上轮账记 1165P 为该收尾提交前实测；本轮以 HEAD 现树实测为准。
- **集成侧**：8992 → 9016（+24 = +3 测试 × 8 个挂载 app 模块树的套件：g_appcore/g_engine/g_supervisor/g_v147/engine_backfill/ui_backfill/v115/mutation 各 +3；未挂载 5 套件 g_slots/g_proxy/g_throttle_err/g_main/hardening 精确恒等）——与 bin +3 完全自洽。
- **demo 侧**：61 恒等（独立子项目零接触）。
- **CRAP/CPD**：100 / 222 与上轮基线精确恒等（main.rs 修复不涉复杂度与克隆面）。
- **产品改动面**：仅 main.rs CLI 种子化（5 行级）；`git diff` 面见 `src/main.rs` 一处 + `project/qa/runners/*` + `project/notes/*` + 本文件。

### 待办与待批（未决项）
- **无待办、无待批**——QA 为 six-pack 链末角色，12-reload-config 沿袭欠账（QA-RC-01–14 可执行化）本轮清账，软变异词汇面 9 survived 的压缩路径（产品面 handler 可执行化）随套件全绿兑现。

### 移交建议
- **six-pack 流转路径（specifier → coder → cleaner → architect → hardender → QA → Done）已全部走完，工作流完结**；本轮 `bin/swarm complete` 归档即终点（宪法第一章不自动流转）。
- 如需新需求/新轮次：由操作者从 `six-pack/specifier` 重新开工（新需求建档/修订走 specifier 首角色职责）。
- 沿袭受限项（>100 变异点存量 / CRAP 100 / CPD 222）如需收敛，由后续 architect 轮裁决后按既有口径拆分。

## 交接四要素（mutation-hardening.md §6）

### ①本地验证命令清单（按序，cwd=project/ezr 除非另注）
1. `bash scripts/arch_check.sh` → 19/19
2. `cargo fmt --check` + `cargo clippy --all-targets`（ezr）；demo：`cargo +nightly fmt --check` + `cargo +nightly clippy --all-targets`（cwd=ezr-demo）
3. `cargo test --bin ezr` → 1168P/0F
4. 13 集成套件逐个 `cargo test --test hardening_<名>`（名清单见验证证据行）→ 合计 9016P/0F
5. `cargo test`（cwd=ezr-demo）→ 61P/0F
6. 属性独立：`cargo test --bin ezr property_tests::` → 92P/0F
7. 加固套件独立入口：`cargo test --bin ezr -- -- hardening_restore:: stats_hardening:: overlay_hardening:: keys_hardening:: hardening_app_core::` → 31P/0F
8. **reload 套件（本轮新增）**：`python3 suite_reload_config.py`（cwd=project/qa/runners）→ 14/14 P/0F/0S；`QA_ONLY=QA-RC-<n>` 点跑单用例
9. CRAP：`RUSTFLAGS="-C instrument-coverage" CARGO_TARGET_DIR=<abs>/.work/tmp/cov-target cargo test --bin ezr --no-run` → 直跑测试二进制（路径以构建日志 `Executable unittests` 行为准，nightly cargo 新布局无 deps/）→ `llvm-profdata merge -sparse` → `llvm-cov export <bin> --instr-profile --format=lcov` → `awk '/^SF:/{keep=index($0,"/project/ezr/src/")>0} keep'` → `python3 project/qa/runners/crap_calc.py <lcov> <src_root>`（两参）→ ACTIONABLE_COUNT=100
10. CPD：`.tools/pmd-bin-7.29.0/bin/pmd cpd --dir src --language rust --minimum-tokens 70`（EXIT=4=有克隆属正常）→ 222 组

### ②会话运行时产物位置与复现方法
- `.work/tmp/`（不进归档，complete 自带清理后可再生）：`f-*.log`（bin/集成/demo 落盘日志）、`crap-report.txt`、`cpd-report.txt`、`rc-full2.log`（reload 套件全量日志）、`ezr-src.lcov`（段级过滤后 lcov）；scratch/插桩 cov-target 已按磁盘时序纪律清理（复现见①9/①10 与 testing.md/environment.md 本轮条目）。
- reload 套件失败现场快照：`.work/tmp/qa-run/<case_id>.screen.txt`（Suite._snapshot 钩子；全绿时无快照）。
- 工具缓存 `.tools/pmd-bin-7.29.0/`（跨会话复用）；cargo-mutants/bb/APS 本轮未使用（QA 不跑变异），缺则按 environment.md 既有条目重建。

### ③feature 注释块与差分复用注意事项
- 本轮零 feature/规格/等价体变更：21 个 stamped feature 的 stamp/manifest、equivalence-filter（17 条目）与软 Gherkin 账目全部沿上轮原样有效；后续若再跑软 Gherkin 仍按上轮①⑨口径（经 /home/z/swarm-ezr2 符号链接路径调用——本轮克隆树恰在该路径，manifest feature_path 逐字一致）。
- mutation 侧账目（mutants-out*）沿上轮：--iterate 账目对六文件仍有效，测试再有非追加增删须整体重建（§2）。

### ④等价突变体清单及失效条件（回补触发）
- 沿上轮清单原样有效（12 个等价体 + timeout 击杀 1 + 伪幸存 37，见上轮 handoff ④表）；本轮未跑语言变异、未新增/删除任何加固测试，回补触发条件不变。

By QA.
