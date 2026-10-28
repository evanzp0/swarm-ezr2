# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder（2026-10-28，BT 外部
> 夹具互操作测试轮）
> 上游交接：project/handoff.md（ezr-p2-coder 前轮交接）
> 当轮任务：操作者钦定——引入第三方 tracker（HTTP+UDP）与 Python BT 客户端夹具，
> 跑通「造种 → tracker → seed → ezr 与夹具同时下载 → hash 对账」全链路，归档收尾。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经前轮 BT 启动收口修复，详见既有登记。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02） | features/ 01 十份 + 06…09 四份；qa/ 对应十四份 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及注释删除/tilde/BT 启动收口轮） | `project/ezr/` 全量 | 已交付 | 前轮 5685P/0F；clippy 0 警告 |
| coder（本轮：BT 外部互操作夹具轮） | `project/harness/`（第三方 tracker×2 + 客户端×2 + swarm_e2e.py 编排 + README）；产品修复 ×3（见下）；新增回归测试 ×1 | 已交付 | 全量 5693P/0F；clippy --bins/--tests 0 警告；互操作 e2e 三连 PASS |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；peer 贪心调度、最小
  choke 状态机（满足 D52）；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **hardening_g_v147 `single_stream_downgrade_emits_toast` 时序抖动**（既有登记项，
  观测 ~1/6 失败率，失败签名与改动前一致；判定为既有环境抖动非本轮引入）。
- 源码注释全量删除口径延续；挂载套件剥头副本随产品源同步（对账口径见产物总账）。
- 已闭环（原遗留项）：~~nightly 工具链未装成~~（2026-10-28 追加轮 TUNA 镜像装成，
  `cargo fmt --check` 已补跑并套用，全绿证据见追记）；~~PyTorrent 产物不完整~~（同轮
  按操作者指令移除，libtorrent 为唯一对照客户端）。

### 追记（2026-10-28 追加轮：操作者指令）
1. **harness 移除 PyTorrent**：删 `client/PyTorrent/` 与 `swarm_e2e.py` 内全部
   pytorrent 路径（含 `--skip-pytorrent`），libtorrent（Python 绑定 2.1.1）成为唯一
   对照客户端（seeder 17903 + leecher 17904）；README 重写：libtorrent 安装（阿里云
   pip 镜像、版本、验证命令）/用法/重建前置说明。
2. **nightly 换镜像装成**：8MB 探针实测 TUNA 13.7MB/s（rsproxy ~20KB/s 且无断点续传；
   USTC 403/SJTU 不通/BFSU 404），minimal+rustfmt+clippy 一次装成
   （1.101.0-nightly 41089d07a）。
3. **fmt 补跑闭环**：`cargo fmt --check` 全仓仅 src/bt/engine.rs 有 diff（上轮新增
   字面量换行风格），套用后干净；nightly 下回归 bin ezr 748P/0F + hardening_g_engine
   563P/0F + hardening_engine_backfill 651P/0F + clippy --bins --tests 0 警告。
4. **过程修正（夹具加固）**：`bin/swarm complete` 清 target/ 后 e2e 首跑失败暴露两点：
   ① 二进制需重建（README 已补前置说明）；② `--ezr-bin` 相对路径在 pty 子进程 chdir
   隔离 HOME 后失效（exec 127 + tty 回显，症状似“TUI 未就绪”）——脚本改为 chdir 前
   解析绝对路径；TUI 就绪/对话框等待改 `wait_for` 持续 pump。回归 e2e 2× PASS
   （ezr 2.2s 做种，三方 SHA-256 一致）。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ 前轮两项补充（引擎监听 127.0.0.1:0 随机端口、
  首轮 announce 延迟 2s）。
- 本轮新增实现口径：
  - **piece 落盘三步查错**：seek → write_all → flush 任一失败即中断，不标记 piece 完成
    （flush 为数据可见性保证，见 notes/testing.md）。
  - **peer 连接 5s 收割窗**：writer 退出后 reader 至多 5s 内随对端关闭/超时终止。
  - **互操作夹具端口**：HTTP tracker 17901 / UDP tracker 17902 / libtorrent seeder 17903 /
    leecher 17904（夹具专用，避开 QA fixture 端口段）。

## 二、当前产出情况

### 本会话产物清单
- **`project/harness/`**（新目录，跨会话保留）：
  - `tracker/pybtracker/` + `tracker/pybtracker_server.py`——第三方 elektito/pybtracker
    （BEP 15 UDP tracker，纯 Python asyncio）及其启动包装器；
  - `tracker/mini_http_tracker.py`——自研极简 BEP 3 HTTP tracker（stdlib-only，BEP 23
    compact peers，JSONL announce 日志可观察）；
  - `client/PyTorrent/`——第三方 gallexis/PyTorrent（纯 Python leech/seed）；
  - `.ref-*` 三个不可用候选存档（py-http-tracker 同 IP 自过滤、trondhumbor Flask 1.1.2
    死依赖、pieces 未启用）；
  - `swarm_e2e.py`——主编排（torf 造种 → 双 tracker → libtorrent seeder → pty 驱动 ezr
    → libtorrent leecher 同时下载 → PyTorrent 附加 → SHA-256 三方对账，证据落
    result.json / http_announces.jsonl / udp_tracker.log / ezr_screen_final.txt）；
  - `README.md`——选型记录、用法、首轮结果、已知坑。
