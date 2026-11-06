# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（本轮已完成）→ 建议下一节点 QA 轮或收尾
> 会话：ezr-bug25-cleanup（2026-11-05，cleaner 轮：bug25 UI 修正轮 coder 产出的保持行为
> 清理 + ezr-demo clippy 基线清零 + App.windows 死状态裁决）
> 上游交接：project/handoff.md（coder 轮交接 = bug25 三项实现 + ezr-demo 同构同步 +
> cleaner 候选清单）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器。需求权威 `project/mission.md`
  （分期模式 v1.42）+ `project/mission/phase-01…04.md`。
- 本轮为 **bug25-cleanup cleaner 轮**：保持行为清理（死代码/命名/重复/测试可读性）+
  覆盖率/CRAP/DRY/变异点四项度量 + 经验沉淀 + 交接重写。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier/coder（上轮） | bug25 三项规格 + 实现 + demo 同构 | 已交付 | 沿袭全绿 |
| **cleaner（本轮）① 死状态清理** | **App.windows 死状态裁决=删除**：`app/mod.rs` 字段+init、`engine/bt.rs` on_progress 喂窗块+2 清窗、`engine/evt.rs` 6 处喂窗/清窗、`app/tasks.rs` 4 处清窗、`app/dialogs.rs` remove_task 清窗；测试改判 8 处（含 3 处改名去窗口语义：`progress_ghost_is_noop_without_touching_tasks`/`progress_updates_task_view`/`paused_done_snapshots_breakpoint`）+ 删 2 个窗口守护测试（`bt_progress_does_not_feed_speed_window`/`bt_progress_while_seeding_does_not_recreate_speed_window`）；SpeedWindow/WINDOW import 按残余使用保留（conn_windows 活跃） | 已交付 | **920→918P 精确对账，全绿**；六挂载套件各 -2 对账吻合 |
| **cleaner（本轮）② demo clippy 基线清零** | `ezr-demo/` 9 处既有 clippy 警告全清：机械修 4（cmd.rs redundant_guard `Some(("", _))`、dialogs.rs collapsible_if 合并、mouse.rs collapsible_match guard 化、multiline.rs manual_div_ceil×2）；同构 allow 3（multiline.rs to_first/to_last 单点 `#[allow(clippy::wrong_self_convention)]` 与 ezr 同款、main.rs `#![allow(clippy::too_many_arguments)]` 与 ezr main.rs 同口径） | 已交付 | **clippy 0 警告（9→0）+ 44P/0F + fmt 干净** |
| **cleaner（本轮）③ 变更面审查与 DRY 收敛** | ①conns.rs HTTP 序号格硬编码 `4` → `HTTP_COLW[0]` 常量化（coder 登记遗漏点）；②fmt_speed 补 `-0.0` 归一意图注释；③testutil.rs 删冗余 `#[cfg(test)]`；④**DRY：提取 `App::resolve_confirm_dir`**（dialogs.rs+bt_dialog.rs 两处 dir 默认解析 9 行重复单点化，判重口径与目录口径耦合防漂移） | 已交付 | 925P/0F + clippy 0 + fmt 净 |
| **cleaner（本轮）④ 覆盖率补测** | 纯追加 7 个测试：`bt_dialog.rs` 5 个（add/modify 对话框 enter/char/activate 入口路由覆盖——原 bins+集成双零覆盖）；`app/mod.rs` 2 个（`finish_add_duplicate_opens_error_window` 覆盖 bug25 HTTP 判重臂——原双零覆盖 + 不同目录不判重对照） | 已交付 | 923→925P；bt_dialog 函数 114/114、dialogs 62/62 全覆 |
| **cleaner（本轮）⑤ 度量与经验** | llvm-cov 全量汇总 + CRAP 受限近似 + jscpd DRY 扫描 + cargo-mutants 变异点计数（scan 模式）；notes 三文件 + INDEX 沉淀（cargo 镜像 404 挂死陷阱/clippy metadata 噪音/挂载套件 ±N 对账/死状态裁决模式） | 已交付 | 见验证证据表 |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（历轮登记，全部不变）：死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；
  QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态观察项；
  leech 判据无 e2e 自动化；bad_piece 归属口径（D67 允许）；QA-TD-23 与
  QA-UC-07/17/18 环境预存 FAIL 待 QA 轮复验；`periodic_flush_writes_sidecar_mid_download`
  HTTP 滴流 flake 单跑绿沿例；同 IP 拓扑残余启动段；leech 封禁按裸 IP 连坐面；
  ezr 不发 KeepAlive。
