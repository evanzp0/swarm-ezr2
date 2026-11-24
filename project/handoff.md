# 交接（handoff）

> 流程：two-pack　当前节点：two-pack/coder（快速工作流——ezr-demo Ctrl+R toast 演示轮）
> 路径：…specifier 新增需求轮（上轮，FR-01-119 规格已交付归档）⇒ **two-pack/coder（本轮，操作者 chat 指令「修改 ezr-demo：只是在 ctrl+r 时显示下 toast 消息，并且 help 窗口中加个快捷键说明就可以了」；完成后 complete 归档）**
> 会话：two-pack-coder-20261123（上游交接 = 上轮 handoff（specifier 新增需求轮）+ 操作者 chat 指令；ezr 不做旧版本兼容——操作者钦定沿 v1.45，本轮不涉持久化格式无兼容负担）
> 环境：rustup stable 1.100.0（本轮全新沙箱经 TUNA 镜像前台重装，minimal + rustfmt/clippy 组件，位置 ~/.cargo/bin、工具链 ~/.rustup；crates 走 rsproxy sparse）；pyte 0.8.2（PTY 捕获依赖，官方 pypi 装）。
> 受限项：无（输入完备，验证全绿）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.58**）；phase-01/02 为现行详述（03/04 计划登记稿）。本轮 coder 只读，mission 零改动。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；本轮按操作者显式指令落地 **FR-01-119 demo 同构面**（Ctrl+R toast 演示 + 帮助条目），产品端实现仍归下一 coder 轮。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 20 份 + runners + APS 四件套 | 已交付（沿上轮账） | bins 1152P + 集成 8482P + 属性 90P + fmt/clippy/arch_check 全过 |
| specifier 新增需求轮（上轮） | mission v1.58 + phase-01 v1.54（FR-01-119、D81①–⑤）+ features/12-reload-config.feature（13 场景）+ qa/12-reload-config-qa.md | 已交付 | APS parser 13/13 + dry-check high 0 |
| **two-pack/coder 本轮（ezr-demo Ctrl+R toast 演示）** | demo 源码 4 文件（keys.rs / popups.rs / ui/mod.rs / dialogs.rs）+ scripts/capture.py + README 变更记录 42 + docs 设计规范五处同步 + notes/testing.md 增补 | 已交付 | demo 单测 61P/0F（56→61）+ clippy 零警告 + PTY 实证 2 项全过 |

### 遗留受限项（当前有效，沿上轮账 + 本轮新增）
- 上轮验证面受限项全部沿袭有效（workers.rs 错误臂保留、bin 口径 cov=0 五处、2 条 unviable、bt/mod.rs pub mod 面、CPD 残留、CRAP nightly 口径漂移，详见上轮归档与 `qa/mutation-gap-registry.json`）。
- D81①–⑤ 产品侧待批项沿袭（见「待办与待批」）。
- **本轮新增（登记）**：①ezr-demo 树无 rustfmt.toml，存量 fmt 漂移未重格式化（conns.rs/dialogs.rs/list.rs/ui/mod.rs 既有段落 diff 属历史口径；本轮新增代码全部不在 fmt diff 中，未做大范围重格式化——two-pack/coder 最小改动面纪律）；②`.work/tmp/pty_frame_verify.py` 为本轮会话运行时验证脚本，不进归档。

### 实现定义值登记
- 本轮 demo 侧**无新增 D 值**：帮助条目文案 = FR-01-119 ⑧ 钦定原文「重新加载配置文件（热加载，无需重启）」；toast 文案 = D81② 主干「✓ 配置已热加载」+ 演示标记「（演示）」（demo 无真实 config，标记为操作者指令「只是显示下 toast 消息」的如实呈现）。
- D81①–⑤（phase-01 D 表，可回退）沿上轮登记不变，产品端实现轮如改判以最新 D81 口径为准。

## 二、当前产出情况

