# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：QA-20261007-final-verification
> 版本注：QA 终局独立验证轮——复验 coder v116 两缺陷修复、12 份端到端套件执行
> （3 套规程脚本化 +31 用例）、3 处产品缺陷 TDD 修复、CRAP/DRY 终局度量。
> 上游 coder v116 交接内容已消化。**six-pack 六角色流水线到此走完（QA 为末棒）。**

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.12，本轮未改动）。
- **流水线**：specifier→coder→cleaner→architect→hardender→QA 六棒**全部完成**；
  QA 为最后一名角色，本轮为终局独立验证与完成通知（six-pack 工作流完结）。
- **本轮（QA）做了什么**：①独立复验 v116 全部门槛（两轮全量 1644P/0F、四项回归
  组、clippy/fmt/arch 全绿）；②执行 12 份 PTY 端到端套件（9 份既有 + 3 份本轮
  脚本化：QA-CT/QA-MT/QA-NP 共 31 用例），终态 **132P/0F/9S**（S 均为预登记
  环境受限项）；③端到端验证发现并 TDD 修复 3 处产品缺陷（单实例锁回退、
  D19 完成校验用最新值、恢复路径缺失引用提醒被覆盖）+ 2 处 QA 套件基建缺陷；
  ④CRAP（受限近似）0 热点、DRY（PMD CPD）0 产品重复。
- **质量终态**：全目标 **1656P/0F**（11 目标，含本轮 +4 新测试与挂载树 cfg(test)
  传播 +8 计次，逐目标见验证证据）；clippy 0；fmt 0；arch 11/11。
- **v116 移交复验清单三项全部闭环**：窄端下拉不 panic ✓、浮层高于终端只显示
  前若干项 ✓、连续多轮 cargo test 全绿 ✓（另 #4 下拉回归组 5P/0F、#7
  confirm_add_validates 转绿 ✓）。

## 术语速查（本文件用到的黑话）

| 词 | 意思 |
|---|---|
| P / F / S | 用例通过 / 失败 / 受限跳过（S 内嵌环境判据） |
| 失败先行（TDD） | 先写测试并确认在旧实现上失败（锁住缺陷指纹），再实现使其通过 |
| CRAP 受限近似 | Rust 无标准 CRAP 工具，按 comp²(1-cov)³+comp 以 llvm-cov 行覆盖+分支计数估算（engineering.md 受限口径条款） |
| CPD | PMD Copy-Paste Detector（DRY 度量） |
| toast 槽位 | 产品单一瞬时提示槽，同刻多次 set_toast 后发覆盖先发 |
| spec 快照 | 引擎任务启动时构建的 TaskSpec；完成事件标志属快照口径 |

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01（HTTP/HTTPS
  真实下载内核与 TUI 正式版）已交付；历轮增量至 v1.12（操作者第九批指令）后，
  经 coder v116 缺陷修复轮，本轮 QA 终局验证收官。
- 需求权威来源：`project/mission.md`（v1.12，QA 只读未动）；分期详述
  `project/mission/phase-01.md`；功能规格 `project/features/`（12 份）；QA 规程
  `project/qa/`（12 份规程 / 12 份套件脚本 + 共用 harness——本轮 +3 份脚本）。

### 产物总账
| 阶段 | 做了什么 | 验证状态 |
|---|---|---|
| 基线交付（specifier + coder + QA） | 需求文档 + 下载内核 + TUI + 测试夹具 | 已交付；QA 终局 966P/0F |
| 历轮加固与 QA | 历次加固约 110 条测试、度量收敛、架构复核、PTY 端到端 | 已交付 |
| 操作者第 7–9 批指令（v1.10–v1.12） | 配置模板最小化、详情面板删 2 行、大小行去后缀 | 已交付 |
| cleaner v113 / architect v114 / hardender v115 | 度量收敛 / 架构评审 / 变异加固 | 已交付 |
| coder v116 | 两移交缺陷修复（下拉钳制 + testenv 单源） | 已交付；本轮复验通过 |
| **QA 终局（本轮）** | 12 套件端到端（+3 套脚本化）、3 处产品缺陷 TDD 修复、2 处套件基建修复、CRAP/DRY 终局度量 | **1656P/0F；132P/0F/9S（e2e）；clippy 0；fmt 0；arch 11/11** |

