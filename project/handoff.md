# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（**mktorrent bencode 合规修订轮**——操作者显式授权 hardender 越界修改产品源码，方案 A 落地结清历史受限项）
> 路径：…hardender 过滤器扩展轮 ⇒ **hardender 合规修订轮（本轮，操作者 chat 两裁决：①方案 A 授权开轮；②engineering.md 授权越界例外条款批准）**
> 会话：six-pack-hardender-20261118-bencode（SESSION_TASK = mktorrent bencode 方案 A + 授权越界例外条款增补；上游交接 = 上轮 handoff + 操作者 chat 裁决）
> 环境：rustup stable 1.100.0 + nightly 1.102.0（rsproxy `/dist/` 清单双变量前台一次装齐，minimal + rustfmt/clippy/llvm-tools-preview）、bb 1.13.229（.tools/bin/bb 沿位）、APS 仓扩展沿 patch 流转（qa/aps-mutator-equivalence-extension.patch）；PMD 本会话不在位（cpd 证据以 rg 唯一定义点实证替代，登记受限项）。
> 受限项：CRAP 直采管线全链已复现（ACT=101 与基线精确相等）；**verdict 级软 Gherkin 全量重跑仍待 APS runtime 重建**（沿上轮，机制与数据验证已闭环）；软 Gherkin 历轮总账 1124 = 1107 killed + 17 等价类 + 0 errors 沿 r3 口径有效。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA 轮 | 五规程缺口可执行化 + harness 退出礼仪修复 + 产品一处 bug 修复 | 已交付 | 06 = 8P/0F；07 = 11P/1S；11 = 6P/0F；09 = 17P/7S；10 = 1P/7S；产品单测 1090P/0F、fmt/clippy clean |
| specifier 裁决落实轮 | 规格分歧两处裁决落地 | 已交付 | rg 残留清零；三层口径对齐；产品零改动 |
| hardender 过滤器扩展轮 | mutator `--equivalence-filter`（授权越界）+ 17 格登记 + 补丁落盘 | 已交付 | bb test 11 测/51 assertions 全绿；探针四对照 + 复用对照全过；17/17 精确命中；产品源码零改动 |
| **hardender 合规修订轮（本轮）** | **mktorrent bencode 方案 A（自包含化 + #[path] 单源，授权越界）+ engineering.md 授权越界例外条款** | 已交付 | 验证门全绿：fmt clean / clippy 0 警告 / arch_check 18 规则全绿 / 产品基线 1090P/0F 精确复现 / 工具 bin 54P / CRAP ACT=101 与基线相等 / 232-token 克隆按构造清零 |

### 待办承接（历轮待办 → 本轮处置）
- **mutator 等价突变过滤器扩展**：✅ 上轮结清（bb 版 `--equivalence-filter` + 17 格过滤器登记 + 补丁落盘）。
- **规格分歧登记两处**：✅ 已裁决落地结案。
- **制度性修订建议（engineering.md 授权越界例外条款）**：✅ **本轮操作者批准并落地结清**——`packs/_common/engineering.md`「上游钉死路径的兼容桥接」条款后新增「授权越界轮的例外条款（操作者 20261118 裁决增补）」，批准文本原文落地；同类授权越界轮可直接引用本条。
- **mktorrent bencode（232-token 跨 bin 克隆）**：✅ **本轮彻底解决（方案 A）**——bencode.rs 自包含化（decode 族返回模块局部 `BencodeError`，移除 `use super::BtError`，消息串逐条保留）→ `bt/mod.rs` From 升格（产品调用点零改动）→ `ezr-mktorrent.rs` 退役内嵌 47 行克隆、改 `#[path = "../bt/bencode.rs"]` 硬包含。构建图零变更（不 lib 化）。遗留待办**结清**。
- **历轮受限项（缺口账目 ID / trackpool.rs:644 等价论证 / 2 条 unviable / RB-09+UC-10）**：沿历轮登记不变（mktorrent bencode 自本行移除——已结清）。

