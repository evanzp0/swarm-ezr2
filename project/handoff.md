# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bug14（2026-10-31，「D63 五项落地 + QA runner 同步 + ezr-demo 同步 +
> 全量验证 + harness 测试」实现轮）
> 上游交接：project/handoff.md（bug14 specifier 轮交接：规格 v1.39/v1.5/v1.37/v1.13/v1.19 +
> feature ×4 + qa ×4 + D63，任务依据＝已批规格 + 操作者 bug14 指令，含 ezr-demo 同步与
> harness 完整测试授权）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）已收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。bug14 轮为操作者指令级需求修订落地轮（D63 全项），本轮 coder
  全量交付（实现 + 测试 + runner + harness）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | bug13 轮登记沿袭 |
| specifier（bug14 规格轮） | mission ×2 + feature ×4 + qa ×4 | 已交付 | APS dry-check 4/4（specifier 轮登记） |
| **coder（本轮 bug14 落地）** | 产品 ×6 文件 + 单测 ×11 + runner ×3 文件 + ezr-demo 同步 ×3 文件 | **已交付** | **cargo test bin 844P/0F + 集成 12 目标 5642P/0F + clippy --all-targets 零警告 + fmt 干净；QA runners 14 套件冒烟（13 全绿 + MT-03 沿袭基线同败见下）；harness swarm_e2e PASS + bench_multipeer PASS；ezr-demo cargo test 30P/0F** |

### 遗留受限项（当前有效，沿袭 + 本轮更新）
- 沿袭：BT 下载限速未接；DHT/BEP 10 完整扩展/uTP 属 phase-03/04；cargo-mutants 缺位；
  死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化；
  QA-SQ-13 竞态（本轮冒烟未复现，沿袭待办）；QA-RS-06（本轮 14/14 全绿未复现）；
  supervisor block_worker 写入 fatal 竞态观察项；APS dry-checker 已装（.tools/，gh-proxy 镜像）。
- **QA-MT-03（沿袭，本轮 2 连败与登记一致）**：「重启应续传」稳定 F——bug13 轮 A/B 对照
  已证基线同败（与二进制无关），本轮续传路径零改动；QA 轮 refinement 待办不变。
- **任务级速度滑窗 `windows` 失去读者（本轮登记，cleaner 候选）**：D63③ 废除任务级 EMA
  链后 `App.windows` 仅剩喂入与生命周期测试（evt/bt 进度路径），rate() 无读者；
  退役属 cleaner 轮裁决（涉及 evt.rs/bt.rs/mod.rs 三处生命周期测试语义）。
- **Inversion 访谈受限项（specifier 轮沿袭）**：chat 选择题面板不可用，操作者 bug14 指令
  文本即阶段 1 答案（D63⑥ 登记）；D63②（proxy 示例块保持注释）与 D63③（显示层舍入差
  实现定义值）可回退，改动均为注记/单点接线级。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 D1–D63 全部裁决（D63 = bug14 轮已批项，见 mission/phase-02.md）。
- **本轮实现定义值（coder 细化，规格框架内）**：
  - **解析失败分层最终口径**：toml 反序列化层错误（语法错 + 键类型不匹配）= try_from_toml
    Err = 启动致命（exit 1，stderr `ezr: 配置文件解析失败：<path>\n<toml 错误>`）；语义非法
    （0/负/越界/非法字符串值）= 逐键回退默认不变。规格措辞已随此分层澄清（specifier 产物
    v1.39/D63① 同步修订——类型不匹配从「逐键回退」表述中移出，01-persistence-config-04
    的 `abc` 行退役、01-config-template-03 承接该类）。
  - **浮点键整数容忍（num_f64）**：backoff_initial/backoff_cap/throttle_burst_secs/
    seed_duration_secs 四键 ConfigRaw 字段改 Option<toml::Value>，Integer/Float 通吃、
    其余类型回退默认——模板 v1.13 的 `seed_duration_secs = 1800`（整数）与用户整数写法
    （`throttle_burst_secs = 1`）由此不致命。
  - **HTTP 任务速度 Σ 接线**：`sample_conn_speed_display` 聚合 per-conn EMA 写 t.speed
    （Downloading 态；无活跃行 = 0）；`speed_display` EMA 链整链退役（字段/初始化/evt
    清理/tick entry 全删）；`windows` 保留（见遗留受限项）。
  - **conns 列修复参数**：client_w = (vw < 94) ? max(12, vw − 60) : 34；内容宽 = 94 − 34 +
    client_w；吸附条件 `edge + 4 > content_w && edge < content_w`（edge = hscroll + 视口宽）；
    渲染 Paragraph.scroll 与水平滚动条位置同用吸附后偏移；HTTP 三列不参与。
  - **种子路径归一接线点**：`dlg_confirm_add_bt` 确认链（normalize_save_dir 复用：~ 展开 +
    相对转 cwd 绝对）；fs::read 直读归一后路径；判重（同来源 + 同保存目录）与 Task.url
    同用归一值；预填层不改写（所见即所输）。

