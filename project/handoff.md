# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bug13（2026-10-31，「D62 四项落地 + QA runner 同步 + ezr-demo 对比 +
> 全量验证 + harness 测试」实现轮）
> 上游交接：project/handoff.md（bug13 specifier 轮交接：规格 v1.38/v1.4 + feature ×4 +
> qa ×4 + D62，任务依据＝已批规格 + 操作者 bug13 指令，含 ezr-demo 同步与 harness 完整
> 测试授权）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经互操作夹具多轮收口。bug13 轮为操作者指令级需求修订落地轮（D62 全项），本轮
  coder 全量交付（实现 + 测试 + runner + harness）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | bug12 轮登记沿袭 |
| **specifier（bug13 规格轮）** | mission ×2 + feature ×4 + qa ×4 | 已交付 | APS dry-check 4/4（specifier 轮登记） |
| **coder（本轮 bug13 落地）** | 产品 ×6 文件 + 单测 ×14 + runner ×6 文件 + ezr-demo 核证 | **已交付** | **cargo test bin 837P/0F + 集成 12 目标分块 5400P/0F + clippy --all-targets 零警告 + fmt 干净；QA runners 15 套件冒烟（12 全绿 + 3 例预先存在 flake 见下）；harness swarm_e2e PASS + bench_multipeer PASS；ezr-demo cargo test 30P/0F** |

### 遗留受限项（当前有效，沿袭 + 本轮更新）
- 沿袭：BT 下载限速未接；DHT/BEP 10 完整扩展/uTP 属 phase-03/04；cargo-mutants 缺位；
  死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化。
- **QA-SQ-13 判据固有竞态**（沿袭）：不自动重试任务 R 后重排队列的判据竞态，QA 轮
  refinement 待办；本轮冒烟复现 1 次。
- **QA-RS-06 / QA-MT-03 预先存在 flake（本轮新登记，非本轮回归）**：两例在本轮二进制与
  bug12 基线二进制上均可复现（A/B 对照，基线构建自 HEAD 原样源码）——RS-06「重启后排队
  位次」间歇（同轮既可 P 也可 F）、MT-03「重启应续传」稳定 F（基线同败）。两套件自
  D44 命令窗口迁移后未全量冒烟，期望口径与现行实现存在漂移嫌疑，**QA 轮 refinement 待办**
  （修复归属 QA 轮，本轮不越权）。
- **supervisor block_worker 写入 fatal 竞态（观察项，本轮登记）**：QA-PC-06 全量套件语境
  曾低频复现「无法写入目标文件」Fatal（worker `OpenOptions::write(true).open` 失败）；
  隔离 11 连跑零复现，runner 清场提速后 3 连跑稳定。疑与 resume 作废重建目标文件和
  worker（重）开文件的时序交错相关（ENOENT → Fatal）。本轮未改引擎；根治方向 =
  supervisor 先 touch 目标文件再派 worker，或 worker 打开失败加有界重试窗。归 QA/引擎轮。
- **APS dry-checker 已补装**（.tools/Acceptance-Pipeline-Specification + .tools/bb
  v1.12.209，符号链接桥接 .tools/bin/bb）；runner 内嵌 dry-check 步骤是否接入由 QA 轮定夺。
- **Inversion 访谈受限项（specifier 轮沿袭）**：chat 选择题面板不可用，操作者 bug13 指令
  文本即阶段 1 答案（D62 登记）；如对 D62 任一裁决有异议可回退，改动均为注记/单点接线。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 D1–D62 全部裁决（D62 = bug13 轮已批项，见 mission/phase-02.md）。
- **本轮实现定义值（coder 细化，规格框架内）**：
  - `normalize_save_dir_with(path, home, cwd)`：trim → 末尾 `/` 去除（长度 >1 才去，根
    `/` 保持）→ `~`/`~/` 经既有 `expand_tilde_with` 展开（主目录不可得保持字面）→ 非
    绝对路径 join cwd（cwd 不可得保持原样）→ `to_string_lossy`。单入口
    `normalize_save_dir(path)` 取真实 HOME/cwd。配置解析层不归一（D62①）。
  - `default_save_dir_prefill(cfg)`：`cfg.download_dir` → trim 后非空即用 → 否则
    `default_download_dir()`（from_toml 已过滤空白，此处双保险；D62② 预填链）。
  - 归一接线点 ×4：`dlg_finish_add`（HTTP 对话框，含命令窗口 URL 分流路径）、
    `dlg_confirm_add_bt`（BT 对话框）、`add_cli_task`（CLI `-d`/配置链）——均确认时归一，
    判重（同 URL/来源 + 同保存目录）与 namegen/sidecar 路径均用归一后路径。
  - 预填接线点 ×2：`open_add_dialog` / `open_add_dialog_with` 的 `dir` 字段 =
    `default_save_dir_prefill(&self.cfg)`（BT 对话框既有 D48 预填不动，预填层不改写）。
  - tick 钳制同源：`tick_clamp_selection` 的 clen = BT 任务取 `bt_conns.get(id).len()`、
    HTTP 任务维持 `t.connections.len()`（渲染行集同源；D62④）。

