# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（规格修订轮 2026-11-09，本轮已完成）——**下游 coder 待操作者显式启动**
> 路径：specifier → coder → cleaner → architect（2026-11-07）→ hardender（操作者指示跳过并关闭）→ QA 终验轮（2026-11-07，Done）→ **specifier 修订轮（本轮，2026-11-09）⇒ coder（待启动）**
> 会话：ezr-specifier-ws-webrtc-20261109（任务 = tracker 统一 + ws(s) 支持 + BT WebRTC 数据传输 + config peer_connection_protocol 与严格校验修订轮规格；上游交接 = project/handoff.md（QA 终验轮）+ 操作者 chat 修订指令与 Inversion 选择题作答「1a 2b 3a 4a 5a」）
> 环境注记：本沙箱会话曾中断，按 plan.md 勾选续做（重 begin，工作区无损）；rustc/cargo 本沙箱未装（操作者另有前台安装指令）；APS 验收工具走 Go 版装机（bb release 资产不可达，见 notes/environment.md）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.45 本轮修订**）+ `mission/` 4 份 phase 详述（**phase-02 v1.15 本轮修订**、phase-01 v1.48 注记、phase-03/04 计划登记稿）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程（phase-02 v1.14 时点） | 已交付 | 历轮对账在案 |
| coder 历轮 | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug27 修复 | 已交付 | 历轮全绿 |
| cleaner 历轮 | 保持行为清理 + 四件度量；bug27 批次三单点化 | 已交付 | bin/集成/clippy 全绿 |
| architect（2026-11-07） | 四阶段评审 + 五项裁决 + arch_check 17 规则 + 属性测试 76P | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者指示跳过并关闭，不回补） | 不适用 | — |
| QA + 复验 + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化等 12 项 + cmd_complete 补测 +3 | 已交付 | 全绿（bin 954P / 集成 6378P / 套件 187P / CPD 181 零漂移） |
| **specifier（本轮 2026-11-09）** | phase-02 v1.15（FR-02-21/31/33/08 v1.15 注记 + FR-02-34/35 新增 + D70）+ phase-01 v1.48 + mission v1.45；features 9 份修订；qa 8 份同步；harness mini_ws_tracker 规格 | 已交付 | APS parser+dry-check 9/9 exit=0（findings 全为 aps.md 登记的中文分词假阳性类） |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链长期受限项，历轮维持）。
- **变异实跑未执行**（hardender 关闭，`--file` 分块口径悬置）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记）。
- **本轮新增：D70 待批**（Inversion 题目录重建映射与「协议」列宽 8，见待办与待批）。

### 实现定义值登记（累计，当前有效口径）
- 历轮 D44–D69 沿 QA 终验轮登记不变；**新增 D70**（specifier 裁决 7 项，待批）：①题目录重建登记；②toast `<url 截尾>` = 首 44 显示字符硬截尾无省略号（沿现行代码口径）；③「协议」列宽 8（操作者作答 2b）；④scheme 白名单五族 {http, https, ws, wss, udp} 大小写不敏感，announce-list 白名单外 scheme 保留忽略+警告；⑤[bt] peer_connection_protocol 默认 `["tcp","webrtc"]`（代码默认 = 模板示例值，缺省双栈全开）；⑥留空 trackers 默认 4 条（http/https/udp）不变；⑦mini_ws_tracker 端口 ws 17905 / wss 17906。

## 二、当前产出情况

