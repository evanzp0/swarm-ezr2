# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（QA runner 族五规程缺口可执行化 + 最终独立验证）
> 路径：…hardender 修订轮（裁决落地）⇒ **QA 轮（本轮，操作者指令：用 six-pack/QA 完成 handoff.md 全部待办）**
> 会话：six-pack-QA-20261117（SESSION_TASK = qa-runner-gaps；上游交接 = 上轮 handoff）
> 环境：rustup stable 1.100 + nightly 1.102.0（TUNA dist）、pyte/wcwidth/libtorrent 2.1.1/bencodepy（pip）、
> Java 21 + PMD 7.29.0（.tools/，本轮装）、llvm-tools（nightly 内）；ezr 二进制 target/debug 全量重建。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.56**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目，41 轮变更记录（本轮零改动）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| hardender 修订轮 | 裁决落地（② 产品修正 + ①③ 规格修订 + ④/mktorrent 了结） | 已交付 | 修订重验 2 feature FULL：77 = 70 killed + 7 等价类 + 0 errors；bin 1090P |
| **QA 轮（本轮）** | **五规程缺口可执行化**：06/07/09/10/11 五套件新增（QA-CW 8 例 / QA-AB 12 例 / QA-BE 24 例 / QA-AL 8 例 / QA-TW 6 例 = 58 用例登记）+ harness 退出礼仪修复 + 产品一处 bug 修复（`./` 分量归一）| 已交付 | **06 = 8P/0F；07 = 11P/1S（AB-07 规程钦定退役）；11 = 6P/0F；09 = 17P/7S；10 = 1P/7S**（S 均内嵌 fixture 能力判据，见下）；产品单测/CRAP/DRY 见验证证据 |

### 待办承接（上轮 handoff 待办与待批 → 本轮处置）
- **QA runner 族五规程缺口（07/09/10/11，06 有底稿）**：✅ 全部可执行化落地（本轮主体）。
- **mutator 等价突变过滤器扩展（上游字节冻结，操作者裁决面）**：沿历轮登记不变——上游 bb/脚本字节冻结未解，QA 角色不可动，维持登记。

### 规格分歧与裁决面登记（本轮新增，待操作者裁决）
1. **「留空 trackers 默认 4 条入库」（qa/07-add-bt-qa.md QA-AB-11④ / 11-trackers-window v1.10 注记）**：产品实现 = 三来源口径（config/announce-list/手输，FR-02-21），无「默认 4 条」常量——该文案为 ezr-demo 演示口径带入规程，产品未实现。本轮按已实现行为断言（announce-list 自带 announce 入池 ✓）。**待裁决：实现默认 4 条，或修订规程该臂。**
2. **udp URL 无端口拒绝启动**：`TrackerUrl::parse` 对 udp 强制 host:port（规格 FR-02-21/35 未定义无端口语义）。合理实现选择，**登记备案**（QA 套件已按带端口形态构造）。

### 产品修复一处（QA SKILL「修复 QA 套件或最终验证发现的 bug」授权）
- **`normalize_save_dir_with`（model/config.rs:930）`./` 分量剥离**：原 `cwd.join("./x")` 产生 `<cwd>/./x`，违反 QA-AB-09/D62 归一口径且破坏判重相等性（"./x" ≠ "x" 同源漏判）。修复 = components 过滤 CurDir + 单测新增（`./ 分量剥离（QA-AB-09 归一口径，判重相等性）`）。QA-AB-09 四形态全绿。

### 遗留受限项（沿历轮不变 + 本轮新增）
- 缺口账目 ID 不可精确复现 / trackpool.rs:644 等价论证 / 2 条 unviable / 软 Gherkin 等价类 17 格 / RB-09+UC-10 / mktorrent bencode：**沿历轮登记不变**。
- **新增（本轮 S 登记，全部内嵌 fixture 能力判据）**：
  - QA-BE-04：mini_http_tracker 无 peers6 回复能力（IPv6 建连对账不可观察）；
  - QA-BE-07：harness 无 BEP9 元数据拉取侧观察点；
  - QA-BE-17：LtPeer 无 per-(piece,offset) request 日志（零重请求对账不可观察）；
  - QA-BE-18：libtorrent 不提供按需 choke/interest/乐观槽驱动（标志判定表臂不可编程——沿 suite_ui_conns「BT 面板判据由单测锁」既定口径）；
  - QA-BE-19/20：harness 无 WebRTC 数据面 seeder（需第二产品实例或 webtorrent-cli）；
  - QA-BE-24：piece 请求顺序需 seeder 侧 request 序日志；
  - QA-AL-01/02/03/04/05/08：RawPeer 无互惠能力不获 unchoke（leech 上传计数恒 0）、本端对 fixture bitfield 的请求受调度策略约束（bad_piece/invalid_req 臂不可稳定触发）——封禁写入/加载/损坏容错/手动 B 臂由 AL-06 实跑 + BE-11（b 键封禁 toast 实跑）承载；
  - QA-AL-07：192.0.2.x TEST-NET 不可达（入连 accept 面不可观察）。