### 实现定义值登记（累计，覆盖为最新全量）
- 沿袭 phase-01/02 全部口径（D1–D62）+ bug11 轮键位缺省表 + bug12 轮客户端列样例集。
- 客户端列显示样例集（测试已锁，沿袭不变）：qB2500→`qBittorrent/2.5.0`、TR4030→
  `Transmission/4.0.3`、LT2110→`libtorrent/2.1.1`、v="BitComet 1.64"→`BitComet 1.64`、
  全零→`Unknown`（全表见 bug12 轮登记）。
- 本轮新增：保存目录归一口径五组验收例已锁单测——`~/Documents`+home→`<home>/Documents`、
  `qadl-rel/sub`+cwd→`<cwd>/qadl-rel/sub`、`/tmp/qadl/`→`/tmp/qadl`、`~/`→`<home>`、
  `~`（裸）→`<home>`；主目录缺失时 `~/x` 保持字面再按 cwd 转绝对；cwd 不可得时相对保持。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×6**：
  - `model/config.rs`：新增 `normalize_save_dir_with`/`normalize_save_dir`/
    `default_save_dir_prefill`；单测 ×5（~ 展开/末尾斜杠与空白/主目录缺失/cwd 缺失/
    预填链三态）。
  - `app/dialogs.rs`：`dlg_finish_add` 归一接线；`open_add_dialog` dir 预填；单测 ×3
    （预填链/`~` 展开落盘/相对→cwd 绝对落盘）；既有 normalized_dir 测试适配预填（清空
    预填后再输入）。
  - `app/cmd.rs`：`open_add_dialog_with` dir 预填（命令窗口 URL 分流路径同口径）。
  - `app/bt_dialog.rs`：`dlg_confirm_add_bt` 归一接线；单测 ×1（BT `~` 展开 + 相对转绝对）。
  - `app/cli.rs`：`add_cli_task` 归一接线；单测 ×1（相对配置 download_dir → cwd 绝对）。
  - `app/engine/tick.rs`：`tick_clamp_selection` clen 同源化（BT=bt_conns 行数）。
- **单测（keys.rs）×2**：BT 多 peer 选择 tick 不复位 / 行数收缩越界钳制保持。
- **QA runner ×6 文件**：
  - `harness.py`：新增 `clear_dir_prefill`/`clear_url_prefill`（一次写 64 bs + 单 pump）；
    `add_task_via_dialog` 显式目录前清空预填（留空=走预填语义保持）。
  - `suite_add_task.py`：QA-01-11 重写为五行组合（预填断言 + `~` 展开 + 相对转绝对 +
    字面 `~` 目录负对照）；QA-01-07/17 按 D44 迁移修复（对话框内改写 URL 可达路径 /
    粘贴前清 URL 预填）。
  - `suite_persistence_config.py`：新增 QA-PC-13（全量 36 键合法非默认值生效冒烟：零
    警告 + download_dir 预填落盘 + http_concurrency 回填 + BT 对话框 bt_concurrency=61/
    trackers 预填合并；mini torrent 生成器内嵌）；run 循环注册 case 13。
  - `suite_named_proxy.py`/`suite_tui_display.py`/`suite_resume_sidecar.py`：dir 输入点
    接入 clear_dir_prefill（×8 处）。
  - `suite_ui_conns.py`：v1.37 同步注（BT 判据由单测 + QA 会话覆盖，runner 维持 HTTP 驱动）。
- **ezr-demo 核证（无需同步）**：并发 panel 选择计数 = `active_conn_map().len()` 单源
  （sim.rs 无复位缺陷）；对话框保存到预填 = 钦定演示值 `/srv/downloads`（D48 明示不采用
  为产品口径，demo 无 config.toml）；demo 无真实文件系统（归一无适用面）。demo cargo
  test 30P/0F + fmt 干净——本轮 ezr 侧修改无 demo 侧对应物，零同步改动。
- **notes**：testing.md ×2（预填清场辅助与节奏纪律 + A/B 归因组合矩阵）；architecture.md
  ×1（block_worker 写入 fatal 竞态观察项）；INDEX 同步。

