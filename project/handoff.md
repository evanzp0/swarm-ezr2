# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（**PMD 销项轻量轮**——操作者 chat 显式指令：装 PMD + cpd 复扫销项 + 顺手补证 spawn_outgoing unviable）
> 路径：…hardender 合规修订轮 ⇒ **hardender PMD 销项轻量轮（本轮，操作者 chat 指令，产品源码零改动）**
> 会话：six-pack-hardender-20261118-pmdcpd（SESSION_TASK = PMD 7.29.0 装机 + cpd 双扫描销项对账 + spawn_outgoing unviable 人工补证；上游交接 = 上轮 handoff + 操作者 chat 指令）
> 环境：rustup nightly 1.102.0（rust-toolchain.toml 钉版）、**PMD 7.29.0 本轮装机在位**（~/.tools-pmd；ghfast.top 直链 7.5MB/s + gh-proxy 首段断点续传装齐，zip 校验/chmod 启动器/--version 三步独立验证）；cargo-mutants 仍不在位、APS runtime 未重建（沿登记）。
> 受限项：**cpd 复扫销项本轮结清**（受限项行移除，见待办承接）；**verdict 级软 Gherkin 全量重跑仍待 APS runtime 重建**（沿上轮，机制与数据验证已闭环）；软 Gherkin 历轮总账 1124 = 1107 killed + 17 等价类 + 0 errors 沿 r3 口径有效。

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
| hardender 合规修订轮 | mktorrent bencode 方案 A（自包含化 + #[path] 单源，授权越界）+ engineering.md 授权越界例外条款 | 已交付 | 验证门全绿：fmt clean / clippy 0 警告 / arch_check 18 规则全绿 / 产品基线 1090P/0F 精确复现 / 工具 bin 54P / CRAP ACT=101 与基线相等 / 232-token 克隆按构造清零 |
| **hardender PMD 销项轻量轮（本轮）** | **PMD 7.29.0 装机 + cpd 双扫描销项对账 + spawn_outgoing unviable 人工补证** | 已交付 | cpd 前态 220 组与历轮基线精确恒等 / 后态 219 组、差集 = 唯一被删 232-token 克隆组、其余字节级恒等 / unviable 人工实证 E0599 / 产品源码零改动（M 集合与轮前恒等） |

### 待办承接（历轮待办 → 本轮处置）
- **mutator 等价突变过滤器扩展**：✅ 上轮结清（bb 版 `--equivalence-filter` + 17 格过滤器登记 + 补丁落盘）。
- **规格分歧登记两处**：✅ 已裁决落地结案。
- **制度性修订建议（engineering.md 授权越界例外条款）**：✅ 上轮操作者批准并落地结清。
- **mktorrent bencode（232-token 跨 bin 克隆）**：✅ 上轮方案 A 按构造消除；**本轮 cpd 双扫描复扫零残余销项结清**（220→219 差集全额归因）。
- **PMD cpd 本会话不在位**：✅ **本轮结清**——PMD 7.29.0 装机 + 双扫描对账复扫，目标克隆零发现（详见本轮产物 2），受限项移除。
- **历轮受限项（缺口账目 ID / trackpool.rs:644 等价论证 / 2 条 unviable / RB-09+UC-10）**：沿历轮登记不变（其中 spawn_outgoing 删臂 unviable 本轮升格人工实证终态，见遗留受限项；本轮改动仅文档，行号账目不受影响）。

### 遗留受限项（当前有效）
- **APS 执行层 runtime 未重建（沿上轮登记）**：`.work/tmp/aps/` 四件套 + 59 handler + column_schema.json 需重建后才能跑 verdict 级软 Gherkin 全量。**回补触发 = 操作者开 runtime 重建轮**（重建口径见 notes/testing.md 修订轮条目）。重建后 `--level full --equivalence-filter` 直跑即得 verdict 级账目（预期 = 1107 killed + 17 filtered_equivalents + 0 survived + 0 errors）。
- **Go 版 mutator 未扩展（沿上轮）**：`--equivalence-filter` 仅 bb 版实现；回退 Go 备用路径时过滤不生效、等价体如实回现为幸存（安全失败模式）。
- **缺口账目 ID 不可精确复现 / trackpool.rs:644 等价论证 / 2 条 unviable / RB-09+UC-10**：沿历轮登记不变——本轮状态更新：**spawn_outgoing 删臂 unviable 已人工实证为编译错类终态**（删 `Ok(Ok(s))` 臂 → E0599 于 trackpool.rs:687 `sock.set_nodelay`（`!` 类型方法解析失败），工具判定经 scratch 人工复现确认正确）；count_invalid_request→Some(Default) 编译错类沿工具判定。644 失效条件（webrtc 栈可用测试环境）、缺口 ID 再生条件（cargo-mutants 27.1 装机 + 339 scope 重跑）均不变。本轮改动仅 notes/handoff 文档，产品源码与变异 scope 清单零触碰。
- **07-add-bt.feature 场景哈希滞后**：沿上轮登记（下次 gherkin 变异运行自刷新）。

