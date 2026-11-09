# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）——**下游 cleaner 待操作者显式启动**
> 路径：specifier → coder（本轮，2026-11-09）⇒ cleaner → architect（2026-11-07）→ hardender（操作者指示跳过并关闭）→ QA 终验轮（2026-11-07，Done）→ specifier 修订轮（2026-11-09）→ **coder 修订实现轮（本轮）**
> 会话：ezr-coder-ws-webrtc-20261109（任务 = 依据 phase-02 v1.15 实现 tracker 统一 + ws(s) 支持 + BT WebRTC 数据传输 + peer_connection_protocol + config fail-fast；上游交接 = project/handoff.md specifier 修订轮）
> 环境注记：本沙箱中途重置一次（rustup/仓库/未提交改动全失），按 plan.md 勾选恢复续做；恢复后实行「每里程碑 git commit」纪律，工作树当前全部已提交（基线 381bdf7 之上逐批提交）。rustup（TUNA 前台装齐 nightly 1.101）与 webrtc 0.14/tokio-tungstenite 0.24 依赖树就绪。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，v1.45）+ `mission/` 4 份 phase 详述（phase-02 v1.15 为本轮实现依据、phase-01 v1.48 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder 历轮 | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug27 修复 | 已交付 | 历轮全绿 |
| cleaner 历轮 | 保持行为清理 + 四件度量；bug27 批次三单点化 | 已交付 | bin/集成/clippy 全绿 |
| architect（2026-11-07） | 四阶段评审 + 五项裁决 + arch_check 17 规则 + 属性测试 76P | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者指示跳过并关闭，不回补） | 不适用 | — |
| QA + 复验 + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化等 12 项 | 已交付 | 全绿（bin 954P / 集成 6378P / 套件 187P / CPD 181 零漂移） |
| specifier（2026-11-09） | phase-02 v1.15（FR-02-34/35 新增 + D70）+ features 9 份修订 + qa 8 份同步 + harness mini_ws_tracker 规格 | 已交付 | APS parser+dry-check 9/9 exit=0 |
| **coder（本轮 2026-11-09）** | FR-02-21/31/33/08 v1.15 + FR-02-34/35 全量实现（清单见当前产出节）；ezr-demo 同构同步 | 已交付 | bin 972P + 11 集成目标 5896P 全绿；clippy 0 警告；fmt 干净；harness 冒烟全过 |
| **bug28 轮（2026-11-09）** | 操作者四项指令（①多文件 b/b 双层修复 ②多文件删除目录树 ③WebRTC 地址列裸 hex（D71⑤ 改判）④扩容重连调优）+ features 09 v1.42/01-resume-sidecar v1.47 注记 | 已交付 | bin 983P + 集成 12 目标 6761P 全绿；clippy 0 警告；fmt 干净 |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链长期受限项，历轮维持）。
- **变异实跑未执行**（hardender 经操作者 20261109 裁决定为 six-pack **可选节点**——默认链不含、按需显式 begin；`--file` 分块口径仅在启用时适用，悬置转政策性关闭）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记）。
- **本轮新增：ws(s) tracker announce 无代理面**（FR-02-34④「announce 各按自身协议」——ws 臂未接 reqwest 等价代理通道，ws/wss tracker 经命名代理的场景不覆盖；QA 轮无对应判据，登记为已知边界）；**操作者 20261109 chat 裁决：确认不补齐，作为长期已知边界关闭**。
- **沿specifier轮待批**：D70（题目录重建映射与「协议」列宽 8）——**已操作者 20261109 chat 批准定案，待批撤销**。**沿 QA 终验轮未决三项（20261109 操作者 chat 全部裁决，结案）**：①判重谓词「已完成任务」张力——钦定「同 URL 即重复任务，一律拒绝（含已完成）」；实现核证 url_task_exists 本无状态过滤 = 现实现即钦定口径（零代码变更），FR-01-26 引用句「非完成」限定删除（phase-01 v1.49 注记），结案；②变异加固悬置——hardender 定为 six-pack **可选节点**（CONSTITUTION/chains/INDEX/hardender SKILL 同步标注），默认链不再含 hardender，按需显式 begin；③QA-01-11 预填断言口径——钦定「以能保证产品准确性与正确性的做法」，实施 = 双锚口径（短路径整值逐字符断言恢复全路径断言 + 深路径截断渲染尾部两段锚 + 落盘证据；runner 新增 QA_ONLY 单用例复跑入口），结案。

