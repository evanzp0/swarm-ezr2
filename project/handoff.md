# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（最后一名角色）　会话：qa-20261009-v119（任务 v119-qa-final）
> 版本注：QA 终局独立验证 v1.19——对 hardender v1.18 交接批次做全量验收：
> 单元/加固/属性测试 **2393P/0F**（13 目标逐一对账一致）+ clippy/fmt 0 + arch 12 规则过 +
> 端到端 12 套件 **132P/0F/9S 与上游基线逐位一致**（初轮 112P/20F/9S，20F 归因为套件层
> 陈旧断言——v1.13/v1.14 UI 对齐后仅 2 份规程同步过；已同步 9 套件 + harness 2 处 +
> 3 行规程，**产品源码零改动**，git 基线核验）+ CRAP comp 锚点逐点一致（13/7）+
> DRY 实测 37 ≤ 41 与 v1.17 登记一致。**six-pack 流转路径（specifier → coder → cleaner
> → architect → hardender → QA → Done）已走完，工作流完结。**

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.14）；分期详述 `project/mission/phase-01.md`（v1.14）。
- **本轮（QA v119）做了什么**：①全量 2393P/0F 与 609P 加固套件复测，13 目标逐一对账与
  v1.18 基线一致；②12 个端到端 PTY 套件全量首轮运行——暴露 20 例失败，逐例取证归因为
  **套件层陈旧断言**（非产品缺陷）：v1.13 布局定稿撤销「任务队列/全局速度」面板、头部
  「并发」语义改为连接总数、v1.14 详情类型行去并发数，而仅 01-tui-display/04-ui-conns
  两份规程同步过；③同轮修复：harness 公共层 2 处（detail_text 面板箱体锚点、新增
  chart_points 内嵌流量图按列去重助手）+ 9 个套件陈旧锚点 + 3 行规程同步；
  ④CRAP/DRY 复测登记（见二节）；⑤环境从零重建（新沙箱）。
- **质量终态**：全量 **2393P/0F**；e2e **132P/0F/9S**（S=9 全部规程预登记）；clippy 0；
  fmt 0；arch 12 规则过；CRAP comp 锚点 13/7 一致；CPD 37 ≤ 41；**产品源码零改动**
  （git 基线核验，fresh clone 无本地修改）。
- **移交**：six-pack 工作流**已完结**（QA 为最后一名角色）。操作者如认可当前终态，
  `bin/swarm complete` 归档即可；phase-02（BT/磁力链）为新工作流起点，由首角色依
  操作者输入建档 `mission/phase-02.md` 后开工。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01 已交付；v1.13
  UI 对齐 + v1.14 详情面板批次 + cleaner/architect/hardender 各轮（v1.15–v1.18）均已交付；
  本轮 QA v1.19 终局独立验证交付，phase-01 收官。
- 需求权威来源：`project/mission.md`（v1.14）；分期详述 `mission/phase-01.md`（v1.14，
  FR-01-94…101 + D26/D27）；功能规格 `project/features/`（13 份）；QA 规程 `project/qa/`
  （13 份规程 / 12 份套件脚本 + 共用 harness；本轮起规程与脚本已对齐 v1.14 口径）。
- **外部参照**：`ezr-demo/docs/UI交互需求文档-定稿.md` v1.4 与 `ezr-demo/src/`
  （v1.14 权威 = ezr-demo 源码提交 5492c7c 修订-3/5/6/7）。

### 产物总账
| 阶段 | 做了什么 | 验证状态 |
|---|---|---|
| 基线交付 + 历轮加固 + 操作者第 7–9 批指令（至 v1.12） | 需求文档 + 下载内核 + TUI + 加固 + 度量收敛 | 已交付 |
| coder v116 / QA 终局（v116 收官轮） | 两缺陷修复；12 套件端到端 + 3 处缺陷 TDD 修复 | 1656P/0F；e2e 132P/0F/9S |
| specifier v1.13 + coder v1.13 | phase-01 v1.13 增补 + FR-01-94…99 全量实现 | 1729P/0F；clippy 0 |
| specifier v1.14 + coder v1.14 | FR-01-100/101 全量实现（TDD）+ 7 条新测试 | 1752P/0F；clippy/fmt 0 |
| cleaner v1.15 | 覆盖率 100%/0 缺口实测 + CRAP 67→64（5 拆分）+ CPD 41 块去重 + 变异点全仓扫描 | 1752P/0F；clippy/fmt 0；arch 过 |
| architect v1.16 | 四阶段评审 + 分发核/渲染核边界拆分 + 跨模块 DRY 单源五处 + 死 API 清理 + 属性测试 35→47 + arch 规则 12 | 1770P/0F；clippy/fmt 0；arch 12 规则过 |
| hardender v1.17 | 主清单 144 点变异补跑（零存活）+ checksum 加固测试 2 条 + CRAP/DRY 复测刷新 + 软 Gherkin 登记 | 1784P/0F；clippy/fmt 0；arch 过；pristine EXIT=0 |
| hardender v1.18 | UI 层+交互分发层 1072 点补跑全收敛（零未处置存活）+ 加固目标 2 个 609P + 副本/守卫体系 + 杀灭链修复 + CRAP 复测 | 2393P/0F；clippy/fmt 0；arch 过；pristine EXIT=0 |
| **QA v1.19（本轮）** | 终局独立验证：全量/加固/静态/e2e/CRAP/DRY 六线复测 + e2e 套件陈旧断言全量同步（9 套件 + harness 2 处 + 3 行规程） | **2393P/0F；e2e 132P/0F/9S；clippy/fmt 0；arch 过；CRAP 锚点 13/7；CPD 37≤41；src 零改动** |