## 二、当前产出情况

### 本会话产物清单（hardender PMD 销项轻量轮）
1. **PMD 7.29.0 装机**（会话外工具缓存 ~/.tools-pmd，不入工作区产物）：gh-proxy 首段 63KB/s 断点续传 33.8MB → 10s 速测择优 ghfast.top 7.5MB/s 续传余量 → 134.6MB zip testzip 校验通过（127 entries 无损）→ 解包 → chmod +x 启动器（zip 权限丢失教训沿历轮）→ `pmd --version` 独立验证（PMD 7.29.0 47b3135b2）。镜像面实证：aliyun maven / repo1.maven.org 对 pmd-dist bin.zip 均 404（maven 元数据同步但二进制不入镜像），GitHub 代理前缀为唯一装机路径。
2. **cpd 双扫描销项对账**（`pmd cpd -d src --language rust --minimum-tokens 70`，PMD 7.29.0，目录参数 `-d` 与历轮参数漂移记录吻合）：
   - 前态（`git archive HEAD` 提取 = 方案 A 前源码）：**220 组，与历轮基线（environment.md 11-17 扫描）精确恒等**；
   - 后态（工作树 = 方案 A 后）：**219 组**；bencode.rs / ezr-mktorrent.rs 作为克隆位点**零涉及**（全文仅 2 处克隆组代码体片段含 `use crate::bt::bencode` 语句，非克隆位点）；
   - 路径规范化 diff（sed 归一 `project/ezr/` 前缀）：**差集 = 唯一组 35 行 / 232 tokens（src/bin/ezr-mktorrent.rs ↔ src/bt/bencode.rs，encode/encode_into 函数体）**，其余 219 组头部/行号/片段字节级恒等、零新增组；
   - 结论：232-token 克隆账目零残余且零漂移，mktorrent bencode 受限项**销项结清**（比 rg 唯一定义点单证强一档：双扫描同时证"零残余"与"零漂移"）。
3. **spawn_outgoing_peer unviable 人工补证**（scratch 副本 `.work/tmp/unviable-probe/ezr`，真实树零接触）：
   - 靶点基线先行：unmutated `cargo test --bin ezr gap112_trackpool_spawn_outgoing_peer` = 2P/0F（live_endpoint_no_immediate_close + completes_handshake_connected，同语境）；
   - 删 `Ok(Ok(s)) => s,` 臂（trackpool.rs:675）→ `cargo check --bin ezr` 实测 **E0599**：`no method named 'set_nodelay' found for type '!'`（trackpool.rs:687）——机制 = match 仅剩 `_` 兜底臂全臂发散 → 表达式类型推断 `!` → 报错落在远离变异位点的方法调用处（非 E0004 非穷尽）；
   - 结论：cargo-mutants 的 unviable 判定**人工确认正确**（编译错类），账目从「工具判定」升格「人工实证终态」，无需测试段（编译不过则变异体不存在，杀灭语义无定义面）。