### 实现定义值登记（累计，当前有效口径）
- 历轮 D44–D69 沿 QA 终验轮登记不变；**新增 D71**（coder 轮实现定义值）：①WebRTC 信令 offer 角色按 peer_id 字节序确定分裂（小于对端者发 offer，防 glare；规格未锁定信令机制，属实现自由度）；②ws announce JSON info_hash/peer_id = 小写 hex（peer 含 ip/port/peer_id，peer_id hex；与 mini_ws_tracker.py 同契约自洽）；③offer_id = `our_peer_id 前 4 字节 hex-{seq:04x}` 进程级单调；④ws tracker 信令相关常量：OFFERS_PER_ANNOUNCE=4、PENDING_OFFER_CAP=8、offer 超时 45s、ICE gather 等待 3s、outbound 队列 64 帧；⑤WebRTC 连接并发面板地址列 = `WebRTC:<peer_id 前 5 字节 hex>`（规格未锁定 WebRTC 地址形态）；⑥DataChannel label = "data"、可靠有序默认配置；⑦trackers 判重归一 = scheme 小写化后全串比较；⑧ws announce 超时 connect 10s / response 15s。
- **D71 追记（操作者 20261109 chat 裁决，八项落账生效）**：①offer 角色分裂/③offer_id 格式/④信令常量——钦定「与参考实现同策略」；核证：qBt/libtorrent 无 WebRTC 实现（无策略可循），本线协议参考 = WebTorrent 家族（ws 信令协议出处）：offer 随 announce 携带 + to_peer_id 定向 answer + offer_id 匹配去重——ezr 协议层已同构，peer_id 字节序角色分裂为防 glare 的确定性补充（WebTorrent 无角色分裂、靠 offer_id 去重容忍竞态），裁定 = 维持现实现（线协议兼容、不削弱）；②小写 hex/⑦判重归一/⑧超时——批准维持；⑤地址列 = peer_id 前 5 字节 hex（现实现 `WebRTC:<前 5 字节 hex>` 保留前缀作列内自识别，「协议」列已显 WebRTC）；⑥**改判**——DataChannel label = "data_channel"（原 "data" 退役；可靠有序默认配置不变；发起侧 create_data_channel 单点改码，应答侧 on_data_channel 随对端；已重验），实现定义值转操作者钦定。**本轮重验（沙箱二次重置后从 my-project 归档 081517 恢复重建 git 基线 839b8e4，nightly 1.102 全量重装重编）：cargo check --all-targets 0 error；bin 972P/0F（与重置前基线同数）；clippy 全目标 0 警告；fmt 干净；QA_ONLY=QA-01-11 实跑 P=1/0F/0S（13.8s，双锚断言生效）**。
- **新增 D72（bug28 轮实现定义值，操作者 20261109 chat 指令即裁决，已实现并验证）**：①BT 落盘布局钦定——多文件 torrent 落盘 = `save_dir/<info.name>/…`（metainfo 多文件 path 首段即 info.name），单文件 = `save_dir/<info.name>` 文件；修复前缺陷 = file_abs_path 多段 rel 再叠任务名（`/a/b/b` 双层，bug28①）；②「删除任务和文件」多文件 BT 语义 = `save_dir/name/` 目录树整删（remove_dir_all），单文件仍文件删（bug28②）；③并发面板 WebRTC 地址列 = 对端 peer_id 前 5 字节 hex **裸值**（D71⑤ 改判：`WebRTC:` 前缀退役，「协议」列已显 WebRTC，bug28③ 调整项）；④扩容重连调优（bug28④ 调优项）：修改并发扩容/仍有缺口时全 tracker 提前 re-announce（AnnounceCtx.reannounce_notify：ws 持久会话内层 select 即断重发携新 offers；http/udp 顶层同步提前；尊重 tracker min_interval，未到最早许可时刻补足剩余等待）+ 本端主动收口（缩容 shed/空闲置换 Drop/自身拒绝）的 WebRTC peer 记 mark_peer_idle（撤活跃集+清冷却，不记失败）——区别于异常断开 mark_peer_closed 惩罚性退避；TCP 候选语义不变（self/dup 删候选、其余 fails+1 沿 QA-BE-15 口径）；⑤WsAnnounceResponse 增 min_interval 字段解析（"min interval"，此前 ws 响应未解析该字段——D72⑤ 顺带补齐契约面）。

