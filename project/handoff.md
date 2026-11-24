# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（Ctrl+R 配置热加载产品端实现轮 + 进度条行尾溢出 bug 修复轮）
> 路径：…six-pack/specifier 修订轮 reload-toast-unify（上轮）⇒ **six-pack/coder（本轮，按 FR-01-119 v1.57 固定文案口径 + D82 引用消失落库转直连实现产品端；并同步修复操作者 bug 轮报告的进度条行尾 `SH` 溢出缺陷——ezr + ezr-demo 同构；完成后 complete 归档）**
> 会话：six-pack-coder-20261124-reload-hotload-impl（上游交接 = 上轮 handoff；ezr 不做旧版本兼容——操作者钦定沿 v1.45，本轮纯产品端实现，无持久化格式变化）
> 环境：rustup nightly 1.102.0（rust-toolchain.toml 钉 nightly，USTC 镜像，本轮全新沙箱装机）+ cargo registry aliyun sparse + bb v1.13.230（.tools/bin）+ APS 仓库 .tools/Acceptance-Pipeline-Specification（gherkin-parser / gherkin-ir-dry-checker 复证可用）。
> 受限项：QA 套件 runner 可执行化与端到端 UI 层验证归 six-pack/QA 轮（coder 不实现 QA 套件，SKILL.md 边界）；其余无新增受限项。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.61**）；phase-01/02 为现行详述（03/04 计划登记稿）。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；上轮已四面同步（v1.57 演示 toast）；**本轮随操作者 bug 轮指令同步同构修复（v1.62 同步补记 + README 变更记录 44，进度条行尾恒空）**。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 20 份 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径） |
| specifier 修订轮 reload-toast-unify（20261124，v1.61/v1.57） | mission v1.61 + phase-01 v1.57 + features/12-reload-config.feature v1.57（14 场景）+ qa/12-reload-config-qa.md v1.57 + ezr-demo 四面同步 | 已交付 | APS parser exit 0 + dry-checker exit 0 + rg 残留核验 + demo cargo test 61P/0F |
| **coder 实现轮 reload-hotload-impl（20261124，本轮）** | 产品端 Ctrl+R 配置热加载（FR-01-119 全量）：src/app/reload.rs（新增）+ engine Cmd::ApplyGlobals + EngineShared burst/sleep 原子化 + keys.rs CONTROL 分支 + App.cli_max_speed_override + main.rs CLI 覆盖登记 + ui/help.rs Ctrl+R 条目（帮助 83 行）+ 单测 20 新增 | 已交付 | bin 1119P/0F + 集成 13 套件 8646P/0F + fmt/clippy 零警告 + APS parser/dry-checker exit 0（IR 14 场景） |
| **coder bug 修复轮 progressbar-tail（20261124，本轮追加）** | 操作者 bug 轮报告「HTTP 任务 SHA-512 校验失败，进度条末尾出现 `SH`」修复：行尾提示臂整臂退役、行尾恒空（ezr task_lines/list/app + demo list/app/docs/README 五面同步 + spinner 链路退役 + 守卫测试 2 新增） | 已交付 | ezr bin 1119P/0F + 集成 8646P/0F + demo 61P/0F + 两项目 fmt/clippy 零警告 |

### 遗留受限项（当前有效）
- 上轮验证面受限项全部沿袭有效（workers.rs 错误臂保留、bin 口径 cov=0 五处、2 条 unviable、bt/mod.rs pub mod 面、CPD 残留、CRAP nightly 口径漂移，详见历轮归档与 `qa/mutation-gap-registry.json`）。
- **QA-RC-01–14 的端到端 UI 层验证（按键注入 + 画面断言 + fixture 日志对账）与 suite runner 可执行化未执行**——归 six-pack/QA 轮职责；本轮以单元/集成/引擎级测试覆盖行为契约（reload 16 测 + keys 路由 3 测 + ApplyGlobals 1 测 + 全量回归），场景级真机判据（代理 fixture 来源日志、tracker announce 端口、PTY toast 捕获）待 QA 轮。
- chat 选择题面板不可用（IM 通道）沿袭——本轮无决策闸门，不受影响。
- ezr-demo 树 rustfmt 存量漂移沿袭（demo 轮遗留）；`bin/ezr-tui-demo` 预编译二进制不属 git 树，本轮免重建。