### 遗留受限项（当前有效）
1. **ezr-proxy 三类监听扩展未做**（https / socks5+账号密码 / IPv6 回环，绑定现仅
   127.0.0.1 http）：QA-NP-08 https+socks5 相位、NP-13 https 相位、NP-15 认证生效
   判定、NP-18 下载相位、NP-04 代理侧强制认证子判据按 S 登记（各 S 注记已验部分）。
2. **e2e 预登记跳过 4 例**：RB-09（64MB 受限目录）、TP-02/03/06（测量方法学）。
3. **变异覆盖为时间盒口径**：4 高点文件 288 点（v115）；v116 下拉钳制 + testenv 与
   **本轮 3 处修复 + DownloadDone 事件契约变更（has_checksum 字段移除）**未入变异集，
   下一轮 hardender 以本轮后工作区为新基线补跑。
4. **CRAP/CPD 历史口径不可复现注记**：v113「45 块」的 CPD 参数未随交接落盘，本轮
   以新口径重录（见对账口径节）；CRAP 受限近似为估算口径，非标准工具输出。
5. **沙箱无 TTY**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以 pyte 完成
   （本轮已全量执行）。
6. **环境**：沙箱 2 核 / 4G / 磁盘 10G；工具链 rustup nightly 1.101.0-nightly
   （`rust-toolchain.toml` 钉定）、cargo-llvm-cov 0.9.1（`~/.cargo/bin`）、
   PMD 7.28.0（`.tools/pmd-bin-7.28.0/`）、pyte + wcwidth（操作者 venv）。
   `export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"` 后验证命令可复现。

### 实现定义值登记（本轮新增/修订）
1. **单实例锁回退扩展**（main.rs `acquire_lock_at`）：锁落点按候选序列
   `[<state>/ezr.lock, <系统临时目录>/ezr.lock]` 逐点「create_dir_all + 打开 +
   flock」；state 目录**不可写**（如 ~/.ezr 555）时回退系统临时目录（01-config-
   template-04「启动不报错不崩溃」）。退化说明：目录先可写后变只读的混合场景下，
   二次启动会以临时目录锁放行（单实例保护在该退化环境降级）。
2. **D19 完成校验用最新值**（app/engine.rs DownloadDone）：是否校验以**任务当前
   checksum** 为准（事件不再携带 has_checksum 快照标志，契约字段已移除）；引擎按
   spec 快照跳过的物理收尾（改名 + 删 sidecar）由 App 幂等补齐；校验目标按文件
   实际位置选择（spec 无校验快照时引擎已先行改名）。
3. **恢复路径提示顺序**（app/tasks.rs Paused 恢复臂）：恢复提示先落、引用缺失
   提醒后发覆盖之——缺失提醒必须可见（02-named-proxy-05「toast 提醒一次」）。

## 二、当前产出情况（QA-20261007-final-verification）

### 本会话产物清单

**产品缺陷修复（3 处，全部失败先行 + 回归测试锁定）**
- `project/ezr/src/main.rs`：`acquire_lock_at` 不可写 state 目录锁回退
  （指纹：`instance_lock_falls_back_when_state_dir_unwritable`，旧实现 Err 退出）。
- `project/ezr/src/app/engine.rs`：DownloadDone 按 `t.checksum.is_some()` 判定 +
  幂等补收尾 + 校验路径按文件实际位置（指纹：`download_done_checksum_cleared_
  midflight_skips_verify` / `..._set_midflight_verifies`，旧实现假性校验失败 /
  跳过校验）。事件契约：`Evt::DownloadDone` 移除 `has_checksum` 字段（supervisor
  两 emit 点 + 测试构造点同步）。
