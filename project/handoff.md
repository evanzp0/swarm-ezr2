# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（architect 边界收口批次变异加固轮）
> 路径：…six-pack/architect（上轮 keys/mod 双 >100 边界收口 + 属性测试 +2 + arch_check 规则 19）⇒ **six-pack/hardender（本轮：architect 批次 307 变异点语言变异 + 软 Gherkin 变异 + CRAP + DRY 全链，完成后 complete 归档）**
> 会话：six-pack-hardender-20261125-hardening（上游交接 = 上轮 handoff）
> 环境：nightly 1.102.0-nightly (c7d5ba59d 2026-11-20，与 QA 20261123 轮同构建；rust-toolchain.toml channel=nightly + TUNA dist 前台装链）+ cargo registry aliyun sparse（fetch 3.2s）+ cargo-mutants 27.1.0（.tools/bin，真实仓 sourcefrog/cargo-mutants，gh-proxy 资产直链）+ bb 1.13.230 + APS（patch 干净应用，bb test 11 测 51 断言 0F/0E）+ PMD 7.29.0（gh-proxy.com -C - 续传装机）+ llvm-cov 直采管线（llvm-tools-preview 随链装齐）。
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭——本轮无决策闸门，不受影响；②12-reload-config 软变异为词汇面执行（9 survived = 词汇覆盖声明，见二节），产品面 handler 归 QA 轮（QA-RC-01–14 可执行化沿袭）；③bb test 11 测 0F/3E 历史受限项定性修正为 PATH 注入假受限（bb 入 PATH 即 0F/0E），销项。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.61**）；phase-01/02 为现行详述（03/04 计划登记稿）。ezr 不做旧版本兼容沿袭（操作者钦定沿 v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；历轮已同构同步（v1.62）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径） |
| coder + cleaner + architect 轮（20261124） | Ctrl+R 热加载全链路 + 进度条行尾恒空修复 + keys/mod 目录化 + app/mod 拆分 + 属性测试 +2 + arch_check 规则 19 | 已交付 | 上轮账：bin 1137P/0F + 集成 8768P/0F + demo 61P/0F + fmt/clippy 零警告 + arch_check 19/19 + 属性 92P/0F |
| **hardender 变异加固轮（20261125，本轮）** | architect 批次 307 变异点语言变异收敛 + 加固测试 +31（bin 1165P）+ 软 Gherkin 全 21 feature（Σ=1124 守恒 + reload 15 执行）+ CRAP 直采 + CPD 222 恒等 | 已交付 | 见二节逐项 |

### 遗留受限项（当前有效）
- **全仓 >100 变异点存量**：mouse.rs=223、textfield.rs=226、scrollbar.rs=223、multiline.rs=204、bt/engine/pieceio.rs=209、bt/peer_id.rs=181、ui/bt_dialog.rs=179、model/config.rs=161、bt/tracker.rs=159、app/dialog_keys/routing.rs=145、app/bt_tree.rs=111、app/dialogs.rs=101——历轮登记沿袭（非本轮批次；如需拆分按 rust.md 口径由后续 architect 轮裁决）。本轮批次（keys/{mod,overlays,conns} + app/{mod,stats,restore}，307 点）已全量变异收敛。
- **CRAP 残余沿袭**：ACTIONABLE_COUNT=100（基线 99±1 属 nightly 插桩行漂移既定口径；双锚恒等见二节）；keys/mod:7 分发器 comp=33 口径下限沿守卫链委托化拆解（keys/mod.rs=87）；main.rs 适配器外壳历轮登记不硬凑。行覆盖 40676/43293=94.0%（bin 口径，本轮实测，nightly c7d5ba59d-20261120 同构建可比）。
- **CPD 残留沿袭**：全仓 222 组（min-tokens 70，PMD 7.29.0；产品侧 18 组集中在 bt/engine/mod.rs、app/dialogs.rs、ui/dialog.rs、mouse.rs 等历轮旧账文件）——总数与 architect 轮精确恒等，非本轮批次。
- **12-reload-config 产品面执行层欠账**：软变异词汇面 9 survived 为覆盖声明（非等价论证），压缩路径 = QA 轮 reload 场景产品面 handler 可执行化（QA-RC-01–14 沿袭面）。
- ~~bb test 11 测 0F/3E~~：**本轮销项**——根因 = bb 不在 PATH 时 mutation_test.clj worker 测试面 Exec ENOENT；PATH 注入后 11 测 51 断言 0F/0E。

### 实现定义值登记
- **D81①–⑤ / D82①–③（v1.57 落定版）**：固定文案 toast `✓ 配置已重新加载`（常量 TOAST_RELOADED 单点）；失败 toast `⚠ 配置热加载失败：<明细>`；无配置文件臂「无配置文件」；缺失文件臂 = 默认配置整体热加载且不重生成模板；引用消失 = Task.proxy 改写 Direct + save_registry 落库 + 双对话框下拉重建 + 运行任务端点比较发重配命令。
- **运行端点判定口径**：运行端点 = 当前 cfg.resolve_proxy(task.proxy)（三入口同源不变式）；比较 = ProxyEndpoint 全等（url/kind/username/password）；运行态判定 = Downloading | Seeding；重配命令 = Reconfigure/ReconfigureBt 仅换端点。
- **ApplyGlobals 命令面**：`Cmd::ApplyGlobals { max_download_speed, max_upload_speed, throttle_burst_secs, throttle_sleep_ms }` 单命令原子下发。
- **CLI 覆盖登记**：App.cli_max_speed_override + set_cli_max_speed_override()；每次热加载重应用，CLI > config 贯穿运行期。
- **进度条行尾恒空**：行 = 2 列前导 + bar(width−4)，行尾恒无内容。

## 二、当前产出情况

### 本会话产物清单（hardender 变异加固轮）
1. **语言变异收敛（architect 批次 = keys/{mod,overlays,conns} + app/{mod,stats,restore}，307 变异点）**：发现轮分块前台 + 三角复判（窄面发现/补面定真伪）→ 真缺口 56 全部由新增加固测试杀灭；等价体 12 个书面论证（清单见交接四要素④）；timeout 击杀 1（app/mod.rs:508 alloc_task_id 比较符翻转死循环，窗/裸倍率 11 ≥3 判真挂起）；unviable 2。**非等价存活率 = 0**；--list 计数恒等证明（87/46/27/68/59/20，追加式测试零位移）。
2. **加固测试 +31（bin 1137 → 1165P）**：`hardening_restore`（resumable 翻转/越界零块分类/BT 恰等边界陈旧片过滤）、`stats_hardening`（全局限速仅和下载中/display 总数活跃口径/conn_is_active 相位区分/峰值缩放/clear retain 反演）、`overlay_hardening`（pieces/help Up 正值回滚）、`keys_hardening`（Ctrl+J 守卫链 follow 重挂锚 + cmd 关闭/Ctrl+Enter 确认关窗/Space 暂停/d 删除框/c 清已完成/g 焦点守卫/u·j 方向 toast/List Up·PageUp·End 三向导航/conn 滚动跟随中间态×2/tasklist 滚动跟随中间态）、`hardening_app_core`（used_slots 三态计数/queue_pos 顺位/overlay_open 单标志析取/set_filter 越界拒/set_toast 未来窗/CLI 覆盖双落点/push_hist 缩放与 180 顶盖）。
3. **软 Gherkin 变异（--level soft，全 21 feature）**：20 个 stamped feature 经符号链接桥接（/home/z/swarm-ezr2 → 持久区，manifest feature_path 逐字一致）全差分复用，Σ=1124=1107 closed+17 FILTERED 精确守恒（manifest 复算 survived=0）；新 feature 12-reload-config 词汇面 adapter 执行 15 突变体 = 6 killed（限速字符串封闭形态域外）+ 9 survived（域内词汇声明，回补触发见④）。
4. **CRAP 直采（受限近似口径）**：llvm-cov 插桩 → 直跑测试二进制三块采 profraw → merge/export → 段级过滤 → crap_calc.py。ACTIONABLE_COUNT=100；双锚恒等：mouse.rs:178=77.09、trackpool.rs:413=186.96；行覆盖 40676/43293=94.0%（登记口径，LF 随 nightly 微版本漂移不可跨构建直比）。
5. **DRY（PMD 7.29.0 CPD min-tokens 70）**：全仓 222 组 = architect 轮基线 222 **精确恒等零漂移**；产品侧 18 组全在历轮旧账文件；本轮触及文件零产品克隆、新增测试 +641 行零新增克隆组。
6. **notes 沉淀**：`packs/_common/notes/mutation-hardening.md` 无改动（既有模式库已覆盖）；`project/notes/testing.md`（cargo-mutants 仓源/CARGO_TEST_ARGS 参数位与 `-- --` 多滤选形态/基线闸门 timeout 假死/三角复判协议/--iterate 用 -E 控预算/追加式 --list 恒等收敛捷径/等价新三类）+ `project/notes/environment.md`（gh 前缀探针 403≠不可用/bb PATH 假受限销项/网关取消子进程存活复证/持久区符号链接桥）+ INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd 见行注） |
|---|---|---|
| 基线 bin（改动前对账） | 1137P/0F（192.04s） | `cargo test --bin ezr`（cwd=project/ezr） |
| arch_check | 19/19 全过 | `bash scripts/arch_check.sh` |
| fmt / clippy（ezr + demo，改动后复验） | 双项目 fmt --check 干净 + clippy --all-targets 零警告 | `cargo fmt --check` / `cargo clippy --all-targets`（demo 用 `+nightly`） |
| 加固后 bin 全量 | 1165P/0F（194.16s，= 1137 + 31 新增） | `cargo test --bin ezr` |
| 加固测试独立命令 | 28→31 全绿（含 Ctrl+Enter 场景补测后 31） | `cargo test --bin ezr -- -- hardening_restore:: stats_hardening:: overlay_hardening:: keys_hardening:: hardening_app_core::` |
| 集成 13 套件 | 8992P/0F（8768 + 28×8 套件挂载差值）：g_appcore 927(+28)/g_slots 254(0)/g_proxy 16(0)/g_engine 924(+28)/g_supervisor 897(+28)/g_throttle_err 549(0)/g_main 2(0)/g_v147 911(+28)/hardening 262(0)/engine_backfill 1047(+28)/ui_backfill 1077(+28)/v115 1070(+28)/mutation 1056(+28) | `cargo test --test hardening_<名>`（串行前台，日志 .work/tmp/f-*.log） |
| demo | 61P/0F | `cargo test`（cwd=ezr-demo） |
| 属性独立命令 | 92P/0F | `cargo test --bin ezr property_tests::` |
| 语言变异（批次 307 点） | 杀灭 255 + 等价 12 + timeout 1 + unviable 2；非等价存活 = 0；验证轮全并面 rc=0 | `.work/tmp/aps-soft` 外的 mutants-out* 账目 + 复现命令见交接四要素② |
| 软 Gherkin | Σ=1124 守恒（1107+17）+ reload 15 执行（6 killed/9 survived 词汇声明） | 见交接四要素② |
| CRAP | ACTIONABLE_COUNT=100；mouse.rs:178=77.09 恒等；trackpool.rs:413=186.96 | 见交接四要素② |
| CPD | 222 组恒等（min-tokens 70） | 见交接四要素② |

### 对账口径
- **bin 侧**：1137 → 1165（+31 = 本轮加固测试，纯追加）；产品代码纯追加零改动（pristine cmp deleted=0；git diff 641 insertions 0 deletions）。
- **集成侧**：8768 → 8992（+224 = 31 新测 × 各套件 include 树挂载，8 套件各 +28：凡 include keys/mod+overlays+stats+restore+app/mod 的套件差值恒等 +28；未挂载 5 套件恒等）。
- **demo 侧**：61 恒等（独立子项目零接触）。
- **变异账目**：307 = 255 killed + 12 equivalent + 1 timeout-killed + 2 unviable + 37 既有测试面外击杀（伪幸存，补面复判转 killed）。批次零产品改动证明：--list 恒等 + pristine cmp。
- **软 Gherkin 账目**：1124（stamped 守恒）+ 15（reload 执行）= 1139；survived 仅 reload 词汇面 9（覆盖声明，非等价类）。

### 待办与待批（未决项）
- 无待批项——本轮无决策闸门。
- 待办（QA 轮）：12-reload-config 产品面 handler 可执行化（QA-RC-01–14 沿袭欠账，软变异词汇面 9 survived 的压缩路径）。

### 移交建议
- **下一步运行 `six-pack/QA`**：承接 QA-RC-01–14 端到端 UI 层验证与 suite runner 可执行化（沿袭口径），顺带压缩 12-reload-config 软变异词汇面幸存体。
- 全仓 >100 变异点存量（mouse/textfield/scrollbar/multiline/pieceio/peer_id/bt_dialog/config/tracker/dialog_keys-routing/bt_tree/dialogs）沿历轮受限项，如需拆分由后续 architect 轮裁决后按本轮 `--file` 分块 + 三角复判 + 追加式测试同款流程跑。
- 归档：`bin/swarm complete`（产物已按操作者指令复制 /home/z/my-project/download/ 交付）。

## 交接四要素（mutation-hardening.md §6）

### ①本地验证命令清单（按序，cwd=project/ezr 除非另注）
1. `bash scripts/arch_check.sh` → 19/19
2. `cargo fmt --check` + `cargo clippy --all-targets`（ezr）；demo：`cargo +nightly fmt --check` + `cargo +nightly clippy --all-targets`（cwd=ezr-demo）
3. `cargo test --bin ezr` → 1165P/0F
4. 13 集成套件逐个 `cargo test --test hardening_<名>`（名清单见验证证据行）→ 合计 8992P/0F
5. `cargo test`（cwd=ezr-demo）→ 61P/0F
6. 属性独立：`cargo test --bin ezr property_tests::` → 92P/0F
7. 加固套件独立入口：`cargo test --bin ezr -- -- hardening_restore:: stats_hardening:: overlay_hardening:: keys_hardening:: hardening_app_core::` → 31P/0F
8. 语言变异（复现）：cd scratch（重建法见②）；`cargo mutants --in-place --iterate --cargo-arg=--bin --cargo-arg=ezr --file <目标文件> --timeout 180 -o <账目目录> -- -- <滤选面>`；批次六文件与面：keys/mod.rs+overlays+conns → `app::keys:: app::mouse app::dialogs app::cmd app::reload app::paste app::dialog_keys app::interaction_tests`（keys/mod 验证轮加 app::）；stats.rs/app/mod.rs → `app:: ui:: cli_parse_tests`（app/mod 亦含 complement 补面）；restore.rs → `app::tests::restore cli_parse_tests::restore app::restore:: app::tick_tests::make_app_same_tag`。等价体/伪幸存已判线以 `-E` 排除（清单见④与账目）。
9. 软 Gherkin：cd .tools/Acceptance-Pipeline-Specification；`PATH=<swarm>/.tools/bin:$PATH bb gherkin-mutator --feature /home/z/swarm-ezr2/project/features/<f>.feature --work-dir <W>/<stem> --generated-dir <G>/<stem> --level soft --runner-worker "python3 <W>/adapter.py" --equivalence-filter /home/z/swarm-ezr2/project/qa/equivalence-filter.json --status-interval 30s`；每 feature 先 `bb gherkin-parser <feature> <W>/<stem>.base.json`。**必须经 /home/z/swarm-ezr2 符号链接路径调用**（manifest feature_path 钉死；真实树 = /home/z/my-project/swarm-ezr2）。
10. CRAP：`RUSTFLAGS="-C instrument-coverage" CARGO_TARGET_DIR=<abs>/.work/tmp/cov-target cargo test --bin ezr --no-run` → 直跑测试二进制（分块、LLVM_PROFILE_FILE=<dir>/%p-%m.profraw，模块并集覆盖全量）→ `llvm-profdata merge -sparse` → `llvm-cov export <bin> --instr-profile --format=lcov` → `awk '/^SF:/{keep=index($0,"/project/ezr/src/")>0} keep'` 段级过滤 → `python3 project/qa/runners/crap_calc.py <lcov> <src_root>`（两参）。
11. CPD：`<tools>/pmd-bin-7.29.0/bin/pmd cpd --dir src --language rust --minimum-tokens 70`（EXIT=4=有克隆属正常）。

### ②会话运行时产物位置与复现方法
- 全部运行时产物在 `.work/tmp/`（不进归档）：`aps-soft/`（软 Gherkin：adapter.py 词汇面 runner-worker + 各 feature work/gen 目录 + sum_manifest.py Σ 复算脚本）、`pristine/`（六文件字节级备份，零改动证明基准）、`mutants-out*`（cargo-mutants 账目：mutants-out=restore/conns/overlays/keys、mutants-out-stats、mutants-out-appmod，含 outcomes.json/caught.txt/missed.txt）、`f-*.log`（基线与集成套件日志）、`cpd-report.txt`、`crap-report.txt`、`ezr-src.lcov`。
- scratch（变异执行树）已删（磁盘时序纪律）；复现语言变异 = 按①⑧的命令重建：`cp -a project/ezr <scratch> && rm -rf <scratch>/target <scratch>/tests`（tests/ 必删——mutants Build 阶段并行编译全部集成目标在 4G 沙箱 OOM，历轮实录本轮复现）+ `~/.cargo/config.toml` 装镜像 + `PATH=<swarm>/.tools/bin:$HOME/.cargo/bin`。
- 复现软 Gherkin：见①⑨；adapter 依赖 mutator 的 job 协议 = `feature_json` 是**文件路径**（变异 IR），base IR 固定在 `<work-dir>/../../base/feature.json`。
- 会话结束即清 .work/tmp（complete 自带清理）；上述命令即可再生。

### ③feature 注释块与差分复用注意事项
- 20 个 stamped feature 的 stamp/manifest 经本轮软跑刷新（tested_at 2026-11-25T03:26Z；feature_path = /home/z/swarm-ezr2/... 符号链接路径）。**后续调用必须用同一符号链接路径**（或先删 manifest 块强制全量重跑——删除 = 丢弃 1107 closed 复用账，重跑需 APS 执行层）。
- equivalence-filter 未变更（17 条目、filter-hash 一致）→ 过滤账随复用守恒；若编辑 filter 文件 → 相关场景如实重执行（fail-loud 设计）。
- 12-reload-config 的 stamp 只落了 2 复用场景（5 mutations）；其余 12 场景每次重跑会重执行（词汇面 adapter 路径，秒级）——**非账目缺陷**：执行语义全在 adapter（词汇域校验），verdict 由运行日志与账目目录承载。
- mutation 侧：本轮六文件 `cargo mutants --list` 计数恒等证明（测试纯追加）→ 下游若继续对这些文件跑变异，--iterate 账目仍有效；测试若再有增删（非追加）须按 mutation-hardening.md §2 整体重建。

### ④等价突变体清单及失效条件（回补触发）
| # | 位置 | 突变 | 等价论证（模式） | 失效条件（回补触发） |
|---|---|---|---|---|
| 1 | keys/conns.rs:28:16 | `<`→`<=` | 等值写：new==conns_scroll 时写同值无可观察面（卫语句先行子类） | conns_scroll 写入引入副作用（日志/dirty 标记）或新增等值观察消费方 |
| 2 | keys/conns.rs:53:56 | `-`→`+` | 死钳位：早退卫语句（conns_sel >= rows.len() 已 return）保证 min(conns_sel, len±1)≡conns_sel | 移除/弱化 46-49 行早退卫语句，使钳位承载边界职责 |
| 3 | keys/conns.rs:53:56 | `-`→`/` | 同上（len/1=len，钳位仍空转） | 同上 |
| 4 | keys/mod.rs:64:74 | 删 `-`（cycle_filter(-1)→(1)） | mod-2 环同余：FILTERS.len()==2 时 ±1 rem_euclid(2) 恒等 | **FILTERS 增至 >2 项**（第三个页签即触发，方向可观察） |
| 5 | keys/mod.rs:153:41 | `+`→`-` | 同上（delta 取反在 n=2 环上恒同） | 同上 |
| 6 | keys/mod.rs:159:24 | `>`→`==` | 等值写：scroll==selected 分支写同值 | scroll 写入引入副作用或消费方观察中间等值写 |
| 7 | keys/mod.rs:159:24 | `>`→`>=` | 同上 | 同上 |
| 8 | keys/mod.rs:101:26 | `>`→`>=`（Home/Conns） | 双层卫语句冗余：外层 n>0 与 move_conn_sel 内层 n==0 早退嵌套，翻转后 move_conn_sel(0) 内层守卫空转 | move_conn_sel 的 n==0 内层守卫移除或改动 |
| 9 | keys/mod.rs:110:26 | `>`→`>=`（End/Conns） | 同上（同构嵌套） | 同上 |
| 10 | stats.rs:35:40 | `>`→`>=`（global_ul_speed） | 零贡献过滤放宽：upload_speed==0 任务入和加 0.0，总和恒等 | 求和引入非纯语义（NaN 传播、副作用计数）或 upload_speed 允许负值且消费方区分 |
| 11 | stats.rs:72:9×2 + 72:12 | active_conn_count 体替换/删 ! | 死函数：#[allow(dead_code)] 全仓零调用点 | 任何未来调用点出现（届时该函数入正常变异面） |
| 12 | app/restore.rs:19:45 | `&&`→`||` | 死守卫操作数：restore_tasks() 重建 Task 恒 done_blocks=Vec::new()（registry.rs 快照无断点字段），左操作数恒 true → 与 || 同值 | TaskSnapshot 持久化 done_blocks/chunk_done（注册表格式演进）；或 App::restore 出现携带非空 done_blocks 的新调用方 |

附：timeout 击杀 1（app/mod.rs:508:82 alloc_task_id `==`→`!=` 死循环，窗 180s/裸 16s 倍率 11≥3 判真挂起，无需复跑）；伪幸存 37（既有测试他面击杀，账目 missed.txt 记录 + 补面复判转 killed，明细见 mutants-out* 各 missed.txt 与本轮会话记录）。

By hardender.
