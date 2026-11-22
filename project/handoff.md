# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（架构评审轮——cleaner SKILL 标准移交路径）
> 路径：…竞态销项轮（上上轮）⇒ 清理轮（上轮）⇒ **架构评审轮（本轮，操作者 chat 指令「用 six-pack/architect」）**
> 会话：six-pack-architect-20261122（上游交接 = 上轮 handoff（清理轮）+ 操作者 chat 指令；ezr 不做旧版本兼容）
> 环境：rustup stable 1.100.0 + nightly 1.102.0-nightly（c7d5ba59d 2026-11-20，nightly 移动目标以测试全绿为准；rust-toolchain.toml 钉 nightly 全项目统一）、cargo registry rsproxy sparse、llvm-cov 0.9.2 + cargo-mutants 27.1.0 + PMD 7.29.0（.tools/pmd-bin-7.29.0，与历轮基线同版本）全部就位，CARGO_INCREMENTAL=0 + CARGO_PROFILE_DEV_DEBUG=0 + CARGO_PROFILE_TEST_DEBUG=0 三件套全程前台分块。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA runtime 重建轮 | APS 四件套重建 + verdict 级全量 + 07-add-bt 哈希销项 | 已交付 | verdict Σ = 1107 killed + 17 filtered + 0S/0E |
| 遗留受限项销项轮 | ③644 改判杀灭 + ①缺口账目 ID 权威清单 + ④LO_ONLY 敞口消灭 + ②Go 版 mutator filter 移植 | 已交付 | verdict Σ = 1107 killed + 0 survived + 17 filtered；bin 基线 1091P/0F 全绿 |
| 竞态销项轮 | architecture:389 观察项销项：block_worker 打开失败有界重试窗（workers.rs）+ 两行为锁定单测 | 已交付 | bin 1147P/0F + 集成 8440P/0F；fmt/clippy clean |
| 清理轮 | 竞态销项改动面度量收口 + 3 个纯追加 worker 级测试 | 已交付 | bins 1150P/0F + 集成 8467P/0F 全绿；fmt clean；clippy 零警告；CRAP/DRY/变异点三度量全过 |
| **架构评审轮（本轮）** | 四阶段架构评审 + model↔bt 模块环销项（词表迁 model/bt.rs + persist 提取）+ arch_check 规则 1 扩域 + PeerTransport 属性测试补缺 | 已交付 | bins 1152P/0F + 集成 8479P/0F（13 目标）+ 属性 90P/0F；fmt clean；clippy 全目标零警告；arch_check 18 规则全过（见二节） |

### 遗留受限项（当前有效）
- **workers.rs 227-228（磁盘写入失败 fatal）与 239-240（resp.chunk 流错误 classify）为集成级错误臂**：bin 口径纯追加测试不可确定性触发（需 fs/流层注入基建），由集成链覆盖；bin 口径不追测，非缺陷。
- **workers.rs 257-258 死防御臂保留**（`if failure.is_some() { return; }` 于 `if aborted` 返回之后）：控制流证明不可达，属防御性不变式护栏，按历轮裁决登记保留；若未来新增 failure 赋值臂未同迭代置 aborted 则该臂转活，须重新对账。
- bin 口径 cov=0 五处登记：property_tests.rs:2223（失败闭包 by design）、main.rs:303（入口壳 env 兜底）、webrtc.rs:213（webrtc 覆盖非确定性受限项）、mouse.rs:516（交互层）、metainfo.rs:52（批次外积压）。
- 2 条 unviable 终态（mutation 账目人工实证定案）+ trackpool.rs:412 CRAP 竞速对账口径（已接受，非可执行任务）沿历轮登记不变。
- **bt/mod.rs 8 子模块 pub mod 面维持现状（本轮评审登记观察）**：bin-only crate 无 crate 外泄漏面；消费点 = engine↔bt 协调枢纽（裁决在案）与属性测试；收窄需引入 re-export 间接层，收益不抵风险，留待 hardender 按需裁决。
- 上轮 QA 登记的 schema 口径演进（open_int 三分退役为二分）维持交付态，操作者否决路径仍留档（notes/testing.md 锚回补轮先例）。

## 二、当前产出情况

### 本会话产物清单（架构评审轮）
1. **四阶段架构评审**（UI/核心分离、依赖规则、信息隐藏、局部代码质量）：阶段一/三/四合规零违规（自动化证据 = arch_check 18 规则全过 + ui 回填白名单在案 + g_appcore/g_engine 可测性套件全绿）；阶段二发现 **model↔bt 产品级模块环**一处（唯一违规项）。
2. **模块环销项（行为不变迁移，唯一产品改动面）**：
   - 语法词表 `TrackerUrl`/`udp_host_port`/`PeerTransport`/`PeerTransports`（纯数据 + parse，零 IO）自 `bt/tracker.rs`、`bt/webrtc.rs` 字节级迁入 **`model/bt.rs`**（签名/可见性不变）；依赖方向改为 bt→model 单向，model/config.rs 校验引用同层词表。
   - 消费方路径更新：bt/engine/{mod,conn,pieceio,trackpool}.rs、app/bt_dialog.rs、engine/supervisor/mod.rs、model/config.rs、property_tests.rs、bt/{tracker,webrtc}.rs（`#[cfg(test)]` 门控导入）。
   - `save_json_atomic`/`remove_missing_ok` 自 model 根提取 **`model/persist.rs`**（pub(crate) 保持）+ 根模块 `pub(crate) use` re-export——`super::` 调用点零改动，`#[path]` 手搭树（tests/hardening）补 `bt`/`persist` 两声明后闭环。
   - 改动面合计：14 文件 +217/−156 + 新增 persist.rs（git diff --stat 口径）。