### 实现定义值登记
- **D81①–⑤ / D82①–③（v1.57 落定版）全部按规格实现**：固定文案 toast `✓ 配置已重新加载`（常量 TOAST_RELOADED 单点）；失败 toast `⚠ 配置热加载失败：<明细>`（try_load 逐键文案，\n→；单行化）；无配置文件臂 toast「无配置文件」；缺失文件臂 = 默认配置整体热加载且不重生成模板；引用消失（删除/改名统一）= Task.proxy 改写 Direct + save_registry 落库 + 双对话框下拉重建（已选名仍在池保持、消失回退直连）+ 运行任务端点比较发重配命令。
- **新增实现定义（规格「有变化才发命令」的判定口径）**：运行端点 = 当前 cfg.resolve_proxy(task.proxy)（三入口同源不变式，无独立记录状态）；比较 = old_cfg.resolve(旧 choice) vs new_cfg.resolve(新 choice) 的 ProxyEndpoint 全等（url/kind/username/password 四元组）；运行态判定 = Downloading | Seeding（与 shows_conns 同源）；重配命令 = Reconfigure/ReconfigureBt 仅换端点，concurrency/files/限速/sequential 原值不变。
- **新增实现定义（ApplyGlobals 命令面）**：engine 新增 `Cmd::ApplyGlobals { max_download_speed, max_upload_speed, throttle_burst_secs, throttle_sleep_ms }`——限速 set_rate 即时重定目标（A 类）+ burst/sleep AtomicU64 位存储原子更新（C 类新启动任务生效、运行中桶参数不触）；单命令原子下发避免半新半旧窗。
- **新增实现定义（CLI 覆盖登记）**：App 新增 `cli_max_speed_override: Option<u64>` + `set_cli_max_speed_override()`（main.rs 在 App::new 后登记）；每次热加载重应用，CLI > config 文件优先级贯穿运行期。
- **新增实现定义（bug 轮：进度条行尾恒空）**：进度条行 = 2 列前导 + bar(width−4)，**行尾恒无任何内容**（全状态同口径）——校验中/后期处理中行尾提示臂（`<算法> 分块校验 ⠋`/`后期处理中 ⠋`）整臂退役；根因 = 提示与行宽预算不符溢出恰露 2 列（SHA-512 → `SH`）；校验/后期状态由行 1 `[校验中]`/`[后期处理中]` 状态徽标承载；braille 转轮显示面（App.spinner()/SPINNER/task_lines spinner 参数）随之整体退役；走马灯滑块（anim 驱动）不受影响；守卫测试 progress_row_tail_always_empty（五状态 × SHA-512）+ progress_row_tail_empty_at_narrow_width（窄宽度行宽恒等）。

## 二、当前产出情况