## 二、当前产出情况

### 本会话产物清单（产品代码 + UI + harness + 经验）
1. **TrackerUrl 三族（FR-02-21 v1.15）**：`bt/tracker.rs` 增 `Ws` 变体；parse 白名单五族 {http,https,ws,wss,udp} 大小写不敏感（udp 保留 host:port 校验）。
2. **config 单键 + fail-fast（FR-02-35/FR-02-21②）**：`model/config.rs` [bt] `trackers` 单键（http_trackers/udp_trackers 退役=未知键静默忽略）+ `peer_connection_protocol`（默认 ["tcp","webrtc"]，元素限 tcp/webrtc 大小写不敏感去重）；全 33 键语义非法值启动失败（FR-01-71 回退臂退役；报错含配置文件路径/键点路径/原因，exit 1；listen_port 按规格保留回退面）；模板 v1.16 与钦定原文逐字节一致（python 从 feature 内嵌原文提取拼接）；旧写法限速（KB/MB/GB）= 非法拒绝启动。
3. **WebRTC DataChannel 传输臂（FR-02-34）**：新模块 `bt/webrtc.rs`——PeerTransport/PeerTransports 开关、DataChannelStream（RTCDataChannel→AsyncRead/AsyncWrite 适配：收 on_message 队列 + 发后台任务有界队列背压）、SignalState（swarm peer 集合/候选退避/pending offers·answers）；`bt/engine.rs` run_peer_loop/peer_pump 泛化 `<S: AsyncRead+AsyncWrite>` 复用全消息族 peer wire；ws tracker 信令中继（offer 定向 to_peer_id + tracker 转发 offers_to_answer/answers；角色分裂防 glare；无跨协议回退——TCP 候选/WebRTC offer 各按自身退避）；入连 accept 与 fill 拨号按允许集门控（不允许传输一律不发起不接受）。
4. **ws(s) tracker 客户端**：`announce_ws`（tokio-tungstenite，文本帧 JSON；wss 自签信任口径 = accept-any verifier，QA-BE-20③）；spawn_tracker Ws 臂 + stopped 事件。
5. **UI 协议列（FR-02-33 v1.15/FR-02-08 v1.15）**：`ui/conns.rs` BT_COLW[7] 4→8、BT_CONTENT_W 111→115、收缩基线 77→81、协议数据单元格左对齐值 TCP/WebRTC（ConnStat.protocol→BtConnView.protocol→BtConnRow.protocol 全链路）；视口 107/115 基线平移；末列吸附区间随列宽 8。
6. **toast + 三来源分流（FR-02-31 v1.15）**：`app/bt_dialog.rs` toast 钦定文案 `⚠ 不支持的 tracker 协议（仅 http(s)/ws(s)/udp）：<44 字硬截尾>`；merge_trackers 单键三来源合并（config→announce→announce-list→手输去重保序）+ 白名单外 announce-list 行忽略+警告 + 手输整单拒。
7. **harness（QA-BE-20）**：`harness/tracker/mini_ws_tracker.py`（websockets 单依赖；ws 17905/wss 17906 自签 TLS；注册/互换 peers/complete·incomplete/offers·answers 定向中继/断连容错）+ `cert_gen.py`；**冒烟全过**（双臂注册互换、计数、信令中继、wss TLS）；harness/README.md 实现状态登记。
8. **ezr-demo 同构同步（任务③）**：`ezr-demo/src/ui/conns.rs` 协议列 8/左对齐/基线 81/内容宽 115；docs《UI界面及交互设计规范》v1.43 轮修订注（六点同构口径）；demo cargo test 44P 全绿；守卫脚本 verify_ezr/verify_phase2_ui 与基线逐字对账**零新增失败**（既有 FAIL 为先在项，登记移交 cleaner/architect）。
9. **notes 沉淀**：`project/notes/` architecture.md（WebRTC 适配器与信令设计、webrtc-rs 五坑、MutexGuard 假阳性规避）/ testing.md（accept_async 本地 ws 单测、hardening 垫片第三形态、#[path] 基目录陷阱、每里程碑 commit 纪律）/ environment.md（重置后镜像复测、10G 盘三件套纪律）+ INDEX.md 追加。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单元测试 | 972P / 0F | `cargo test --bin ezr` |
| 集成 11 目标 | 5896P / 0F（hardening 240、engine_backfill 870、ui_backfill 900、v115 893、g_v147 751、g_appcore 768、g_main 2、g_slots 243、g_proxy 16、g_supervisor 737、g_throttle_err 476） | 逐目标 `cargo test --test <t>` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| fmt | 干净 | `cargo fmt --check` |
| harness 冒烟 | ws/wss 双臂 + 信令中继 + 计数全过 | `python3 harness/tracker/mini_ws_tracker.py` + websockets 客户端探针 |
| 模板逐字节 | v1.16 = feature 钦定原文 | python diff（提取脚本见本轮 git 历史） |
| demo 守卫 | 与基线零漂移 | verify_ezr.py / verify_phase2_ui.py（stash 基线对照） |

