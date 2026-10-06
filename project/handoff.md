# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：cleaner-20261006-v113（度量收敛轮 +
> 行为保持清理：覆盖率/CRAP/DRY/变异点扫描 + 10 条补测 + 3 处产品去重 + dialog_keys 拆分）

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（ezr download）：TUI 高性能多协议下载器，phase-01（HTTP/HTTPS 真实下载
  内核与 TUI 正式版）已交付并通过 QA 终局独立验证；QA 后增量轮：coder 裁决落地轮、
  specifier v1.4–v1.12 + coder v14–v1.12（FR-01-85..93 建档与实现、D17/D20/D24/D25 裁决
  落账、操作者第七/八/九批指令直落），本轮 cleaner v113（全库度量收敛与行为保持清理，
  上游 = coder-20261005-v112 交接）。
  需求权威来源 `project/mission.md`（v1.12），phase-01 详述见 `project/mission/phase-01.md`（v1.12），
  feature 规格在 `project/features/`（12 份），QA 规程在 `project/qa/`（12 份规程 / 9 份脚本）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（基线）+ coder（phase-01） | mission/features/qa 9 份 + 下载内核 + TUI + fixtures | 已交付 | 上游既定口径 |
| cleaner / architect / hardender / QA（历轮） | 加固 ~110 测试、度量收敛、架构复核、PTY e2e 终局验证 | 已交付 | QA 终局 966P/0F；行覆盖 90.5% |
| coder（20261005 裁决落地轮）~ coder v1.9 | 见上游各轮登记（on_evt 单测、锁回退收口、FR-01-85..93 全链） | 已交付 | 逐轮对账自洽（1276P/0F @ v1.8） |
| specifier（v1.10）/ coder（v1.10） | 操作者第七批指令：配置模板最小化（钦定文本逐字节权威 54 行） | 已交付 | 钦定文本 9/9 值一致 |
| specifier（v1.11）/ coder（v1.11） | 操作者第八批指令：详情面板移除「状态」「速度」行 + 死字段 Task.elapsed 清除 | 已交付 | 1312P/0F |
| specifier（v1.12）/ coder（v1.12） | 操作者第九批指令：详情「大小」行移除「（剩余 …）」后缀（TDD） | 已交付 | 1313P/0F（10 二进制 300/30/11/107/236/235/2/12/213/167）；clippy 0；fmt 0；arch 全通过 |
| **cleaner（20261006 v113）** | **全库度量收敛 + 行为保持清理（详见第二节）**：覆盖率 91.49%→**93.42%**；CRAP>6 34→**30**（cov=0 杠杆全兑现）；PMD CPD 57→**48**（产品代码三处真重复收敛 + 测试夹具单源化）；变异点全库 1785、已更改文件全部 <100（dialog_keys 118 拆分为 49+69）；新增 10 条单测；fixture 双撞名修复；1313P→**1350P**/0F | 已交付 | **1350P/0F（310/30/11/107/245/244/2/12/222/167）**；clippy 0；fmt 0；arch_check 10/10；各二进制测试名唯一性 0 重复 |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游；操作者已裁决维持现状）：无标准 CRAP 工具；本轮按
  「llvm-cov lcov BRDA 弧数/2 + 1 代入 comp，闭包并入外层取极值，测试符号排除」复算并
  复核（脚本存于 `.tools/crap-approx-lcov.py`，口径登记在脚本头注）。偏差方向：`?` 早退
  不计 BRDA → comp 低估（入口胶水类 CRAP 偏乐观）；残余 CRAP>6 的 30 个函数见第二节
  登记口径，交 hardender 以变异为最终准绳。
- **e2e 套件预登记 S 项（3 用例）**：RB-09；TP-02/03/06（既有口径不变）。
- **沙箱无 TTY**：TUI 进程级端到端不可自动化；pty E2E 由 QA 会话以 pyte 口径完成。
- **QA 规程就绪、套件脚本未落**：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08 共三套
  （12 份规程 / 9 份脚本，v1.6 起 suite_throttle_proxy.py 收敛为 7 限速用例）；
  QA-CT-01/05 的模板内容断言为与钦定文本整文本 `cmp` 逐字节比对。QA-NP 另需
  ezr-proxy 扩展 https 代理监听（NP-08）、socks5+user/pass 认证监听（NP-15）、
  IPv6 回环监听（NP-18）三种形态（未就绪登记 S；NP-11/12/16/17 无连接尝试不依赖扩展）。

