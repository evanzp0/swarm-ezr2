# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（reload-clean 后边界收口轮）
> 路径：…six-pack/cleaner（上轮 reload-hotload-impl + progressbar-tail 清理轮）⇒ **six-pack/architect（本轮：keys/mod 双 >100 变异点模块边界收口 + 属性测试补充 + arch_check 规则 19；完成后 complete 归档）**
> 会话：six-pack-architect-20261124-arch-boundaries（上游交接 = 上轮 handoff）
> 环境：nightly 1.102.0（rust-toolchain.toml 钉版；本轮 TUNA dist 镜像前台装链，rsproxy dist 清单本轮 404 沿历轮漂移观察）+ cargo registry rsproxy sparse + cargo-mutants 27.1.0（.tools/bin，gh-proxy release 装机）。本轮沙箱为全新 clone：bb / APS / PMD 未重建（架构轮不需要；QA 轮按 notes/environment.md 口径重建，APS 等价扩展 patch 随 qa/ 流转）。
> 受限项：chat 选择题面板不可用（IM 通道）沿袭——本轮无决策闸门，不受影响。其余无新增受限项。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.61**）；phase-01/02 为现行详述（03/04 计划登记稿）。ezr 不做旧版本兼容沿袭（操作者钦定沿 v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；历轮已同构同步（v1.62）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 20 份 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径） |
| coder + cleaner 轮（20261124，上轮） | Ctrl+R 热加载全链路 + 进度条行尾恒空修复 + 触碰面清理纯追加 16 测 + reload 夹具提取 + demo fmt 销项 | 已交付 | 上轮账：bin 1135P/0F + 集成 13 套件 8768P/0F + demo 61P/0F + fmt/clippy 零警告 + APS 42/42 |
| **architect 边界收口轮（20261124，本轮）** | keys.rs 目录化三文件 + app/mod.rs 拆出 stats.rs/restore.rs + 属性测试 +2 + arch_check 规则 19 + notes 两处沉淀 | 已交付 | bin 1137P/0F + 集成 13 套件 8768P/0F + demo 61P/0F + fmt/clippy 双项目零警告 + arch_check 19/19 + 属性独立命令 92P/0F + 变异点复测全部 <100 |

### 遗留受限项（当前有效）
- **全仓 >100 变异点存量（本轮收口后）**：mouse.rs=223、textfield.rs=226、scrollbar.rs=223、multiline.rs=204、bt/engine/pieceio.rs=209、bt/peer_id.rs=181、ui/bt_dialog.rs=179、model/config.rs=161、bt/tracker.rs=159、app/dialog_keys/routing.rs=145、app/bt_tree.rs=111、app/dialogs.rs=101——历轮登记残留沿袭（非本轮批次，hardender 全量变异前如需拆分按 rust.md 口径由后续 architect 轮裁决）。
- **CRAP 残余沿袭**：keys.rs:7 分发器 comp=33 口径下限随本轮守卫链委托化拆解（on_key 缩至守卫链 + 主键表，keys/mod.rs=87）；main.rs 适配器外壳历轮登记不硬凑；全仓 ACTIONABLE 99 项沿袭。行覆盖 95.0%（bin 口径，cleaner 轮实测）沿袭。
- **CPD 残留沿袭**：全仓 222 组（min-tokens 70；bt/engine/mod.rs 362 处等）——非本轮批次。
- **bb test 11 测 0F/3E 沿袭**：mutation_test.clj worker 测试面受限（mutator 非 QA/架构轮范围）。
- QA-RC-01–14 的端到端 UI 层验证与 suite runner 可执行化归 six-pack/QA 轮（沿袭）。

### 实现定义值登记
- **D81①–⑤ / D82①–③（v1.57 落定版）**：固定文案 toast `✓ 配置已重新加载`（常量 TOAST_RELOADED 单点）；失败 toast `⚠ 配置热加载失败：<明细>`；无配置文件臂「无配置文件」；缺失文件臂 = 默认配置整体热加载且不重生成模板；引用消失 = Task.proxy 改写 Direct + save_registry 落库 + 双对话框下拉重建 + 运行任务端点比较发重配命令。
- **运行端点判定口径**：运行端点 = 当前 cfg.resolve_proxy(task.proxy)（三入口同源不变式）；比较 = ProxyEndpoint 全等（url/kind/username/password）；运行态判定 = Downloading | Seeding；重配命令 = Reconfigure/ReconfigureBt 仅换端点。
- **ApplyGlobals 命令面**：`Cmd::ApplyGlobals { max_download_speed, max_upload_speed, throttle_burst_secs, throttle_sleep_ms }` 单命令原子下发。
- **CLI 覆盖登记**：App.cli_max_speed_override + set_cli_max_speed_override()；每次热加载重应用，CLI > config 贯穿运行期。
- **进度条行尾恒空**：行 = 2 列前导 + bar(width−4)，行尾恒无内容；校验/后期状态由行 1 状态徽标承载。

## 二、当前产出情况

