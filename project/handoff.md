# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/coder（D90–D93 裁决落地轮：族A/族B hardening 断言退役改写 + `[http] scheduler` 键退役 + 钦定模板重建）——已完成**
> 路径：**six-pack/coder（本轮）** → cleaner → architect → (hardender) → QA
> 会话：six-pack-coder-d90-landing（上游交接 = specifier 裁决落实轮 handoff（D90–D93 登记 + 可回退窗口待确认）+ 操作者 20261127 chat 裁决确认：「D91：scheduler 键整键退役；D92：block_size_http 的语义改判为『动态模式网格细化上限』」——可回退窗口关闭，按登记原文落地）
> 环境：nightly 1.102.0-nightly（rustup 镜像 20261127 实测 rsproxy 12.3MB/s 最快，`RUSTUP_DIST_SERVER=https://rsproxy.cn` + cargo registry rsproxy sparse + rust-toolchain.toml 钉 nightly；工作区 /home/z/swarm-ezr2 由 specifier 归档 005252 重建；~/.cargo 与 .tools/ 非持久；**APS/bb 本轮未重建**——零 feature 编辑，gherkin 解析不适用，runners 本就未可执行沿袭）
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭；②runner 不可执行沿袭（qa/runners 归 QA 轮同步）；③MDM 性能基准 = 操作者线下验收口径；④P3 增益层沿规格排除；⑤hardening_ui_backfill 族C 1 例沿袭（gen 剥头副本，归 cleaner）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，v1.63）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D93，v1.1）；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固、phase-05 规格轮 + 实现轮 + pieces 各修复轮 + 速度回退修复轮 | features 01–13 + qa 规程 + runners + refine_http_grid 同源接线 | 已交付（沿上轮账） | 当时口径 bin 1256P/0F + 实机 PTY 三重实证 |
| 族A/族B 裁决落实轮（20261127，specifier） | mission.md v1.63 + phase-05.md v1.1（D90–D93 登记）+ 5 份 feature 修订 + 6 份 QA 文档同步 | 已交付 | APS parser 5/5 + ir-dry-checker 5/5；8 例既有失败实测取证 |
| **本轮：D90–D93 裁决落地轮（20261127，coder）** | **①D91：`[http] scheduler` 键退役三件套（任意值拒绝启动 + Config 字段/解析删除 + make_spec 常态 Dynamic）+ 钦定模板常量重建（42 → 41 键、211 行，按 v1.63 钦定原文逐字节，顺带修复 v1.51 起三重漂移）；②D90/D93：8 例 hardening 断言退役/改写并按新语义更名；③零产品行为变更（族A/B 均「保留现行为」裁决，refine_http_grid 等实现零改动，D92 为口径追认）** | 已交付 | **bin 1256P/0F + hardening 13 目标 8 例既有失败全部转绿（仅剩族C 1 例登记沿袭）+ clippy --tests 0 警告 + fmt --check 干净 + check --bins 0 警告** |

### 遗留受限项（当前有效）
- **①全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；归 architect/cleaner 轮。
- **②runner 断言同步面（归 QA 轮）**：QA-DE-04/05/06/07、QA-HT-12/13b、QA-PC-05 退役行（执行记 S）；QA-HT-13 按「含 scheduler 键任意值拒绝启动」改判执行；QA-PC-13 键集 42 → 41；suite_resume_sidecar.py:211 blocks 断言随 D90/D91 失效；**suite_config_template.py 解析锚需对 v1.63 适配**（起点锚版本行 + 终点「）」行 + 行数口径：钦定文本 177 = 非空行计数，代码常量 lines() = 211；退役注记行「（v1.63/族A 退役：…）」是规格编辑器留痕不是模板行）。
- **③族C（gen 剥头副本漂移，1 例）**：hardening_ui_backfill `gen_fixtures_match_product_sources`，比对面 = src/ui/{chart,detail,btn}.rs + src/app/clipboard.rs 四文件（本轮未触碰），重新生成即可，归 cleaner 轮顺带。
- **④qa/09-bt-engine-qa.md:70 字面真缺陷 1 处**：归 QA 轮顺带修复（沿袭登记）。
- **⑤MDM 性能基准**：线下口径（沿袭）。
- **⑥blocks 引擎代码本体仍在编译面**：TaskSpec.scheduler 与 supervisor/download.rs 分支保留、`HttpScheduler::Blocks` 变体 `#[allow(dead_code)]` 标注（config 面退役后生产不可达，仅引擎测试直构 TaskSpec 触达）；引擎代码退役/瘦身归 cleaner/architect 轮裁决。