### 实现定义值登记
- 无新增产品契约值（本轮为行为保持清理：新增 helper 均为既有逻辑收敛，对外语义零变化；
  模板/警告/凭证运行期文案沿 v1.9 口径不变）。

## 二、当前产出情况（cleaner-20261006-v113）

### 本会话产物清单（行为保持清理轮，产品代码零行为变化）
**① 覆盖率提升（测试补齐纯追加，共 10 条单测）**
- `src/app/dialog_keys.rs`（现 `dialog_keys/mod.rs` 测试模块末尾追加 4 条）：
  `modify_dialog_char_routes`（Modify 字符输入全焦点路由）、`modify_dialog_enter_routes`
  （Enter 五臂路由）、`proxy_dropdown_open_key_routes`（代理下拉逐键 + n=0 守卫 +
  Add/Modify 双布局跳焦点）、`add_dialog_char_routes`（Add 字符输入：控制字符拒绝、
  双下拉展开、校验码 128 上限、确认/取消按钮）。
- `src/app/mouse.rs`（新增 `mouse_tests` 模块，该文件此前零测试，3 条）：
  `click_selects_and_scroll_bounded`（列表点击选中/越界/滚轮钳制/Drag 忽略）、
  `dialog_click_routes`（字段聚焦、算法/代理下拉点选与点外关闭、按钮激活、
  非左键与滚轮消费）、`dialog_click_modify_delete_buttons`（Modify/Delete 按钮臂）。
- `src/app/paste.rs`（测试模块末尾追加 1 条）：`on_paste_routes_by_dialog_state`
  （App::on_paste 入口路由：无对话框/下拉展开/Delete 一律忽略）。
- `src/ui/mod.rs`（测试模块末尾追加 1 条）：`resume_with_free_slot_direct_start`
  （toggle_pause 槽位空闲直接恢复 + 续传双文案臂 + Completed 兜底臂；URL 指向
  127.0.0.1:9 即刻拒绝端口，真实 Cmd::Start 不产生外网副作用）。
- `src/app/engine.rs`（evt_tests 模块末尾追加 1 条）：`disk_precheck_skips_passes_and_fails_overneed`
  （磁盘预检三分支：未探测跳过/小需求通过/u64::MAX 需求转 Fatal + toast）。
- 成效：行覆盖 91.49%→**93.42%**（11306 行，漏行 921→744）；函数执行率 93.95%→**94.63%**；
  CRAP 最大点塌缩：modify_dialog_char 132→6 以下、on_mouse 108.8→25.5、add_dialog_char
  62→11 以下、on_paste/proxy_dropdown_open_key/modify_dialog_enter→6 以下。

**② DRY 收敛（PMD CPD `-l rust`，阳性对照 exit 4 实证过滤生效；57→48 项 ≥50 tokens）**
- `src/app/dialogs.rs`：dlg_confirm_add/dlg_confirm_modify 校验码校验块（97 tokens 重复）
  → 提取 `App::checksum_from_input(ck_type, ck_raw, ck_focus)`（Some(None)=清除 /
  Some(Some)=设置 / None=非法中止并聚焦回校验码字段）。
- `src/app/dialog_keys/`：add/modify 字符处理的十六进制追加臂（62 tokens 重复）→
  `push_hex_capped(d, c)`。
- `src/engine/mod.rs`：`build_endpoint_client` 与 `EngineHandle::start` 的 client 策略
  重复（redirect 10 / no_proxy / 15s+30s 超时）→ `base_client_builder()` 单一事实来源。
- 测试夹具单源化：Dialog 14 字段字面量在 dialog_keys/paste/mouse 各测试模块重复 4 份 →
  新增 `src/app/testutil.rs`（`#[cfg(test)] pub(crate) mod testutil::dialog(kind, focus)`，
  默认值对齐既有 Add/Modify 夹具）。