### 本会话产物清单（two-pack/coder：ezr-demo Ctrl+R toast 演示轮）
1. **`ezr-demo/src/app/keys.rs`**：on_key CONTROL 分支新增 Ctrl+R 臂（`Char('r')|Char('R')` 含 Ctrl+Shift+R kitty 修饰形态）→ `set_toast("✓ 配置已热加载（演示）")`；位置在 Ctrl+C 之后、浮层分发之前 = 全局可达（主界面与全部浮层内响应，FR-01-119 ① demo 同构），不改浮层状态、不退出。
2. **`ezr-demo/src/ui/popups.rs`**：help_lines()【全局】组新增 `Ctrl+R` 条目（FR-01-119 ⑧ 钦定文案），帮助内容 83→84 行；文案 38 显示列 ≤ 46 列预算，70 列窗口内完整可见（PTY 实证）。
3. **`ezr-demo/src/ui/mod.rs`**：帮助行数守卫 83→84 + Ctrl+R 条目断言（并入 `help_window_contains_five_flag_legend_entries`）；新增守卫测试 3 项：`ctrl_r_shows_toast_on_main_screen`（文案逐字）、`ctrl_r_reaches_inside_overlays`（帮助窗口/添加对话框内仍达 + 不改浮层状态 + Ctrl+Shift+R 同臂）、`plain_r_keeps_retry_semantics`（裸 r 走 retry 臂不串台）。
4. **`ezr-demo/src/app/dialogs.rs`**：工具链漂移基线恢复——format_speed_limit 三处 `v % N == 0` → `v.is_multiple_of(N)`（rustc 1.100 clippy manual_is_multiple_of，行为不变，零行为改动面）。
5. **`ezr-demo/scripts/capture.py`**：注入键 named 表补 `c-r`（0x12），供 PTY 实证复用。
6. **`ezr-demo/README.md`**：变更记录新增条目 42（本轮全貌 + 验证基线）。
7. **`ezr-demo/docs/ezr-demo UI界面及交互设计规范.md`**：头部 v1.58 同步补记（①键 ②帮助条目 ③测试与基线恢复）+ §4.2.4 键盘交互表 Ctrl+R 行 + §6.6 帮助窗口 84 行注记与【全局】组表格行 + §10 附录速查表 Ctrl+R 行。
8. **`project/notes/testing.md`**：新增「demo PTY toast 验证须逐帧捕获」条目（capture.py 末帧假阴性根因与逐帧断言法、needle 逐字匹配陷阱）；INDEX.md 该文件已有索引行，无需变更。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd = ezr-demo/） |
|---|---|---|
| demo 单测 | 61P/0F（基线 56 + 本轮 5：守卫 3 + 帮助守卫扩 2 断言） | `cargo test` |
| clippy | 0 警告（含 dialogs.rs 漂移修正后） | `cargo clippy --all-targets` |
| fmt | 本轮新增代码不在 fmt diff 中；存量漂移未动（受限项登记） | `cargo fmt --check`（diff 仅 conns/dialogs/list/mod 既有段落） |
| PTY：Ctrl+R toast | HIT（66 帧，注入 c-r 后 toast「✓ 配置已热加载（演示）」出现） | `python3 ../.work/tmp/pty_frame_verify.py ./target/debug/ezr-tui-demo 110 34 6 "c-r" '配置已热加载（演示）'` |
| PTY：帮助条目 | HIT（58 帧，h 开窗后「Ctrl+R」与「重新加载配置文件（热加载，无需重启）」完整可见） | `python3 ../.work/tmp/pty_frame_verify.py ./target/debug/ezr-tui-demo 110 34 5 "h" 'Ctrl+R\|重新加载配置文件（热加载，无需重启）'` |
| 产品源码零改动 | 本轮为 demo 轮，`project/ezr/` 未触碰，上轮产品基线继续有效 | `git -C project/ezr status --short src`（空输出） |

### 对账口径
- **改动面**：仅 ezr-demo 树（源码 4 + 脚本 1 + 文档 2）+ project/notes/testing.md 一条；mission/phase/features/qa 零改动（coder 只读纪律 + 本轮操作者指令未涉需求修订）。
- **帮助行数**：源码 help_lines() 实测 84 行 = 守卫断言值（v1.53 基线 83 + 1）。
- **数字口径**：单测计数 61 = cargo test 实测；PTY 帧数为实测捕获帧数，随环境抖动仅供参考。

### 待办与待批（未决项）
- **D81①–⑤ 五项产品侧微口径沿上轮待批（可回退，原文见 phase-01 D 表 D81 行）**：①缺失文件臂 = 默认配置整体热加载；②成功 toast = 单条拼接式；③C 类「需重启生效」段仅实际变化时出现；④失败 toast = `⚠ 配置热加载失败：<明细单行化>`；⑤「代理切换：N 个任务」N = 运行中且端点实际变化任务数。产品端 coder 轮开工前以最新 D81 口径为准。
- **本会话无阻塞项**——操作者指令边界明确（「只是…就可以了」），验证全绿。

### 移交建议
- **下一步运行 `six-pack/coder`**：按 FR-01-119 实现产品端 Ctrl+R 配置热加载（沿上轮移交建议：keys.rs CONTROL 分支 / App::reload_config / Cmd::ApplyGlobals / EngineShared burst·sleep 原子化 / Task.running_proxy / help.rs 条目；验收契约 = FR-01-119 + features/12-reload-config.feature；demo 侧本轮已就绪可作交互对照）。
- 归档：请操作者运行 `bin/swarm complete`。

By coder.