### 实现定义值登记（本轮新增）
- 历轮登记（D81–D93）全部沿袭有效；本轮实现口径 3 项：
  - **scheduler 键拒绝语义（D91 落地）**：`Config::try_from_toml` 遇 `[http] scheduler` 键（任意值，含 `dynamic`）→ `Err("键 http.scheduler：{v}（scheduler 键已退役，未知键拒绝启动）")`；`Config::try_load` 包装为 `配置文件 {path} 非法：{e}`（配置路径 + 键点路径 + 原因三要素齐全）。Config 结构体不再有 `scheduler` 字段；`make_spec` 恒 `HttpScheduler::Dynamic`（引擎选择唯一入口 = sidecar `is_dynamic()` 语义，B 类快照由 `min_split_kib` 等键继续承载）。
  - **钦定模板常量 = v1.63 钦定原文逐字节**（AUTHORIZED_TEMPLATE 211 行/41 键）：[http] 键序 = min_split_kib → block_size_http → …（钦定原文口径）；**顺带修复 v1.51 起三重漂移**（代码侧键序、block_size_http 注释停留在 v1.16「（1 MB）」文案、行数）。钦定原文内嵌的退役注记行是编辑器留痕，不入模板。
  - **hardening 8 例更名映射**（旧断言面退役，新名钉 D90/D93 语义）：`invalidated_resets_and_toasts` → `blocks_ledger_ignored_no_invalidation_toast`、`orphan_adopt_mismatch_redirects_silently_and_preserves_sidecar` → `blocks_orphan_ledger_not_adopted_downloads_in_place`（g_engine）；`unknown_size_orphan_redirects_not_transient_fail` → `blocks_unknown_size_ledger_downloads_fresh_to_completion`、`inconsistent_block_count_orphan_redirects_even_when_size_matches` → `blocks_inconsistent_ledger_downloads_in_place`、`worker_block_403_fails_fatal_not_transient` → `worker_block_403_transient_transport_interrupted`、`single_stream_200_failure_writes_no_ledger` → `single_stream_burst_cut_keeps_downloading`、`matching_sidecar_resumes_with_nonzero_range` → `trickle_disconnect_with_progress_keeps_downloading`（g_v147）；`range_denied_fails_task` → `range_denied_downgrades_to_completion`（v115）。

## 二、当前产出情况

### 本会话产物清单（D90–D93 裁决落地轮）
1. **`src/model/config.rs`**：①`try_from_toml` scheduler 键任意值拒绝启动（+`Config.scheduler` 字段/Default/构造删除、`scheduler_val`/`HttpScheduler::parse` 退役）；②`AUTHORIZED_TEMPLATE` 按 v1.63 钦定原文重建（42 → 41 键、214 → 211 行，双路径构建互证）；③config 测试 7 处同步（scheduler 语义测试改 D91 全值拒绝口径、模板键数/行数/计数断言 42→41、拒绝表增 dynamic 行）。
2. **`src/app/engine/mod.rs`**：`make_spec` 恒 Dynamic（sidecar 条件分支失去前提）。
3. **`tests/hardening_g_engine.rs`**：族A 2 例改写更名（D90 不校验不接管：原名完成、无作废提示、不孤儿改道）。
4. **`tests/hardening_g_v147.rs`**：族A 2 例 + 族B 3 例改写更名（D90 ×2 + D93：403→Transient「传输中断」、有进展断连持续原地重试不失败 ×2）。
5. **`tests/hardening_v115.rs`**：族B 1 例改写更名（Range 全拒 → 降级单流 Completed）。
6. **`project/notes/`**：environment.md（镜像速度漂移每会话重测 / rustup-init 前导点 argv[0] 陷阱 / 链接期 SIGBUS=磁盘满同根因）+ testing.md（钦定原文↔代码常量 runner 不可执行期静默漂移与重建口径）+ INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd = project/ezr，前台串行） |
|---|---|---|
| 全量单元 + proptest（bin ezr） | **1256P/0F**（基线持平） | `cargo test --bin ezr`（212s） |
| hardening_g_engine（全目标） | **1012P/0F**（含族A 2 例新断言） | `cargo test --test hardening_g_engine` |
| hardening_g_v147（全目标） | **999P/0F**（含族A 2 + 族B 3 新断言） | `cargo test --test hardening_g_v147` |
| hardening_v115（全目标） | **1161P/0F**（含族B 1 新断言） | `cargo test --test hardening_v115` |
| 其余 hardening 目标 | g_supervisor 985P / engine_backfill 1138P / g_appcore 1015P / g_main 2P / g_throttle_err 627P / g_proxy 16P / g_slots 274P / mutation 1147P 全 0F | `cargo test --test <name>` |
| hardening_ui_backfill | 恰 1F = `gen_fixtures_match_product_sources`（族C 登记内，归 cleaner） | `cargo test --test hardening_ui_backfill` |
| clippy（含 tests） | **0 警告** | `cargo clippy --tests` |
| rustfmt | 干净 | `cargo fmt --check` |
| 编译 warning | **0 警告** | `cargo check --bins` / `cargo check --all-targets` |
| 8 例改写实测取证 | 改写前 8 例失败输出即现行为观测（fail 原文存 .work/tmp/fam_*.log：orphan name=f.bin、403 kind=Transient err 传输中断、range_denied Completed、断连 Downloading 等），改写后全绿 | `cargo test --test <target> -- --exact <new_names>` |

