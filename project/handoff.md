# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（**裁决落地修订轮**——规格-产品分歧 ①②③ 按操作者裁决落地 + ④/mktorrent 了结，操作者 chat 显式指令）
> 路径：…coder demo 结算轮（20261116h）⇒ hardender 待批③④轮（缺口 112 回补 + 软 Gherkin 执行层基建）⇒ hardender 待批⑤轮（词汇面幸存 218 回补，全量收敛）⇒ **hardender 修订轮（本轮，操作者裁决驱动）**
> 会话：six-pack-hardender-20261117c（SESSION_TASK = verdict-landing-revision；上游交接 = 上轮 handoff + 操作者裁决 chat 指令）
> 环境：rustup stable 1.100 + nightly 1.102.0（rsproxy dist）、bb 1.13.229（.tools/bin/bb）+ APS 仓 .tools/、PMD 7.29.0、llvm-tools（nightly 工具链内）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.56**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（`ezr-demo/`，工作区外），41 轮变更记录；自带 PTY 级 verify 脚本。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| hardender 待批③④轮 | 缺口回补测试 37 条 + 软 Gherkin 执行层基建四件套 | 已交付 | bin 1089P；词汇面 1121 突变体 = 903 killed + 218 survived |
| hardender 待批⑤轮 | 执行层双层架构 + 全量收敛 | 已交付 | 软 Gherkin 全量 1124 = 1107 killed + 17 等价类 + 0 errors（非等价存活率 = 0）；bin 1089P；CRAP/DRY 恒等；产品源码零改动 |
| **hardender 修订轮（本轮）** | 操作者裁决落地：② 产品行为修正（URL 补全不响应）+ ①③ 规格文本按裁决修订 + ④/mktorrent 了结登记 | 已交付 | **修订重验 2 feature FULL：77 = 70 killed + 7 等价类 + 0 errors（零跳过，执行数 = 突变体总数）**；bin 1090P（+1 新增 URL 单测）；fmt/clippy 零警告；CRAP 行覆盖 94.6% 恒等；DRY 220 组恒等 |

### 遗留受限项（当前有效）
- **缺口账目 ID 不可精确复现**：沿历轮登记不变（以同 scope 全量重建口径承接）。
- **ws_process_response:644 `delete !` 等价论证（面内）**：沿历轮登记不变（失效条件 = webrtc 栈可用的测试环境）。**同源登记：trackpool.rs:412 `ws_prepare_offers`（webrtc 传输分支 + tokio spawn 任务）的 CRAP/ACT 名义值随进程退出竞速非确定（同二进制两遍实测 9/105 → 3/79 行覆盖），ACT 对账以稳定函数集为准（见验证证据 CRAP 行）。**
- **2 条 unviable（工具判定）**：沿历轮登记不变。
- **软 Gherkin 等价类 17 格**：见「等价类清单」，每格登记模式与失效条件；非等价存活率 = 0。
- **RB-09 磁盘预检 / UC-10 校验中窗口（操作者裁决④，本轮了结）**：沙箱无 mount/配额手段且校验中窗口 ≈109ms 不可靠采样，沿操作者 v140 S 口径 handler 走 SkipExecution（基线记 SKIP、变异 job 记 infrastructure_error，不虚报 killed）——**永久受限项了结**；失效条件 = 可 mount/配额的测试环境，或产品内嵌预检/校验提供确定性注入点。
- **mktorrent bencode 跨 bin 重复（操作者裁决「能处理就处理」，本轮评估后了结）**：历轮 architect 取证维持成立——src/bt/bencode.rs 首行 `use super::BtError`（依赖 bt/mod.rs 门面类型）非自包含，`#[path]` 硬包含编译必失败；bin-only 结构下消除重复只有 lib 化一途（构建图级变更，全部产物搬 lib + bin 薄壳），将打破 CRAP（按 bin 采集）/DRY（220 组账）/软 Gherkin 产品面（EZR_BIN 依赖）三类已交付账目；232-token 克隆位于构建工具 bin（非变异热点），收益不抵风险。**维持登记，了结**；失效条件 = 未来执行 lib 化重构时一并消除。

