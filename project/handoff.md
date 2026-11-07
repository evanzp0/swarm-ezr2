# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（终验轮，本轮已完成）——**six-pack 流转路径走完**
> 路径：specifier → coder → cleaner → architect（2026-11-07）→ hardender（操作者指示跳过并
> 关闭，不回补）→ **QA 终验轮（本轮，2026-11-07）→ Done**
> 会话：ezr-qa-final-20261107（任务 = hardender 关闭后的最终独立验证：环境全量重建 + 基线
> 复验 + QA 套件实跑 + DRY/CRAP + 规程一致性核对；上游交接 = project/handoff.md（architect 轮））
> 环境注记：本沙箱全新重建——rustup nightly 1.101.0（TUNA 双变量**前台**装齐，探针 36.0MB/s
> 最快且清单新鲜，rsproxy 20.4/USTC 12.0/NJU 404；minimal+rustfmt+clippy，PATH=~/.cargo/bin）；
> registry rsproxy sparse；pip 阿里云镜像（pyte/torf/libtorrent 2.1.1/wcwidth）；DRY = PMD CPD
> 7.29.0 落位 `swarm/.tools/pmd`（tag 形态 `pmd_releases/7.29.0`，Java 21）；CRAP = llvm-tools
> preview + llvm-cov 手工管线（instrument-coverage 临时 target-dir，已清理口径见下）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui 交互层 + tokio 引擎）。需求权威来源 =
  `mission.md`（分期模式）+ `mission/` 4 份 phase 详述（phase-01 v1.47 / phase-02 v1.14）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | features/ 全套 Gherkin 规格 + qa/ 全套 QA 规程；phase-01 v1.47 / phase-02 v1.14 | 已交付 | 历轮对账在案 |
| coder 历轮 | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug27 修复 | 已交付 | 历轮全绿 |
| cleaner 历轮 | 保持行为清理 + 四件度量；bug27 批次三单点化 | 已交付 | bin/集成/clippy 全绿 |
| architect（2026-11-07） | 四阶段评审 + 五项裁决 + 测试基建收编 + 属性测试 +2 + arch_check 17 规则 | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者指示跳过并关闭，链路径改道，不回补） | 不适用 | — |
| QA（2026-11-06） | 对话框焦点几何、fixture cd=、套件过滤机制 | 已交付 | e2e 套件实跑 |
| 复验轮（2026-11-06） | KeepAlive 90s 一处产品变更；受限项 15→2 | 已交付 | 集成精确对账 |
| **QA 终验轮（本轮）** | 环境全量重建 + 全基线复验 + 14 套件 187P 实跑 + 修复 5 族问题 + cmd_complete_path 补测 +3 + DRY/CRAP + 规程一致性核对（详见第二节） | 已交付 | 全绿（见验证证据） |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链唯一长期有效受限项，历轮维持）。
- **变异实跑未执行**：hardender 路径被操作者关闭，变异加固收益按 `--file` 分块口径悬置
  待裁决（architect 轮裁决：交互层三文件不拆）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（07-11 BT 族 = harness/swarm_e2e.py e2e 承载、
  06 命令窗口分流 = suite_add_task 的 open_add_dialog_via_cmd 链路 + harness 承载）——历轮
  既定形态，本轮核对确认在案。
- **CRAP 观察项（bin 套件范围口径）**：`ezr_main`（main 组装根，e2e/CLI 承载）与
  `on_mouse_trackers`（comp 12，鼠标路径，集成/PT 面承载）bin 套件 0 覆盖——承载面差异
  登记，不硬凑覆盖。

## 二、当前产出情况

### 本会话产物清单（净产品行为变更 = 0，净产品 API 面 = 0；测试净追加 +3）
1. **harness 清场自适应化**（`qa/runners/harness.py`）：`clear_dir_prefill/clear_url_prefill`
   由固定 64 退格单发改为「单字节退格 ×16 → 读屏判空（含占位符 `/srv/downloads` 形态）→
   未空再来一轮」自适应循环。根因双缺陷：①PTY 对 `\x7f` 连发投递上限（64 连发仅 ~18 达，
   bug19 口径）②本工作区路径更深（预填 75 字符 > 64）→ 残留与键入拼接 → save_dir 畸变
   （registry 实录 `/home/z/my` + 全路径）→ 伴随校验用例全家假红。