- **本轮沿袭 flake 复测**：`engine_conns_quota_exact_and_unique_ids`（g_v147）本轮
  4 次全量 2 次 FAIL（701P/1F、708P/1F）+ 1 次全量绿 + 单跑恒绿（3 连）——间歇态
  比上轮频发但仍符合单跑绿口径，与改动面（UI 展示层/对话框报错/App.windows 死状态/
  resolve_confirm_dir 提取）零接触面（2 核调度压力敏感），沿袭登记不阻断。
- **本轮新增受限项（CRAP 登记口径）**：`dlg_finish_add` / `dlg_confirm_add_bt`
  函数级 comp≈14（~100 行确认主链，10+ 判定分支）——bug25 仅触碰判重臂 2 行（已
  补测覆盖），整体拆分超保持行为清理边界，登记移交。
- **本轮新增受限项（变异点计数）**：bug25 ezr 触碰 7 文件变异点合计 **437**
  （text 90/conns 52/task_lines 70/ui-mod 24/testutil 0/dialogs 93/bt_dialog 108；
  全库 5710）；demo 触碰 7 文件合计 **1026**（err 20/ui-mod 199/conns 38/app-mod 72/
  keys 162/mouse 228/dialogs 307；全库 3724）。scan 模式仅计数未跑变异；>100 按纪律
  登记移交（变异实跑属 QA/mutants 轮预算）。
- ezr-demo 9 处 clippy 基线 **本轮已清零**（该候选关闭）。

### 实现定义值登记（累计口径，本轮落地处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| 面板层速度格式化 | B/s `N`、KiB/s `N`、MiB/s `N.N`、GiB/s `N.NN`（1024 进位；0 与 -0.0 归一） | ui/text.rs `fmt_speed`（ezr）+ demo 同构 |
| **判重/目录共用目录解析（本轮落地）** | **空输入回退 cfg.download_dir → default_download_dir → normalize_save_dir**（判重 t.save_dir == dir 与任务构造共用口径） | **app/dialogs.rs `resolve_confirm_dir`**（ezr，pub(super)） |
| BT 并发列宽 | 地址(18)/标志(4)/速度(12)×2/累计(10)×2/客户端(34)/协议(4)，内容宽 111 | ui/conns.rs `BT_COLW`/`BT_CONTENT_W` |
| HTTP 并发列宽 | 序号(4)/速度(12)/累计(10)，内容宽 28 | ui/conns.rs `HTTP_COLW`/`HTTP_CONTENT_W` |
| 客户端收缩基线 | 77（下限 26） | ui/conns.rs `BT_CLIENT_SHRINK_BASE` |
| 并发表头对齐 | 速度×2/累计×2（HTTP 速度/累计）pad_left 右对齐 | ui/conns.rs `head_cell_r` |
| 重复任务错误窗口 | `open_error_window("该链接的任务已存在")`（HTTP = dlg_finish_add 判重、BT = dlg_confirm_add_bt 判重；对话框保持打开） | app/dialogs.rs / app/bt_dialog.rs + demo err.rs |
| 其余历轮登记值 | trackers 94×22/限速键口径/标志列结构等 | 不变 |

## 二、当前产出情况

### 本会话产物清单
- **ezr 产品端（清理手术 9 文件）**：`src/app/mod.rs`（windows 字段/init 删除 + 判重
  补测 2）、`src/app/engine/bt.rs`（喂窗块+清窗+2 守护测试删除）、`src/app/engine/evt.rs`
  （6 处窗口操作删除）、`src/app/tasks.rs`（4 处清窗删除）、`src/app/dialogs.rs`
  （remove_task 清窗 + resolve_confirm_dir 提取）、`src/app/bt_dialog.rs`（dir 解析
  替换 + 5 个入口路由测试追加）、`src/ui/conns.rs`（HTTP_COLW[0] 常量化）、
  `src/ui/text.rs`（-0.0 注释）、`src/app/testutil.rs`（冗余 cfg 删除）。
- **ezr-demo（基线清零 6 文件）**：`src/app/cmd.rs`、`src/app/dialogs.rs`、
  `src/app/mouse.rs`、`src/multiline.rs`、`src/main.rs`（机械修 + allow 同构）。
