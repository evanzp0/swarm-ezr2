# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（reload-hotload-impl + progressbar-tail 触碰面清理轮）
> 路径：…six-pack/coder（上轮 reload-hotload-impl + progressbar-tail）⇒ **six-pack/cleaner（本轮：触碰面覆盖率/CRAP/DRY/变异点扫描 + 保持行为清理，ezr 不做旧版本兼容沿袭，无持久化格式变化；完成后 complete 归档）**
> 会话：six-pack-cleaner-20261124-reload-clean（上游交接 = 上轮 handoff）
> 环境：nightly 1.102.0（rust-toolchain.toml 钉版，USTC 镜像 2026-11-23 快照，含 llvm-tools-preview）+ cargo registry aliyun sparse + bb v1.13.230（.tools/bin）+ APS 仓库 .tools/Acceptance-Pipeline-Specification（含等价扩展 patch；parser/dry-checker 复证 exit 0）+ cargo-mutants 27.1.0 / cargo-llvm-cov 0.9.2（~/.local/bin 预编译）+ PMD 7.29.0（.tools/pmd，java 21）。工作区已迁持久区 /home/z/my-project/swarm-ezr2（bin/swarm 会话无损，沙箱回收只重装环境不重做工作）。
> 受限项：bb test 11 测 0F/3E——3 个 error 全在 gherkin-mutator worker 测试面（mutation_test.clj），parser/dry-checker 与本轮全部验证不受影响；mutator 非 cleaner 范围，交 QA/hardender 轮按需归因。其余无新增受限项。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.61**）；phase-01/02 为现行详述（03/04 计划登记稿）。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；上轮已同构修复（v1.62）；本轮同步其 rustfmt 存量漂移销项（12 处换行风格，nightly fmt 后 check 转绿）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 20 份 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径） |
| coder 实现轮 reload-hotload-impl + bug 轮 progressbar-tail（20261124，上轮） | Ctrl+R 热加载全链路（reload.rs 新增 + engine ApplyGlobals + keys/mod/main/help）+ 进度条行尾恒空修复（task_lines/list/app + demo 五面）+ 单测 20 新增 | 已交付 | 上轮账：bin 1119P/0F + 集成 8646P/0F + demo 61P/0F + fmt/clippy 零警告 + APS exit 0 |
| **cleaner 清理轮（20261124，本轮）** | 触碰面覆盖率/CRAP/DRY/变异点扫描 + 纯追加 16 测 + reload 测试夹具提取 4 助手 + testutil::bt_conn_row 共享 + 3 处死抑制器删除 + demo 树 fmt 漂移销项 | 已交付 | bin 1135P/0F + 集成 13 套件 8768P/0F + demo 61P/0F + fmt/clippy 双项目零警告 + APS 42/42 |