### 遗留受限项（当前有效）
- **APS 执行层 runtime 未重建（沿上轮登记）**：`.work/tmp/aps/` 四件套 + 59 handler + column_schema.json 需重建后才能跑 verdict 级软 Gherkin 全量。**回补触发 = 操作者开 runtime 重建轮**（重建口径见 notes/testing.md 修订轮条目）。重建后 `--level full --equivalence-filter` 直跑即得 verdict 级账目（预期 = 1107 killed + 17 filtered_equivalents + 0 survived + 0 errors）。
- **Go 版 mutator 未扩展（沿上轮）**：`--equivalence-filter` 仅 bb 版实现；回退 Go 备用路径时过滤不生效、等价体如实回现为幸存（安全失败模式）。
- **PMD cpd 本会话不在位（本轮新增登记）**：232-token 克隆已按构造消失（rg：`pub enum BValue` ×1、`fn encode_into` ×1 唯一定义点）；下次 PMD 可用轮对 ezr 仓复扫确认零发现即可回补销项。
- **缺口账目 ID 不可精确复现 / trackpool.rs:644 等价论证 / 2 条 unviable / RB-09+UC-10**：沿历轮登记不变（本轮改动文件与上述登记文件不同源，行号账目不受影响）。
- **07-add-bt.feature 场景哈希滞后**：沿上轮登记（下次 gherkin 变异运行自刷新）。

## 二、当前产出情况

### 本会话产物清单（hardender 合规修订轮）
1. **方案 A 代码改动**（操作者授权越界，产品树 2 文件 + 工具 bin 1 文件 + 测试桩 1 文件）：
   - `project/ezr/src/bt/bencode.rs`：新增模块局部 `BencodeError`（pub String 新型 + Display/Error），decode 族（decode/decode_all/decode_with_info_span/decode_inner_at/decode_inner/find_dict_value_span）签名改返回 `Result<_, BencodeError>`，移除 `use super::BtError` —— 文件零外部依赖自包含；encode 半区零改动，11 处错误消息串逐字保留；
   - `project/ezr/src/bt/mod.rs`：新增 `impl From<bencode::BencodeError> for BtError`（消息串直通 `BtError::Bencode`，Display 输出字节级不变）；
   - `project/ezr/src/bin/ezr-mktorrent.rs`：退役内嵌 47 行 `mod bencode` 克隆，改 `#[path = "../bt/bencode.rs"]` + 模块作用域姿态对齐（`#[allow(dead_code, clippy::pedantic, clippy::nursery)]`——被含文件按 ezr crate 姿态编写，豁免不外溢）；
   - `project/ezr/tests/hardening/bt.rs`：测试局部门面桩补 From 镜像（与产品门面同步，修 E0277——桩镜像纪律）。
2. **`packs/_common/engineering.md` 授权越界例外条款**（操作者批准文本）：「APS mutator 项目等价突变过滤器扩展（bb 版）经操作者授权已落地，扩展以 patch 形态随工作区流转（project/qa/aps-mutator-equivalence-extension.patch），重建工具链时先 clone 后 apply；同类经操作者显式授权的越界修改轮可直接引用本条」。
3. **经验沉淀**：`project/notes/architecture.md` 3 条（自包含化合规解、「自包含」判据实操化、#[path] 共享三同步义务）+ `project/notes/testing.md` 2 条（错误类型重构行为等价论证法、CRAP 管线全链复现）+ `project/notes/environment.md` 3 条（镜像漂移实录、crap_calc 两参签名、persistent shell RUSTFLAGS 残留）+ INDEX 同步。
4. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 格式 | clean | `cargo fmt --check`（ezr crate 根） |
| 架构边界 | 18 条规则全绿 | `bash scripts/arch_check.sh` |
| clippy | 0 警告 0 错误（含全部 bin 与 12 集成目标编译） | `cargo clippy --bin ezr --all-targets` |
| 产品基线 | **1090P/0F**（沿 QA 轮基线精确复现，189s） | `cargo test --bin ezr` |
| 工具 bin | mktorrent **9P**（1 形态契约 + 8 bencode 单测随 `#[path]` 并入；工具 bin 口径 46→54）/ fixture 30P / proxy 15P | `cargo test --bin ezr-mktorrent` / `--bin ezr-fixture` / `--bin ezr-proxy` |
| bt 门面桩集成目标 | hardening 247P/0F、hardening_g_slots 254P/0F（桩 From 镜像实跑验证） | `cargo test --test hardening` / `--test hardening_g_slots` |
| CRAP 直采管线 | 行覆盖 39853/42121 = **94.6%**；**ACT=101 与基线精确相等**（零新增可修点）；稳定函数锚 mouse.rs:178=77.09 恒等；bencode 2 条 CRAP>6 条目为既有函数换名漂移（编入既有 101 口径） | 插桩 `RUSTFLAGS="-C instrument-coverage" cargo test --bin ezr --no-run` → 直跑测试二进制（`LLVM_PROFILE_FILE=<dir>/%p-%m.profraw`）→ `llvm-profdata merge -sparse` → `llvm-cov export --format=lcov` + awk 段级过滤 → `python3 qa/runners/crap_calc.py <lcov> <src_root>`（**两参**） |
| 单源证据 | `pub enum BValue` ×1、`fn encode_into` ×1 唯一定义点（克隆按构造消失） | `rg -c "pub enum BValue" src/`；`rg -c "fn encode_into" src/` |
| 行为等价论证 | 错误消息串逐条保留（Display 字节级不变）；消费点三类零改动：`?` 经 From 自动转换（metainfo.rs:26/35、tracker.rs:672）、`.ok()?` 类型擦除（peer.rs:317/361）、`.is_ok()` 类型无关（metainfo.rs:253）；测试断言面零触碰 | `git diff src/bt/ src/bin/ezr-mktorrent.rs tests/hardening/bt.rs` |