### 规格-产品形态分歧裁决落地（本轮，操作者 chat 显式裁决）
- **① 路径补全 tilde 形态**：裁决 =「输入框中保持 `~/downloads/`，生成任务时转化成绝对路径」。落地 = 规格文本修订（06-cmd-window-05 Examples `~/down` 行 completed/toast 改 `~/` 形态）+ 产品现状核实（补全 tok_dir 保留 tilde 前缀，与 HOME 解耦；任务生成侧 `normalize_save_dir_with`（model/config.rs:930，expand_tilde + cwd 拼接为绝对路径）经 dialogs.rs:91 / bt_dialog.rs:425 消费——现状满足裁决，零产品改动）。
- **② URL token 补全**：裁决 =「URL 不是文件路径，补全完全不搭理——输入保持原样、不弹任何提示」。落地 = **产品行为修正**（app/cmd.rs `cmd_complete_path` 入口新增 scheme 排除臂：`http://`/`https://` token 直接 return，判定与 Enter 确认分流 URL 臂同源）+ 新增单测 `complete_url_token_unresponded`（输入不变 + 无补全类 toast）；规格文本保持「（无 toast——非 path token 不响应）」不变（产品已符合规格）；产品面基线实跑验证（06-05 URL 行断言通过）。
- **③ 失败分类形态**：裁决 =「ezr 代码里如何处理就怎么处理」。落地 = 规格文本修订（01-retry-backoff-01 Examples reason 列改产品直接形态：连接失败 / 连接超时 / 传输中断 / HTTP 404 / HTTP 503 / 无法创建目标文件）——依据 engine/error.rs `network_reason` 直接形态臂 + supervisor/single.rs:103 与 supervisor/download.rs:172「无法创建目标文件（{e}）」；「连接被重置」注入按实测归「传输中断」臂（产品无该文案）；产品面分类表锚（上轮⑤类锚）语义不变。
- **④ RB-09/UC-10 与 mktorrent**：见「遗留受限项」，均已了结。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）**——本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（hardender 修订轮）
1. **产品行为修正一处**（app/cmd.rs：`cmd_complete_path` URL 排除臂 + `complete_url_token_unresponded` 单测）——操作者裁决②落地，其余产品源码零改动（`git diff HEAD -- src/` 仅此一文件）。
2. **规格文本修订两处**（features/01-retry-backoff-01 reason 列 6 格、features/06-cmd-window-05 `~/down` 行 2 格）——裁决①③落地，以产品现状为准。
3. **APS 运行时重建**（`.work/tmp/aps/`，会话运行时产物不入归档）：runtime.py（IR 展开 + 全场景格级列域校验 + 产品面分派 + SkipExecution 通道）、generator.py（acceptance-entrypoint-generator：薄入口 + metadata，implementation_hash 仅覆盖生成文件）、adapter.py（NDJSON 持久 worker + 差分定位唯一变更格 → APS_AFFECTED 收窄 + rc 语义：0=survived/1=killed/其余=infrastructure_error）、drive.py（parse→generate→基线全绿→mutator→JSON 报告全链驱动）、build_schema.py（格级 schema 生成：closed=值集封闭〔含产品面格〕/ open_int=lo0、hi base+15 / PIN_CLOSED 微域钉死一条目）、column_schema.json（77 格级条目 = closed 68 + open_int 9）、handlers/（world.py 独立 HOME 隔离 + cmd_window.py 06-05 场景 handler，复用 qa/runners/harness.py PTY 基建）。
4. **修订轮账目**：mut/report_01-retry-backoff.json、mut/report_06-cmd-window.json（FULL 级全执行）；两 feature 头部 manifest 由 mutator 写回（修订场景新 hash + 新结果；幸存场景不写入，与上轮形态一致）。
5. **product/notes/**：修订轮经验沉淀（testing.md 6 条/environment.md 2 条：显示通道真值判别、全角括号陷阱、双层统一校验、CRAP 采集三坑、soft differential 语义、磁盘时序纪律）。
6. **本 handoff.md** 重写为修订轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单测 | **1090P / 0F**（基线 1089 + 1 新增 URL 单测） | `cd project/ezr && export PATH=$HOME/.cargo/bin:$PATH && cargo test --bin ezr` |
| fmt / clippy --all-targets | clean / 0 warnings | `cargo fmt --check` / `cargo clippy --bin ezr --all-targets` |
| 软 Gherkin FULL（修订重验 2 feature） | **01-retry-backoff 40 = 34 killed + 6 等价类 + 0 errors；06-cmd-window 37 = 36 killed + 1 等价类 + 0 errors；执行数 = 突变体总数，零跳过** | `bb gherkin-mutator --feature <path> --work-dir .work/tmp/aps/mut/<stem> --generated-dir .work/tmp/aps/mut/<stem>/generated --runner-worker "python3 .work/tmp/aps/adapter.py" --level full --workers 1 --json`（基线先行：`python3 .work/tmp/aps/drive.py --feature <stem>`） |
| 修订格验证 | 06-05 基线 5 行全绿（`~/down` → `~/downloads/` + toast `补全目录：~/downloads/`；URL 行输入不变 + 无补全 toast = 裁决①②产品面实跑证据）；01-01 修订 reason 格词汇面基线全绿 | 同上（baseline 步骤） |
| 等价类复现 | 7 格（retry_after ×4 + status ×2 + extra_lines ×1）δ 确定性逐格复现上轮登记值（10→14/60→68/90→99/120→112、403→399/404→401、0→2） | mut/report_*.json 幸存清单 |
| CRAP（bin 单目标直采） | 行覆盖 39848/42112 = **94.6% 恒等**（+17 行 = 新增 cmd 代码）；**稳定函数集恒等**（mouse.rs:178 = 77.09 等），ACT 名义差异归因 trackpool.rs:412 webrtc 竞速（见遗留受限项同源登记） | 直跑插桩测试二进制（LLVM_PROFILE_FILE）→ llvm-profdata merge → llvm-cov export → **段级过滤**（`awk '/^SF:/{keep=index($0,"/project/ezr/src/")>0} keep'`）→ `python3 project/qa/runners/crap_calc.py <lcov> <src_root>` |
| DRY（PMD 7.29 CPD 阈值 70） | 总 220 组 = 与历轮基线恒等 | `.tools/pmd-bin-7.29.0/bin/pmd cpd -d src --language rust --minimum-tokens 70`（rc=4=有克隆） |
| 产品改动面 | `git diff HEAD -- src/` = **仅 app/cmd.rs**（裁决②修正 + 单测） | `git -C project/ezr diff HEAD --name-only -- src/` |

### 对账口径
- **修订重验范围 = 2 feature（77 突变体，FULL 级全执行）**：01-retry-backoff 40 = 34 + 6、06-cmd-window 37 = 36 + 1——与上轮分账**逐格一致**（等价类 7 格 δ 复现同值；修订格 killed 口径不变：01-01 的 18 格全杀、06-05 的 15 格全杀）。
- **其余 15 个 feature 账目继承有效**：本轮产品改动（cmd 补全入口 URL 排除）仅影响 06-05 独占的补全交互；两 feature 之外的 Examples/steps 内容零改动（mutation stamp/manifest 场景 hash 不变，differential 语义下账目复用合法）。全量总账维持 **1124 = 1107 killed + 17 等价类 + 0 errors**。
- **杀灭路径调整登记**：上轮 cmd-window 的 02/08 场景为产品面 handler 杀灭；本轮重建分派收敛为 02/08 词汇面封闭杀灭（closed 值集 = 规格枚举，变异域外先拒）+ 05 保留产品面 handler（裁决①②实跑证据）——杀灭能力等价迁移，分账不变；01-retry-backoff-02 的 max_retries 格同口径钉死微域 {5}（PIN_CLOSED 逐条登记于 build_schema.py docstring）。
- **产品改动面 = cmd.rs 一处（裁决②授权）**；CRAP/DRY 与基线恒等。

### 等价类清单（17 格，沿上轮登记，本轮 7 格逐格复现）
全部属**值镜像等价**：产品对该输入值泛化（任何合法值都正确处理），断言只能从突变值派生期望，无「非派生锚」可区分。逐格（本轮 FULL 复现格以 ✱ 标注）：
| 格 | 模式 | 论证 | 失效条件（回补触发） |
|---|---|---|---|
| ✱ retry-backoff retry_after ×4（10→14/60→68/90→99/120→112） | 输入镜像 | fixture 发突变头值、产品按其倒计时，断言与显示同源自洽；指数退避表 8/16/32/60 为固定锚但 Retry-After 优先级语义对任意值均成立 | 产品对 Retry-After 引入校验边界，或规格新增区分性断言（如倒计时节奏采样） |
| ✱ retry-backoff status ×2（403→399/404→401） | 分类镜像 | 突变值落在同类语义臂（399/401 与 403/404 同为「语义性停等」），产品分类表按类生效 | 分类表按状态码逐值区分（如 401 单列认证失效臂） |
| slots-queue slots ×2（2→5/5→9） | 容量镜像 | handler 按突变值配置容量并等量填充，徽标/排队/递补全链路自洽 | 规格钉死槽位测试点，或 fixture 文件清单有限使填充不可满足 |
| modify-task concurrency ×2（3→6/2→9）+ before ×2（4→1/0→9） | 输入回显镜像 | 添加/修改/重启往返均按突变值生效并回显，prefill/注册表/徽标三面同源自洽 | 并发或限速引入规格校验边界，或新增非回显派生的可观察量 |
| ui-conns secs ×2（12→9/8→11）+ conns ×1（16→7） | 采样/向下镜像 | 采样窗与连接数均按突变值驱动，图表点数/连接行数随值自洽 | 流量图/连接面板引入固定容量锚 |
| named-proxy port ×1（18766→18767） | 端点镜像 | handler 按突变值起桩实例并配置引用，日志链路自洽 | 规格钉死代理端口清单，或引入端口可用性断言 |
| ✱ cmd-window extra_lines ×1（0→2） | 计数镜像 | 词汇面 open_int（lo=0）使 0→2 界内幸存、负 δ（0→-3/2→-4 实测）域外被杀；产品 toast 计数对任意行数自洽（上轮产品面实跑论证继承有效） | 产品对 0 行输入产生区分性行为，或规格钉死行数测试点 |

> 压缩路径：mutator 扩展「项目等价突变过滤器」（mutator-spec §Equivalent Mutation Filters）——上游 bb.edn/脚本字节冻结，扩展属操作者裁决面（沿历轮登记不变）；扩展后上述 17 格登记为过滤器条目。

### 待办与待批
- **操作者四项裁决全部落地，无遗留待批**：①②③ 落地（产品修正一处 + 规格修订两处 + 重验 FULL 两 feature）+ ④/mktorrent 写入遗留受限项了结。
- **（沿历轮，非裁决项）QA runner 族五规程缺口**：06-cmd-window 已产品面可执行化（handlers/cmd_window.py 场景级 handler 可作 QA 化底稿）；其余四规程（07/09/10/11）handler 仍缺——独立小轮，由操作者显式决定是否开轮。
- **（沿历轮，非裁决项）mutator 等价突变过滤器扩展**：上游字节冻结，维持操作者裁决面登记。

### 移交建议
- **six-pack 流转路径保持完结态**：hardender 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 如需续做：五规程可执行化（07/09/10/11）为独立小轮，由操作者显式决定。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

### 交接四要素（hardender 要素）
1. **本地验证命令清单（按序）**：`export PATH=$HOME/.cargo/bin:$PATH` → `cargo fmt --check` → `cargo clippy --bin ezr --all-targets` → `cargo test --bin ezr` → 软 Gherkin 修订重验：`python3 .work/tmp/aps/drive.py --feature <stem>`（基线全绿先行；硬证据加 `--level full` mutator 直跑）→ CRAP：插桩直跑测试二进制（LLVM_PROFILE_FILE 注入 + CARGO_INCREMENTAL=0，见 notes/environment.md）→ llvm-profdata merge → llvm-cov export → **lcov 段级过滤**（awk，见 notes/testing.md）→ `python3 project/qa/runners/crap_calc.py <lcov> <src_root>` → DRY：`.tools/pmd-bin-7.29.0/bin/pmd cpd -d src --language rust --minimum-tokens 70`。
2. **会话运行时产物位置与复现**：执行层 `.work/tmp/aps/`（runtime/generator/adapter/drive/build_schema/column_schema.json/handlers/）；修订报告 `mut/report_*.json`（2 份）；handlers 依赖 qa/runners/harness.py。全部会话级 complete 即清；重建 = 先 `bb gherkin-parser` 出 IR → `python3 build_schema.py` → `python3 drive.py --feature <stem>`（命令细节见 notes/testing.md 修订轮条目）。
3. **feature 注释块与差分复用**：mutator 的 manifest 注释块由工具自管理写回（宪法允许）；修订轮后 01-retry-backoff manifest = 修订场景新 hash 条目（幸存场景不写入）；**强制全量重跑 = mutator `--level full`**（忽略 stamp/manifest）。
4. **等价突变体清单及失效条件**：见「等价类清单」17 格（值镜像类，逐格论证 + 失效条件；本轮 7 格 FULL 复现）；另沿历轮：trackpool.rs:644 `delete !`（失效条件 = webrtc 栈可用测试环境）+ 2 条工具判定 unviable；**新增同源登记：trackpool.rs:412 webrtc 竞速导致 CRAP/ACT 名义值非确定（非回归对账以稳定函数集 + 行覆盖恒等为准）**。
