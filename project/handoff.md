# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-bug25-ui-units-columns-dup-error（2026-11-06，coder 轮：bug25 三项 UI 修正实现
> + ezr-demo 同构同步）
> 上游交接：project/handoff.md（specifier 轮交接 = bug25 三项规格 + ezr-demo 同步口径 +
> 实现定义值登记）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器。需求权威 `project/mission.md`
  （分期模式 v1.42）+ `project/mission/phase-01…04.md`（本轮 bug25 轮修订）。
- phase-01/02 已实现并多轮收口；本轮为 **bug25 UI 修正轮**：①面板层速度单位二进制统一
  ②并发面板列宽/表头对齐 ③重复任务报错载体改判（错误窗口「该链接的任务已存在」）+
  **ezr-demo 文档与代码同构同步**（操作者指令第 3 条）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（上轮） | mission/phase/features/qa 四层 bug25 规格 | 已交付 | APS dry-checker 四份通过（沿袭） |
| **coder（本轮，ezr 产品端）** | `project/ezr/`：①`src/ui/text.rs::fmt_speed` 二进制四档（B/s {:.0} / KiB/s {:.0} / MiB/s {:.1} / GiB/s {:.2}，1024 进位）+ 9 断言单测改判；`src/property_tests.rs::prop_fmt_speed_suffix_domain` 域改判（GiB/MiB/KiB/B）；`src/ui/task_lines.rs` 两测试改判（MiB/s/KiB/s 二进制值锚点）。②`src/ui/conns.rs`：BT_COLW **[18,4,12,12,10,10,34,4]**、BT_CONTENT_W **111**、HTTP_COLW **[4,12,10]**、HTTP_CONTENT_W **28**、BT_CLIENT_SHRINK_BASE **77**；head_cell_r（pad_left）用于下载速度/上传速度/累计下载/累计上传四表头（HTTP 下载速度/累计下载同）；HTTP 行单元格改读 HTTP_COLW；测试改判 6 处 + 新增 `bt_speed_cum_columns_binary_and_right_aligned` + `seed_bt_conn_speeds` 测试助手（app/testutil.rs + ui/testfx 转发）。③`src/app/dialogs.rs::dlg_finish_add` 与 `src/app/bt_dialog.rs::dlg_confirm_add_bt` 判重命中处改 `open_error_window("该链接的任务已存在")`（toast 承载退役）+ 4 处测试改判 | 已交付 | **bins 920P/0F**（919 基线 + 1 新增）+ **集成 12 组全绿**（hardening 142、engine_backfill 823、g_appcore 721、g_engine 716、g_main 2、g_proxy 16、g_slots 202、g_supervisor 690、g_throttle_err 451、g_v147 704（flake 单跑绿沿例）、ui_backfill 853、v115 846）+ **clippy 0 警告** + **fmt 干净** |
| **coder（本轮，ezr-demo 同构同步）** | `ezr-demo/`：①`src/ui/mod.rs::fmt_speed` 二进制四档（与 ezr 同口径）+ 新增 fmt_speed_binary_tests；②`src/ui/conns.rs`：colw [18,4,12,12,10,10,34,4]/[4,12,10]、收缩基线 65→77、head_cell_r 右对齐、行单元格改读 colw；③**新增 `src/ui/err.rs`**（66×7「错误」窗口：dialog_rect_at 居中、RED 消息行、[确定] 按钮、draw_close_btn [✗]；z 序 = 对话框/命令窗口之后、pieces 之前）+ App 六字段（err_open/err_msg/err_pos/err_drag/err_area/err_btn_rect）+ keys/mouse 路由（Esc/Enter/q/Q/[确定]/[✗] 关闭、拖拽移动、点击外部无操作、模态消费）+ overlay_open 收编；④`src/app/dialogs.rs` 两处判重改 open_error_window + 新增 duplicate_error_window_tests；⑤docs《ezr-demo UI界面及交互设计规范.md》bug25 轮注记 + §5.1/§5.2 速度单位 + §5.4 列宽表头（111/28/基线 77/右对齐/单位）+ §6.1/§6.9 重复任务错误窗口 + **新增 §6.13 错误窗口** + TOC；⑥`scripts/verify_ezr.py` 场景 R 同步（R1-2/R1-3 断言改「该链接的任务已存在」、重复后 esc→esc esc 双关） | 已交付 | **demo 测试 44P/0F** + **clippy 9 处既有基线零新增** + **fmt 干净** + cargo check 干净 |
| coder（历轮，速度追查二轮等） | bt/engine.rs 端点化去重 + 管线 EMA | 已交付 | 沿袭全绿 |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（历轮登记，全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；TUI 恐慌
  自愈边界（bug6）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal
  竞态观察项；`App.windows` 失去读者（cleaner 候选裁决）；leech 判据无 e2e 自动化；
  bad_piece 归属口径（D67 允许）；QA-TD-23 与 QA-UC-07/17/18 环境预存 FAIL 待 QA 轮复验；
  `periodic_flush_writes_sidecar_mid_download` HTTP 滴流 flake 单跑绿沿例；同 IP 拓扑
  残余启动段；leech 封禁按裸 IP 连坐面；ezr 不发 KeepAlive。
