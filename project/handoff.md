# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/coder（D94 实现轮：HTTP 并发列表「信息」列四列形态 + AUTHORIZED_TEMPLATE 两注释行 v1.64 同步 + 测试面）——已完成**
> 路径：**six-pack/coder（本轮）** → cleaner → architect → (hardender) → QA
> 会话：six-pack-coder-d94（上游交接 = specifier v1.64 裁决落实轮 handoff + 操作者 20261127 chat 裁决：「①phase-05 §5 示例 toml 表头 ttp] 修复；②D92 矛盾落地——删除 config.toml 两处适用范围注释；③增加需求——http 并发 panel 每个并发列表最后一列增加『信息』列，内容参考 misterdownloadmanager」）
> 环境：bb 1.13.230（`.tools/bb-bin/bb`）+ APS 仓 `.tools/Acceptance-Pipeline-Specification` + MDM 参考仓 `.tools/misterdownloadmanager` 沿上轮重建口径在位；rust 工具链 stable 1.100.0 + nightly 双链在位（会话恢复按 notes/environment.md 判别法重建 PATH 注入）；cargo registry rsproxy sparse；`~/.cargo` 与 `.tools/` 非持久沿袭
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭——Inversion 受限退化沿先例；②runner 不可执行沿袭（qa/runners 归 QA 轮同步）；③MDM 性能基准 = 操作者线下验收口径；④P3 增益层沿规格排除

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，v1.64）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D94，v1.2）；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固、phase-05 规格轮 + 实现轮 + pieces 各修复轮 + 速度回退修复轮 + 族A/族B 裁决落实轮（specifier）+ D90–D93 落地轮（coder） | features 01–13 + qa 规程 + runners + 动态引擎/模板常量/断言面全量落地 | 已交付（沿上轮账） | 当时口径 bin 1256P/0F + hardening 13 目标仅剩族C 1 例 + clippy/fmt/check 全零警告 |
| v1.64 裁决落实轮（specifier） | mission/phase-05.md v1.2（ttp] 两处修复 + FR-05-13 v1.2 + D92/D94 登记）+ mission.md v1.64 + 两 feature v1.64 + QA 三份同步 + notes 追加 | 已交付 | APS parser 2/2 + dry-checker 与 HEAD 基线持平零新增；字节级终审零附带损伤 |
| **D94 实现轮（20261127，coder）** | **①`ConnActivity` 三态枚举 + `ConnView.activity` 字段（engine/mod.rs）；②dynamic worker 三态标记（租约落位 Connecting / 响应开启 Receiving / 三处原地重试退避 Retrying(reason)）；③single.rs 单流降级臂快照 = Receiving；④workers.rs blocks 引擎构造点编译适配（占位 Connecting，生产不可达）；⑤App 侧 `conn_activity` 快照表（update_conn_stats 同步写入 + clear_conn_stats 五表连带 + `conn_activity_of` 查询）；⑥conns.rs HTTP 四列形态（信息列宽 18 / 内容宽 47 / 表头左对齐 / 数据左对齐 FG / 超宽 `…` 截断 / 横滚沿用通用机制）+ `activity_text` 三态文案映射（MDM zh-hans 文案族逐字）；⑦AUTHORIZED_TEMPLATE 两注释行整短语删除（v1.64 钦定原文逐字对齐，行数 212/键数 41 不变）；⑧测试面 bin +8 + stats 保留断言扩展 + ucs 剥头副本重生成；⑨notes 三文件 + INDEX 同步** | **已交付** | **fmt --check PASS；clippy --all-targets 0 警告；bin 1264P/0F；hardening 四套件 1146/1014/1001/1169 全 0F** |

### 遗留受限项（当前有效）
- **①全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；归 architect/cleaner 轮。
- **②runner 断言同步面（归 QA 轮）**：QA-DE-04/05/06/07、QA-HT-12/13b、QA-PC-05 退役行；QA-HT-13、QA-PC-13 键集 41；suite_resume_sidecar.py:211 blocks 断言失效；`suite_config_template.py` 解析锚需对 v1.63/v1.64 一并适配（起点锚版本行 + 两注释行新文本 + 非空行计数口径：钦定文本 177 = 非空行计数，代码常量 lines() = 211，内嵌块物理行 212）。
- **③族C（gen 剥头副本漂移，1 例，归 cleaner 轮顺带）**：hardening_ui_backfill `gen_fixtures_match_product_sources`，比对面 = src/ui/{chart,detail,btn}.rs + src/app/clipboard.rs 四文件，重新生成即可。（本轮 engine 侧 ucs 剥头副本已随签名变更重生成清零，不在遗留内。）
- **④qa/09-bt-engine-qa.md:70 字面真缺陷 1 处**（`ttp] 调优键` 应为 `[http] 调优键`）：归 QA 轮顺带修复（沿袭登记）。
- **⑤MDM 性能基准**：线下口径（沿袭）。
- **⑥blocks 引擎代码本体仍在编译面**：沿袭（config 面退役后生产不可达）；引擎代码退役/瘦身归 cleaner/architect 轮裁决。
- **⑦D94 可回退窗口**：信息列三项微口径（列宽 18 / 超宽横滚 / MDM 三态映射）已实现，沿 D83–D93 先例登记可回退；操作者否决 = 直接以裁决文本 begin six-pack/specifier 改判（feature/QA 判据行 + coder 实现面改判成本低）。