- **CPD 残余 48 项登记口径（不再收敛，理由）**：a) 测试场景 Evt::Probed/渲染 setup 字面量
  ——字段各异，提取即参数膨胀；b) ui 层渲染习语对（边框/段落/居中矩形）——展示层内部
  模式，重排属 architect 视觉布局裁决；c) supervisor block_worker 12 参数 spawn 表——
  参数对象化为设计变更（architect）；d) 三个二进制入口样板（main/ezr-proxy/ezr-fixture）
  ——去重需 lib target 重构（模块边界，architect）；e) Task::new_queued 调用点参数表
  ——10 参工厂无净收益。

**③ 变异点扫描（cargo-mutants 27.1.0 `--list --line-col=true`，scan 模式，未跑变异测试）**
- 全库 1785 变异点；每文件计数登记：dialog_keys/mod.rs 49 + routing.rs 69（拆分后）、
  app/engine.rs 94、model/chunk/lease.rs 88、model/timefmt.rs 78、model/config.rs 68、
  ui/task_lines.rs 67、model/namegen.rs 65、ui/text.rs 64、ui/dialog.rs 62、
  engine/supervisor.rs 61（其余 <55）。已更改/新增文件全部 <100。
- **dialog_keys.rs 118>100 已拆分**：`src/app/dialog_keys.rs` → `src/app/dialog_keys/mod.rs`
  （App 对话框处理器）+ `src/app/dialog_keys/routing.rs`（下拉/文本字段键路由纯函数，
  `pub(super)` + mod.rs 显式 use）。「xxx.rs → xxx/ 目录」对既有 `#[path]` 挂载兼容
  （挂载点为 app/mod.rs，子模块相对解析两种形态等价），hardening 各 crate 全量复跑实证。
- 高计数文件未拆分登记：lease.rs 88/timefmt.rs 78 等均为纯函数热路径且 <100；>100 的
  未更改文件（无）不触发 SKILL 拆分义务；hardender 全量变异前可按上表 `--file` 分块。

**④ fixture 健壮性（行为保持，测试基建两类撞名修复）**
- `src/model/sidecar.rs` 测试夹具 `tmp_dir()`：原 `pid+毫秒` 双后缀在**同进程并发测试
  同一毫秒内**会撞出同一目录（先完成者 `remove_dir_all` 令并发者 save 报 NotFound，
  本轮 llvm-cov 首跑 appcore 1F 复现；清 /tmp 残留复跑全绿与上游 v1.12 登记同形态）。
  修复：进程内原子自增序号参与命名（pid + seq + ms），同撞库与跨会话 pid 复用一并消除。
- `src/engine/supervisor.rs`：`keep_name_restart_skips_dedupe` 与
  `verify_computes_matching_digest` 两测试曾共用 `ezr-e2e4-{pid}` 同一目录且并行运行
  （前者中途 remove_dir_all 会删后者工作目录）——后者改唯一名 `ezr-e2e8-{pid}`。
- 经验已按归属沉淀：同进程同粒度撞名 → `packs/_common/engineering.md`「测试执行纪律」
  （pid 条目增补）；#[path] 挂载模块目录化拆分断 `use super::*` 隐式路径、NLL 路径
  敏感借用方法化撞 E0502 → `packs/_common/notes/rust.md` §6。

**⑤ 工具与环境（本轮沙箱重建，跨会话复用）**
- 工具链：rustup 1.29.1 + nightly `rustc 1.101.0-nightly (282215592 2026-10-04)`、
  rustfmt/clippy 组件 + llvm-tools-preview（`export PATH="$HOME/.cargo/bin:$PATH"` 注入；
  项目 rust-toolchain.toml 钉 nightly 生效）。
- 度量工具：cargo-llvm-cov 0.9.1、cargo-mutants 27.1.0（`~/.cargo/bin`）；PMD 7.28.0
  （`.tools/pmd-bin-7.28.0`，GitHub API 403 时按 expanded_assets 页读资产清单直链下载）；
  CRAP 受限近似脚本 `.tools/crap-approx-lcov.py`（输入：`cargo llvm-cov --lcov --branch
  --hide-instantiations` 产物）。