### 实现定义值登记（累计，覆盖为最新全量）
- 沿袭 phase-01/02 全部口径（D1–D63）+ bug11 键位缺省表 + bug12 客户端列样例集 + bug13
  保存目录归一五组验收例 + 本轮：速度显示舍入容差（Σ fmt ≤ 0.1 KB/s×行数亚视觉差）；
  conns 收缩下限 12 / 基数 60 / 吸附窗口 (c−4, c)；BT 下载速度展示链单位换算 KB/s 头部
  历史口径不变。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×6**：
  - `model/config.rs`：try_from_toml/try_load 严格解析 + from_toml 收为 #[cfg(test)] 助手 +
    num_f64 四浮点键容忍 + AUTHORIZED_TEMPLATE 整体替换 v1.13（174 行活跃键值）；
    单测 ×4 新增（strict err / try_load 路径与原因 / 模板解析=默认等价 / 整数浮点通吃）
    + 模板测试组适配（v1.13 活跃结构/36 键活跃行/三表活跃声明）。
  - `main.rs`：启动臂 Config::try_load → Err 时 eprintln + exit(1)（损坏配置拒绝启动）。
  - `app/engine/tick.rs`：sample_conn_speed_display 聚合 Σ → t.speed；任务级 EMA 链退役。
  - `app/mod.rs` + `app/engine/evt.rs` + `app/engine/mod.rs`：speed_display 字段链删除 +
    ghost 清理测试适配；单测 ×3 新增（Σ 多连接/单连接同值/无活跃行归零）。
  - `ui/conns.rs`：客户端列自适应收缩 + 末列完整可见吸附 + BT_CLIENT_* 常量；
    单测 ×3 新增（视口 92 默认视图表头完整/hs 48–51 吸附后完整/视口≥94 维持 34 列）。
  - `app/bt_dialog.rs`：dlg_confirm_add_bt 种子来源归一接线；单测 ×3 新增（相对→cwd 绝对/
    ~→主目录绝对/判重归一命中）+ 既有 url 原文断言翻转（D63⑤）。
- **QA runner ×3 文件**：`suite_config_template.py`（v1.13 锚点：起点锚/活跃表声明/36 活跃
  键行 + 活跃 39 行断言 + CT-01 判据改版 + **CT-03 翻转**：exit_code 轮询 + 非零退出 +
  报错语义 + 文件不变 + 修复恢复）；`suite_persistence_config.py`（**PC-14 新增**：模板
  起步改值生效链 bt_concurrency=10 → BT 对话框预填 10 + PC-04① 类型不匹配翻转断言）；
  `suite_ui_conns.py`（UC-09 增 ①b 同帧 Σ 断言 + _speed_to_bs 助手 + v1.38 同步注）。
- **ezr-demo 同步 ×3 文件 + README**：`ui/conns.rs`（自适应收缩 + 右端吸附，colw 动态化）；
  `app/sim.rs`（t.speed/t.upload_speed = Σ per-conn EMA 同源，废除任务级窗口 EMA）；
  `README.md` 变更记录 22/23 条。
- **notes**：architecture.md ×5（解析分层最终口径/num_f64/Σ 接线/收缩吸附参数/归一接线点）；
  testing.md ×4（to_string vs 裸 buffer 两形态/PC-04① 翻转法/throttle 外部驱动补跑/MT-03
  归类）；environment.md ×1（heredoc 写盘不持久化三连对策）；INDEX 同步。