### 验证证据
| 验证项 | 结果 | 说明 |
|---|---|---|
| cargo test --bin ezr | 837P/0F | 含本轮新增单测 ×14（config ×5、dialogs ×3+适配、bt_dialog ×1、cli ×1、keys ×2、prefill ×2） |
| cargo test（集成 12 目标分块） | 5400P/0F | chunk1 763 / chunk2 740+644 / chunk3 649+186+618 / chunk4 398+632+770 |
| cargo clippy --all-targets | 零警告 | field_reassign_with_default 已修（cli 测试改结构体初始化） |
| cargo fmt --check | 干净 | ezr + ezr-demo |
| QA runners | 12/15 全绿 | config-template 5/5、persistence-config **10/10 ×3 连跑（含新 QA-PC-13）**、add-task 19/19（含重写 QA-01-11 与 D44 迁移修复 07/17）、ui-conns 18/18、tui-display 21/21、download-engine 16/16、integrity-check 12/12、named-proxy 18/18、ui-pieces 7/7、retry-backoff 9/9、throttle-proxy 7/7（5+2 分块）；slots-queue 14/15（SQ-13 沿袭竞态）、resume-sidecar 13/14（RS-06 间歇，双二进制同现）、modify-task 8/9（MT-03，基线同败） |
| harness swarm_e2e | PASS | 4MB：ezr 与 libtorrent leecher 双向做种、sha256 逐字节一致、HTTP announce 7 行可观察 |
| harness bench_multipeer | PASS | 8MB 单 seeder：32/32 块、sha256_match true |
| ezr-demo cargo test | 30P/0F | 零同步改动核证后回归 |

### 对账口径
- 产品净改动 ×6 文件 + 单测净增 14；测试计数 bin 825→837。
- runner 净改动 ×6 文件：harness helper ×2 新增 + add_task_via_dialog 一处接线；add-task
  用例 07/11/17 重写；persistence 新增 QA-PC-13（用例数 9→10）；named-proxy/tui-display/
  resume-sidecar 共 8 处 clear 接线。
- QA-PC-06 曾随慢速清场（64×0.008s pump ≈ 0.5s/次）暴露 block_worker 打开竞态（见遗留
  受限项）；清场提速后 3 连跑稳定——非本轮产品回归（该路径本轮零改动）。
- 磁盘纪律：本轮 QA 全量跑前 target/ 增量与基线对照 target 已按 notes/environment.md
  口径清理（沙箱 9.9G 盘，qa-run 临时区在 .work/tmp/ 随 complete 清理）。

### 待办与待批
- **待办（QA 轮）**：①RS-06/MT-03 期望口径 refinement（D44 迁移漂移嫌疑）；②SQ-13 竞态
  refinement（沿袭）；③supervisor block_worker 打开竞态观察项转引擎轮裁决（touch 先行
  或 worker 重试窗）；④QA-UC-11 BT peer 选择段的 QA 会话脚本化（本轮由单测
  tick_keeps_bt_peer_selection_* 承接）；⑤runner 内嵌 APS dry-check 步骤接入定夺。
- **待批（沿袭）**：监听端口随机范围/死 tracker 静默窗可视状态/做种任务重启不重挂/
  多协议喂窗公共化/BT partial 不持久化/`EZ` 码映射表外加项。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：
  ①`normalize_save_dir_with` 与 `expand_tilde_with` 的参数链（home/cwd 显式注入便于
  测试，可考虑收拢为结构体参数）；②`dlg_finish_add`/`dlg_confirm_add_bt`/
  `add_cli_task` 三处「缺省链 + 归一」两步式的重复形态（可提公共 helper）；③
  `default_save_dir_prefill` 的 trim 过滤与 from_toml 过滤的双保险是否保留。
- 给操作者的手测口径：①`~/.ezr/config.toml` 设 `download_dir = "~/Documents"` → 启动
  ezr 按 `A`（或命令窗口输入 URL）→ 对话框保存目录字段显示展开后的绝对路径
  `<主目录>/Documents` → 确认后任务详情「路径」为绝对路径、文件落在主目录 Documents
  （不再出现字面 `~` 目录）；②config 未设 download_dir → 对话框预填 `<主目录>/Downloads`；
  ③对话框保存目录手输相对路径（如 `dl/sub`）→ 任务创建后详情「路径」= `<启动目录>/dl/sub`；
  ④BT 下载中并发面板（多 peer）：↓/↑ 与鼠标可选中任意 peer 行、选择不被刷新复位、
  `B` 断开选中行；⑤config.toml 全量键写入合法非默认值 → 启动零警告、各键生效
  （QA-PC-13 口径，runner 可复跑）。
- 后续链：coder → cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