- `project/ezr/src/app/tasks.rs`：Paused 恢复提示先发、缺失引用提醒后发
  （指纹：`toggle_pause_resume_missing_named_proxy_warns`，hardening_g_appcore.rs）。

**QA 套件基建修复（2 处，harness.py 共享层）**
- `detail_text()`：区域锚定从整右栏切片改为「任务详情」箱体（标题行起、图表面板
  标题行止）——修复 TD-03 假失败（「全局速度」面板标题误入详情断言区）。
- 新增共享助手 `task_pct()`：进度百分比读全宽行 + 光标定位行（进度渲染在列表行
  右缘列 60–65，列 64 起切片永远读不全 → 5 处百分比门失效）；修正
  persistence/resume/slots 三套件的 5 处调用点——修复 PC-01/08/09 假失败。

**QA 规程脚本化（3 套 31 用例，project/qa/runners/）**
- `suite_config_template.py`（QA-CT-01..05）：钦定文本逐字节比对（基线从 feature
  内嵌原文解析落盘）、不重写不覆盖、损坏配置、只读目录、EZR_HOME 重定位。
- `suite_modify_task.py`（QA-MT-01..08）：预填与焦点次序、并发热调升降、代理
  切换日志在案、校验改值判定、清空校验、非法校验码、已完成不可改、修改落盘重启。
- `suite_named_proxy.py`（QA-NP-01..18）：下拉选项与凭证零泄漏、多代理并行不串线、
  作废条目族（重名/空名/type 缺失或未知/凭证违规/ip/port）、旧形态注册表、
  失效引用回退、无效地址回退、IPv6 字面量加载。
- 共享基建：harness.py 新增 `start_proxy/proxy_reqs/stop_proxy`（ezr-proxy 实例
  管理收口，throttle_proxy 套件同步收编）。

**套件加固（1 处）**
- suite_tui_display.py TD-15 相位④：恢复后观察窗 12→20 样本 / 9→15s（恢复延迟 =
  连接重建秒级方差 + 首块 ~1.4s，原窗裕度不足，单跑复现即消失的负载型边缘）。

**端到端验证发现后修复的缺陷链（佐证 e2e 价值）**
- QA-CT-04 → 单实例锁回退缺陷；QA-MT-04/05 → D19 快照标志缺陷；
  QA-NP-05 → 恢复提醒覆盖缺陷。三处均为 cargo 测试全绿下漏网、端到端首捕。

### 验证证据（四要素①：本地验证命令清单，按序；crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 全量测试（修复后终态） | **1656P/0F** | `cargo test` |
| 2 | 逐目标对账 | bin 320 / ezr-fixture 30 / ezr_proxy 11 / hardening 107 / g_appcore 249 / g_engine 247 / g_main 2 / g_proxy 12 / g_supervisor 225 / g_throttle_err 167 / v115 286 | `cargo test` 输出逐行 |
| 3 | 静态检查 / 格式 / 架构 | clippy 0 / fmt 0 / arch 11/11 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| 4 | 端到端 12 套件 | **132P/0F/9S** | `python3 suite_<name>.py`（runners 目录，pyte 口径） |
| 5 | CRAP 受限近似 | >6 热点 **0**（1222 函数聚合） | `cargo llvm-cov --json` + 解析脚本（口径见对账节） |
| 6 | DRY（CPD） | 产品逻辑重复 0 | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 70 --dir src` |

### 对账口径
- **1656 对账**：1644（v116 总账）+ 4 本轮新增测试（main.rs 1 + app/engine.rs 2 +
  appcore 1）+ 8 = app/engine.rs 两条 cfg(test) 测试随 `#[path]` 挂载树在
  g_appcore/g_engine/g_supervisor/v115 四个 crate 各重复注册 2 次（合法副本，
  非 `--list | uniq -d` 可检的重复属性虚增；notes/rust.md 已落对账条款）。
