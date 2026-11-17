# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（待批③④执行轮——缺口 112 回补测试 + 软 Gherkin 执行层基建）
> 路径：…coder demo 结算轮（20261116h）⇒ **hardender 待批③④轮（本轮，操作者 chat 显式指令「用 six-pack/hardender 完成 handoff.md 中的 ③ 缺口 112 回补测试 ④ 软 Gherkin 执行层基建」）**
> 会话：six-pack-hardender-20261117（SESSION_TASK = gap-112-backfill-and-soft-gherkin-infra；上游交接 = 上轮 handoff + 操作者 chat 待批③④批准指令）
> 环境：rustup/nightly 全新安装（TUNA）、cargo-mutants 27.1.0、bb v1.13.229 + APS 主仓、PMD 7.29.0、llvm-tools（直采管线）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.56**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（`ezr-demo/`，工作区外），41 轮变更记录；自带 PTY 级 verify 脚本。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| coder demo 结算轮（20261116h） | PTY Alt+Enter 受限项定性收口 + notes 补记 | 已交付 | 无代码改动；定性更新 |
| **hardender 待批③轮（本轮）** | 缺口回补测试 37 条（src/bt/engine/mod.rs 新增 `#[cfg(test)] mod gap112_backfill_tests`）+ 变异杀灭账目 + pristine 零改动证明 | 已交付 | 覆盖函数 140 突变体 = 137 杀灭 + 1 等价 + 2 unviable；bin 1089P、backfill 975P、supervisor 828P；fmt/clippy 零警告 |
| **hardender 待批④轮（本轮）** | 软 Gherkin 执行层基建四件套（runtime/generator/adapter/drive，`.work/tmp/aps/`）+ 20 feature soft 级全量执行 + 产品冒烟 3/3 | 已交付 | 1121 突变体 = 903 killed + 218 survived（执行数对账自洽）；冒烟 P=3 F=0（真实 ezr 二进制 PTY 执行） |