### 对账口径
- 代码面：src/bt/{tracker.rs,webrtc.rs,engine.rs,mod.rs}、src/model/config.rs、src/ui/conns.rs、src/app/{bt_dialog.rs,mod.rs,engine/evt.rs,engine/bt.rs,keys.rs,mouse.rs}、src/engine/{mod.rs,supervisor/mod.rs}、src/property_tests.rs、tests/hardening*/（垫片）、Cargo.toml/lock。
- 测试增量：新增单测 18 项（ws 解析/JSON 契约/announce 往返/三族白名单/transport parse/SignalState 角色分裂/backoff/offer_id/trackers 校验/pcp 校验/try_load 报错三要素/PC-04 33 行表/v1.16 模板）+ 既有回退断言全量翻转。
- git：全部改动已提交（消息前缀 coder:，批次 A-F + 修正 3 笔）。

### 待办与待批
- **操作者已批（20261109 chat）**：①D70 题目录重建映射与「协议」列宽 8——批准定案，待批撤销（phase-02 v1.15 注记与 D70 行已同步追记）；②ws(s) tracker announce 无代理面——确认不补齐，作为长期已知边界关闭。原「mission/phase 注记审批入口」随之办结；③D71 八项实现定义值逐项裁决（①③④与参考同策略维持/②⑤⑦⑧批准/⑥ label 改判 data_channel 已改码，详见 D71 追记）；④QA 终验轮沿袭未决三项全部结案（判重钦定一律拒绝/hardender 定可选节点/QA-01-11 双锚断言实施），详见上文遗留受限项。
- **移交 cleaner**：①本轮新增代码的度量四件（CRAP/CPD/DRY/覆盖）与保持行为清理；②ezr-demo 守卫脚本先在 FAIL（A2-A5/B0/M3/P10/R1-3 等，stash 基线对照证实与本轮无关）裁决修否；③ws announce 无代理面——**操作者已裁决不补齐（20261109 chat），此项关闭无需处理**。
- **QA 轮注意**：mini_ws_tracker 依赖 python `websockets`（沙箱已预装 16.0）；wss 臂客户端须按 harness 自签信任口径（ezr 已 accept-any）；e2e 全链路（两个 ezr 实例经 ws tracker + WebRTC 下载）属 QA-BE-19/20 判据，coder 轮仅单测+harness 冒烟。

