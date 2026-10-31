# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-coder-bug12（2026-10-30，
> 「模板 v1.12 + BT 并发 panel 客户端列修正（qBittorrent 源码口径识别 + 左对齐）+
> ezr-demo 同步」落地轮）
> 上游交接：project/handoff.md（bug12 specifier 轮交接：规格 v1.12/v1.36 ×3 + qa ×3 +
> mission ×2，任务依据＝已批规格 + 操作者 bug12 指令，含 ezr-demo 同步与 harness 完整
> 测试授权）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经互操作夹具多轮收口。bug12 轮为操作者指令级需求修订落地轮（D61 全项），本轮
  coder 全量交付。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | bug11 轮登记沿袭 |
| **coder（本轮 bug12 落地）** | 产品 ×5 文件 + 测试适配 + runner 锚点 + ezr-demo 同步 ×3 | **已交付** | **cargo test 13 目标全绿（bin ezr 825P/0F + 集成目标 5452P/0F）+ clippy --all-targets 零警告 + fmt 干净；harness swarm_e2e PASS（ezr↔libtorrent 双向做种 sha256 一致）+ bench_multipeer PASS（sha256 一致）；QA：config-template 5/5（v1.12 逐字节）、ui-conns 18/18、persistence-config 9/9；ezr-demo cargo test 30P/0F + fmt 干净** |

### 遗留受限项（当前有效，沿袭）
- QA-SQ-13 判据固有竞态（QA 轮 refinement 待办）；BT 下载限速未接；DHT/BEP 10 完整
  扩展/uTP 属 phase-03/04；cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；TUI 恐慌
  自愈边界（bug6）；BT partial 进度不持久化；APS dry-checker 缺位；QA runners 尚有
  9 套件未在本轮冒烟（add-task/modify-task/named-proxy/retry-backoff/resume-sidecar/
  tui-display/download-engine/integrity-check/ui-pieces——迁移已就绪，完整跑绿属 QA 轮）。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 D1–D60 + bug12 轮 D61 全部裁决（两级识别/左对齐/列宽 34/模板 v1.12/Unknown
  边界，见 mission/phase-02.md）。
- **本轮实现定义值（coder 细化，规格框架内）**：
  - `client_display(v: Option<&str>, peer_id)` 单入口两级：`v` 经 sanitize（控制字符
    剔除 + 空白归并，trim 语义）非空即用；否则 identify_client 全量移植——generic
    37 项（offset+前缀子串，仅名）→ -BOW/eXeem/Experimental → TR legacy → az →
    shadow → mainline；lookup = name_map 102 项精确码匹配（区分大小写）、未命中用
    码本体；版本 `M.m.r`（tag≠0 追加 `.tag`）；显示 `<全名>/<版本号>`（decode 路径）、
    自报名原样（v 路径）、generic/特判仅名。
  - 引擎 `v` 捕获：`PeerMsg::Extended ext_id==0` 臂新增 `parse_ext_handshake`（宽容
    契约：三字段全 Option，仅含 `v` 的握手可解析）；`st.client` 就地更新，经
    conn_views 周期快照自然上屏（无新事件类型）。握手期 client 仍为 peer id 解码值，
    扩展握手到达后覆盖（对端先发 v 时首帧即正确）。
  - 极值守护：peer id 解码路径极值 = Propagate Data Client/35.35.35.35 = 33 列（≤列宽
    34）；v 自报名可超极值 → 渲染层 `truncate` 至列宽（… 收尾）。`EZ` 码不在 lt
    name_map（faithful 口径）：ezr↔ezr 对端显示 `EZ/2.0.0.0`（与 qBittorrent 对照
    同形，登记不改）。
  - 模板 v1.12 由 feature 内嵌原文**程序化提取**写入 AUTHORIZED_TEMPLATE（保证逐字节
    契约）；单测改两行式表头/174 行/无键名前缀断言。
  - fmt 补跑：cargo fmt 全仓（ezr + ezr-demo）修复沿袭漂移（bt_dialog/engine/mod/
    tracker/throttle/download/config_fallback + demo 6 文件，均为纯格式 reflow，
    行为不变；已复跑全量测试确认）。

### 实现定义值登记（累计，覆盖为最新全量）
- 沿袭 phase-01/02 全部口径（D1–D60）+ bug11 轮键位缺省表（不变）+ bug12 轮 D61。
- 客户端列显示样例集（测试已锁）：qB2500→`qBittorrent/2.5.0`、qB2531→
  `qBittorrent/2.5.3.1`、TR4030→`Transmission/4.0.3`、TR3010→`Transmission/3.1.0`
  （legacy rev 恒 0）、TR304A→`Transmission/3.4.0.10`、LT2110→`libtorrent/2.1.1`、
  AR1001→`Arctic Torrent/1.0.0.1`、BC0164→`BitComet/0.1.6.4`（peer id 路径）、
  v="BitComet 1.64"→`BitComet 1.64`（v 路径）、QX7777→`QX/7.7.7.7`（码本体）、
  exbc\0\x01L→`BitComet`（generic 无版本）、全零→`Unknown`。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×5**：
  - `model/config.rs`：AUTHORIZED_TEMPLATE 整体替换 v1.12（174 行，feature 内嵌原文
    程序化提取）；模板契约单测 ×3 更新（174 行/两行式表头/无键名前缀 + 取值范围行
    占位校验/proxy 一句头注释去前缀）。
  - `bt/peer_id.rs`：全量重写——NAME_MAP 102 项 + GENERIC_MAPPINGS 37 项（lt 源码
    逐项照抄）+ Fingerprint/decode_digit/parse_az|shadow|mainline|transmission_legacy
    + lookup + Identified 枚举 + sanitize_self_report + `client_display(v, peer_id)`
    两级入口；单测 ×10（验收例/v 优先/空白归并/四 style/generic/Unknown 边界/极值
    ≤33/大小写敏感）。
  - `bt/peer.rs`：parse_ext_handshake 宽容化（ExtHandshake{ut_metadata, metadata_size,
    v} 全 Option）；单测 ×2 更新 + v 捕获金样新增。
  - `bt/engine.rs`：Extended ext_id==0 臂（v 覆盖 st.client）；client_display(None, …)
    适配；tracker 热更新 e2e 测试适配。
  - `ui/conns.rs`：BT_COLW[5] 26→34、BT_CONTENT_W 86→94、明细客户端列 pad_left→
    pad_right+truncate（cell_left）；渲染断言 ×2 更新 + 左对齐/截断新测试。