- **产品修复（`src/bt/engine.rs`）**：
  1. `on_block` 重写：未完成 piece 的块进内存 `PartialPieces`（piece → begin → data
     BTreeMap），按序严丝合拼满才校验，SHA-1 通过才写盘 + `flush`，write/seek/flush 失败
     中断不标完成；修复"piece_len > 16KB 时块被丢弃、死循环重取"（真实 256KB piece 必现）。
  2. `issue_requests` 跳过已取块与在飞块（配合 partial，消除重复请求）。
  3. `peer_pump` 读写分离：reader 独占读半部裸循环 `read_msg`（永不取消），writer 只管
     出站；修复 `select!` 取消 `read_exact` 丢字节导致的帧错位（真实客户端流式多块下
     必现「消息超长」RST）。
- **产品修复（`src/engine/mod.rs start_bt_task`）**：BT 引擎任务加 join 监视器，panic 转
  `Evt::BtFailed`（Fatal，含 panic 消息），消除"panic 静默僵尸任务"。
- **回归测试**：`bt::engine::tests::engine_downloads_multi_block_pieces`（64KB piece =
  4 块/piece + 尾块部分块，TDD 先红后绿）。
- **`project/notes/`**：testing.md 增 4 条（tokio File flush 可见性 / select! 取消安全与
  读写分离 / 同源 fixture 盲区 / pty 驱动 pump 纪律）；environment.md 增 2 条（pip 阿里云
  镜像 / nightly 慢速受限）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试（16 套件） | 5693P / 0F | `cargo +stable test -- --test-threads=2`（project/ezr 下；分块执行） |
| 其中：bin ezr | 748P / 0F | `cargo +stable test --bin ezr -- --test-threads=2`（含新增 multi_block 测试） |
| 其中：12 挂载套件 | 4899P / 0F | hardening 131 + engine_backfill 651 + g_appcore 568 + g_engine 563 + g_main 2 + g_proxy 16 + g_slots 176 + g_supervisor 537 + g_throttle_err 350 + g_v147 551 + ui_backfill 681 + v115 674 |
| 其中：其余 bin 目标 | 46P / 0F | `cargo +stable test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy` |
| clippy | 0 警告 | `cargo +stable clippy --bins --tests` |
| 互操作 e2e | 3 连 PASS | `python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr`（project/harness 下） |
| e2e 关键断言 | 全过 | ezr 2.2s 完成下载转做种；ezr/libtorrent/原始三方 SHA-256 一致；HTTP(BEP3)+UDP(BEP15) announce 证据齐（`-EZ2000-` started/periodic、left→0、downloaded→4194304） |
| 多块回归测试稳定性 | 30+ 连绿 | `cargo +stable test --bin ezr engine_downloads_multi_block_pieces`（连跑 30 次） |

### 对账口径
- 5685 → 5693（+8）：新增 engine_downloads_multi_block_pieces 1 例（bin ezr 747→748）；
  7 个 `#[path]` 挂载套件随产品源收编各 +1（engine_backfill 650→651、g_appcore 567→568、
  g_engine 562→563、g_supervisor 536→537、g_v147 550→551、ui_backfill 680→681、
  v115 673→674）；1 + 7 = 8 对账自洽；g_slots/g_main/g_proxy/g_throttle_err/hardening/
  其余 bin 目标不挂 engine.rs，计数不变。
- e2e 三连 PASS 均在同一 payload（seed 0xE27，SHA-256 e874c932…10aea）下复现，结果与
  证据文件随 harness 工作区保留（`--keep` 可留存全程中间产物）。

### 待办与待批
- 无待批事项。追加轮两则操作者指令均已闭环（见追记）；无规格偏离；
  notes/environment.md 新增 rustup dist 镜像选型口径（TUNA）如操作者有异议可回退重议。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `src/bt/engine.rs` 本轮重写的 on_block/peer_pump 复杂度与重复面审查；
  ② run() 主循环 select 臂职责分布（tick/事件/命令三线）可提纯函数；
  ③ harness/swarm_e2e.py 与 qa/runners/harness.py 的 pty 驱动重复面（可提公共库）；
  ④ 覆盖率基线（cargo-llvm-cov 0.9.1 已装，本轮未跑）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者显式
  续跑）。QA 轮执行 06…09 套件时可直接复用 harness 的真实规格种子与双 tracker。