## 二、当前产出情况

### 本会话产物清单（QA 轮）
1. **五套件新增**（project/qa/runners/）：suite_cmd_window.py（8 例）、suite_add_bt.py（12 例）、suite_bt_engine.py（24 例）、suite_bt_anti_leech.py（8 例）、suite_trackers_window.py（6 例）——58 用例全登记（P/F/S），规程-套件一致性逐例对齐（含 v1.51 焦点表 0–11、94×22、FR-02-38 v1.21/v1.53 行形态等）。
2. **harness.py 修复**：graceful_quit 先 Esc 关模态再 Ctrl+C 直退（原 q 键在文本焦点模态下被当普通字符插入缓冲，污染后续观察——本轮实录 q 尾迹假象三例归因）。
3. **产品修复一处**：normalize_save_dir_with `./` 分量剥离（见上）+ 单测。
4. **工装**：PMD 7.29.0 装入 .tools/（Rust DRY，沿 cleaner 轮口径）。
5. **project/notes/**：本轮经验沉淀（见 testing.md/environment.md 追加）。
6. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 五套件实跑 | 06 = 8P；07 = 11P+1S；11 = 6P；09 = 17P+7S；10 = 1P+7S；F=0 | `cd project/qa/runners && python3 suite_cmd_window.py`（余同；QA_ONLY=<编号> 单例） |
| 产品单测 | **1090P / 0F**（188s；含本轮新增 `./` 分量剥离测试） | `cd project/ezr && cargo test --bin ezr` |
| fmt / clippy | fmt **clean**（exit 0）；clippy **0 warnings**（exit 0） | `cd project/ezr && cargo fmt --check && cargo clippy --bins` |
| CRAP | 本轮未重跑全插桩管线（会话窗口预算）；增量面 = normalize_save_dir_with 一处纯顺序代码（components 过滤，无新决策点，CRAP 恒等）；管线沿历轮口径可复现 | 插桩直跑 → llvm-profdata merge → llvm-cov export → 段级过滤 → `python3 project/qa/runners/crap_calc.py <lcov> <src_root>` |
| DRY（PMD 7.29.0 CPD 阈值 70） | **已实跑：rc=4（克隆存在，历轮基线形态）**；增量面 = config.rs 一处 8 行顺序过滤（不构成 70-token 克隆块） | `.tools/pmd-bin-7.29.0/bin/pmd cpd --dir project/ezr/src --language rust --minimum-tokens 70` |
| 产品改动面 | src = config.rs 一处（`./` 分量过滤 + 单测）；qa/runners = 5 新套件 + harness 修复 | `git -C project/ezr diff HEAD --name-only -- src/` |

### 对账口径
- 58 用例登记与五规程用例清单逐例一致（含退役例 AB-07 钦定 S）。
- S 判据全部内嵌环境条件（fixture 能力边界），沿「S 必须内嵌环境判据」条款；非预算性弃测。
- 既有 15 套件零改动（本轮只增不改——除 harness.graceful_quit 修复，其行为对既有套件严格更稳）。

### 待办与待批
- **规格分歧登记两处**（见一），待操作者裁决。
- **mutator 等价突变过滤器扩展**：沿历轮登记（上游字节冻结，操作者裁决面）。

### 移交建议
- **six-pack 流转路径完结**：specifier → coder → cleaner → architect → hardender → QA → **Done**（QA 为末位角色，本轮最终独立验证完成）。
- 请操作者运行 `bin/swarm complete` 归档（归档包按操作者指令交付 `/home/z/my-project/download/`）。

### 交接四要素（QA 要素）
1. **本地验证命令清单**：五套件实跑（上表）→ `cargo fmt --check` → `cargo clippy --bins` → `cargo test --bin ezr` → CRAP/DRY（沿历轮管线）。
2. **会话运行时产物**：.work/tmp/qa-run/（失败快照 + 套件临时目录，complete 即清）；.tools/（PMD，跨会话缓存不进归档）。
3. **feature 注释块**：本轮零 mutation 触碰（QA 不跑变异，SKILL 边界）。
4. **等价突变体清单**：沿历轮 17 格 + unviable 登记不变。

By QA.