4. **经验沉淀**：`project/notes/testing.md` 2 条（cpd 销项双扫描对账法、unviable 人工补证法/全臂发散型编译错）+ `project/notes/environment.md` 2 条（大文件代理前缀 10s 速测择优、PMD 发行 zip 分发面实证）+ 本 handoff.md 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| PMD 装机 | 7.29.0 在位，zip 无损 | `python3 -c "import zipfile; z=zipfile.ZipFile(...); z.testzip()"` + `pmd --version` |
| cpd 前态恒等 | **220 组 = 历轮基线** | `git archive HEAD project/ezr/src \| tar -x -C .work/tmp/cpd/pre && cd .work/tmp/cpd/pre/project/ezr && pmd cpd -d src --language rust --minimum-tokens 70` |
| cpd 后态销项 | **219 组；目标克隆零涉及** | `cd project/ezr && pmd cpd -d src --language rust --minimum-tokens 70` |
| 差集归因 | 唯一被删组 = 35 行/232 tokens 克隆；其余字节级恒等 | `diff <(sed 's\|.*project/ezr/\|\|' pre.txt) <(sed 's\|.*project/ezr/\|\|' post.txt)` → 42 行全为被删组 |
| 靶点基线 | 2 测试 2P/0F（同语境） | scratch `cargo test --bin ezr gap112_trackpool_spawn_outgoing_peer` |
| unviable 补证 | **E0599 编译错（人工实证）** | scratch `sed -i '/^            Ok(Ok(s)) => s,$/d' src/bt/engine/trackpool.rs && cargo check --bin ezr` |
| 真实树零改动 | 产品源码 M 集合与轮前精确恒等（方案 A 4 文件） | `git status --short project/ezr/` |

### 对账口径
- **「零发现」语义**：销项条件 = 目标克隆（232-token bencode 对）零发现，**非全仓零克隆组**——219 组 = 历轮存量基线（产品面 19 + 测试面克隆，按 environment.md 493 行分类口径），与本轮改动无关、不构成新登记项。
- **双扫描可比性前提**：同工具版本（7.29.0）+ 同参数（min-tokens 70 + `-l rust`）+ 同 scope（project/ezr/src）；前态源码 = HEAD（7f7b90c）——11-17 基线扫描后至方案 A 前的两轮（分歧裁决落地轮、过滤器扩展轮）Rust 源码零改动（git 差异集实证），故 HEAD 源码 = 历轮基线扫描源码。
- **最终验证序列**：按操作者指令本轮仅执行 DRY 段（cpd 双扫描）；变异工具段（cargo-mutants 未装）、软 Gherkin 段（APS runtime 未重建）、CRAP 段均沿受限项登记，不在本轮范围。
- **工作区零残留**：全部探针在 `.work/tmp/`（cpd 报告、pre 提取树、unviable-probe scratch、pmdl zip）——complete 自动清理；产品树 `git status` 与轮前恒等。

### 待办与待批
- **本会话无待批无待办**——操作者指令三项（装 PMD / cpd 复扫 / spawn_outgoing 补证）均落地结清。

### 交接四要素（hardender 要素）
1. **本地验证命令清单（本轮新增段，按序）**：`pmd --version`（装机验证）→ cpd 双扫描（上表前态 + 后态命令）→ 路径规范化 diff → （可选）spawn probe 复现（上表 unviable 补证命令）。历轮全套验证命令清单沿上轮 handoff 四要素 1 不变——产品源码零改动，基线 1090P/0F、CRAP ACT=101、arch_check 18 规则全绿均沿上轮有效。
2. **会话运行时产物位置与复现**：`.work/tmp/cpd/`（pre/post.txt + .norm + pre/ 提取树）、`.work/tmp/pmdl/`（zip）、`.work/tmp/unviable-probe/`（scratch 副本）——complete 即清；复现 = 上表命令（PMD 装机沿 environment.md 本轮条目）。
3. **feature 注释块与差分复用注意事项**：本轮真实 features 零触碰，manifest 写回面无变化（沿上轮口径不变）。
4. **等价突变体清单及失效条件**：沿历轮不变——17 格权威登记 `project/qa/equivalence-filter.json`；trackpool.rs:644 `delete !`（失效 = webrtc 栈可用测试环境）；**2 条 unviable（spawn_outgoing 删臂本轮人工实证 E0599 编译错类终态；count_invalid_request 沿工具判定编译错类）**；trackpool.rs:412 CRAP/ACT 名义值竞速登记——本轮改动仅文档，行号与失效条件均不受影响。

### 移交建议
- **six-pack 流转路径保持完结态**：hardender 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 遗留受限项可解路径沿上轮移交建议不变：①APS runtime 重建轮（QA，一轮收多项：verdict 级账目 + RB-09/UC-10 入账 + 07-add-bt 哈希自刷新）；②Go 版 mutator 移植（QA 或授权越界）；③缺口账目 ID 再生（hardener，需装 cargo-mutants 27.1）；④644 等价论证重验（webrtc 环境）——均由操作者显式决定。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By hardender.