### 遗留受限项（当前有效）
- **变异点 >100 登记移交（rust.md 口径）**：keys.rs=155、app/mod.rs=147（交互层分发/胶水文件）——拆分依赖 architect 模块边界裁决，登记 handoff 移交 hardender 全量变异前先收边界；触碰面其余：engine/mod.rs=63、task_lines.rs=73、main.rs=38、list.rs=22、reload.rs=17、help.rs=7（全仓 6301）。
- **CRAP 残余口径**：keys.rs:7 分发器 cov=0.946 / CRAP=33.18（comp=33 的口径下限=comp，再压需拆分）；main.rs 适配器外壳（进程/TTY 绑定，历轮登记不硬凑）；全仓 ACTIONABLE 99 项（bt/* 等遗留面沿历轮受限项，非本轮批次）。行覆盖 95.0%（bin 口径）。
- **CPD 残留沿袭**：全仓 222 组（min-tokens 70；bt/engine/mod.rs 362 处等）——历轮登记残留，本轮触碰面（reload.rs/keys.rs）已清零。
- 历轮验证面受限项沿袭有效（workers.rs 错误臂保留、2 条 unviable、bt/mod.rs pub mod 面、CRAP nightly 口径漂移，详见历轮归档与 `qa/mutation-gap-registry.json`）。
- QA-RC-01–14 的端到端 UI 层验证与 suite runner 可执行化归 six-pack/QA 轮（沿袭）。
- chat 选择题面板不可用（IM 通道）沿袭——本轮无决策闸门，不受影响。

### 实现定义值登记
- **D81①–⑤ / D82①–③（v1.57 落定版）**：固定文案 toast `✓ 配置已重新加载`（常量 TOAST_RELOADED 单点）；失败 toast `⚠ 配置热加载失败：<明细>`（try_load 逐键文案，\n→；单行化）；无配置文件臂 toast「无配置文件」；缺失文件臂 = 默认配置整体热加载且不重生成模板；引用消失（删除/改名统一）= Task.proxy 改写 Direct + save_registry 落库 + 双对话框下拉重建 + 运行任务端点比较发重配命令。
- **运行端点判定口径**：运行端点 = 当前 cfg.resolve_proxy(task.proxy)（三入口同源不变式，无独立记录状态）；比较 = old_cfg.resolve(旧 choice) vs new_cfg.resolve(新 choice) 的 ProxyEndpoint 全等（url/kind/username/password 四元组）；运行态判定 = Downloading | Seeding；重配命令 = Reconfigure/ReconfigureBt 仅换端点，原值不变。
- **ApplyGlobals 命令面**：engine `Cmd::ApplyGlobals { max_download_speed, max_upload_speed, throttle_burst_secs, throttle_sleep_ms }`——限速 set_rate 即时重定目标 + burst/sleep AtomicU64 原子更新；单命令原子下发。
- **CLI 覆盖登记**：App.cli_max_speed_override + set_cli_max_speed_override()（main.rs 登记）；每次热加载重应用，CLI > config 文件优先级贯穿运行期。
- **进度条行尾恒空**：行 = 2 列前导 + bar(width−4)，行尾恒无内容（全状态同口径）；校验/后期状态由行 1 状态徽标承载；braille 转轮显示面整体退役。

## 二、当前产出情况

### 本会话产物清单（cleaner 清理轮）
1. **纯追加测试 16 个（bin 侧，1119→1135）**：
   - `src/app/keys.rs` +7（overlay_key_nav_tests）：val 弹窗全键位（滚动/复制/关闭）、err 窗口键路由、trackers 弹窗全键位、Conns 面板导航（方向/翻页/Home/End/水平滚动）、Detail 焦点 no-op、Ctrl+J 命令窗口确认；
   - `src/app/mod.rs` +5：DetailField 四变体标签、open_val_popup 各字段臂与守卫、run_footer_cmd 四臂、global_ul_speed 求和口径、启动期配置警告 toast；
   - `src/engine/mod.rs` +2（client_for_tests）：client_for 指纹缓存/直连回退、proxy_fingerprint 区分度；
   - `src/ui/list.rs` +2（draw_list_tests）：区域过小提示臂、selected 越界钳制臂（TestBackend）。
2. **DRY 提取（测试侧，行为不变）**：`src/app/reload.rs` 测试夹具 4 助手（cfg_app/assign_named_p1/seed_named_dl/spy_engine，8 处测试改写）；`src/app/testutil.rs` 新增 bt_conn_row，keys.rs 两模块 row() 改委托——触碰面克隆组 7→0（PMD CPD 7.29.0，min-tokens 70）。
3. **死抑制器证据化清理**：`src/engine/mod.rs` cfg(test) re-export 组 4 处 allow(unused_imports) 裁决——classify_reqwest 为 include! 集成测试服务路径保留 allow，其余 3 处（HostsFile/parse_retry_after_header/throttle 组）删除后 clippy 0 警告验证。
4. **ezr-demo 树 fmt 存量漂移销项**：12 处换行风格（conns/dialogs/list/mod），nightly fmt 后 fmt --check 转绿（受限项销项）。
5. **notes 沉淀**：testing.md（include! 树复制对账口径 / App 构造必须 tokio::test / cfg(test) re-export allow 判据 / field_reassign_with_default）+ environment.md（USTC 整链路 + aliyun sparse + ghfast.top + sourcefrog 归属 + 持久区迁移纪律）+ INDEX.md 两行摘要同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd = /home/z/my-project/swarm-ezr2/project/ezr） |
|---|---|---|
| bin 全量（含属性 90P） | 1135P/0F（196s） | `cargo test --bin ezr` |
| 集成套件 | 13 套件 8768P/0F：g_appcore 899 / g_slots 254 / g_proxy 16 / g_engine 896 / g_supervisor 869 / g_throttle_err 549 / g_main 2 / g_v147 883 / hardening 262 / engine_backfill 1019 / ui_backfill 1049 / v115 1042 / mutation 1028 | `cargo test --test <名>`（串行前台，日志 /tmp/f-*.log） |
| demo 全量 | 61P/0F | `cargo test`（cwd = /home/z/my-project/swarm-ezr2/ezr-demo） |
| fmt / clippy | 双项目 fmt --check 干净 + clippy --all-targets 零警告 | `cargo fmt --check` / `cargo clippy --all-targets` |
| 覆盖率（bin 口径） | 行覆盖 40812/42966 = 95.0%（基线 94.7%）；reload.rs 418/423（余=测试守卫 panic 臂+幻影括号）、help/task_lines/list 满覆盖、keys 887/913、app/mod 772/783 | `cargo llvm-cov --bin ezr --lcov --output-path .work/tmp/ezr-bin2.lcov` |
| CRAP（受限近似） | keys.rs:7 CRAP 58.40→33.18（cov 0.714→0.946）；ACTIONABLE 100→99 | `python3 ../qa/runners/crap_calc.py .work/tmp/ezr-bin2.lcov <src_root>`（两参签名，尾参 src_root 必填） |
| DRY（PMD CPD） | 触碰面克隆组 7→0；全仓 229→222（余为历轮残留） | `.tools/pmd/pmd-bin-*/bin/pmd cpd --dir src --language rust --minimum-tokens 70 --format text`（EXIT=4=有克隆属正常） |
| 变异点扫描 | 全仓 6301；触碰面 keys 155 / app/mod 147 / engine 63 / task_lines 73 / main 38 / list 22 / reload 17 / help 7（scan 模式未跑变异） | `cargo mutants --list` |
| APS 结构验收 | 21 feature × parser + dry-checker = 42/42 exit 0 | `(cd .tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-parser <feature> <ir>.json && ../bin/bb gherkin-ir-dry-checker <ir>.json <out>.dry.json)` |

### 对账口径
- **bin 侧**：1119 → 1135（+16 = keys 7 + app 5 + engine 2 + list 2，纯追加，产品代码零改动）。
- **集成侧**：8646 → 8768（+122 = 16 新测在各套件 include! 树中的复制运行，逐套件差值精确自洽：含 app+keys+engine 树的 4 套件各 +14、仅 engine 树 +2、含 list 树的 4 套件各 +16，其余 +0）。
- **行为变化面**：零——本轮全部改动为测试追加、测试夹具提取、lint 抑制器裁决与 demo 换行风格；产品运行时行为与上轮完全一致（fmt/clippy 双清 + 全量三面复验背书）。
- **产品源文件改动面**：engine/mod.rs 仅删 3 个 allow 属性行（代码零改动）；testutil.rs/keys.rs/reload.rs/app/mod.rs/list.rs 改动均在 #[cfg(test)] 区内。

### 待办与待批（未决项）
- 无待批项——本轮无决策闸门，验证全绿。
- 登记移交（非阻塞）：keys.rs/app/mod.rs 变异点 >100 与 keys.rs:7 分发器拆分，待 architect 轮收模块边界后由 hardender 兑现（rust.md 口径，不属 cleaner 职权）。

### 移交建议
- **下一步运行 `six-pack/architect`**：收 keys.rs/app/mod.rs 模块边界（变异点 155/147 拆分裁决）+ 属性测试覆盖率评审（cleaner SKILL 移交建议链口）。
- 或按操作者计划直接进入 `six-pack/QA`（承接 QA-RC-01–14 端到端 UI 验证与 suite runner 可执行化）；如需变异加固则 architect 后运行 `six-pack/hardender`（可选节点，显式 begin）。
- 归档：`bin/swarm complete`（产物已按操作者指令复制 /home/z/my-project/download/ 交付）。

By cleaner.