- **本轮沿袭 flake**：`engine_conns_quota_exact_and_unique_ids`（g_v147）本轮复现 1 次
  FAIL，**单跑即刻绿**（沿 notes/testing.md 非回归口径；上轮曾单跑复验绿），与 bug25
  改动面（UI 展示层 + 对话框报错文案）无接触面。
- **本轮新增受限项**：`ezr-demo scripts/verify_ezr.py` 场景 R 及部分场景在本沙箱 PTY
  环境预存 FAIL（R1-1 基本添加亦失败）——**经 git stash 对照基线归因：基线同样失败，
  非本轮回归**（同 verify_phase2_ui.py A 段沙箱预存同形）；R1-2/R1-3 断言已按错误窗口
  新口径同步，待 PTY 可用环境复验。
- ezr-demo clippy 9 警告为未触碰区域既有基线（本轮触碰面零新增）。

### 实现定义值登记（累计口径，本轮落地处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| **面板层速度格式化（本轮落地）** | **B/s `N`、KiB/s `N`、MiB/s `N.N`、GiB/s `N.NN`**（阈值 1024/1048576/1073741824；四舍五入；0 与 -0.0 归一 `0 B/s`） | **ui/text.rs `fmt_speed`**（ezr）+ **ui/mod.rs `fmt_speed`**（demo） |
| **BT 并发列宽（本轮落地）** | 地址(18)/标志(4)/**速度(12)×2/累计(10)×2**/客户端(34)/协议(4)，内容宽 **111**（数据 104 + 7 分隔） | **ui/conns.rs `BT_COLW`/`BT_CONTENT_W`**（ezr）+ demo colw 同构 |
| **HTTP 并发列宽（本轮落地）** | 序号(4)/**速度(12)/累计(10)**，内容宽 **28** | **ui/conns.rs `HTTP_COLW`/`HTTP_CONTENT_W`**（ezr）+ demo colw 同构 |
| **客户端收缩基线（本轮落地）** | **77**（下限 26；整表可见视口 ≥ 103；≥ 111 维持 34） | **ui/conns.rs `BT_CLIENT_SHRINK_BASE`**（ezr）+ demo 77 同构 |
| **并发表头对齐（本轮落地）** | 速度×2/累计×2（HTTP 速度/累计）表头 pad_left 右对齐；地址/标志/客户端/协议/序号维持 pad_left 左对齐 | **ui/conns.rs `head_cell_r`**（ezr）+ demo head_cell_r 同构 |
| **重复任务错误窗口（本轮落地）** | `open_error_window("该链接的任务已存在")`（D56 第三臂；HTTP = dlg_finish_add 判重、BT = dlg_confirm_add_bt 判重；对话框保持打开） | **app/dialogs.rs / app/bt_dialog.rs**（ezr）+ demo dialogs.rs 同构 + demo ui/err.rs 新组件 |
| 管线深度目标/连接去重口径 | 端点级 (ip,port) + pipe_target slow-start/EMA（上轮登记，不变） | bt/engine.rs |
| 标志列结构（D69③④⑤） | 地址(18)/标志(5)/速度(12×2)/累计(10×2)/客户端(34)/协议(4)，内容宽 111（v1.42 口径） | ui/conns.rs BT_COLW |
| trackers 弹窗几何（D67⑤） | 94×22；消息列预算 12（不变） | ui/popups.rs |
| 限速单位口径（config 键） | KiB/MiB/GiB 二进制 + 裸整数与 B/s = B/s；KB/MB/GB 十进制废弃（不变） | config.rs |

## 二、当前产出情况

### 本会话产物清单
- **ezr 产品端（7 文件）**：`src/ui/text.rs`（fmt_speed + 单测）、`src/ui/conns.rs`（列宽/
  表头/收缩基线 + 测试改判与新增）、`src/ui/task_lines.rs`（2 测试改判）、
  `src/property_tests.rs`（prop 域改判）、`src/ui/mod.rs`（testfx 转发助手）、
  `src/app/testutil.rs`（seed_bt_conn_speeds）、`src/app/dialogs.rs` + `src/app/bt_dialog.rs`
  （判重错误窗口 + 测试改判）。
- **ezr-demo（9 文件）**：`src/ui/err.rs`（新增）、`src/ui/mod.rs`（fmt_speed + 模块注册 +
  z 序 + overlay_open + fmt 测试）、`src/ui/conns.rs`（列宽/表头/基线）、`src/app/mod.rs`
  （err 六字段）、`src/app/keys.rs`、`src/app/mouse.rs`（err 路由）、`src/app/dialogs.rs`
  （open/close_error_window + 判重改判 + 测试）、`docs/ezr-demo UI界面及交互设计规范.md`
  （bug25 注记 + §5.1/5.2/5.4/6.1/6.9 + §6.13 + TOC）、`scripts/verify_ezr.py`（场景 R 同步）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr bins 单元测试 | **920P/0F** | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bins` |
| ezr 集成测试（12 组） | **全绿**（142+823+721+716+2+16+202+690+451+704（flake 单跑绿）+853+846） | `CARGO_INCREMENTAL=0 cargo test --test <套件名>` 逐组 |
| ezr clippy / fmt | **0 警告 / 干净** | `cargo clippy --all-targets` / `cargo fmt --check` |
| ezr-demo 测试 | **44P/0F**（含新增 duplicate_error_window_tests + fmt_speed_binary_tests） | `cd ezr-demo && cargo test` |
| ezr-demo clippy / fmt / check | **9 处既有基线零新增 / 干净 / 干净** | `cargo clippy --all-targets`（触碰面零新增） |
| demo PTY 守卫 | verify_ezr.py 场景 R 断言已同步错误窗口口径；本沙箱 PTY 预存 FAIL 经 stash 基线对照归因（非回归） | `python3 scripts/verify_ezr.py r`（待 PTY 可用环境复验） |

### 对账口径
- 规格基线 = specifier 轮交接（mission v1.42 / phase-01 v1.44 / phase-02 v1.13 +
  features 四份 + QA 四份）；实现全部落在登记表加粗处，未触碰 features/qa/mission 文件。
- 速度单位口径：面板层 fmt_speed 唯一实现点改判完成；尺寸类 fmt_size/fmt_size_pair、
  限速 fmt_speed_limit/parse_speed（输入层 kb/mb 二进制速记）与 config 键旧写法废弃
  口径均未触碰（规格明确不在范围）。
- 列宽口径：`content_w_matches_colw` 不变式自动成立（111 = 104+7、28 = 26+2）；
  表头右对齐仅速度/累计四列（HTTP 两列），数据单元格对齐零改动。
- 错误窗口口径：ezr err 组件/键位/鼠标路由零改动（仅新增两个调用点）；D53/D55 两臂
  文案与分流臂③行为不变（cmd.rs `confirm_other_inputs_open_unsupported_error` 断言
  原样通过）；demo 侧为新增组件 + 路由（对齐 ezr 行为语义）。
- demo 守卫脚本：R1 键序 esc→esc esc（第一 Esc 关错误窗口）、R1-2/R1-3 断言改
  「该链接的任务已存在」；场景 I 注记概念不变。

### 待办与待批
- 无待批。沿袭待办（cleaner 轮候选）：`App.windows` 失去读者裁决；ezr-demo 9 处 clippy
  基线清理；（QA 轮）QA-TD-23 与 QA-UC-07/17/18 复验；场景 R PTY 复验。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`（可选）；操作者授权的收尾口径：直接
  `bin/swarm complete`，并将产物 tar.gz 复制到 `/home/z/my-project/`（沿历轮授权）。
- 工具链交接：rustup nightly 1.101.0（2026-11-02，PATH = ~/.cargo/bin）经 TUNA 镜像
  装毕（RUSTUP_DIST_SERVER/RUSTUP_UPDATE_ROOT 已写入 ~/.bashrc；rustup-init 本体经
  Aliyun rustup 镜像装——Aliyun 速最快但 dist 快照停在 2026-05，故工具链走 TUNA）；
  cargo registry = TUNA sparse（~/.cargo/config.toml）；pyte 经 pip 阿里云镜像装（demo
  verify 脚本依赖）；APS bb + 仓库缓存于 `.tools/`（specifie 轮重建）。
