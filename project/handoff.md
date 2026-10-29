# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder-bug2（2026-10-28，
> 「q 退出乱码 + BT 连不上 tracker」双 bug 修复轮）
> 上游交接：project/handoff.md（ezr-p2-coder 前轮交接：BT 速度归零修复轮）
> 当轮任务：操作者报告两个 bug——①按 q 退出主界面后终端出现乱码 `35;62;12M`；
> ②BT 任务连不上 tracker、无 peer（操作者怀疑上一轮速度修复引入）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经互操作夹具多轮收口，详见既有登记。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02） | features/ 01 十份 + 06…09 四份；qa/ 对应十四份 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及注释删除/tilde/BT 启动收口轮） | `project/ezr/` 全量 | 已交付 | 前轮 5685P/0F；clippy 0 警告 |
| coder（BT 外部互操作夹具轮） | `project/harness/`（双 tracker + libtorrent 对照客户端 + swarm_e2e.py + README）；产品修复 ×4；新增回归测试 ×1 | 已交付 | 全量 5694P/0F（分项复算）；clippy 0 警告；互操作 e2e 三连 PASS |
| coder（BT 速度归零 bug 修复轮） | 产品修复 ×1（`on_bt_progress` 喂速度滑窗）；新增回归测试 ×2 | 已交付 | 全量 5710P/0F；clippy 0 警告；fmt 干净 |
| coder（本轮：q 退出乱码修复轮） | 产品修复 ×1（`restore_into`/`drain_input_events` 排空契约重写）；测试删 1 增 1 持平 | 已交付 | 全量 5710P/0F（总账持平）；clippy 0 警告；fmt 干净；pty 复现红→绿 + e2e PASS |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；peer 贪心调度、最小
  choke 状态机（满足 D52）；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **hardening_g_v147 环境性抖动**（既有登记项）：本轮批量连跑再复现一次同族签名
  （`engine_conns_quota_exact_and_unique_ids` 连接视图得 0 行，两连败），单测 ×3 绿 +
  整套件单跑 553P/0F 绿，三证齐判环境性，与上轮判定同族；已并入 notes/testing.md
  判据条目的复现记录。
- **死 tracker 的用户可见延迟**（本轮发现、按设计行为登记）：tracker 不可达时任务停留
  「下载中 0.0%」约 4 分钟才报「全部 tracker 失败」（UDP 15+30+60+120s 重试耗尽 +
  HTTP 秒败但需双 tracker 全败）；是否要加「announce 进行中/失败原因」可视状态由
  操作者裁定，见待办与待批。
- 源码注释全量删除口径延续；挂载套件剥头副本随产品源同步（对账口径见产物总账）。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ 前轮补充（引擎监听 127.0.0.1:0 随机端口、
  首轮 announce 延迟 2s、piece 落盘三步查错、peer 连接 5s 收割窗、互操作夹具端口
  HTTP 17901 / UDP 17902 / seeder 17903 / leecher 17904）。
- 本轮新增（退出排空窗契约，`src/main.rs`）：`MOUSE_SETTLE` 50ms；
  `RESIDUAL_MIN_SPAN` 80ms（最短排空跨度）；`RESIDUAL_QUIET` 40ms（静默判定）；
  `RESIDUAL_CAP` 250ms（截止上限）。语义：关鼠标捕获后至少保持 80ms 排空窗、且最后
  40ms 无事件才收口，250ms 硬顶；排空全程维持 raw mode（ECHO off），termios 恢复是
  排空后的第一步写入。

## 二、当前产出情况

### 本会话产物清单
- **产品修复（`src/main.rs`）**：q 退出乱码根因为「在途鼠标 SGR 字节晚于 termios 恢复
  到达，被 tty 行规程 ECHO 上屏」——`?1006l` 写出后 web 终端中继仍有在途字节，而旧
  排空逻辑「首个 5ms 静默即 break」（且 crossterm `EventStream` 后台线程持锁时
  `poll(timeout)` 直接假阴性返回 `Ok(false)`），排空窗在字节落队前关闭；termios 恢复
  （ECHO on）后到达的字节被立即回显，`ESC[<35;62;12M` 中 `ESC[<` 被终端模拟器当未
  识别 CSI 吞掉，屏幕只剩 `35;62;12M`——与操作者所见逐字符吻合。修复：`restore_into`
  固定顺序（关鼠标捕获 → settle 50ms → 排空循环 → 恢复 termios → 关 paste/离备屏），
  排空循环改「截止时间 + 静默判定」双条件（80ms 最短跨度 / 40ms 静默 / 250ms 上限，
  全安全 API，`unsafe_code = deny` 口径下未用 ioctl 字节排空）。
- **测试（`src/main.rs` `cli_parse_tests`）**：删 `settle_and_drain_spends_primary_window_only_without_events`
  （settle_and_drain 并入 restore_into，函数取消）；增 `drain_input_events_keeps_min_span_open_for_inflight_bytes`
  （锁定「空队列也必须保持 80ms 最小排空跨度」契约——旧实现空队列即时返回必假）。
  删 1 增 1，bin ezr 750 持平。