2. **bracketed paste 三段分片**（7 处：add_task/modify_task）：PTY 对 ESC 序列连发也丢字节
   （19B 单发整段无效），按语义单元三写 + pump；逐字节写破坏 crossterm 解析（实测否决）。
3. **判重口径适配（v1.43/v1.47 同 URL 即任务身份）**：同 env.home 内多相位/多迭代复用同
   URL 的用例改查询参数差异化——IC-01（七算法异文件）、PC-05/PC-09（两相位）、MT-02/MT-04
   （相位 B）、NP-08（https/socks5 相位）、NP-14/16/17（invalid-entry 各相位）、TP-05（五档
   循环 + 2 GiB 对照档）、AT-12（CLI 三算法）。旧「独立子目录避守卫」假设随判重改判失效。
4. **瞬时 toast 竞态改持久证据锚**：SQ-13（重排队 toast 被再探失败 toast 顶替，回环全链
   87ms、可见窗 65ms，20ms 连拍取证）→ 改锚 fixture 访问日志新增请求；TD-02（完成 toast 被
   并发慢任务同刻完成 toast 顶替）→ toast 观测 + wait_file_size 落盘双臂。
5. **RS-14 重写到 v1.46 改判口径**：规程行已改而 runner 滞留 v1.28 原地作废口径——按
   feature 场景 Then 链重写（序号改道 `<名>.1` 静默、孤儿 sidecar/部分文件原样保留、
   `.1.ezr` 独立生成随完成删除；新任务限速 400KB/s 使 sidecar 首刷可观察）。
6. **环境时序适配**：UC-13 大任务降速 40KB/s（100MB@16 连接×150KB/s ≈ 42s 与用例序列同长，
   完成迁移污染列表锚）；TD-06 rate 8KB/s（12 任务须活过加序列，本沙箱 5 槽位下 60KB/s ×4
   连接在加序列内完成前 7 个 → 排队任务不存在）。
7. **PC-04 磁盘真实损坏修复**：`suite_persistence_config.py:205` 写入侧 `ttp]`（chat 通道
   改写遗留）→ 恢复 `[http]`（bug18 判别法定位；该行两种形态均解析失败故此前未显形）。
8. **QA-01-21 对齐 Gherkin 前提**：中位忽略需字段已满 2 位（feature When 明文 + 单测锚），
   先补满再验；光标归位用 Right（产品 End 键仅作用于 trackers 列表，文本字段无 End 处理）。
9. **AT-11 预填断言改尾部锚**：pyte display 对超宽字段「…头截断」，全路径包含断言在深
   工作区必败 → 锚 `<home basename>/cfgdl` 尾部两段。
10. **cmd_complete_path 补测 +3**（`ezr/src/app/cmd.rs` 测试模块纯追加）：命令窗口 Tab 补全
    comp 24 × bin 套件 0 覆盖（CRAP 600 全仓最差）→ 唯一目录追加 / + toast / 多候选 lcp
    列表 / 无匹配 toast 三测 → cov 0.824、CRAP 27.17。零行为变更。
11. **notes 沉淀 + INDEX 同步 + RAG 索引增量刷新**（188 文件/1606 分块，健康）。
12. **QA 规程↔runner 一致性核对**：14 runner ↔ 14 规程一一对应（头部标注规程路径），
    6 个无专属 runner 规程的承载形态在案（见遗留受限项）。