- **e2e 132P/0F/9S 明细**：add_task 17 / download_engine 16 / integrity_check 11 /
  tui_display 14 / persistence_config 9 / slots_queue 13 / resume_sidecar 13 /
  retry_backoff 9P1S / throttle_proxy 4P3S / config_template 5 / modify_task 8 /
  named_proxy 13P5S。S 全部为预登记环境受限项（NP-04/08/13/15/18、RB-09、
  TP-02/03/06），注记均含「已验部分」。
- **CRAP 口径**：`cargo llvm-cov --json`（全目标）→ 函数级行覆盖比 + 分支计数
  comp=分支+1 → CRAP=comp²(1-cov)³+comp →（文件, 剥 crate-disambiguator 规范化名）
  聚合取任一实例覆盖即覆盖；全目标 1222 函数聚合，CRAP>6 热点 0；总行覆盖
  93.68%（与 v113 基线 93.42% 同口径量级）。近似估算口径，非标准工具输出。
- **CPD 口径**：PMD 7.28.0 `-l rust --minimum-tokens 70 --dir src`（src 含
  cfg(test)）：5 组 / 11 处 / 全部为既有 cfg(test) 测试模式（namegen 去重夹具、
  config 警告断言、supervisor 探针等待、ui 详情测试夹具），本轮改动文件 0 命中，
  产品逻辑 0 命中。历史「45 块」参数未随交接落盘、无法复算（40/50/70/100 档
  实测 37/5 组），自本轮起以上述口径为准。

### 四要素②：会话运行时产物位置与复现方法
- 覆盖率原始 JSON：`.work/tmp/coverage.json`（随会话终结清除；复现见要素① #5）。
- e2e 运行时临时目录 `.work/tmp/qa-run/`（用例隔离 HOME/保存目录，套件自清理），
  失败现场快照 `.work/tmp/qa-run/<case>.screen.txt`（本轮全部用例通过，无快照）。
- 各套件日志：`.work/tmp/qa-logs/<suite>.log`（会话终结清除；重跑即复现）。

### 四要素③/④
- 无 feature 注释块变化；`project/qa/` 三份新规程文档尾部补「套件脚本」节
  （runner 路径 + 执行方式），规程判据未改动。
- 无新增等价突变体（本轮未跑变异）。

### 待办与待批（未决项）
1. **【移交后续 hardender 轮】**变异补跑：以本轮后工作区为新基线（含 v116 钳制
   + testenv + 本轮 main.rs/tasks.rs/engine.rs 三处修复与事件契约变更）。
2. **【移交 coder/specifier 轮】**ezr-proxy 三类监听扩展（https / socks5+认证 /
   IPv6 回环）后补跑 NP-08/13/15/18 的 S 相位与 NP-04 强制认证子判据。
3. **【操作者裁决项（无阻塞）】**NP-04 口径：ezr-proxy 无认证要求模式下「认证
   通过」子判据以 S 登记（已验下载成功 + 凭证零泄漏）；如需闭环请裁决扩展优先级。
4. **mission.md 无待批**（v1.12 权威口径未变，QA 未改动）。

### 移交建议
- **six-pack 流水线已完结**（specifier → coder → cleaner → architect → hardender
  → QA → Done）。`bin/swarm complete` 归档即本轮任务终点，不自动流转。
- 后续如需增量：操作者显式指令后从 `six-pack/specifier` 重新开工（`project/`
  工作区完整保留，mission.md v1.12 为需求权威来源）；或按第八章流程直接指定
  任意角色会话。
- 归档前无需再清理（target/ 等构建缓存由 complete 自动清理；`.tools/` 按
  .gitignore 不入包，PMD 安装位置见遗留受限项 §6）。

By QA.