- **测试适配**：`tests/hardening_v115.rs`（bug11 漏迁的顶层平铺键断言改 [http] 表形态
  + 旧格式回退负对照）。
- **QA runners**：`suite_config_template.py` 锚点 ×3（174 行/specifier v1.12）/两行式
  分隔头逐行断言/无旧形态/无键名前缀 regex 断言）；全 runners grep 无其他残留锚。
- **ezr-demo（仓库根 ezr-demo/）×3**：`ui/conns.rs`（客户端列 34、左对齐+truncate、
  内容宽随求和自动 94）、`app/model.rs`（BT_CLIENTS 池 10 项更新为新口径形态，含
  `BitComet 1.64` 自报样例与 `qBittorrent/5.1.2` 解码样例）、权威文档 §5.4 客户端列
  条目重写 + 头部修订注记（v1.36 口径：两级识别/列宽 34/左对齐/极值/截断）。
- **notes**：environment.md 终端伪影条目升级（布尔判定探针法，bug12 实证）；testing.md
  新增「配置解析语义变更时单元测试同在迁移清单内」条目（v115 漏迁案例）；INDEX 同步。

### 验证证据
| 验证项 | 结果 | 说明 |
|---|---|---|
| cargo test --bin ezr | 825P/0F | fmt 后复跑确认；peer_id 10 项新测试 + conns 新断言全绿 |
| cargo test（集成目标 ×12，分块前台） | 5452P/0F | hardening 128 / engine_backfill 728 / g_appcore 637 / g_engine 632 / g_main 2 / g_proxy 16 / g_slots 181 / g_supervisor 606 / g_throttle_err 393 / g_v147 620 / ui_backfill 758 / v115 751 |
| cargo clippy --all-targets | 零警告 | ezr 全目标（1 处 manual_is_ascii_check 已修） |
| cargo fmt --check | 干净 | ezr + ezr-demo（全仓补跑修复沿袭漂移后） |
| harness swarm_e2e | PASS | 4MB：ezr 与 libtorrent leecher 双向做种、sha256 逐字节一致、HTTP announce 可观察 |
| harness bench_multipeer | PASS | 8MB 单 seeder 5.4s、avg 24.7MB/s、sha256 一致 |
| QA suite_config_template | 5/5 P | v1.12 模板逐字节（cmp）、两行式表头、无键名前缀、行为默认 |
| QA suite_ui_conns | 18/18 P | 面板改版端到端（列宽 34/左对齐/滚动上限） |
| QA suite_persistence_config | 9/9 P | 点路径回退/钳制（config 模块改动回归） |
| ezr-demo cargo test | 30P/0F | 面板/模型同步后全绿 |

### 对账口径
- 本轮产品代码净改动 ×5 文件 + 测试适配 ×2 + runner ×1；测试计数 bin 819→825
  （peer_id 重写净 -8 旧 +14 新、conns +1）；集成目标分块跑绿（全量单跑 580s 超时，
  按 testing.md 分块纪律逐目标执行）。
- 极值账：新映射表最长全名 Propagate Data Client(21) + /35.35.35.35(11) = 33 列 →
  列宽 34、内容宽 18+8+8+8+8+34+4+6 = 94（content_w_matches_colw 守护）。

### 待办与待批
- **待办（QA 轮）**：①其余 9 套件完整跑绿；②SQ-13 判据 refinement；③APS dry-checker
  补装补跑；④QA-BE-06 的 fixture 构造样例（-AR1001-/-BC0164- peer id 路径 + v 自报
  路径对账）在 QA 会话脚本化。
- **待批（沿袭）**：监听端口随机范围/死 tracker 静默窗可视状态/做种任务重启不重挂/
  多协议喂窗公共化/BT partial 不持久化；`EZ` 码不在 lt 映射表（faithful 口径，ezr↔ezr
  显示 `EZ/2.0.0.0`——若操作者希望自识别 `EZR Downloader` 需钦定映射表外加项）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：①peer_id.rs
  重写后的 helper 参数链与 Fingerprint 结构（String name 可考虑 [u8;2] 优化，属
  cleaner 职责）；②conns.rs cell_left 闭包与 ip_display 的 pad_right+truncate 重复
  形态；③引擎 Extended ext_id==0 臂的 v 更新频率（每连接至多数次，无热路径风险）。
- 给操作者的手测口径：①首次启动生成 v1.12 模板（两行式表头 `# ===== 公共配置 =====` +
  `# [common]`、注释无键名前缀）；②BT 下载中并发面板：客户端列左对齐、列宽 34、
  libtorrent 对端显示 `libtorrent/2.1.1`（v 自报）或 `libtorrent/2.1.1.0` 形态（peer id
  解码，取决于对端是否发 v）；③真实 swarm 中 BitComet 对端显示 `BitComet 1.64`（与
  qBittorrent 一致——验收例）；④qBittorrent 对端显示 `qBittorrent/<版本>`。
- 后续链：coder → cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
