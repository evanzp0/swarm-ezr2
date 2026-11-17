# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（待批⑤执行轮——词汇面幸存 218 的产品面 handler 回补，操作者 chat 显式指令「不需要授权，完全放行，全部杀完才能结束任务」）
> 路径：…coder demo 结算轮（20261116h）⇒ hardender 待批③④轮（缺口 112 回补 + 软 Gherkin 执行层基建）⇒ **hardender 待批⑤轮（本轮，操作者显式 begin）**
> 会话：six-pack-hardender-20261117b（SESSION_TASK = vocabulary-survivor-218-product-handler-backfill；上游交接 = 上轮 handoff + 操作者 chat 指令）
> 环境：rustup stable 1.100 + nightly 1.102.0（rsproxy dist）、bb 1.13.229（gh-proxy）+ APS 仓 .tools/、PMD 7.29.0（gh-proxy 长超时）、llvm-tools（整链重装落盘）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.56**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（`ezr-demo/`，工作区外），41 轮变更记录；自带 PTY 级 verify 脚本。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| hardender 待批③④轮（上轮） | 缺口回补测试 37 条 + 软 Gherkin 执行层基建四件套 | 已交付 | bin 1089P；词汇面 1121 突变体 = 903 killed + 218 survived（执行数对账自洽） |
| **hardender 待批⑤轮（本轮）** | 执行层双层架构（词汇面按场景列域封闭 + 产品面 59 bespoke PTY handler）+ 全量收敛 | 已交付 | **软 Gherkin 全量 1124 = 1107 killed + 17 等价类 + 0 errors（非等价存活率 = 0）**；bin 1089P；fmt/clippy 零警告；CRAP ACT=101 非回归；DRY 220 组恒等；产品源码零改动 |