### 对账口径
- bin 基线 1256P/0F 持平（本轮零新增/零删除 bin 测试；config 测试为原地改写）。
- hardening 既有失败 9F → 1F：族A 4 + 族B 4 全部转绿（净 −8），族C 1 沿袭（登记内）。
- 测试改名 8 条（映射见实现定义值登记），总数不变；三目标通过数与上轮账差额 = 失败例转绿，无新增用例。
- 模板常量 214 → 211 行（−3 = scheduler 三行式）、42 → 41 键；钦定原文双路径互证一致（v1.62 原文减三行 ≡ v1.63 原文减注记行）。

### 待办与待批（未决项）
- **规格层缺陷 2 处上报（mission/ 只读，归操作者/specifier 裁决）**：
  1. `mission/phase-05.md` §5 配置示例 toml 块表头行损坏：独立行 `ttp]` 应为 `[http]`（194 行；上游 v1.62 完好，v1.1 编辑引入，纯文字层缺陷）。
  2. **D92 修订注与钦定原文不一致**：修订注宣称「模板注释行『仅 blocks 回退模式适用』同步退役」，但 v1.63 钦定原文内嵌的 block_size_http 注释行（`# HTTP 分块块大小（字节；仅 blocks 回退模式适用）`）与场景 01 取值范围表行仍保留该短语。模板常量已按钦定原文逐字节落地（含该短语）；若操作者确认按 D92 语义修订钦定原文（改 1–2 行），coder 侧 1 行同步即可。**操作者对 D90–D93 改判 = 直接以裁决文本 begin six-pack/specifier（移交建议沿袭）。**
- **QA 轮接手面**：遗留受限项② 全清单（runner 锚点/退役行/键集同步）+ ④。
- **族C gen 剥头副本重新生成**：归 cleaner 轮顺带。
- **blocks 引擎代码本体退役/瘦身**：归 cleaner/architect 轮裁决（遗留受限项⑥）。
- **变异/CRAP/DRY 度量**：沿袭，归 cleaner/architect 轮。
- **QA-HT-10 du 判据 / 性能基准**：沿袭，线下/操作者裁决。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：族C gen 剥头副本重新生成顺带 + 变异/CRAP/CPD 面复核；本轮改动面（config.rs 解析/模板/测试 + app/engine/mod.rs 一行 + 3 个 hardening 测试文件）无大审计需求。
- QA 轮接手时：按遗留受限项② 清单同步 runner；8 例新名（见映射）纳入回归口径；`cargo test --bin ezr` + 三 hardening 目标为最小回归集。
- 操作者对 D90–D93 任一改判：直接以裁决文本 `bin/swarm begin six-pack/specifier` 修订规格（三层同步口径已建立，改判成本低）；coder 侧跟随同步（模板 1–2 行 / 断言面）。

By coder.