### 移交建议
- 下一步运行 **six-pack/cleaner**（以本文件为上游交接内容）；之后沿 six-pack 链 architect → QA。
- 操作者审批入口：handoff 本节待办与待批、D71 实现定义值登记。

---

## 诊断轮（2026-11-09 下午）：「webtorrent 官方测试 .torrent 不能下载」排查与修复

### 操作者报告与结论
操作者报告用 webtorrent 官方测试 .torrent 无法下载。实测诊断（沙箱内 ezr debug 构建 + 真实互联网）：

1. **Sintel.torrent（webtorrent.io 官方，129.3MB）在当前工作树构建下两次 100% 下载成功**（约 100s/2.1MiB/s，SHA 产物一致）。数据面走 announce-list 的 udp tracker（opentrackr 等）TCP peer（bt-hs-ok 42 个：qBittorrent/Transmission/Deluge/rTorrent/WebTorrent 桌面）。用户遇到的失败对应**本工作树 bb6a05c 之前的旧二进制**（ws 持久会话/二进制 id/answer 顶层契约改造之前）。
2. **wss 信令面对官方 tracker 全通**：wss://tracker.openwebtorrent.com announce 被接受（interval/complete/incomplete 正常回）、ezr 盲 offer 收到官方 WebTorrent 客户端 answer（answer-accepted）、官方 offer 到达并被正确应答（answer-sent）。另两个 tracker 已停运：tracker.btorrent.xyz 连接拒绝、tracker.fastcast.nz 域名失效（非 ezr 问题）。
3. **真实网络下 dc-open=0 的归因**：沙箱 NAT 对称型（srflx 映射端口 ≠ rport，且对端多为浏览器/NAT 后 peer），ICE 连通性检查无法穿透——环境限制，非实现缺陷。同机受控实验证明数据面完好（见下）。

### 根因实锤（与官方 webtorrent-cli 6.0.1 受控互通实验）
`webtorrent-cli`（官方 bittorrent-tracker 客户端栈）+ 本地 tracker + ezr 三方受控实验，逐层抓包/打桩定位：

- **tracker 响应缺 `action:'announce'` 与 `info_hash` 字段 → 官方客户端必死**：bittorrent-tracker `websocket-tracker._onSocketData` 按 `data.action` 分派，缺省走 `_onSocketError(new Error('invalid action in WS response: undefined'))` → **销毁全部 pending offers 与 socket**；补 action 后又在 `_onAnnounceResponse` 的 `bin2hex(data.info_hash)` 因 undefined **TypeError 崩溃**。通过 TCP 代理抓包 + streamx/simple-websocket 打桩（Node ESM preload 不生效须直接 patch 文件）拿到调用栈实锤。
- **offer/answer 中继必须顶层即时推送**（`{action:'announce', info_hash, peer_id:<from>, offer|answer:{type,sdp}, offer_id}`）：官方客户端不解析响应内捎带数组（offers_to_answer/answers 是 ezr/harness 扩展契约）。
- **修复**：`harness/tracker/mini_ws_tracker.py` ①响应补 action/info_hash；②offer/answer 中继在捎带数组之外**新增顶层即时推送**（CONNS 注册表，双投递幂等：ezr busy/orphan 容错天然去重）。QA-BE-20 规格注记同步更新（qa/09-bt-engine-qa.md）。
- **修复后三方冒烟**：ezr ↔ mini_ws_tracker(修复版) ↔ webtorrent-cli 6.0.1 **纯 WebRTC 面端到端下载成功**（6MB/38s，SHA-256 一致；dc-open → rtc-ready → bt-hs-ok client=WebTorrent/0.3.0 全链路日志）。exp_tracker（诊断用推送式 tracker）同样 8MB/24s 全过。

