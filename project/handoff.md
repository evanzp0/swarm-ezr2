# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（bug27 任务URL唯一性轮，本轮已完成）
> 路径：specifier → coder → cleaner → architect → hardender（操作者指示跳过并关闭）→ QA → Done
> 　　→ 复验轮（2026-11-06）→ specifier 规格修订轮（v1.45 守卫）→ specifier 审批改判轮（v1.46）
> 　　→ coder 实现轮（v1.45+v1.46，2026-11-07）→ coder bug26 限速修复轮（2026-11-07）→ **coder bug27 修复轮（本轮，2026-11-07）**
> 会话：ezr-coder-bug27-url-unique（2026-11-07，操作者报告「同一个url 生成的下载任务（bt/http），第一次添加的保存路径和后续添加的保存路径不同会被当作两个任务都能生成并下载；任务是通过 url 来确定唯一性的，URL 相同应提示任务已存在」，
> 测试定位并修复；上游交接 = project/handoff.md（bug26 轮）+ 操作者 chat 指令）
> 环境注记：工具链沿上轮（rustup nightly 1.101.0，PATH=~/.cargo/bin；rsproxy sparse；bb+APS 在 `.tools/`）。
> pyte/torf/libtorrent 已装（阿里云镜像），真机 pty e2e 夹具可用（harness tracker 17901/seeder 17903，
> 会话级临时进程，每轮手测前重启，见 notes/environment.md）。

## 一、本轮结论（bug27 任务 URL 唯一性）

### 根因判定（实测证据链）
1. **判重口径缺陷（已修复）**：现行规格 FR-02-02 ③臂 / FR-01-01 v1.44（01-add-task-16）判重谓词
   =「同 URL/同来源 + 同保存目录」——同 URL 不同保存路径不判重，直接生成第二个任务并行下载；
   **CLI 添加路径（`add_cli_task`）则完全没有 URL 判重**（只有同名 resume/dedupe 逻辑），同 URL
   二次 `ezr download <url>` 恒新建任务。这就是操作者报告的两个症状。
2. **操作者裁决（规格改判）**：任务唯一性 = **URL**（与保存目录无关）；同 URL 添加 → 提示
   「该链接的任务已存在」。沿 bug13–25 轮先例，操作者 chat 指令即修订裁决（Inversion 不适用）。
3. **修复（三入口同口径落地）**：
   - HTTP 添加对话框（`app/dialogs.rs::dlg_finish_add`）：判重谓词 `t.url == url && t.save_dir == dir`
     → `t.url == url`；命中载体不变 = 错误窗口（D56）`该链接的任务已存在`。
   - BT 添加对话框（`app/bt_dialog.rs::dlg_confirm_add_bt`）：同上，谓词去 `&& t.save_dir == dir`。
   - CLI（`app/cli.rs::add_cli_task`）：URL 校验后新增 URL 判重，命中 = toast `⚠ 该链接的任务已存在`
     （CLI 无对话框/错误窗口载体，沿 FR-01-80 ⚠ 前缀 toast 语义），不创建任务。
4. **两层拦截不变式（行为面核查）**：判重（URL 维度，先行）与 FR-01-26 接续守卫（目标路径维度，
   后置）为独立两层——同 URL 异目录在判重层即被拒、不流到接续链；异 URL 同推导名才进守卫/序号臂。
   判重收紧只减不加：name-dedupe（异 URL 同名 `.1` 递增）、断点接续（注册表无同 URL 时 sidecar
   路径基匹配）、v1.45 占用守卫、v1.46 孤儿改道臂全部不受影响（restore 等相关套件实测全绿）。