- **`project/notes/`**：architecture.md 增 1 条（退出路径排空契约不变式 + crossterm
  EventStream 持锁假阴性陷阱）；testing.md 增 1 条（pty 复现终端态残留的 oracle 口径：
  回显数 + bash 会话首进程拓扑）并合并 g_v147 同族复现记录；environment.md 合并小盘
  第二层清理空间 + 新增 harness tracker/seeder 会话级生命周期（死 tracker 判定路径）；
  INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| 全量测试（16 套件） | 5710P / 0F | `cargo +stable test`（分套件执行，明细见下） |
| 其中：bin ezr | 750P / 0F | `cargo +stable test --bin ezr -- --test-threads=2` |
| 其中：12 挂载套件 | 4914P / 0F | hardening 131 + engine_backfill 653 + g_appcore 570 + g_engine 565 + g_main 2 + g_proxy 16 + g_slots 176 + g_supervisor 539 + g_throttle_err 350 + g_v147 553 + ui_backfill 683 + v115 676 |
| 其中：其余 bin 目标 | 46P / 0F | `cargo +stable test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy` |
| clippy | 0 警告 | `cargo +stable clippy --bins --tests` |
| fmt | 干净 | `cargo fmt --check`（nightly，rust-toolchain.toml 钉版） |
| bug A 系统级先红 | 3 轮全红 | `.work/tmp/repro_quit_mouse.py <release/ezr> <隔离HOME>`：修复前 3 轮 `mouse_echoes=7`（7 个鼠标事件全部回显上屏） |
| bug A 系统级后绿 | 8 轮全绿 | 同脚本：修复后 5 轮 + 最终代码重建后再 3 轮，`mouse_echoes=0` 且 `leftover_bytes=0` |
| bug B 互操作 e2e | PASS ×2 | `cd project/harness && python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150`（修复前树与最终树各一次，ezr 2.2s 完成、双 tracker、三方 SHA-256 一致） |
| bug B 死 tracker 复现 | 症状吻合 | `.work/tmp/repro_dead_tracker.py`：无 tracker 时任务停留「下载中 0.0%」（45s 窗口内不报错） |

### 对账口径
- 上轮基线 5710（bin ezr 750 + 12 挂载套件 4914 + 其余 bin 46）。本轮改动全部在
  `src/main.rs`（不在 app/ 挂载子树）：删 1 增 1 → bin ezr 750 持平、12 挂载套件与其余
  bin 计数逐一实测不变，5710 与实测分项逐套件吻合，总账持平。
- g_v147 批量连跑中 FAIL 一次（签名：连接视图 0 行 ≠ 预期 4 行），按 notes/testing.md
  三证判据处置：单测 ×3 绿 + 整套件单跑 553P/0F 绿 + 签名与既有登记同族 → 环境性假
  FAIL，最终对账以单跑结果计。
- 磁盘事件登记：全量回归中段磁盘 100% 满（error 28）导致一次编译失败假象，清
  incremental + deps 内测试可执行后复跑全绿（环境经验已并入 notes/environment.md），
  不涉及产品代码。

### 待办与待批
- **待批（体验改进，非本轮缺陷）**：tracker 不可达时约 4 分钟的「下载中 0.0%」静默窗
  （UDP 重试耗尽 + 双 tracker 全败才报失败）——是否增加「announce 进行中/最近一次失败
  原因」的任务行/详情展示，由操作者裁定后再立项。
- **待批（重构，沿袭上一轮提问）**：多协议喂窗公共化（`Evt::Progress` 与
  `on_bt_progress` 的建窗/推窗重复面，未来新协议接入时同一义务会再复制一份）——建议交
  six-pack/refactorer 或 cleaner 轮评估，本轮未动（bug 轮不夹带重构）。
- 无阻塞、无规格偏离。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `src/main.rs` 退出路径（restore_into/drain_input_events）与既有 MOUSE_SETTLE 系列
  的常量与函数归并审查；② `src/app/engine/bt.rs` 三轮修补后的复杂度与重复面审查
  （on_block/peer_pump/on_bt_progress，含上述待批的重构项）；③ run() 主循环 select 臂
  职责分布；④ harness/swarm_e2e.py 与 qa/runners/harness.py 的 pty 驱动重复面。
- 给操作者的 BT 手测口径（bug B 自证/自愈）：harness 的 tracker/seeder 是会话级临时
  进程（complete/会话结束即死），手测前按 `project/harness/README.md` 重起夹具，或直接
  跑 `swarm_e2e.py` 全自动验证；种子指向 `127.0.0.1:17901/17902` 在夹具未运行时必然
  「无 peer」，且任务会静默停在「下载中 0.0%」约 4 分钟才报「全部 tracker 失败」。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者显式
  续跑）。