### 改动清单（本轮）
- `harness/tracker/mini_ws_tracker.py`：响应契约 action/info_hash + 顶层推送（docstring 同步）。
- `qa/09-bt-engine-qa.md`：QA-BE-20 harness 行为要求注记（真实契约两字段 + 推送语义）。
- ezr 源码**零改动**（诊断插桩已全部移除；上一轮未提交的 ws 持久会话改造即本次互通通过的基础）。
- 诊断产物（实验脚本/日志）：沙箱 `/home/z/ezr-diag/`（不入库）。

### 验证证据
| 验证项 | 结果 |
|---|---|
| Sintel 真实 swarm 下载（TCP 面） | 129.3MB ×2 次 100%，sha 一致，2.1MiB/s |
| wss 信令面（openwebtorrent.com） | announce 接受 + offer/answer 双向中继成功 |
| 纯 WebRTC 数据面（exp_tracker 受控） | 8MB/24s，sha 一致，dc-open/bt-hs-ok |
| 三方冒烟（mini_ws_tracker 修复版） | 6MB/38s，sha 一致，WebTorrent/0.3.0 对端 |
| bin 单元测试 | 978P / 0F（含上轮改造新增用例） |

### 待办（20261109 复验轮已清零）
- ~~集成测试与 clippy/fmt 复跑~~——**已结清（复验轮）**：集成 **12 目标 6720P / 0F 全绿**（hardening 246、hardening_engine_backfill 876、hardening_ui_backfill 906、hardening_v115 899、hardening_g_v147 757、hardening_g_appcore 774、hardening_g_engine 770、hardening_g_main 2、hardening_g_slots 249、hardening_g_proxy 16、hardening_g_supervisor 743、hardening_g_throttle_err 482；注：目标名自 081517 归档基线起已统一带 `hardening_` 前缀并新增 hardening_g_engine，上表 11 目标行系改名前旧称）；clippy --all-targets **0 警告**；fmt --check **干净**；bin 沿诊断轮 978P/0F。磁盘紧张纪律（CARGO_INCREMENTAL=0/DEBUG=0）全程启用。
- ~~归档重打替换 my-project 副本~~——**已完成**：改用 `git archive HEAD`（7b6d5b8）重打，449 文件与 git ls-files 严格一致（剔除旧 120500 归档误入的 `__pycache__/*.pyc` 杂物），产物 `/home/z/swarm-20261109-124117-coder-reverify.tar.gz`，已替换 my-project 副本。
- **coder 轮至此无未完成事项**；开放项仅剩移交 cleaner 两件（新代码度量四件与保持行为清理 / ezr-demo 守卫先在 FAIL 修否）与下游节点启动（cleaner → architect → QA，待操作者显式 begin）。

---

## bug28 轮（2026-11-09 下午）：操作者四项指令（两 bug + 一调整 + 一调优）