### 本会话产物清单（零产品代码改动；规格与需求文档面）
1. **需求文档**：`mission.md` v1.45（大纲注记八项：trackers 统一/ws(s)/toast 钦定文案/announce-list 入池/peer_connection_protocol/WebRTC 承载/协议列 8/config 全局规则与退役规则 + 分期索引行 + 标题版本）；`mission/phase-02.md` v1.15（修订注含题目录重建登记 + FR-02-21/31/33/08 v1.15 注记 + **FR-02-34**（WebRTC DataChannel 传输 + peer_connection_protocol 全局开关，无跨协议回退）与 **FR-02-35**（config fail-fast 全局规则：未知值/重复项启动失败、TOML 解析失败拒绝启动、标量语义非法拒绝启动、旧条目退役不兼容直接废弃）+ D70 + 验收清单第 8 条）；`mission/phase-01.md` v1.48（FR-01-71 逐键回退臂全局退役注记，钳制臂一并翻转；未知键静默忽略口径维持）。
2. **features/**（9 份）：`07-add-bt` v1.43（场景 11 反转：ws(s) 同权放行 + 白名单外 ftp 拒绝 + toast 钦定文案 + 大小写不敏感示例行）；`08-modify-bt` v1.8（场景 09 反转：ws(s) 放行即热生效 + ftp 拒绝）；`11-trackers-window` v1.13（场景 06 反转：ws(s) 入池照常显示 + 白名单外 scheme 三来源分流处置）；`09-bt-engine` v1.41（场景 05 分型 = http(s)/ws(s)/udp 三族 + wss 示例行、场景 09 统一单键 trackers + 旧两键废弃 + config 白名单外启动失败、场景 14 非法值臂翻转、**场景 19 新增**：peer_connection_protocol 四臂全局开关 + WebRTC DataChannel 传输语义、**场景 20 新增**：ws(s) tracker harness e2e + 未知值/重复项启动失败）；`04-ui-conns` v1.43（场景 08：协议列 4→8、值 = TCP/WebRTC 按连接实际传输、内容宽 111→115、收缩基线 77→81、视口 ≥107 完整可见、≥115 维持 34、末列吸附区间 (内容宽−8, 内容宽)）；`01-config-template` v1.16（键表与模板块：http_trackers/udp_trackers → trackers + peer_connection_protocol（默认 `["tcp","webrtc"]`），62 处取值范围注释回退口径翻转）；`01-persistence-config` v1.21（场景 04 行为翻转：33 行非法值全改「启动失败并报错」+ 数组两行、场景 12 退役键废弃判据）；`01-throttle-proxy` v1.12（场景 04/05 回退臂翻转、场景 09 旧写法行剪除）；`01-add-task` v1.45（场景 20 旧写法行剪除）。
3. **qa/**（8 份）：QA-AB-11 / QA-MB-09 / QA-TW-06 反转改写；QA-BE-05/09 修订 + **QA-BE-19/20 新增**（WebRTC 四臂与无跨协议回退判据；ws(s) tracker harness `mini_ws_tracker.py` 行为规格：WebSocket 文本帧 JSON 消息契约、ws 17905/wss 17906 双监听自签、多 peer 注册互换、started/completed 事件）+ 头注 v1.41；QA-UC-08 v1.43 增判据（⑥⑦⑧）；QA-PC-04 行为翻转 + 头注 v1.21；QA-TP-04/05 翻转、QA-TP-09 片段翻转；QA-CT v1.16 同步注记。
4. **harness/README.md**：目录结构登记 `mini_ws_tracker.py`（[v1.41 规格登记，coder/QA 轮实现]）+ 端口段 17905/17906。
5. **notes 沉淀 + INDEX 同步 + RAG 索引增量刷新**（192 文件/1613 分块，健康）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| APS parser（Go 版） | 9/9 exit=0 | `.work/tmp/gherkin-parser project/features/<f>.feature .work/tmp/<f>.json`（f ∈ 本轮 9 份） |
| APS dry-checker（Go 版） | 9/9 exit=0；findings 全为 near-duplicate/possible-synonym 中文分词假阳性类（aps.md 登记口径） | `.work/tmp/gherkin-ir-dry-checker .work/tmp/<f>.json .work/tmp/<f>.dry.json` |
| 钦定 toast 文案字节级校验 | features 2 处 + qa 3 处全角闭括号正确（`grep -c "仅 http(s)/ws(s)/udp）："） | grep 逐文件 |
| RAG 索引 | 健康 | `bin/rag/rag status` |

### 对账口径
- 零产品代码改动（specifier 规格轮）；`src/`、`Cargo.toml` 零接触。
- 场景数：09-bt-engine 18→20（+19/20）；其余文件场景数不变（07-11/08-09/11-06 原位反转，PC-04/TP-04/05 行翻转或剪行留痕，序号不重排）。
- 需求文档版本链：mission v1.44→v1.45；phase-02 v1.14→v1.15；phase-01 注记追加 v1.48。

### 待办与待批
- **D70 待批（操作者）**：①题目录重建映射——会话中断致上轮题目原文散失，本轮按 plan 基线记录 + 操作者原始需求重建五题并与作答「1a 2b 3a 4a 5a」对位（题1 toast 截尾=44 字硬截尾 / 题2 协议列宽=8 / 题3 ws(s) 三来源完全同权 / 题4 WebRTC=仅 peer 数据传输、tracker announce 各按自身协议 / 题5 fail-fast=全部条目）；如重建与操作者原意有偏差，指认题号即可回退（改动均为规格注记级）；②「协议」列宽 8（作答 2b）。
- **沿 QA 终验轮未决**：判重谓词「已完成任务」张力（phase-01 FR-01-26 引用句「非完成」限定 vs v1.47 无限定口径，待操作者裁决）；变异加固悬置；QA-01-11 全路径预填断言口径（如需恢复须行为面变更裁决）。
- **coder 轮实现清单（已规格化，Phase-02 v1.15）**：TrackerUrl 增 ws(s) 分型与三族归一；[bt] `trackers`/`peer_connection_protocol` 两键接入 config（含 fail-fast 校验接入启动链，FR-02-35①③）；FR-01-71 语义非法值全局翻转接入；WebRTC DataChannel 传输臂（无跨协议回退）；并发面板协议列 8/TCP·WebRTC；对话框 toast 新文案；模板 v1.16 文本；`mini_ws_tracker.py` harness。

### 移交建议
- 下一步运行 **six-pack/coder**（以本文件为上游交接内容，依据 phase-02 v1.15 规格实现）；之后沿 six-pack 链 cleaner → architect → QA。
- 操作者审批入口：mission.md v1.45 注记、phase-02.md v1.15 修订注与 D70、handoff 本节待办与待批。