### 遗留受限项（当前有效）
1. **ezr-proxy 三类监听扩展未做**（https / socks5+账号密码 / IPv6 回环）：QA-NP 5 例
   S 相位沿规程环境前置登记（phase-02/后续承接）。
2. **e2e 预登记跳过 9 例**：QA-NP 5（上述受限项 1）+ RB-09（64MB 受限目录，沙箱无
   mount/配额权限）+ TP-02/03/06（测量方法学，限速生效语义由 TP-01 100MB 总量口径
   权威覆盖）。合计 S=9，与本轮 e2e 9S 逐位对应。
3. **沙箱无 TTY（进程级）**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以
   pyte 完成（本轮 12 套件 132 断言即此路径）。
4. **沙箱无图形会话**：arboard 系统剪贴板路径不可运行时验证（OSC 52 回退臂生效）。
5. **软 Gherkin 的 runner 适配器需重建**：项目侧 APS 适配器属会话级产物已随会话清除；
   feature 清单未变（产品源零改动），soft 差分当前为定义上的空操作；feature 内容一旦
   变更需先重建适配器再跑 mutation（v1.17 登记延续）。
6. **CRAP>6 残余为登记口径**：本轮脚本从零重建后清单 **70 项**（本次 lcov 运行无
   tests/ 节，全仓=src 范围；v1.18 登记 55/48——差值为脚本重建聚合粒度，非产品变化，
   产品源码零改动已 git 核验）。**comp 口径锚点逐点一致：consistency::check=13、
   find_companion=7**（与 v1.17/cleaner 登记连续，comp 为 BRDA 上界近似，`&&` 链每项
   天然记 2）；按 cleaner 先例登记移交、不据此硬拆守卫链；清单明细见
   `.work/tmp/crap_result_v119.txt`（复现见二节要素②）。
7. **bin 入口前导抽取裁决：不做（v1.16 改判维持）**；**join 语义双口径**；**parse_retry_after
   饱和口径**——均已裁决/锚定（v1.18 登记延续）。
8. **环境（2026-10-09 本会话从零重建）**：rustup nightly 1.101.0-nightly（a30aa9064
   2026-10-08，`~/.cargo/bin`，minimal + rustfmt/clippy/llvm-tools-preview）+ rustfilt
   0.2.1 + cargo-llvm-cov 0.9.1（`.tools/bin`）+ PMD 7.28.0（`.tools/pmd-bin-7.28.0`）+
   python3 pyte。`export PATH="/home/z/swarm-ezr2/.tools/bin:$HOME/.cargo/bin:$PATH"`
   后验证命令可复现。

### 实现定义值登记（累计，当前有效全量）
1. **单实例锁回退扩展**（main.rs）：state 目录不可写时回退系统临时目录。
2. **D19 完成校验用最新值**（app/engine/evt.rs）：完成判定以任务当前 checksum 为准。
3. **恢复路径提示顺序**（app/tasks.rs）：恢复提示先落、缺失引用提醒后发覆盖。
4. **D26 并发明细 toast 文案**：照录 demo 定稿原文。**v1.14 裁决维持**。
5. **连接级累计/速度口径（FR-01-99）**：App 消费侧计量、EMA α=1/5 平滑、清零触发
   与保留规则（v1.13 登记未触碰）。
6. **并发明细选择序号口径**：conns_sel 为连接 id 升序展示序。
7. **流量图采样**：speed_hist 每秒 ≤1 点、双向 EMA、窗口缩放（chart::chart_columns 纯函数）。
8. **D27 URL 热区复制内容**：复制详情展示值（final_url 优先）；「校验」热区复制完整校验码值。
9. **复制反馈账本**：App::last_copied 记录最近复制内容。
10. **剪贴板实例生命周期**：arboard 懒初始化复用，失败回退 OSC 52。
11. **热区几何**：字段名整段 9 列热区（行号超出内框高度不设）。
12. **fmt_block_size 钦定锚点（维持保留）**：`bytes == MB → "1 MB"`、
    `bytes == BT_CHUNK_SIZE → "256 KB"` 两臂为定稿口径显式锚点。