### 本会话产物清单（coder 实现轮 reload-hotload-impl + bug 修复轮 progressbar-tail）
1. **`project/ezr/src/app/reload.rs`（新增）**：App::reload_config（Ctrl+R 入口 glue）+ reload_config_from（路径缺失臂 / try_load 失败臂单行化 / apply_reloaded 成功臂：CLI 覆盖重应用 → max_slots → 引用消失落库转直连 → 运行任务端点比较重配 → 下拉重建三步序 → ApplyGlobals 下发 → 固定文案 toast）+ 16 个单测（全臂覆盖 + 引擎 spy 命令序断言）。
2. **`project/ezr/src/engine/mod.rs`**：Cmd::ApplyGlobals 新增 + handle_cmd 臂（set_rate × 2 + set_throttle_params）+ EngineShared.throttle_burst_secs/throttle_sleep_ms 改 AtomicU64（f64::to_bits）+ getter/setter + 4 处构造点同步（start/for_test/shared_for_tests/supervisor test_shared）+ ApplyGlobals 引擎级测试。
3. **`project/ezr/src/app/keys.rs`**：CONTROL 分支 Ctrl+R（先于浮层分派，与 Ctrl+C 同级）+ 3 个路由测试（主界面与七类浮层可达且不关闭、命令窗口幂等、裸 R 语义不受扰）。
4. **`project/ezr/src/app/mod.rs`**：App.cli_max_speed_override 字段 + set_cli_max_speed_override 方法 + `mod reload;` 注册。
5. **`project/ezr/src/main.rs`**：CLI --max-speed 从 App::new 前改 cfg 迁到 App::new 后 set_cli_max_speed_override（登记 + 应用一体）。
6. **`project/ezr/src/ui/help.rs`**：全局节新增 `Ctrl+R 重新加载配置文件（热加载，无需重启）` 条目（帮助 82→83 行）；两处行数守卫测试同步（End 钳制 62→63、总行数 82→83）。
7. **notes 沉淀**：architecture.md（运行端点不变式免新增状态 / ApplyGlobals 原子下发 / 下拉重建三步序 / CLI 覆盖登记点 / CONTROL 分支锚 / reload 臂序）+ testing.md（reload_config_from 注入口避 env 竞态 / spy 命令序断言 / 帮助双守卫面 / pretty JSON 断言形态 / 网关取消进程存活采信）+ environment.md（USTC/aliyun/ghfast.top 镜像复测 + 套件节奏）+ INDEX.md 追加段。
8. **bug 修复轮（progressbar-tail，ezr）**：`src/ui/task_lines.rs` progress_row 尾臂删除 + spinner 参数退役 + 行尾恒空注释 + 守卫测试 2 新增（五状态 × SHA-512 全算法名 + 窄宽度行宽恒等）+ 原 progress_row_activity_hints 改判；`src/ui/list.rs` render_task/draw_list spinner 链退役；`src/app/mod.rs` SPINNER 常量与 App::spinner() 删除；`tests/hardening_g_appcore.rs` spinner 变异测试随代码退役。
9. **bug 修复轮（progressbar-tail，ezr-demo 同构）**：`src/ui/list.rs` task_lines 尾臂删除 + spinner 退役 + `src/app/mod.rs` SPINNER/spinner() 删除 + `src/ui/mod.rs` 测试调用点 6 处同步 + docs 设计规范 v1.62 同步补记/§4.4 动画条目退役注记/§5.2 行 2 规格改判 + README 变更记录 44。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd = /home/z/swarm-ezr2/project/ezr） |
|---|---|---|
| reload 单测 | 16P/0F | `cargo test --bin ezr reload_tests` |
| keys 路由测试 | 3P/0F | `cargo test --bin ezr reload_key` |
| 引擎 ApplyGlobals | 1P/0F | `cargo test --bin ezr apply_globals` |
| 行尾恒空守卫（bug 轮新增） | 2P/0F | `cargo test --bin ezr progress_row_tail` |
| bin 全量（含属性 90P） | 1119P/0F（195s） | `cargo test --bin ezr` |
| demo 全量 | 61P/0F | `cargo test`（cwd = /home/z/swarm-ezr2/ezr-demo） |
| 集成套件 | 13 套件 8646P/0F：g_appcore 885 / g_slots 254 / g_proxy 16 / g_engine 882 / g_supervisor 855 / g_throttle_err 547 / g_main 2 / g_v147 869 / hardening 262 / engine_backfill 1003 / ui_backfill 1033 / v115 1026 / mutation 1012 | `cargo test --test <名>`（串行前台，日志 .work/tmp/*.log） |
| fmt / clippy | 两项目 fmt --check 干净（ezr）；clippy --all-targets 零警告（demo 树存里 nightly-fmt 漂移沿袭、clippy 零警告） | `cargo fmt --check` / `cargo clippy --all-targets` |
| APS parser | exit 0，IR 14 场景/122 步齐序 | `(cd ../../.tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-parser ../../project/features/12-reload-config.feature /tmp/aps/12-reload-config.json)` |
| APS dry-checker | exit 0 | `(cd ../../.tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-ir-dry-checker /tmp/aps/12-reload-config.json /tmp/aps/12-reload-config-dry.json)` |

### 对账口径
- **改动面（实现轮）**：产品端 ezr 六文件（reload.rs 新增 + engine/mod.rs + app/keys.rs + app/mod.rs + main.rs + ui/help.rs）；测试新增 20（reload 16 + keys 3 + engine 1）；帮助守卫 2 处同步改判。
- **改动面（bug 修复轮）**：ezr 四文件（ui/task_lines.rs + ui/list.rs + app/mod.rs + tests/hardening_g_appcore.rs——spinner 变异测试随代码退役）+ 守卫测试 2 新增/1 改判；ezr-demo 五文件（ui/list.rs + app/mod.rs + ui/mod.rs + docs 设计规范 + README 变更记录 44）；mission/、features/、qa/、runners/ 零改动（features 无行尾提示断言面，规格零改动）。
- **测试计数**：bin 套件 1098 → 1119（实现轮 +20、bug 轮 +1 净：守卫 2 新增 − activity_hints 1 改判）；集成 13 套件全绿 8646（g_appcore 886→885：spinner 变异测试随代码退役）；demo 61 全绿。
- **行为变化面**：①新增 Ctrl+R 热加载链路（既有快捷键语义零改动）；②**进度条行尾恒空（bug 轮）**——校验中/后期处理中行尾提示臂退役，全状态行宽恒 = 2 前导 + bar(width−4)；braille 转轮显示面随之退役（App.spinner()/SPINNER 删除）；engine 新命令 ApplyGlobals 不触既有命令面；Throttle 桶语义不变。
- **上轮建议实现要点核对**：keys.rs CONTROL 分支 ✓ / App::reload_config ✓ / Cmd::ApplyGlobals ✓ / EngineShared burst·sleep 原子化 ✓ / help.rs 条目 ✓ / 固定文案 toast ✓ / 引用消失落库转直连 + 注册表持久化 ✓ / proxy_options/proxy_labels 重建双对话框面（含浮层内同步与已选名回退直连）✓；**Task.running_proxy 未落地**——以「运行端点 = cfg.resolve_proxy(task.proxy)」不变式等价替代（无需新增状态，实现定义值已登记，architecture.md 有裁掉理由）。

### 待办与待批（未决项）
- 无待批项——本轮按 FR-01-119 v1.57 既有口径实现，无新登记定义值争议；无阻塞项，验证全绿。

### 移交建议
- **下一步运行 `six-pack/cleaner`**：对本轮触碰面做保持行为不变的清理、覆盖率提升、CRAP/DRY 审查与变异点扫描（coder 不做这些检查，SKILL.md 边界）；reload.rs 为新增文件，变异点扫描与 CRAP 口径按既有 nightly 流程接手。
- 或按操作者计划直接进入 `six-pack/architect` / `six-pack/QA`（QA 轮承接 QA-RC-01–14 的端到端 UI 层验证与 suite runner 可执行化）。
- 归档：请操作者运行 `bin/swarm complete`（产物复制 /home/z/my-project/download/ 由操作者执行）。

By coder.