### 测试与验证账
| 验证项 | 结果 | 命令 |
|---|---|---|
| bin 全量（含本轮净增 2 测） | **945P / 0F**（历轮 943 + 2） | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bin ezr` |
| 集成 12 目标 | **6329P / 0F**（supervisor 712 + engine 739 + appcore 743 + throttle_err 458 + v147 726 + hardening 142 + engine_backfill 845 + slots 203 + main 2 + proxy 16 + ui_backfill 875 + v115 868） | `cargo test --tests`（逐目标 --no-run + 单跑，盘见 notes/environment.md） |
| clippy | 0 warning / 0 error | `cargo clippy --bins --tests` |
| rustfmt | 全仓干净 | `cargo fmt --check` |

### TDD 红绿账（复现测试先失败后过）
- 复现红灯（3）：`finish_add_different_dir_is_duplicate`（HTTP 同 URL 异目录→错误窗口，旧代码 err=""+
  且新建任务）、`confirm_add_duplicate_different_dir_rejected`（BT 同来源异目录→错误窗口）、
  `cli_add_same_url_rejected_as_duplicate`（CLI 同 URL 二次添加→toast 拒绝，旧代码直接 ✓ 创建）。
- 翻转/改数据（2）：`finish_add_different_dir_is_not_duplicate` → `_is_duplicate`（旧口径测试按名
  翻转保边界覆盖连续性）；`cli_add_dedupes_duplicate_names` 同 URL 两连加 → **异 URL 同推导名**
  两连加（不变式「名字去重 .1 递增」仍成立，换数据保语义），另立同 URL 拒绝测试。
- blast radius 机械盘点：rg `add_cli_task` 30+ 调用点按所属测试函数聚合 → 仅 1 个多调用函数
  （restore_keeps_tasks_and_ids，异 URL）→ 集成测试零翻转，实测证实。

## 二、产物清单（本轮增量）
- `project/ezr/src/app/dialogs.rs`：HTTP 判重谓词去 save_dir 条件 + 注释口径更新（一行谓词改动）。
- `project/ezr/src/app/bt_dialog.rs`：BT 判重谓词同上 + 新测试 1。
- `project/ezr/src/app/cli.rs`：add_cli_task 新增 URL 判重（toast 载体）+ 测试翻转 1 / 新增 1。
- `project/ezr/src/app/mod.rs`：HTTP 旧口径测试翻转 1。
- 规格：`mission/phase-02.md` 头注 **v1.14**（操作者 bug27 轮指令全文存档 + FR-02-02 ③臂改判）；
  `mission/phase-01.md` 头注 **v1.47**（FR-01-01 v1.47 注记 + 01-add-task-16a 新增 + FR-01-26 引用句同步）；
  `features/01-add-task.feature` **v1.43**（场景 16 改判 + **16a 新增**[CLI 判重] + Examples 异目录行）；
  `features/07-add-bt.feature` **v1.42**（04/09 判据改判）。
- QA：`qa/01-add-task-qa.md`（QA-01-16 修订 + **QA-01-16a 新增**）；`qa/07-add-bt-qa.md`
  （QA-AB-04 ③臂修订 + QA-AB-09 旧 toast 残留修正[bug25 漏改]）。
- notes：`architecture.md` + `testing.md` + `INDEX.md` bug27 经验条目。
- 产品行为变更净额：判重谓词两行收紧 + CLI 判重四行新增；无几何/文案/引擎改动。

### 遗留受限项
- 沿袭不变：leech 判据无 e2e 自动化（six-pack 链唯一有效受限项）。本轮无新增。
- 已知边界（规格已登记，非缺陷）：BT 唯一性按种子文件归一路径比对（FR-02-09③：URL = 归一后绝对
  路径）；同内容异种子文件（同 infohash 不同路径）不判重——属 phase-03 infohash 身份范畴。

### 移交建议
- 下一步运行 `six-pack/cleaner`（six-pack 链顺序）；或操作者直接复测（release 二进制
  `project/ezr/target/release/ezr`——注意本轮仅测过 debug 测试面，复测前 `cargo build --release`）：
  ① 同 URL（bt/http）第一次添加后，改保存目录再添加 → 应弹「错误」窗口 `该链接的任务已存在`；
  ② CLI `ezr download <同URL>` 二次 → toast `⚠ 该链接的任务已存在`；③ 异 URL 同名 → 照旧 `.1`
  递增；④ 删除任务后重新添加同 URL → 正常创建并按 FR-01-26 接续/改道。
- 工具链交接沿上轮；小盘跑集成测试沿本轮笔记（--no-run 逐目标 + touch 强制重链 + 日志落 /tmp）。