### 操作者指令与处置
1. **bug①多文件 torrent 落盘双层 `/a/b/b`**：根因 = `bt/engine.rs` `file_abs_path` 对多段 rel 再叠任务名，而 metainfo 多文件 path 首段已是 info.name（单文件 path=name 走短分支故无此问题）。**修复 = 落盘路径恒为 `save_dir/rel`**（`file_abs_path(save_dir, rel)` 去任务名参数；`open_selected_file_handle`/`upload_read_plan` 签名同步收敛）；钦定布局登记 **D72①**，features/09 v1.42 注记。影响注记：修复前旧双层布局落盘的历史任务，重启重校验将按新路径判定不符重下（bug 修复的正常代价）。
2. **bug②多文件 BT `d` 删除任务和文件删不掉目录**：根因 = `app/dialogs.rs` `delete_task_files` 仅 `remove_file` 三形态，对目录（多文件落盘 = `save_dir/name/`）无效且静默。**修复 = 目录按 `remove_dir_all` 整树删、单文件仍文件删**（.downloading/.ezr 附带形态不变）；D72②，features/01-resume-sidecar v1.47 注记。
3. **调整③WebRTC 连接地址列显示**：操作者钦定裸值（协议列已显 WebRTC）。**实现 = `WebRTC:` 前缀退役，地址 = 对端 peer_id 前 5 字节 hex**；D71⑤ 改判入 D72③。
4. **调优④扩容后 peers 难连回**（Sintel 9→5→9 只回 6）：根因双链——(a) offer 只随 announce 周期发出（tracker interval 分钟级）；(b) shed 断开的健康 WebRTC peer 被 `mark_peer_closed` 记失败进指数退避（base 5s×2^fails，cap 120s），`plan_offer_targets` 冷却滤除。**修复 = 双管**：(a) `AnnounceCtx.reannounce_notify`——修改并发扩容/仍有缺口时全 tracker 提前 re-announce（ws 持久会话内层 select 即断重发携新 offers；http/udp 顶层 select 同步提前；**尊重 tracker min_interval**，未到最早许可时刻补足剩余等待；`WsAnnounceResponse` 顺带补 `min interval` 字段解析=D72⑤）；(b) 本端主动收口（缩容 shed/空闲置换 Drop/自身拒绝）的 WebRTC peer 改记 `mark_peer_idle`（撤活跃集 + 清冷却，不记失败），异常断开仍惩罚性退避；D72④。TCP 候选语义不变（self/dup 删候选、其余 fails+1，QA-BE-15 口径）。

### 改动清单（本轮）
- `src/bt/engine.rs`：file_abs_path/open_selected_file_handle/upload_read_plan 签名收敛与去叠名；reannounce_notify 接线（AnnounceCtx/start_tracker_pool/spawn_tracker 顶层 select/ws_tracker_session 内层 select/Reconfigure 触发点）；WebRTC Closed 分支 mark_peer_idle/mark_peer_closed 分流；地址列裸 hex；测试调用点同步 + 新增 4 测（file_abs_path 单层 / ws 立即 re-announce / ws min_interval 尊重 / 既有 shed-then-grow 沿用）。
- `src/bt/webrtc.rs`：SignalState::mark_peer_idle 新增 + 单测（signal_state_idle_close_allows_immediate_reoffer）。
- `src/bt/tracker.rs`：WsAnnounceResponse 增 min_interval 字段与解析。
- `src/app/dialogs.rs`：delete_task_files 目录树分支。
- `src/app/engine/mod.rs`：新增 delete_task_files_removes_multi_file_directory_tree 测。
- 规格：`features/09-bt-engine.feature` v1.42 修订注（①②③④）、`features/01-resume-sidecar.feature` v1.47 修订注（场景 12 多文件语义）。

### 验证证据
| 验证项 | 结果 |
|---|---|
| bin 单元测试 | 983P / 0F（978 + 新增 5） |
| 集成 12 目标 | 6761P / 0F 全绿（hardening 247、hardening_engine_backfill 881、hardening_ui_backfill 911、hardening_v115 904、hardening_g_v147 762、hardening_g_appcore 779、hardening_g_engine 775、hardening_g_main 2、hardening_g_slots 250、hardening_g_proxy 16、hardening_g_supervisor 748、hardening_g_throttle_err 486） |
| clippy 全目标 | 0 警告 |
| fmt | 干净 |

### 待办
- 归档重打替换 my-project 副本（本节提交后执行）。
- 移交 cleaner 范围注记：本轮新增代码（engine/tracker/webrtc/dialogs 五文件）并入「新代码度量四件与保持行为清理」范围。
- QA 轮注意：QA-BE-19 观察面新增「扩容提前 re-announce / shed 不记失败 / 地址列裸 hex」可观察行为（判据未新增，登记于 features/09 v1.42 修订注）；多文件落盘布局与删除目录树建议 QA 轮补 e2e 判据（QA-BE-08/RS-12 关联面）。