### 遗留受限项（当前有效）
- **缺口账目 ID 不可精确复现**：历轮变异会话运行时状态（adaptive_resolved.csv 等）为会话级产物、未随归档保留，「缺口 112」（79 ALIVE_BS + 7 btmod + 26 trackpool ws 的重建口径）的逐条 ID 已不可考——本轮以「同 scope 全量重建」替代：cargo-mutants --list 实测 339 突变体（pieceio 209/peerwire 78/trackpool 52，对历轮 308 为口径漂移，以实测为准），覆盖函数面 140 突变体全部给出新账，未覆盖函数面 199 突变体沿历轮登记（本轮纯测试增量不改其杀灭状态）。
- **软 Gherkin 词汇面幸存 218**：突变后示例值仍通过当前校验器集（数值 ±δ/布尔翻转/自由文本 dither 在规格语义内合法）——压缩途径 = 按场景域逐步骤实现产品面 handler（回补触发，见移交建议）。
- **ws_process_response:644 `delete !` 等价论证（面内）**：webrtc=false 时 ws_handle_answer 对未知 offer_id 早退、无可观察差异；失效条件 = webrtc 栈可用的测试环境（pre-insert PendingOffer + 假 sdp answer 走 mark_peer_fail 臂即杀灭）。
- **2 条 unviable（工具判定）**：pieceio:404 `replace count_invalid_request → Option<BanKind> with Some(Default::default())`（BanKind 无 Default 编译错）、trackpool:675 `delete match arm Ok(Ok(s)) in spawn_outgoing_peer`；同点位行为已由握手测试断言间接覆盖。
- **peer_connection_protocol 改判（已结案）**、**QA runner 族五规程缺口**、**CRAP/DRY 登记**、**mktorrent bencode 跨 bin 重复**——沿历轮登记不变。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（hardender 待批③④轮）
1. **src/bt/engine/mod.rs**：新增 `#[cfg(test)] mod gap112_backfill_tests`（37 测，gap112_ 前缀一测一靶；产品源码零改动——diff = 1618 行纯测试模块追加，pieceio/peerwire/trackpool 逐文件 cmp 与 pristine 字节恒等）。
2. **`.work/tmp/aps/`（会话运行时产物，不入归档）**：runtime.py（IR 展开 + 步骤词汇面校验）、generator.py（acceptance-entrypoint-generator + metadata）、adapter.py（NDJSON 持久 worker）、drive.py（20 feature 全量驱动）、smoke.py（产品面冒烟）。
3. **project/features/*.feature**：gherkin-mutator 自管理 stamp/manifest 注释块（工具写入，差分复用依据，见交接要素③）。
4. **project/notes/{testing,environment}.md**：本轮经验沉淀（挂载法/契约/坑/直采管线），INDEX 沿既有条目。
5. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单测（含 37 条回补） | 1089P / 0F | `cd project/ezr && cargo test --bin ezr` |
| BS 双套件挂载对账 | 975P（938+37）、828P（791+37） | `cargo test --test hardening_engine_backfill` / `--test hardening_g_supervisor` |
| bt::engine 面 | 110P / 0F（51.6s） | `cargo test --bin ezr -- bt::engine` |
| 变异杀灭（覆盖函数面） | 137 caught / 1 等价 / 2 unviable（140 突变体） | `.work/tmp/mutation/gap_face.sh <预算> "<函数正则>" <tag>`（复现法见要素②） |
| 软 Gherkin 全量 | 1121 突变体 = 903 killed + 218 survived | `python3 .work/tmp/aps/drive.py` |
| 产品冒烟 | P=3 F=0（01-add-task-12 × SHA-256/MD5/Adler-32） | `python3 .work/tmp/aps/smoke.py` |
| CRAP（bin 单目标直采） | ACT=101（基线 103，方向向下非回归）；行覆盖 41151/43932 | `python3 project/qa/runners/crap_calc.py .work/tmp/cov/ezr_src.lcov project/ezr` |
| DRY（PMD CPD 阈值 70） | 总 220 组 = 产品侧 23（与基线 23 恒等）+ test-only 197 | `.tools/pmd-bin-7.29.0/bin/pmd cpd --dir src --language rust --minimum-tokens 70` |
| fmt / clippy --all-targets | clean / 0 warnings | `cargo fmt --check` / `cargo clippy --bin ezr --all-targets` |

### 对账口径
- **挂载对账三方精确**：bin 1052→1089（+37）、backfill 938→975（+37）、supervisor 791→828（+37）；全量 bin 基线实测 1052 对 handoff 口径 1040 偏差 +12（历轮增量，以实测为准登记）。
- **③ 账目闭合**：scope 339 = 覆盖函数面 140（137+1+2，逐函数实测）+ 未覆盖函数面 199（沿历轮）；「缺口 112」重建口径 = 79+7+26 算术复原，ID 级不可考（受限项如实登记）。
- **④ 对账**：突变体总数 1121（历轮登记 1123 口径漂移 -2，以本轮实测为准）；执行数/verdict 交叉核验通过（修复 examples 平铺形态展开 bug 后全量执行数 = 突变体总数）。
- **产品改动面 = 零**（仅测试模块追加）；**CRAP/DRY 产品侧与基线一致**（ACT 101≤103、产品侧克隆组 23=23）。

### 待办与待批
- **（沿历轮）QA runner 族五规程缺口**：五规程暂无可执行套件。
- **（本轮登记）词汇面幸存 218 的产品面 handler 回补**：按场景域分轮实现（01-tui-display/06-cmd-window/04-ui-conns 幸存密度最高），每域需 PTY 级 handler 编写——属 QA 套件职责邻接面，建议操作者裁决归属（QA 或继续 hardender 小轮）。
- **（沿历轮）mktorrent bencode 跨 bin 重复**：产品侧克隆组 23 中含 ezr-mktorrent/bencode 跨文件组——沿历轮登记不变。

### 移交建议
- **six-pack 流转路径保持完结态**：hardender 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 如需续做：词汇面 handler 回补（按 feature 域分轮）、五规程可执行化均为独立小轮，由操作者显式决定。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

### 交接四要素（hardender 要素）
1. **本地验证命令清单（按序）**：`cargo fmt --check` → `cargo clippy --bin ezr --all-targets` → `cargo test --bin ezr` → `cargo test --test hardening_engine_backfill` → `cargo test --test hardening_g_supervisor` → `cargo test --bin ezr -- bt::engine` → 变异抽检（要素② runner）→ `python3 .work/tmp/aps/drive.py`（软 Gherkin）→ `python3 .work/tmp/aps/smoke.py`（产品冒烟）→ CRAP/DRY（命令见验证证据表）。
2. **会话运行时产物位置与复现**：变异 runner `.work/tmp/mutation/gap_face.sh`（用法 `gap_face.sh <预算秒> "<-F 函数正则>" <tag>`；scratch 副本 `.work/tmp/mut-scratch/ezr`（已删 tests/ 防 OOM）+ pristine `.work/tmp/pristine/`；cargo-mutants 输出 `.work/tmp/mutants-out/`）；软 Gherkin 四件套 `.work/tmp/aps/`（IR `.work/tmp/aps/ir/`、生成物 `.work/tmp/aps/mut/generated/`、逐 feature 报告 `.work/tmp/aps/mut/report_*.json`）；CRAP 管线 `.work/tmp/cov/`（ezr_src.lcov 已过 SF 前缀过滤）。全部为会话级，complete 即清；重建 = 按上述命令重跑（llvm-tools 若缺失按 notes/environment.md 重装工具链）。
3. **feature 注释块与差分复用**：gherkin-mutator 已把 stamp/manifest 注释块写入 20 个 feature 头部（工具自管理）；内容未变时二轮跑 = skipped_mutations（复用首轮判定）；**强制全量重跑 = 删除 feature 头部 stamp/manifest 注释块**；metadata 的 feature_path 必须与 `--feature` 实参一致（drive.py 已按 feature 路径 slug 重写 metadata）。
4. **等价突变体清单及失效条件**：trackpool.rs:644:8 `delete ! in ws_process_response`——失效条件 = webrtc 栈可用的测试环境（届时构造 PendingOffer + 假 sdp answer 断言 mark_peer_fail 臂即杀灭）；另有 2 条工具判定 unviable（pieceio.rs:404 编译错、trackpool.rs:675 删臂），非等价类，随工具重跑自然复判。