3. **arch_check.sh 规则 1 扫描域扩至 `crate::bt`**（原判例 app/ui/engine 未含 bt；扩域后 model 层 bt 依赖禁令生效），阳性对照实测（插真实 use 探针 → FAIL exit 1；移除 → 全过；修复前真实违规引用使扩域规则即红、迁移后转绿，规则扩域与修复同轮互证）。
4. **PeerTransport 属性测试补缺**（评审实测 88 个既有属性零覆盖）：`prop_peer_transport_case_fold_stability_and_round_trip`（casefold/trim 判定稳定性 + label 右逆往返 + 接受域精确性）+ `prop_peer_transports_allows_is_field_projection`（allows 字段投影）；属性套件 88→90。
5. **经验沉淀**：notes/architecture.md 架构评审轮 4 条（词表归属判据/persist 提取法/双载体计数传导第二形态/items_after_test_module 纪律）；notes/environment.md 新沙箱开工顺序 4 条；notes/INDEX.md 同步；本 handoff.md 重写。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单测全量 | 1098P/0F（ezr，含 2 新属性）+ 30/9/15P（fixture/mktorrent/proxy）= 1152P/0F | `cargo test --bins`（DEBUG 三件套） |
| 集成全量 | 13 目标 8479P/0F（hardening 259 / engine_backfill 982 / ui_backfill 1012 / g_appcore 866 / g_engine 862 / g_main 2 / g_proxy 16 / g_slots 254 / g_supervisor 835 / g_throttle_err 546 / g_v147 849 / mutation 991 / v115 1005） | `cargo test --test <目标>`（逐块前台） |
| 属性测试（独立显式命令） | 90P/0F（88 既有 + 2 新增） | `cargo test --bin ezr property_tests::` |
| fmt / clippy | clean / 全目标零警告（13 个编译视角均零） | `cargo fmt --check && cargo clippy --all-targets` |
| 自动化架构检查 | 18 条规则全过（规则 1 扩域后） | `bash scripts/arch_check.sh` |
| 编译警告 | `cargo build` 0 警告 | `cargo build` |

### 对账口径
- **集成 +12 归因**：8467→8479，唯一差值 = hardening 247→259 的 +12 = src/model/bt.rs 自带测试模块（BtTaskData 持久化族 12 个）随文件新被 tests/hardening 手搭树包含（双载体计数传导，notes/architecture.md 本轮条目）；其余 12 目标逐个与基线恒等（982/1012/866/862/2/16/254/835/546/849/991/1005）——行为保持的编译级实证。
- **bins 对账**：ezr bin 1096→1098 = 仅 2 个属性测试增量（属性测试不入基线对账公式，notes/testing.md 在案）；产品单测零增减——行为不变的计数实证。
- **行为不变裁决**：词表迁移 = 定义字节级搬家 + use 路径改指，签名/可见性/行为零变化（SKILL「移动实现块到所属模块」安全类）；persist 提取 = `super::` 调用点经根 re-export 零改动。
- **工具口径**：本轮沿用上轮交接的全部度量工具与版本（llvm-cov 0.9.2 / cargo-mutants 27.1.0 / PMD 7.29.0 已重装就位）；CRAP/DRY/变异点三度量本轮未重跑（cleaner 上轮基线在案，本轮改动面为纯移动无新增函数/克隆，非度量触发面）。

### 交接四要素
1. **本地验证命令清单（按序）**：`cargo test --bins`（1152P）→ 逐集成目标（8479P）→ `cargo test --bin ezr property_tests::`（90P）→ `cargo fmt --check` + `cargo clippy --all-targets` → `bash scripts/arch_check.sh`。
2. **环境**：rust-toolchain.toml 钉 nightly（fmt/clippy 需其 rustfmt unstable 选项），测试运行兼容 stable/nightly 双通道；cargo registry = rsproxy sparse（~/.cargo/config.toml，新沙箱需重建）；llvm-cov 直采管线需 llvm-tools-preview 组件（已装）；新沙箱开工顺序纪律见 notes/environment.md（先装钉定 nightly 再进项目目录跑 cargo）。
3. **行为不变式（下游注意）**：词表唯一源 = model/bt.rs（TrackerUrl/udp_host_port/PeerTransport/PeerTransports），bt/tracker.rs 与 bt/webrtc.rs 仅 `#[cfg(test)]` 导入消费——下游不得在 bt 内重建定义（arch_check 规则 1 会响亮失败）；model 根的 save_json_atomic/remove_missing_ok 经 persist.rs re-export 供给，`#[path]` 手搭树包含 model 子文件时须同步补根辅助声明（persist + use，判据见 notes/architecture.md）。
4. **夹具纪律**：worker 级测试目标文件先预建再 spawn（notes/testing.md 清理轮条目）；stop watch sender 保活纪律沿上轮；tests/hardening/model/mod.rs 为手搭最小树（自定义 Connection 桩）——新增对 model 子文件的 #[path] 包含须同步评估其自带测试模块对树计数的影响（+N 传导归因）。

### 待办与待批
- **本会话无待批无待办**——操作者指令的架构评审轮已落地结清（唯一违规项 model↔bt 环已销项，规则域已扩）。

### 移交建议
- 下一步建议运行 `six-pack/hardender` 技能（architect SKILL 标准移交路径；变异点计数上轮登记 workers.rs=41 / supervisor/mod.rs=23 均远低于 100，词表迁移面无新增高复杂度函数）。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By architect.