### 本会话产物清单（architect 边界收口轮）
1. **keys.rs 目录化拆分（保持行为，155 → 87/46/27）**：`src/app/keys/mod.rs`（on_key 守卫链 + 主键表 + 任务列表导航 + 既有 7 测试模块原位保留）、`keys/overlays.rs`（浮层 match 体委托化 on_val/on_about/on_pieces/on_trackers/on_help_key，对齐既有 on_cmd/on_err/on_dialog 家族）、`keys/conns.rs`（conn_count/move_conn_sel/disconnect_conn 并发面板导航单置）。
2. **app/mod.rs 拆分（147 → 68/59/20）**：`src/app/stats.rs`（13 个统计读数器 + clear_conn_stats/clear_bt_conn_stats 簿记，59 点）、`src/app/restore.rs`（会话恢复适配器 restore()，pub(super)，20 点）——mod.rs 收口为 App 结构 + 构造 + 选择/过滤/toast + 小胶水（68 点）。
3. **可见性修正**：clear_conn_stats/clear_bt_conn_stats 迁入 stats.rs 后 pub(super) 语义收窄（原 app/mod.rs 内 = crate 可见），#[path] 套件 tests/hardening_engine_backfill.rs 直呼 E0624——改 pub(crate)（bin 内两者等价，套件内恢复原可见面）。
4. **属性测试 +2（90 → 92，bin 1135 → 1137）**：`prop_bt_partial_block_round_trip`（BtPartialBlock 编码→解码→JSON 序列化三重往返，begin/data 全域）；`prop_bt_selected_bytes_invariants`（selected_bytes_of_pieces 上界 ≤ Σ选中字节 / 逐片累加次序无关 / 全片覆盖守恒 = Σ选中字节 / 越界片号零贡献）。
5. **arch_check.sh 规则 19**：会话恢复适配器单源——Registry::load 产品区调用点只允许 src/app/restore.rs（产品区口径 = 各文件首个 #[cfg(test)] 之前，对齐规则 14；阳性对照：stats.rs 注入 dup 臂 → FAIL、移除 → 绿）。
6. **notes 沉淀**：`packs/_common/notes/rust.md`（proptest! 属性 fn 须显式 #[test]：宏只重发 $(#[$meta])*，漏写即编译过但不注册；mutants --list 行号分桶法）+ `project/notes/architecture.md`（#[path] 套件目录化免疫但 pub(super) 语义随迁移位置改变，跨根消费改 pub(crate) 判据；本轮实测分桶拆分口径）+ INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd = /home/z/swarm-ezr2/project/ezr） |
|---|---|---|
| bin 全量（含属性 92P） | 1137P/0F（192s） | `cargo test --bin ezr` |
| 集成套件 | 13 套件 8768P/0F（与上轮账恒等）：g_appcore 899 / g_slots 254 / g_proxy 16 / g_engine 896 / g_supervisor 869 / g_throttle_err 549 / g_main 2 / g_v147 883 / hardening 262 / engine_backfill 1019 / ui_backfill 1049 / v115 1042 / mutation 1028 | `cargo test --test <名>`（串行前台，日志 .work/tmp/f-*.log） |
| demo 全量 | 61P/0F | `cargo test`（cwd = /home/z/swarm-ezr2/ezr-demo） |
| fmt / clippy | 双项目 fmt --check 干净 + clippy --all-targets 零警告 | `cargo fmt --check` / `cargo clippy --all-targets` |
| 属性测试独立命令 | 92P/0F（64s） | `cargo test --bin ezr property_tests::` |
| 自动化架构检查 | 19/19 规则通过（规则 19 本轮新增） | `bash scripts/arch_check.sh` |
| 变异点复测（scan 模式） | keys 155→87+46+27；app/mod 147→68+59+20；拆分文件全部 <100；全仓 6301→6306 | `cargo mutants --list`（.tools/bin 入 PATH） |

### 对账口径
- **bin 侧**：1135 → 1137（+2 = 本轮属性测试，纯追加；产品代码为纯移动拆分零行为改动）。
- **集成侧**：8768 → 8768 恒等——属性测试属 bin 侧模块（main.rs 挂载，#[path] 套件不含）；clear_* 可见性修正恢复套件原可见面（engine_backfill 1019 复绿）。
- **变异点侧**：keys.rs 155 → keys/{mod,overlays,conns} 87+46+27 = 160（+5 来自守卫链委托化形态变化）；app/mod.rs 147 → 68 + stats 59 + restore 20 = 147 精确守恒；全仓 6301 → 6306（净 +5）。
- **行为变化面**：零——拆分为纯移动（git diff --stat 对账：mod.rs 仅导入调整 + 两段删除；keys.rs 重写为目录 + 机械提取测试段逐字节回接）；全量 bin/集成/demo 三面绿 + fmt/clippy 双清背书。
- **产品源文件改动面**：keys.rs → keys/ 目录（代码移动）、mod.rs 两段迁出 + 导入调整、stats.rs/restore.rs 新文件（内容 = 机械提取段 + 导入头 + restore pub(super) 一处可见性 token）、stats.rs 两 clear fn pub(crate)、property_tests.rs 追加 2 属性、arch_check.sh 追加规则 19。

### 待办与待批（未决项）
- 无待批项——本轮无决策闸门，验证全绿。

### 移交建议
- **下一步运行 `six-pack/QA`**：承接 QA-RC-01–14 端到端 UI 层验证与 suite runner 可执行化（cleaner SKILL 移交链口）。
- 或运行 `six-pack/hardender`（可选节点，显式 begin）：keys/mod 双 >100 登记面已收口（拆分后全部 <100），全量变异无需再收边界；hardender 按 `--file` 分块跑变异即可兑现拆分收益（rust.md 口径）。
- 全仓 >100 变异点存量（mouse/textfield/scrollbar/multiline/pieceio/peer_id/bt_dialog/config/tracker/dialog_keys-routing/bt_tree/dialogs）沿历轮受限项，如需拆分由后续 architect 轮按同款实测分桶口径裁决。
- 归档：`bin/swarm complete`（产物已按操作者指令复制 /home/z/my-project/download/ 交付）。

By architect.