13. **parse_retry_after_header 饱和口径**：`"inf"` → `Some(f64::INFINITY)`；NaN/负数/
    垃圾 → None（属性测试锁定）。
14. **validate_value 越界钳位口径**：算法表下标 ≥7 一律钳到表尾 Adler-32（期望 8 位
    十六进制），不 panic。
15. **update_conn_stats 连接账本口径**：末块完成帧零值速断 `zero()` 无拖尾；速度口径 =
    块内增量/dt 经 EMA（α=1/5）平滑；累计量随增量累加。

## 二、当前产出情况（qa-20261009-v119）

### 本会话产物清单
- `qa/runners/harness.py`（修改，2 处）：①`detail_text` 面板箱体锚点「全局速度」→
  「并发连接」（v1.13 布局：详情正下方为并发连接面板，旧独立图表面板已撤销；
  修正 TD-03 not_in「速度」区域过宽误报）；②新增 `chart_points`：头部右侧内嵌流量图
  采样点数（区域=头部第 1–3 行最右两个「│」之间，按列去重，1 点=1 列含柱条字符），
  供 TD-15/DE-14 复用。
- `qa/runners/suite_*.py`（修改，9 份）：add_task（_task_count 页签行计数、QA-01-01/06
  并发锚点改头部数据面、QA-01-17 已完成 (1)）、tui_display（TD-01 面板撤销断言、
  TD-12 头部新口径、TD-13 页签计数、_bar_count→chart_points）、config_template（2 处
  并发锚点）、modify_task（_active_conns 活跃连接助手——头部「并发 N」为连接总数
  04-ui-conns-04，MT-02 活跃升降以面板「活跃 x」为准 04-ui-conns-08；MT-08 并发还原
  锚点）、persistence_config（PC-01 页签计数 3+1、PC-03 并发锚点）、resume_sidecar
  （RS-06 正在下载 (8) + 已完成 (1)）、download_engine（DE-14 chart_points、DE-16
  已完成 (1)）、throttle_proxy（TP-07 峰值/并发连接锚点）。**产品源码零改动**。
- `qa/01-add-task-qa.md`、`qa/03-modify-task-qa.md`、`qa/01-download-engine-qa.md`
  （修改，各 1–2 行）：期望列锚点同步 v1.13/v1.14 口径（详情类型行无并发数、并发经
  头部数据面字段/活跃计数核验、流量图内嵌头部）。其余 8 份规程经核与脚本一致
  （01-tui-display-qa.md 已在 v1.13 同步过）。
- `.work/tmp/` 会话产物（不进归档）：`crap_v119.lcov` + `crap_result_v119.txt` +
  `analyze_coverage.py`（CRAP 复测三件套，从零重建）+ `cpd_v119.txt`（CPD 原始输出）+
  `qa-e2e/*.log`（12 套件全程日志）+ `llvmcov.log`。

