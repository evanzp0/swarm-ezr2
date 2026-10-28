# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder（2026-10-28）
> 上游交接：project/handoff.md（ezr-p2-adjudication-cleanup 轮交接）
> 当轮任务：依 mission/phase-02.md（定稿）与 features/06…09 实现 phase-02——
> BT 下载内核、做种上传与 BT 任务管理。
> 追加轮（操作者指令）：删除项目源码中的所有注释——`project/ezr` 的 src/ 与 tests/
> 共 123 个 .rs 已全量无注释（行为零变更，见「注释删除轮」节）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`；各期现行口径以
  对应分期文件为准。
- **phase-01（HTTP/HTTPS 下载内核与 TUI 正式版）已收官**：`project/ezr/` 全量源码、
  `project/features/` 与 `project/qa/` 期号 01 存量。
- **phase-02（BT 下载内核、做种上传与 BT 任务管理）已实现**：需求详述
  `mission/phase-02.md`（FR-02-01…23 + D44–D59）全部落地；规格 `features/06…09`
  四份（34 场景）对应实现与单测覆盖；QA 套件 `qa/06…09-*-qa.md` 待 QA 轮执行。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01） | features/ 期号 01 十份 + qa/ 期号 01 十份 | 已收官 | 收官基线 4564P/0F |
| specifier（phase-02） | features/06…09 四份 + qa/06…09 四份 | 全部交付 | 06/07/08 各场景 ↔ qa 逐例对应 |
| coder（phase-02，本轮） | `project/ezr/` phase-02 全量实现（见下清单） | 已交付 | cargo test 16 套件 5596P/0F（--test-threads=2）；clippy --bins --tests 0 警告；fmt clean |
| coder（注释删除轮，操作者指令） | src/tests 123 个 .rs 全量剥离注释（54,872 → 46,154 行，−8,718） | 已交付 | 行首注释残留 0；三线复验全绿（同 phase-02 口径，见验证证据） |

### 遗留受限项（当前有效）
- **BT 下载限速未接**：BtSpec.max_speed 字段保留、本期引擎未接 throttle（phase-02
  规格未钦定 BT 限速口径；`max_speed` 仅对 HTTP 生效）。
- **peer 调度为贪心序**（无 rarity 统计）、choke 状态机最小集（interested/unchoke
  维护 + 合法 request 响应）——满足 D52 口径；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；cargo-llvm-cov 未在本会话使用。
- **demo 回归长脚本沙箱挂起**（沿袭）：按场景子集分半跑。
- **doc 测试不适用**：包为 bin-only（无 lib target），`cargo test --doc` 报
  no library targets——与 phase-01 口径一致。
- 挂载测试 crate（hardening_*）经 `#[path]` 收编产品源：phase-02 新增 `mod bt` /
  `mod multiline` / `mod textfield` 挂载已补齐；`tests/gen/*_stripped.rs` 剥头副本
  已随源码重生成（chart/clipboard/detail/btn 四份），注释删除轮已再次与产品源同步
  （逐字一致，gen-drift 守卫通过）。
- **源码注释已全量删除**（操作者指令，注释删除轮）：src/ 与 tests/ 无任何 // 与 /* */
  注释（含 /// doc 注释）；此前由注释承载的 D 值/FR 对应关系以本文件「实现定义值登记」
  与 mission/phase-02.md 为权威。恢复路径：带注释备份
  /home/z/my-project/ezr-src-with-comments-backup-20261028.tar.gz 或注释删除前归档
  swarm-20261028-035851（均不在本包内）。

### 实现定义值登记
- 累计维持 phase-01 收官口径（D1–D42）；phase-02 新增 D44–D59 已在
  `mission/phase-02.md` §3 定稿登记，实现无偏离。补充两项实现口径：
  - 引擎监听端口 = 127.0.0.1:0 随机绑定（announce port 携带；phase-02 规格未钦定
    固定端口与配置键——做种入向通道按随机端口实现）。
  - tracker 首轮 announce 延迟 2s（连接初期快速发现；协议无此要求，实现口径）。

## 二、当前产出情况

### 本会话产物清单
- **BT 内核（`src/bt/`，七模块）**：
  - `bencode.rs`：bencode 编解码（round-trip 契约、info 精确字节区间定位）；
  - `metainfo.rs`：.torrent 解析（单/多文件、announce-list 全层展开、infohash）；
  - `peer_id.rs`：BEP 20 生成 + D51 客户端缩写解析；
  - `pieces.rs`：piece 映射（选中文件 ⇄ piece 区间、write_plan 仅选中落盘，FR-02-19）；
  - `tracker.rs`：BEP 3 HTTP（BEP 23/7 紧凑 peers、query 逐字节 %XX）+ BEP 15 UDP
    （connect/announce、15s 起倍增重试）；
  - `peer.rs`：peer wire 协议（68B 握手 10s 超时、消息编解码、BEP 9 ut_metadata
    data/reject、ext handshake 最小子集）；
  - `engine.rs`：任务级引擎协程（tracker 池三来源 + 退避 D54 + min interval +
    真实 uploaded/downloaded/left 口径 D58 + completed 一次性事件 D57、监听器、
    peer 池出/入向、piece 调度 5×16KB 在飞、SHA-1 校验落盘、have 广播、
    tit-for-tat 上传 + 做种全量 unchoke、Disconnect/StopSeed/Pause/Cancel 信号），
    含两条引擎级 e2e（下载全链路；做种 + BEP 9 元数据交换）。