### 实现定义值登记（累计覆盖为最新全量）
- 历轮登记（D81–D93）全部沿袭有效（scheduler 键拒绝语义、钦定模板常量 211 行/41 键按 v1.64 钦定原文——两注释行本轮已同步、hardening 8 例更名映射）。
- **D94 三项微口径（源裁决可回退）**：
  - **信息列宽 = 18**：极值算法 = 「正在接收数据...」17 显示列 + 1 空白；HTTP 内容宽 47（4+12+10+18+3 分隔）。
  - **超宽横滚**：120×34 视口 44 < 47 → 水平滚动条 + ←/→ ±2 列（既有通用机制适用 HTTP；上限 = 内容宽 − 视口宽 + 1 沿 v1.34⑤）。
  - **MDM 三态映射**：`正在连接...` / `正在接收数据...` / `正在重试：<原因>`（reason = EngineFailure.reason 既有失败分类文案族零转换）；MDM Finished/Failed/Retired 无 ezr 对应臂。

## 二、当前产出情况

### 本会话产物清单（D94 实现轮）
1. `src/engine/mod.rs`：`ConnActivity` 三态枚举（Connecting/Receiving/Retrying(String)）+ `ConnView.activity` 字段。
2. `src/engine/supervisor/dynamic.rs`：三态标记接线 + 构造点适配（含重试退避路径 Retrying(EngineFailure.reason)）。
3. `src/engine/supervisor/single.rs`：单流降级臂 Progress 快照 = Receiving。
4. `src/engine/supervisor/workers.rs`：blocks 引擎构造点编译适配（占位 Connecting）。
5. `src/app/mod.rs` + `src/app/engine/{mod,evt}.rs` + `src/app/stats.rs`：`conn_activity` 快照表 + update_conn_stats 同步写入 + clear_conn_stats 扩展 + `conn_activity_of` 查询接口。
6. `src/ui/conns.rs`：HTTP 四列形态 + `activity_text` 三态文案映射（取值 = MDM zh-hans 文案族逐字）。
7. `src/model/config.rs`：AUTHORIZED_TEMPLATE 两注释行整短语删除（`；仅 dynamic 模式适用` / `；仅 blocks 回退模式适用`），行数/键数不变。
8. 测试面：bin 新增 8 测（活动传播/覆写清理 2 + 信息列 UI 6）+ stats 保留断言扩展 + hardening_engine_backfill ucs_fix 垫片签名同步 + pure_targets 活动断言。
9. `tests/gen/update_conn_stats_stripped.rs`：剥头副本重生成（随 update_conn_stats 签名变更）。
10. `project/notes/`：environment.md（PATH 误判判别法 + RAG 重建口径）+ architecture.md（三态快照流动链实现侧）+ testing.md（剥头副本三触点联动 + clippy 先行纪律）+ INDEX 三行同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd = project/ezr，前台串行，CARGO_INCREMENTAL=0） |
|---|---|---|
| cargo fmt --check | **PASS** | `cargo fmt --check` |
| clippy 全目标 | **0 警告** | `cargo clippy --all-targets` |
| bin 全量单测 | **1264P / 0F**（210.9s） | `cargo test --bin ezr` |
| hardening_engine_backfill | **1146P / 0F**（163.9s） | `cargo test --test hardening_engine_backfill` |
| hardening_g_engine | **1014P / 0F**（165.1s） | `cargo test --test hardening_g_engine` |
| hardening_g_v147 | **1001P / 0F**（173.0s） | `cargo test --test hardening_g_v147` |
| hardening_v115 | **1169P / 0F**（171.7s） | `cargo test --test hardening_v115` |

### 对账口径
- bin 1256 → 1264 = **+8 全部为本轮新增**（活动传播 2 + 信息列 UI 6）；既有测试零删除、零改名。
- hardening 四套件通过数与上轮基线一致（构造点适配与垫片签名同步不改变用例计数；hardening_engine_backfill 新增活动断言内嵌于既有 pure_targets 用例，不加行）。
- 模板常量：行数断言 212（lines() 计 211）/键数 41 两项既有断言原值通过——两注释行整短语删除属行内文本修订，行数/键数零变化。
- MDM 文案源核证：`.tools/misterdownloadmanager/i18n/zh-hans.tsv`（Connecting.../Receiving data.../Retrying: {error} 词条）与实现文案逐字一致。

### 待办与待批（未决项）
- **D94 可回退窗口**：三项微口径（列宽 18 / 超宽横滚 / MDM 三态映射）沿 D83–D93 先例登记，操作者否决窗口开放；否决 = 直接以裁决文本 `bin/swarm begin six-pack/specifier`。
- **QA 轮接手面**：遗留受限项② 全清单（含 QA-UC-08 v1.64 增判据⑫⑬：fixture 慢速门控观察 `正在接收数据...`、指定连接注入断连观察 `正在重试：传输中断（…）`）+ ④ qa/09:70 顺带修复；最小回归集 = `cargo test --bin ezr` + hardening_g_engine/g_v147/v115。
- **族C ui 剥头副本重新生成**：归 cleaner 轮顺带。**blocks 引擎代码本体退役/瘦身**：归 cleaner/architect 轮裁决。**变异/CRAP/DRY 度量**：沿袭，归 cleaner/architect 轮。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`（沿路径 coder → cleaner → architect → (hardender) → QA）：本轮改动面无大审计需求，cleaner 顺带族C 剥头副本重生成即可。
- QA 轮接手时：QA-UC-08 v1.64 增判据⑫⑬ 纳入回归口径；`suite_config_template.py` 解析锚适配；qa/09-bt-engine-qa.md:70 顺带修复；最小回归集见待办节。
- 操作者对 D94 任一微口径改判：直接以裁决文本 `bin/swarm begin six-pack/specifier`（三层同步口径已建立，改判成本低）。

By coder.