### 验证证据（crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 全量测试（13 目标） | **2393P/0F**，逐目标对账 v1.18 一致（359/30/11/121/290/265/263/2/12/241/167/319/313） | `cargo test --workspace` |
| 2 | 加固套件单列 | 609P/0F（290+319） | `cargo test --test hardening_engine_backfill --test hardening_ui_backfill` |
| 3 | 静态检查 / 格式 / 架构 | clippy 0 / fmt 0 / arch 12 规则过 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| 4 | 端到端 QA（UI 层 PTY，12 套件） | **132P/0F/9S**——与上游 v116 收官轮基线逐位一致 | `cd project/qa/runners && python3 suite_<name>.py`（逐套件） |
| 5 | CRAP 同口径复测 | comp 锚点 13/7 逐点一致；CRAP>6 清单 70 项（登记口径，见受限项 6） | `cargo llvm-cov --workspace --lcov --branch --hide-instantiations -- --skip property_tests > <lcov>` + `python3 .work/tmp/analyze_coverage.py <lcov>` |
| 6 | DRY（PMD 7.28.0 CPD） | **37 重复块 ≤ 41**（-l rust，minimum-tokens 50；与 v1.17 登记一致；exit 4=发现重复正常语义） | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src` |
| 7 | 产品源码零改动 | git 基线核验 src/ 无修改（fresh clone 基线） | `git -C /home/z/swarm-ezr2 status --porcelain project/ezr/src`（无输出） |

### 对账口径
- **e2e 132 对账**：12 套件 P 数 17+5+8+13+9+14+13+13+11+9+4+16 = 132 ✓；
  S=9 = QA-NP 5 + RB-09 1 + TP-02/03/06 3 ✓；F=0。
- **初轮 20F 归因**（全部套件层，非产品层）：v1.13/v1.14 UI 对齐轮只同步了
  01-tui-display 与 04-ui-conns 两份规程，其余 8 份规程的期望列未点名未同步，对应
  套件断言锚定旧布局文本（「任务队列/全局速度/N 并发/任务 N·/并发线程」）；harness
  `detail_text` 区域锚点「全局速度」随面板撤销失效（区域过宽混入连接面板文本）。
  修复以 Gherkin 权威行锚定：04-ui-conns-01（任务队列撤销）/04（并发=连接总数）/
  08（活跃=速度>0 连接数）/16（类型行无并发数）、01-tui-display-03/33。
- **CRAP 70 vs v1.18 登记 55/48**：分析脚本属会话级产物需从零重建（handoff v1.18
  要素④口径），重建聚合粒度（闭包并外层、同行多实例化副本取极值、tests 模块按
  demangled 路径段排除）与上轮脚本存在镜头差；**comp 锚点 13/7 逐点一致**为跨会话
  可比性的校准证据；产品源码零改动，产品度量真实状态与 v1.18 相同。
- **DRY 37 = v1.17 登记 37**：同阈值（minimum-tokens 50）同工具族实测一致，独立佐证
  产品源码零改动。

### 待办与待批（未决项）
- 无阻塞项。QA 套件与 Gherkin/单测无未裁决矛盾（20F 归因为规程/脚本陈旧，已按已接受
  规格同轮同步；产品行为未动）。
- 移交建议：six-pack 流转路径已走完、工作流完结。phase-02（BT/磁力链）开工时由首角色
  （specifier）依操作者输入建档 `mission/phase-02.md`（宪法第一章需求存放两模式）；
  届时建议先复核本轮同步后的 12 套件与规程口径作为 phase-01 回归基线。

### 四要素①：本地验证命令清单（按序）
```bash
export PATH="/home/z/swarm-ezr2/.tools/bin:$HOME/.cargo/bin:$PATH"
cd project/ezr
cargo test --workspace            # 全量 2393P/0F（13 目标）
cargo clippy --all-targets        # 0 警告
cargo fmt --check                 # 0 偏差
bash scripts/arch_check.sh        # 12 规则过
cd ../qa/runners && for s in suite_*.py; do python3 "$s"; done   # e2e 132P/0F/9S
```

### 四要素②：CRAP/DRY 复现
```bash
export PATH="/home/z/swarm-ezr2/.tools/bin:$HOME/.cargo/bin:$PATH"
cd project/ezr
cargo llvm-cov --workspace --lcov --branch --hide-instantiations -- --skip property_tests \
  > ../../.work/tmp/crap_v119.lcov
python3 ../../.work/tmp/analyze_coverage.py ../../.work/tmp/crap_v119.lcov
# 预期：锚点行 comp=13 / comp=7；CRAP>6 清单 70 项
../../.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src
# 预期：exit 4（发现重复），37 块 ≤ 41
```

### 四要素③：上游交接核对
- v1.18 handoff 遗留待办「操作者认可后 complete 归档」：本轮 QA 独立验证通过，
  六线证据齐备，归档条件满足。
- v1.18 CRAP 登记 55/48：本轮重建脚本 70 项（锚点一致），差异已登记（受限项 6）。
- v1.17 DRY 登记 37 ≤ 41：本轮实测 37，一致。
- e2e 预登记跳过口径（v1.18 受限项 2 的 4 例 + QA-NP 规程环境前置 5 例）：本轮
  S=9 逐位对应。

### 四要素④：环境自重建说明
- 工具链：rustup nightly 1.101.0-nightly(a30aa9064)（minimal + rustfmt + clippy +
  llvm-tools-preview）→ `~/.cargo/bin`；cargo-llvm-cov 0.9.1 + rustfilt 0.2.1
  （`cargo install <crate> --locked --root .tools`）→ `.tools/bin`；PMD 7.28.0
  （GitHub releases `pmd_releases/7.28.0` 资产 `pmd-dist-7.28.0-bin.zip` 解压
  `.tools/pmd-bin-7.28.0`，需 `chmod +x bin/*`）；pyte（`pip install pyte`）。
- 会话级状态（.work/tmp/）不随归档流转：CRAP 三件套、CPD 输出、e2e 日志均可按上述
  命令从零重建（llvm-cov 插桩重建 ~5 分钟，CRAP 分析 ~1 分钟）。
- 磁盘预算：llvm-cov 独立 target 目录峰值 ~3.1G（用后可删 `target/llvm-cov-target`，
  不影响主 target 增量性）；主 target（DEBUG=0）~1.9G。