### 遗留受限项（当前有效）
- **缺口账目 ID 不可精确复现**：沿历轮登记不变（本轮以同 scope 全量重建口径承接）。
- **ws_process_response:644 `delete !` 等价论证（面内）**：沿历轮登记不变（失效条件 = webrtc 栈可用的测试环境）。
- **2 条 unviable（工具判定）**：沿历轮登记不变。
- **软 Gherkin 等价类 17 格**：见「二、等价类清单」，每格登记模式与失效条件；非等价存活率 = 0。
- **规格-产品形态分歧 4 项（本轮产品面执行发现，待操作者裁决）**：①路径补全规格期望展开形态 `/home/qa/downloads/`、产品保持 tilde 形态 `~/downloads/`；②URL token 规格期望不响应补全、产品 path_like 含 `contains('/')` → 报「无匹配」toast；③失败分类规格「网络错误（子类）」、产品直接形态（连接失败/连接超时/传输中断/无法创建目标文件）；④RB-09 磁盘预检 / UC-10 校验中窗口沿操作者 v140 S 口径，handler 走 SkipExecution。
- **peer_connection_protocol 改判（已结案）**、**QA runner 族五规程缺口（其中 06-cmd-window 本轮已产品面可执行化）**、**mktorrent bencode 跨 bin 重复**——沿历轮登记不变。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）**——本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（hardender 待批⑤轮）
1. **执行层双层架构**（`.work/tmp/aps/`，会话运行时产物不入归档）：runtime.py（IR 展开 + 词汇面按场景列域封闭校验 + 产品面分派 + SkipExecution 通道）、generator.py（acceptance-entrypoint-generator：入口 + metadata）、adapter.py（NDJSON 持久 worker + 差分定位受影响格 + rc=3 SKIP 语义）、drive.py（strip→parse→generate→基线→mutator→报告全链驱动）、column_schema.json（189 列 431 个按场景列域条目）。
2. **产品面 bespoke handler 59 场景**（`handlers/` 六大域 + persistence_config 增补）：retry_backoff 10 / slots_queue 11 / modify_task 10 / ui_conns 11 / resume_sidecar 14 / cmd_window 3 / persistence_config 1——直驱真实 ezr 二进制（复用 qa/runners/harness.py PTY 基建），每执行独立 HOME/保存目录/fixture 实例。
3. **杀灭锚六类设计**（把示例值变成产品可观察载荷）：跨格锚、步骤文本字面锚、config 越界拒启动锚、输入过滤锚、产品固定产物锚、fixture 块数截断锚。时序归约（事件顺序断言）替代窗口竞速。
4. **product/notes/**：本轮经验沉淀（testing.md/environment.md/INDEX.md）。
5. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单测基线 | 1089P / 0F | `cd project/ezr && cargo test --bin ezr` |
| fmt / clippy --all-targets | clean / 0 warnings | `cargo fmt --check` / `cargo clippy --bin ezr --all-targets` |
| 软 Gherkin 全量（词汇面+产品面双层） | **1124 = 1107 killed + 17 等价类 + 0 errors，执行数 = 突变体总数** | `python3 .work/tmp/aps/drive.py --feature <stem>`（逐 feature）；产物 `mut/report_*.json` |
| 六大目标域分账 | cmd-window 37=36+1、slots-queue 16=14+2、retry-backoff 40=34+6、modify-task 56=52+4、ui-conns 36=34+2、resume-sidecar 36=36+0 | 同上 |
| CRAP（bin 单目标直采） | 行覆盖 39829/42095 = 94.6%；ACT=101（与基线 101 恒等，非回归） | 直跑测试二进制 + llvm-profdata/llvm-cov + `python3 project/qa/runners/crap_calc.py <lcov> <src_root>` |
| DRY（PMD 7.29 CPD 阈值 70） | 总 220 组 = 与历轮基线恒等（产品零改动） | `.tools/pmd-bin-7.29.0/bin/pmd cpd -d src --language rust --minimum-tokens 70`（rc=4=有克隆） |
| 产品源码零改动 | src/ 与 git HEAD 逐文件 diff = 0（20 个差异文件全为 feature 的工具自管理 stamp/manifest 注释块，宪法允许） | `git diff HEAD --name-only`（project/ezr/src/ 无一在列） |

### 对账口径
- **突变体总数 1124**（历轮登记 1121，口径漂移 +3 以本轮实测为准；1124 = 示例格全量，mutator 对空转格跳过后的执行数与此一致）。
- **执行数 = 突变体总数（1124）**，errors=0，verdict 交叉核验通过；每 feature 报告见 `mut/report_*.json`，幸存/等价清单见本表「等价类清单」。
- **上轮词汇面 218 的处置对账**：218 = 本轮词汇面强校验（按场景列域封闭，含实体清单/meta 引用名校验）杀死 + 产品面 handler 杀死（六类锚）+ 17 等价类登记——三类合计覆盖 221 幸存格（218 实存活 + 3 口径漂移格），**非等价存活率 = 0**。
- **产品改动面 = 零**；CRAP/DRY 与基线恒等。

### 等价类清单（17 格，逐格论证）
全部属**值镜像等价**：产品对该输入值泛化（任何合法值都正确处理），断言只能从突变值派生期望，无「非派生锚」可区分。逐格：
| 格 | 模式 | 论证 | 失效条件（回补触发） |
|---|---|---|---|
| retry-backoff retry_after ×4（10→14/60→68/90→99/120→112） | 输入镜像 | fixture 发突变头值、产品按其倒计时，断言与显示同源自洽；指数退避表 8/16/32/60 为固定锚但 Retry-After 优先级语义对任意值均成立 | 产品对 Retry-After 引入校验边界（如上限/白名单），或规格新增区分性断言（如倒计时节奏采样） |
| retry-backoff status ×2（403→399/404→401） | 分类镜像 | 突变值落在同类语义臂（399/401 与 403/404 同为「语义性停等」），产品分类表按类生效 | 分类表按状态码逐值区分（如 401 单列认证失效臂） |
| slots-queue slots ×2（2→5/5→9） | 容量镜像 | handler 按突变值配置容量并等量填充，徽标/排队/递补全链路自洽（跨格锚 others+1 仅护 s03，本两格无未突变格可锚） | 规格钉死槽位测试点（如容量仅允许 {2,5}），或 fixture 文件清单有限使填充不可满足 |
| modify-task concurrency ×2（3→6/2→9）+ before ×2（4→1/0→9） | 输入回显镜像 | 添加/修改/重启往返均按突变值生效并回显，prefill/注册表/徽标三面同源自洽 | 并发或限速引入规格校验边界（如最大并发上限断言），或新增非回显派生的可观察量 |
| ui-conns secs ×2（12→9/8→11）+ conns ×1（16→7） | 采样/向下镜像 | 采样窗与连接数均按突变值驱动，图表点数/连接行数随值自洽（块数截断锚只护向上突变） | 流量图/连接面板引入固定容量锚（如规格钉死采样窗或文件块数） |
| named-proxy port ×1（18766→18767） | 端点镜像 | handler 按突变值起桩实例并配置引用，日志链路自洽 | 规格钉死代理端口清单，或引入端口可用性断言 |
| cmd-window extra_lines ×1（0→2） | 计数镜像 | handler 按突变值构造行数，toast 计数同源自洽 | 产品对 0 行输入产生区分性行为（如无 toast vs 有 toast 的规格判定落码） |

> 压缩路径：mutator 扩展「项目等价突变过滤器」（mutator-spec §Equivalent Mutation Filters）——上游 bb.edn/脚本字节冻结，扩展属操作者裁决面；扩展后上述 17 格登记为过滤器条目（确定性、逐格依据、本表失效条件随附）。

### 待办与待批
- **（沿历轮）QA runner 族五规程缺口**：06-cmd-window 本轮已产品面可执行化（handlers/cmd_window.py 三场景可作 QA 化底稿）；其余四规程（07/09/10/11）handler 仍缺。
- **（本轮登记）规格-产品形态分歧 4 项**：见「遗留受限项」，需操作者裁决（修规格文本 or 修产品行为）。
- **（本轮登记）mutator 等价突变过滤器扩展**：上游字节冻结，需操作者裁决是否解冻。
- **（沿历轮）mktorrent bencode 跨 bin 重复**：不变。

### 移交建议
- **six-pack 流转路径保持完结态**：hardender 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 如需续做：五规程可执行化（07/09/10/11）、规格-产品分歧裁决落地均为独立小轮，由操作者显式决定。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

### 交接四要素（hardender 要素）
1. **本地验证命令清单（按序）**：`cargo fmt --check` → `cargo clippy --bin ezr --all-targets` → `cargo test --bin ezr` → 软 Gherkin 逐 feature：`python3 .work/tmp/aps/drive.py --feature <stem> --skip-baseline`（全量首跑不带 --skip-baseline，基线 17/17 场景须全绿）→ CRAP：直跑测试二进制（LLVM_PROFILE_FILE 注入，见 notes/environment.md）→ llvm-profdata merge/llvm-cov export → `python3 project/qa/runners/crap_calc.py <lcov> <src_root>` → DRY：`.tools/pmd-bin-7.29.0/bin/pmd cpd -d src --language rust --minimum-tokens 70`。
2. **会话运行时产物位置与复现**：执行层 `.work/tmp/aps/`（runtime/generator/adapter/drive/column_schema.json/handlers/）；变异报告 `mut/report_*.json`（20 份）；CRAP 管线 `cov/`。全部会话级 complete 即清；重建 = 按上述命令重跑（bb/APS/PMD/llvm-tools 装机见 notes/environment.md；handlers 依赖 qa/runners/harness.py 与 suite_resume_sidecar._Mutator）。
3. **feature 注释块与差分复用**：gherkin-mutator 的 stamp/manifest 注释块已写回 20 个 feature 头部（工具自管理，宪法允许）；内容未变时二轮跑 = skipped_mutations（复用首轮判定）；**强制全量重跑 = 删除 feature 头部 stamp/manifest 注释块**（drive.py 默认剥离）；metadata feature_path 必须与 --feature 实参一致（generator 经 APS_FEATURE_PATH 注入）。
4. **等价突变体清单及失效条件**：见「等价类清单」17 格（值镜像类，逐格论证 + 失效条件）；另沿历轮：trackpool.rs:644 `delete !`（失效条件 = webrtc 栈可用测试环境）+ 2 条工具判定 unviable。