### 验证证据
| 验证项 | 结果 | 命令（cd project/ezr，CARGO_INCREMENTAL=0） |
|---|---|---|
| cargo fmt --check | 全仓干净（含新测试，fmt 已套用） | `cargo fmt --check` |
| clippy --bins --tests | 0 警告 / 0 错误 | `cargo clippy --bins --tests` |
| arch_check | 17 规则全绿 | `bash scripts/arch_check.sh` |
| bin 单测 | **954P / 0F**（基线 951 + 补测 3） | `cargo test --bin ezr` |
| 属性测试（独立显式命令） | **76P / 0F** | `cargo test property_tests::` |
| 集成 12 目标 | **6378P / 0F**（对账见下） | 逐目标 `cargo test --test <名>` |
| harness BT 互操作 e2e | **PASS**（ezr 2.2s 到做种、三方 SHA-256 一致、HTTP+UDP announce 在案） | `python3 harness/swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150` |
| QA 套件 14 套件 | **187P / 0F / 0S**（分节计数见 plan.md 勾选） | `python3 qa/runners/suite_<名>.py`（TP 分三批 QA_TP_ONLY） |
| DRY（PMD CPD 7.29.0 `-l rust`，≥70tok） | **181 克隆组**（与 architect 基线精确同数，零漂移） | `.tools/pmd/bin/pmd cpd --dir src --language rust --minimum-tokens 70` |
| CRAP（bin 套件 llvm-cov + 语法决策点 comp） | 关注集 = comp≥7 且 cov<1：全部为 architect 裁决保留的本质复杂度链（on_mouse_dialog/on_key/on_dialog_key 等，cov 0.6–0.99）+ 两个承载面 0 覆盖观察项；cmd_complete_path 修复后 27.17 | llvm-cov 手工管线（报告 `.work/tmp/crap-report.md`，会话级） |

### 对账口径
- 测试数：bin 951→954（+3 = cmd_complete 补测）；集成 6357→6378（+21 = 7 个挂载套件各
  +3：g_supervisor 716→719、g_engine 743→746、g_appcore 747→750、g_v147 730→733、
  engine_backfill 849→852、ui_backfill 879→882、v115 872→875；g_throttle_err 458 / hardening
  142 / g_slots 203 / g_main 2 / g_proxy 16 不挂载零变化）。自洽：+3×8 = 24 = 7332−7308 ✓。
- 属性测试 76P 独立口径不变（工程规则：属性测试不进覆盖率/CRAP 命令，本轮 llvm-cov 运行
  以 `--skip property_tests` 执行，实测 875P = 954−76 −3 新测试后 875+3=878 filtered 口径
  一致 ✓）。
- 变更面：`qa/runners/`（harness + 8 个套件文件）、`ezr/src/app/cmd.rs`（测试区纯追加）、
  `project/notes/`（environment/testing/INDEX）；与 `src/` 产品代码零接触（cmd.rs 仅 cfg(test)
  区追加），与 `mission.md`/`mission/` 零接触（QA 只读）。
- 幻影行登记：CRAP 管线 if-let 收尾 `}` 幻影行 306 个（bug27 登记判据，不计真实缺口）。

### 待办与待批
- **判重谓词「已完成任务」张力（登记待操作者裁决）**：v1.47 谓词注记与 feature
  01-add-task-16 Given 均无限定（同 URL 即拦、与状态无关，产品实现 + 单测同向）；而
  phase-01 FR-01-26 引用句带「同 URL **非完成**任务的添加由重复任务判定拦截」。本轮 runner
  按无限定口径适配（两个来源 vs 一个交叉引用句）；若操作者裁决为「已完成不拦」，需走
  specifier 修订轮翻转 feature 16 Given + 产品 `url_task_exists`（一行谓词）+ 本轮 12 处
  runner URL 差异化可保留（不影响两口径）。
- **变异加固悬置**（沿前轮）：hardender 关闭，>100 变异点三文件与高 comp 函数族收益无从
  兑现；如重启 hardender，`cargo mutants --file` 分块口径沿 cleaner 登记。
- **QA-01-11 全路径预填断言已改尾部锚**：若操作者要求恢复全路径包含口径，需产品对话框
  字段渲染改「…中截断」或加宽字段——属行为面变更，须裁决。

### 移交建议
- **six-pack 流转路径已走完**：specifier → coder → cleaner → architect →（hardender 操作者
  关闭）→ QA → **Done**。归档即本轮终点，不自动流转。
- 操作者直接复测（行为面与 bug27 轮交接完全一致，本轮零产品变更）：同 URL（bt/http）异
  目录添加 → 错误窗口 `该链接的任务已存在`；CLI 同 URL 二次 → toast `⚠ 该链接的任务已存在`；
  异 URL 同名 → `.1` 递增照旧；命令窗口 Tab 补全（本轮补测覆盖的行为）。
- 若续跑其他工作流：`bin/swarm begin <工作流>/<角色>`（以本文件为上游交接内容）。