- **project/notes/**：environment.md（cargo 镜像 404 挂死 + clippy metadata 噪音）、
  testing.md（挂载套件 ±N 对账 + quota flake 间歇态）、architecture.md（死状态裁决 +
  判重口径单点化）+ INDEX.md 三行同步。
- **.work/logs/**：基线复核/手术验证/度量收集日志 44 份（clippy17 轮噪音链、lcov
  两版、mutants 两库清单、fetch 尝试链）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr bins 单元测试 | **925P/0F**（920 基线 −2 守护测试 +7 补测） | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bins` |
| ezr 集成测试（12 组） | **全绿**（142+828+726+721+2+16+202+695+451+708（flake 全量复跑绿）+858+851） | `CARGO_INCREMENTAL=0 cargo test --test <套件名>` 逐组 |
| ezr clippy / fmt | **0 警告 / 干净** | `cargo clippy --all-targets` / `cargo fmt --check` |
| ezr-demo 测试 | **44P/0F** | `cd ezr-demo && cargo test` |
| ezr-demo clippy / fmt | **0 警告（9 处基线清零）/ 干净** | `cargo clippy --all-targets` / `cargo fmt --check` |
| llvm-cov 全量 | **TOTAL 行覆盖 93.94%**（上轮视角 93.74% → 补测后 +0.2pp）；bug25 触碰文件：text 97.9%/conns 97.9%/task_lines 99.5%/ui-mod 99.4%/testutil 100%/dialogs 96.4%（函数 62/62）/**bt_dialog 94.5%（函数 114/114）** | `cargo llvm-cov --bins --lcov` + `cargo llvm-cov report --summary-only` |
| CRAP 受限近似 | fmt_speed=4、head_cell_r=1、seed_bt_conn_speeds=2、判重臂（补测后）✓；dlg_finish_add/dlg_confirm_add_bt comp≈14 登记移交 | 见 architecture.md cleaner 轮 |
| DRY（jscpd min-tokens 40） | 7 文件 68 clones（10.48%）；生产重复 2 处已收敛（resolve_confirm_dir）；其余登记 | `jscpd <files> --min-tokens 40` |
| 变异点扫描（scan 模式） | ezr 全库 5710 / bug25 触碰 7 文件 437；demo 全库 3724 / 触碰 7 文件 1026 | `cargo mutants --list` |

### 对账口径
- 测试计数链：bins 920（coder 基线）→ −2（窗口守护测试退役）→ +5（bt_dialog 入口
  补测）→ +2（判重补测）= **925**；挂载套件各 −2+7 = +5 净增对账吻合。
- App.windows 删除为纯死状态清除：字段零读者（get/iter/values 全库无引用），行为面
  无任何变化（918P 阶段全绿 + 集成全绿佐证）。
- resolve_confirm_dir 为等价提取：`if dir_raw.is_empty() → cfg → default → normalize`
  语义逐字保留，2 调用点行为不变（925P 全绿 + 集成全绿佐证）。
- demo clippy 修复均为机械等价变换（`Some(("", _))` ≡ `if d.is_empty()`、div_ceil
  ≡ `(cw+1)/2`、guard 化 ≡ if 折叠）+ allow 登记（与 ezr 既有口径同构），行为零变化
  （44P/0F 佐证）。
- 判重臂补测使用 `ck_type = CK_NONE`（testutil 默认 ck_type=4 需校验码会提前 return，
  判重臂不可达——测试构造陷阱已录入 testing.md）。

### 待办与待批
- 无待批。移交待办：①变异实跑（cargo-mutants 跑模式）属 QA/mutants 轮预算（本轮
  scan 计数已登记）；②dlg_finish_add/dlg_confirm_add_bt 拆分（CRAP 登记口径，需
  重构轮授权）；③（QA 轮）QA-TD-23 与 QA-UC-07/17/18 复验、场景 R PTY 复验。
- 无阻塞。

### 移交建议
- 下一步：QA 轮复验环境预存 FAIL 面，或按操作者授权收尾 `bin/swarm complete` 并将
  产物 tar.gz 复制到 `/home/z/my-project/`（沿历轮授权口径）。
- 工具链交接：rustup nightly 1.101.0（2026-11-04，PATH=~/.cargo/bin）+ cargo-llvm-cov
  v0.9.1 + cargo-mutants v27.1.0 + jscpd 5.4.2（bun 全局）；**cargo registry 本轮已切
  rsproxy.cn sparse**（TUNA 对部分新发布 crate 404 且 fallback 直连挂死，见
  environment.md cleaner 轮；`~/.cargo/config.toml`）；pyte 依赖仍在（demo verify）。