- 无 lld 桥接需求（项目无 `.cargo/config.toml` 链接器钉死）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单元/集成测试 | **1350P/0F**（310/30/11/107/245/244/2/12/222/167）；对账：上游 1313 + 37 = 新增 10 条 × 挂载倍增（app 层 9 条在 main bin + 3 个挂载 crate 各一份、ui 1 条仅 main bin） | `cargo test`（crate 根） |
| 测试名唯一性 | 各二进制内 0 重复（跨 crate 挂载同名属机制固有） | `cargo test --bin ezr -- --list \| sort \| uniq -d` 等 10 目标 |
| clippy / fmt / arch | 0 警告 / 0 差异 / 10 条边界规则全过 | `cargo clippy --all-targets`、`cargo fmt --check`、`bash scripts/arch_check.sh` |
| 行覆盖率 | **93.42%**（上游基线 90.5%、本轮开工 91.49%） | `cargo llvm-cov --summary-only` |
| CRAP 受限近似 | 产品函数 313 个，CRAP>6 共 30 个（开工 34），杠杆点全兑现；残余为分发核/入口胶水/渲染核/100% 覆盖高 comp 纯函数，登记移交 hardender | `.tools/crap-approx-lcov.py`（口径见脚本头注） |
| DRY（CPD ≥50 tokens） | 57→48 项；产品真重复三处已收敛；残余五类登记口径（见②） | `bash .tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --dir src --minimum-tokens 50`（exit 4=发现重复） |
| 变异点扫描 | 1785 点；已更改文件全部 <100（dialog_keys 118 拆为 49+69）；未跑变异测试（SKILL 禁令，归 hardender） | `cargo mutants --list --line-col=true` |

### 待办与待批（未决项）
- **无操作者待批项**：本轮为行为保持清理，产品语义零变化；无实现定义值新增。
- **hardender 承接**：变异测试全量执行（cleaner 仅 scan）；CRAP 残余 30 点以变异为准绳
  复核（受限近似 comp 有低估方向）；可按登记的每文件计数 `--file` 分块跑。
- **architect 承接（若操作者选择续跑）**：CPD 残余中的跨二进制入口样板去重（lib target
  重构）、supervisor spawn 参数对象化、ui 渲染习语收敛，均属模块边界/架构裁决面。
- **QA 会话承接（累积）**：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08 脚本化与
  ezr-proxy 三类监听扩展（https / socks5+认证 / IPv6 回环）。
- **遗留受限项**：沙箱无 TTY（pty E2E 归 QA 会话）；CRAP 受限近似口径（操作者已裁决维持）。

## 三、上一轮产出存档（coder-20261005-v112，本轮基线）
- v1.12 全链（操作者第九批指令）：detail.rs 大小行移除「（剩余 …）」后缀（TDD：红——
  `draw_detail_size_row_omits_remaining_suffix` 负向断言；绿——format 三参删一 +
  v1.12 修订注释）；task_lines.rs 列表/做种「剩余」口径零改动；顺带修 clippy unused_mut。
- 验证基线：1313P/0F（300/30/11/107/236/235/2/12/213/167）、clippy 0、fmt 0、
  arch_check 全通过；本轮开工实跑精确对账一致（同口径复跑 1313P/0F 后再动工）。

## 四、移交建议
- 本轮为行为保持清理轮（six-pack 编排：specifier→coder→**cleaner**→architect→hardender→QA），
  建议操作者运行 `bin/swarm complete` 归档后链式启动 `six-pack/architect`：
  1. **复核本轮两处结构变更**：dialog_keys 模块拆分（mod/routing 两子模块职责边界）与
     testutil 测试夹具单源化，确认模块边界符合其架构意图（变异点 >100 触发的机械拆分，
     依赖方向零变化）。
  2. **CRAP 残余 30 点裁决**：登记清单（draw_task_dialog 35.1 / download 29.1 / on_mouse
     25.5 / tick 25.2 / dlg_confirm_add 20.4 / on_evt 19.0 / block_worker 15.3 / from_toml
     12.0 / spread_bytes 9.0 等，全量见 `.tools/crap-approx-lcov.py` 复算）——其中
     100% 覆盖纯函数（spread_bytes、consistency::check、fmt_block_size 等 comp=7..9）是否
     值得表驱动拆分，或维持登记口径（hardender 变异为最终准绳）。
  3. **CPD 残余五类裁决**（见② a–e）：是否立项 lib target 入口样板收敛与 spawn 参数
     对象化。
- 环境与工具安装位置见第二节⑤；`.work/tmp/` 会话产物（lcov/日志）随 complete 清除，
  度量命令均可按表中命令复现。