### 验证证据
| 验证项 | 结果 | 说明 |
|---|---|---|
| cargo test --bin ezr | 844P/0F | 含本轮新增单测 ×11（config ×4、engine Σ ×3、conns ×3、bt_dialog ×3 − 1 翻转） |
| cargo test（集成 12 目标） | 5642P/0F | hardening 137 / g_engine 654 / g_proxy 16 / g_supervisor 628 / g_main 2 / g_slots 190 / g_v147 642 / g_appcore 659 / g_throttle_err 402 / v115 776 / engine_backfill 753 / ui_backfill 783 |
| cargo clippy --all-targets | 零警告 | nightly 1.101.0-nightly（rsproxy 装机） |
| cargo fmt --check | 干净 | ezr + ezr-demo |
| QA runners | 13/14 全绿 | config-template **5/5（CT-03 翻转实测）**、persistence-config **11/11（含 PC-14 证据链 + PC-04① 翻转）**、ui-conns **18/18（Σ 同帧断言）**、add-task 19/19、tui-display 21/21、download-engine 16/16、integrity-check 12/12、named-proxy 18/18、ui-pieces 7/7、retry-backoff 9/9、throttle-proxy 7/7（TP-01..04 整跑 + 05..07 外部驱动补跑）、slots-queue **15/15（SQ-13 本轮未复现）**、resume-sidecar **14/14（RS-06 本轮未复现）**；modify-task 8/9（MT-03 沿袭基线同败） |
| harness swarm_e2e | PASS | 4MB：ezr 与 libtorrent leecher 双向做种、sha256 逐字节一致、HTTP announce 7 行可观察 |
| harness bench_multipeer | PASS | 512/512 块、sha256_match true、4.5s |
| ezr-demo | 30P/0F + fmt 干净 | 同步后回归；触面（sim.rs/ui/conns.rs）clippy 零命中（其余 11 条为未触碰文件 nightly 漂移，预存） |

### 对账口径
- 产品净改动 ×6 文件 + 单测净增 11；测试计数 bin 837→844（+4 新增 − 1 收编 from_toml
  助手化改造不变计数 + Σ/conns/bt_dialog 新增）。
- runner 净改动 ×3 文件：config_template（锚点 ×3 + CT-01 判据 + CT-03 重写）、persistence
  （PC-14 新增用例数 10→11 + PC-04① 腿重写）、ui_conns（UC-09 ①b + 助手）。
- feature 侧对账：01-persistence-config-04 Examples 减 1 行（`abc` 行退役 → CT-03 口径
  承接）；07-add-bt 场景 8→9；persistence 场景 13→14；模板文本 v1.12→v1.13（174 行不变）。
- 规格措辞澄清 ×2（本轮实现期发现，specifier 产物同步修订）：D63① 解析失败分层
  （反序列化错=致命 / 语义非法=逐键）；FR-02-01 v1.5 预填原文措辞与场景 09 对齐。
  **待操作者知悉（可回退）**。

### 待办与待批
- **待办（cleaner 轮）**：①`App.windows` 失去读者后的退役裁决（涉及 evt/bt/mod 三处
  生命周期测试）；②`Config::load` 已删、from_toml 已收 #[cfg(test)]——归一助手
  normalize_save_dir 与 num_f64 的参数链是否收拢结构体；③demo 未触面文件的 11 条
  nightly 漂移 lint（预存，未越权修）。
- **待办（QA 轮）**：QA-MT-03 refinement（沿袭）；QA-AB-09 的 QA 会话脚本化（本轮由
  单测 confirm_bt_* ×3 承接，runner 无 add-bt 套件沿既有分工）。
- **待批（沿袭）**：D63②③④ 实现定义值（见 specifier 轮登记）；监听端口随机范围/死
  tracker 静默窗可视状态/做种任务重启不重挂/BT partial 不持久化。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：
  ①`windows` 退役裁决（遗留受限项）；②num_f64/pos_f64 双助手是否合并；
  ③conns.rs client_w 与 BT_COLW[5] 的双源（const 34 与动态值并存）可收敛；
  ④suite_config_template 的 operator_template_text 解析逻辑与 feature 内嵌文本的耦合
  锚点（起点行尾串）。
- 给操作者的手测口径：①首启 ezr 生成 v1.13 活跃模板 → 直接改 `bt_concurrency = 10` →
  重启 → `A` → 命令窗口输入 .torrent 路径 → BT 对话框并发预填 = 10（QA-PC-14 实测）；
  ②把 config.toml 改坏（如删掉表头留下键、或把字符串写进数字键）→ 启动直接报错退出
  （含路径与 TOML 错误位置，QA-CT-03 实测）；③改 `bt_concurrency = 10` 后 BT 对话框
  显示 10、改 `http_concurrency = 3` 后 HTTP 对话框显示 3（D63② 证据）；④HTTP 下载
  多连接 → 并发面板各行速度合计 = 任务列表速度（BT 同口径，QA-UC-09 ①b 实测）；
  ⑤宽终端默认视图「协议」表头完整、120 列终端滚到最右「协议」完整（单测探针矩阵锁定）；
  ⑥命令窗口输入 `~/x.torrent` 或相对路径 → 任务详情「URL」为绝对路径。
- 后续链：coder → cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