- **模型/配置层**：`model/bt.rs`（BtFile/BtTaskData）、Task.bt 字段、注册表扩展
  （uploaded/seed_left/bt，serde default 零迁移）、config `http_trackers`/`udp_trackers`
  （FR-02-21 + 模板注释示例三处同步）、Task::chunk_info/pieces() BT 口径（D46）、
  `model/sizefmt.rs`（fmt_size/fmt_size_pair 单源下沉，ui/text.rs re-export）。
- **应用层**：命令窗口（D44 三分流 + Shift+Tab 路径补全 + 64 行上限）、错误窗口
  （D56 66×7）、添加/修改任务：BT 对话框（TreePanel 三态勾选、trackers 多行输入、
  确认校验链、tracker 池三来源合并、D48/D49/D50 预填）、M 键分流单源、A 键/页脚
  chip/空态/帮助两组（FR-02-04/05）、BT 事件消费（进度/做种转换/停止收尾/失败）、
  BT 暂停/取消/删除按协议路由。
- **展示层**：列表做种中形态（FR-02-09② MAGENTA）、详情 BT 臂（无校验行）、头部
  真实上传统计、并发面板 BT 七列（FR-02-08）、P 弹窗 BT 元信息行、`A 命令` chip。
- **QA 基建**：`src/bin/ezr-mktorrent.rs`——.torrent 生成器（单/多文件、--piece-len、
  --announce 多条；QA 预置种子与损坏样本基材）。
- **回归收口（本会话终轮）**：clippy `--bins --tests` 清零（UDP 魔数字面量分组、
  doc 注释行首列表标记、死代码甄别删除、未用导入、hardening 壳模块声明次序）；
  `tests/hardening_v115.rs` 12 例更新为 phase-02 契约驱动（`A` 键改开命令窗口
  FR-02-04①，经分流①命令窗确认进入添加对话框，替代已退役的旧直开路径）。

### 注释删除轮（操作者指令追加轮）
- **范围**：`project/ezr/src/` 与 `project/ezr/tests/` 全部 123 个 .rs；ezr-demo 与
  文档类产物（mission/features/qa/handoff/README）不属"项目源码"，未动。
- **方法**：自研 Rust 感知剥离器（词法状态机：行/块注释含嵌套、字符串转义、
  原始字符串 r#"…"#/br#"…"#、字符字面量、生命周期、原始标识符）；剥离前后行数
  54,872 → 46,154（−8,718，与注释行统计吻合），字符串字面量内容保真校验通过。
- **联动**：`tests/gen/*_stripped.rs` 四份副本随产品源同步（gen-drift 守卫绿）；
  `#[path]` 收编的挂载套件直接引用产品源，无需额外处理。
- **行为零变更**：全量测试通过与剥离前逐数一致（5596P/0F）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试（16 套件） | 5596P / 0F | `cargo test -- --test-threads=2`（project/ezr 下；本会话按目标分块复跑） |
| 其中：bin ezr 单测 | 737P / 0F | `cargo test --bin ezr -- --test-threads=2` |
| 其中：12 个 hardening 挂载套件 | 4813P / 0F | `cargo test --tests -- --test-threads=2`（hardening 128 + engine_backfill 640 + g_appcore 557 + g_engine 552 + g_main 2 + g_proxy 16 + g_slots 173 + g_supervisor 526 + g_throttle_err 346 + g_v147 540 + ui_backfill 670 + v115 663） |
| 其中：其余 bin 目标 | 46P / 0F | `cargo test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy`（30+1+15） |
| clippy | 0 警告 | `cargo clippy --bins --tests` |
| fmt | clean | `cargo fmt --all --check` |
| 引擎下载 e2e | UDP tracker 发现 → 握手 → 4 piece 下载 → SHA-1 全过 → 产物逐字节一致 | `cargo test --bin ezr engine_downloads_via_udp_tracker_and_seeder` |
| 做种 e2e | leecher 拉块字节一致 + BEP 9 元数据 SHA-1==infohash + 上传统计真实增长 | `cargo test --bin ezr engine_seeds_to_leecher_with_metadata_exchange` |

### 对账口径
- 上一登记 4933P 为 clippy 清理轮之前的中间口径；终态 5596P = bin ezr 737 +
  hardening 挂载套件 4813 + 其余 bin 目标 46，三项复算自洽（上表分项）。
- hardening_v115 终轮 12 例由"旧 A 键直开添加对话框"改驱动为新命令窗入口，
  用例总数不变（663），属契约跟升而非用例增减。

### 待办与待批
- 无待批事项（phase-02 规格 D44–D59 全部按定稿实现，无偏离项）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `src/bt/engine.rs` 主循环复杂度审查（协程 select 面较宽，存在可提纯函数）；
  ② app/ui 层 demo 移植代码与 phase-01 存量的重复面（multiline/textfield 原语、
  对话框键路由）；③ 覆盖率基线（cargo-llvm-cov 0.9.1 已装）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者
  显式续跑）。