### 对账口径
- **产品基线**：1090P/0F = QA 轮基线精确复现——本轮为「零逻辑变化」改动（错误类型管道 + 编码器单源化），基线复现即等价实证。
- **CRAP 行覆盖 DA 总量漂移**（42121 vs 历轮 43932）：nightly 1.101→1.102 插桩行数漂移 + webrtc 覆盖非确定（历轮登记同源）；按「方向非回归（93.7%→94.6%）+ ACT 相等（101=101）+ 稳定锚恒等」三证对账，不采信绝对行数比对。
- **变异账目不触发重跑**：本轮改动文件（bencode.rs/bt mod.rs/ezr-mktorrent.rs/hardening/bt.rs）不在变异 scope 清单（pieceio/peerwire/trackpool 339）与 gherkin feature 域（17 格过滤登记）；trackpool.rs:644 等行号登记账目与改动文件不同源，全部沿历轮有效。
- **工作区零残留**：插桩采集的 profraw 散落已清理（`unset RUSTFLAGS` 纪律）；`.work/tmp/crap/` 报告产物会话级。

### 待办与待批
- **本会话无待批无待办**——操作者两裁决（方案 A 开轮 + engineering.md 条款）均已落地结清。

### 交接四要素（hardender 要素）
1. **本地验证命令清单（按序）**：`cargo fmt --check` → `bash scripts/arch_check.sh` → `cargo clippy --bin ezr --all-targets` → `cargo test --bin ezr`（基线 1090P）→ `cargo test --bin ezr-mktorrent / --bin ezr-fixture / --bin ezr-proxy` → `cargo test --test hardening / --test hardening_g_slots` → CRAP 直采管线（上表命令；llvm-tools 已随 nightly 一次装齐）→ 单源 rg 两命令。
2. **会话运行时产物位置与复现**：`.work/tmp/crap/`（profraw/ezr.profdata/ezr.lcov/crap-report.txt）——会话级 complete 即清；复现 = 四要素 1 的 CRAP 命令链（约 3 个前台分块：插桩构建 2m27s / 插桩全量跑 238s / merge-export-calc）。
3. **feature 注释块与差分复用注意事项**：沿上轮（mutator `--equivalence-filter` 语义不变，17 格登记 = `project/qa/equivalence-filter.json`；强制全量重跑 = `--level full`；过滤器变更 → 哈希变 → 如实重执行）。**本轮真实 features 零触碰**，manifest 写回面无变化。
4. **等价突变体清单及失效条件**：沿历轮不变——17 格权威登记 `project/qa/equivalence-filter.json`；trackpool.rs:644 `delete !`（失效 = webrtc 栈可用测试环境）、2 条工具判定 unviable、trackpool.rs:412 CRAP/ACT 名义值竞速登记——本轮改动文件与上述登记不同源，行号与失效条件均不受影响。

### 移交建议
- **six-pack 流转路径保持完结态**：hardender 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 如需续做：①APS runtime 重建轮（verdict 级全量确认，预期 1107 killed + 17 filtered_equivalents + 0 errors）；②五规程 handler 可执行化（07/09/10/11）；③PMD 可用轮 cpd 复扫销项——均由操作者显式决定。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By hardender.
