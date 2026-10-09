# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender　会话：hardender-20261009-v118（任务 v118-ui-backfill）
> 版本注：变异加固 v1.18——v1.17 遗留受限项 3（触碰文件剩余 ~920 点，ui 层为主）的
> 全量补跑收官：定稿口径 **UI 层+交互分发层超集 25 文件/1072 点**，最终 **1051 caught +
> 14 unviable + 7 等价登记 = 1072，零未处置存活**；新增加固测试目标 2 个（609P）+
> 剥头副本 5 份 + gen-drift 守卫 5 处；修复杀灭链缺陷 2 处（守卫缺位 ×3 点 / 金样断言
> 错位 ×1 点）；CRAP 同口径复测刷新（comp 锚点逐点对齐）。产品源码零改动
> （pristine 逐文件 cmp EXIT=0）；全量 **2393P/0F**。

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.14）；分期详述 `project/mission/phase-01.md`（v1.14）。
- **本轮（hardender v118）做了什么**：①UI 层+交互分发层 1072 点变异补跑至收敛——
  首轮 5h（991C+67M+14U+0T）→ iterate 自动续跑以并行 timing 波动杀灭 56 → 顽固
  11 点经杀灭链修复后再跑（4C + 7 等价登记）→ 对账闭合自动停机；②根因三分定案：
  57 点覆盖缺口以两个新加固目标杀灭、5 点数学等价书面论证、5 点进程/环境类沿
  v1.17 先例书面论证；③发现并修复「加固测试绿 ≠ 杀灭力」缺陷 2 处（gen-drift
  守卫漏登记函数提取式副本 ×3 点、金样断言错位 ×1 点），修复后以 mutant 注入
  known-answer 校准实证杀灭链；④CRAP 同口径复测（55 项，src 范围 48，v1.17 为 60）。
- **质量终态**：全量 **2393P/0F**（13 测试目标）；clippy 0；fmt 0；arch 12 规则过；
  产品源码零改动。
- **移交**：v1.17 遗留受限项 3（变异覆盖范围）**已关闭**；全仓变异 2102 点中
  主清单 144 + 本轮 1072 + evt.rs 30 已全部收敛，其余点不在 v1.16 触碰集且无
  登记要求。操作者如认可当前终态，直接 `bin/swarm complete` 归档即可。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01 已交付；v1.13
  UI 对齐 + v1.14 详情面板批次 + cleaner 清理 + architect 架构轮 + hardender v1.17
  变异加固（主清单 144 点零存活）均已交付；本轮 hardender v118 全量补跑收官交付。
- 需求权威来源：`project/mission.md`（v1.14）；分期详述 `mission/phase-01.md`（v1.14，
  FR-01-94…101 + D26/D27）；功能规格 `project/features/`（13 份）；QA 规程 `project/qa/`
  （13 份规程 / 12 份套件脚本 + 共用 harness）。
- **外部参照**：`ezr-demo/docs/UI交互需求文档-定稿.md` v1.4 与 `ezr-demo/src/`
  （v1.14 权威 = ezr-demo 源码提交 5492c7c 修订-3/5/6/7）。

### 产物总账
| 阶段 | 做了什么 | 验证状态 |
|---|---|---|
| 基线交付 + 历轮加固 + 操作者第 7–9 批指令（至 v1.12） | 需求文档 + 下载内核 + TUI + 加固 + 度量收敛 | 已交付 |
| coder v116 / QA 终局（six-pack 收官轮） | 两缺陷修复；12 套件端到端 + 3 处缺陷 TDD 修复 | 1656P/0F；e2e 132P/0F/9S |
| specifier v1.13 + coder v1.13 | phase-01 v1.13 增补 + FR-01-94…99 全量实现 | 1729P/0F；clippy 0 |
| specifier v1.14 + coder v1.14 | FR-01-100/101 全量实现（TDD）+ 7 条新测试 | 1752P/0F；clippy/fmt 0 |
| cleaner v1.15 | 覆盖率 100%/0 缺口实测 + CRAP 67→64（5 拆分）+ CPD 41 块去重 + 变异点全仓扫描 | 1752P/0F；clippy/fmt 0；arch 过 |
| architect v1.16 | 四阶段评审 + 分发核/渲染核边界拆分 + 跨模块 DRY 单源五处 + 死 API 清理 + 属性测试 35→47 + arch 规则 12 | 1770P/0F；clippy/fmt 0；arch 12 规则过 |
| hardender v1.17 | 主清单 144 点变异补跑（零存活）+ checksum 加固测试 2 条 + CRAP/DRY 复测刷新 + 软 Gherkin 登记 | 1784P/0F；clippy/fmt 0；arch 过；pristine EXIT=0 |
| **hardender v1.18（本轮）** | UI 层+交互分发层 1072 点补跑全收敛（零未处置存活）+ 加固目标 2 个 609P + 副本/守卫体系 + 杀灭链修复 + CRAP 复测 | **2393P/0F；clippy/fmt 0；arch 过；pristine EXIT=0** |

### 遗留受限项（当前有效）
1. **ezr-proxy 三类监听扩展未做**（https / socks5+账号密码 / IPv6 回环）：QA-NP 部分
   S 相位沿旧口径登记（phase-02/后续承接）。
2. **e2e 预登记跳过 4 例**：RB-09（64MB 受限目录）、TP-02/03/06（测量方法学）。
3. ~~变异主清单之外的触碰文件未入变异集~~ → **本轮已关闭**：1072 点超集全收敛
   （见二节对账口径）；v1.16 触碰文件精确集虽不可复原，但审核超集（25 文件 1072 点）
   已一次性覆盖并超越其外延，全仓 2102 点剩余部分不在触碰集且无登记要求。
4. **软 Gherkin 的 runner 适配器需重建**：项目侧 APS 适配器（per-job 隔离 worker +
   generated entry points + metadata）属会话级产物已随会话清除；唯一有步骤绑定的
   01-persistence-config 的 manifest 干净且文件未变（详见二节四要素③），soft 差分
   当前为定义上的空操作；feature 内容一旦变更需先重建适配器再跑 mutation。
5. **沙箱无 TTY**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以 pyte 完成。
   本轮 main.rs 5 点进程级等价登记以此为先例口径。
6. **环境（2026-10-08/09 本会话重建）**：rustup nightly 1.101.0-nightly（1d81eb4ad，
   `~/.cargo/bin`，minimal + rustfmt/clippy/llvm-tools-preview）+ rustfilt 0.2.1
   （lcov demangle 用）；度量工具 `.tools/`：cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1
   （本轮重建安装）。PMD/bb 未重建（DRY 沿 v1.17 登记，产品源码零改动无需复测）。
   `export PATH="/home/z/swarm-ezr2/.tools/bin:$HOME/.cargo/bin:$PATH"` 后验证命令可复现。
7. **Ctrl+B 的 BT 断开实际执行**：phase-02 承接（D26；01 期守卫链 + HTTP 拒绝臂已实现）。
8. **沙箱无图形会话**：arboard 系统剪贴板路径不可运行时验证（OSC 52 回退臂生效）。
9. **CRAP>6 残余为登记口径**：本轮同口径复测刷新为 **55 项（全仓）/48 项（src 范围，
   v1.17 范围项 60 → 降 12）**；comp 口径锚点 consistency::check=13、find_companion=7
   与 v1.17/cleaner 登记逐点一致（comp 为 BRDA 上界近似，`&&` 链每项天然记 2）；
   按 cleaner 先例登记移交、不据此硬拆；清单明细见会话产物 `.work/tmp/crap_result.txt`
   （复现方法见二节要素②）。
10. **bin 入口前导抽取裁决：不做（v1.16 改判维持）**；**join 语义双口径**（受限项 11
    沿旧登记）；**parse_retry_after 饱和口径**（实现定义值 13）——均已裁决/锚定。

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
    `bytes == BT_CHUNK_SIZE → "256 KB"` 两臂为定稿口径显式锚点——v1.17 变异实测两臂
    均被现有测试捕获（预登记的等价豁免无需启用）。
13. **parse_retry_after_header 饱和口径**：`"inf"` → `Some(f64::INFINITY)`；NaN/负数/
    垃圾 → None（属性测试锁定）。
14. **validate_value 越界钳位口径（v1.17 加固测试锁定）**：算法表下标 ≥7 一律钳到表尾
    Adler-32（期望 8 位十六进制），不 panic。
15. **update_conn_stats 连接账本口径（本轮加固测试锁定）**：末块完成帧（done==cap 或
    cap==0）零值速断 `zero()` 无拖尾；速度口径 = 块内增量/dt 经 EMA（α=1/5）平滑；
    累计量随增量累加（tests/hardening_engine_backfill.rs 剥头夹具纯函数直测锁定）。

## 二、当前产出情况（hardender-20261009-v118）

### 本会话产物清单
- `tests/hardening_ui_backfill.rs`（新增，319P）：UI 层+交互分发层存活体杀灭套件
  （第一目标：无状态/渲染/纯函数簇 46 靶点 + 等价登记 3 点对账）。布局：`#[path]`
  挂载 app/engine/model/ui 全树 + `tests/gen/*_stripped.rs` 剥头副本壳（私有纯函数
  同模块可达）+ gen-drift 漂移守卫 4 处（chart/clipboard/detail/btn 整文件剥头比对）。
  靶点：getters 12、dialogs 6、keys 2、mouse 5、clipboard 1、chart 11、detail 3、
  delete 3、conns 1、btn 2（+396:40/45:76/位域×2/osc52 等价与环境类登记）。
- `tests/hardening_engine_backfill.rs`（新增，290P）：引擎账本簇杀灭套件（第二目标
  11 靶点）。进程内 mock HTTP 服务器 + `App::tick` 自动发车驱动真实下载灌连接级
  账本（conn_prev/conn_cum/conn_speed），经 pub(crate) getter 断言；`update_conn_stats`
  3 点经剥头夹具纯函数直测（合成 Instant 零抖动）；**gen-drift 守卫 1 处**
  （`gen_ucs_fixture_matches_product_source`：函数提取式片段逐字比对——本轮修复补上）。
- `tests/gen/*_stripped.rs` ×5（新增）：剥头副本（chart/clipboard/btn/detail/
  update_conn_stats）；漂移守卫机制：副本 ≠ 产品源码 → 测试红 → 场景判 caught。
- `mini-services/mutant-runner/index.ts`（修改，v118b）：FILTER_ARGS 6 项过滤
  （1072 点清单口径）；动态终止（Found N + 账本去重对账）；**等价豁免收敛**
  （EQUIV_ALLOWLIST 7 条，missed ⊆ 清单 → done+equivOk 停机，防登记点无限重测）；
  磁盘看门狗 30s（<800MB SIGTERM）；存活体/无进展保护。
- `.work/tmp/` 会话产物（不进归档，复现方法见四要素）：`mutants_scope_list.txt`
  （1072 点定稿清单）、`pristine/src`（字节级备份）、`mutants-resume/`（resume 账本，
  previously_caught.txt 去重 1065 行）、`missed_final.txt`（首轮 67 条 missed 存档）、
  `triage.py`（summary/log 两命令）、`mutants-service.log`（托管服务全程日志）、
  `crap_v118.lcov` + `crap_result.txt` + `analyze_coverage.py`（CRAP 复测三件套）。
- 产品源码（src/）**零改动**：pristine 逐文件 cmp EXIT=0；仅 tests/ 增量。

### 验证证据（crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 全量测试（13 目标） | **2393P/0F** | `cargo test --workspace` |
| 2 | 逐目标对账 | bin 359 / ezr-fixture 30 / ezr_proxy 11 / hardening 121 / **engine_backfill 290** / g_appcore 265 / g_engine 263 / g_main 2 / g_proxy 12 / g_supervisor 241 / g_throttle_err 167 / **ui_backfill 319** / v115 313 | `cargo test` 输出逐行 |
| 3 | 加固套件单列 | 609P/0F（290+319） | `cargo test --test hardening_engine_backfill --test hardening_ui_backfill` |
| 4 | 静态检查 / 格式 / 架构 | clippy 0 / fmt 0 / arch 12 规则过 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| 5 | 变异补跑（1072 点） | **1051 caught + 14 unviable + 0 timeout + 7 等价登记（零未处置存活）** | 见下方要素② resume 复现 |
| 6 | CRAP 同口径复测 | >6 计 55 项（src 范围 48 vs v1.17 的 60；comp 锚点 13/7 逐点一致） | `cargo llvm-cov --workspace --lcov --branch --hide-instantiations -- --skip property_tests > <lcov>` + `python3 .work/tmp/analyze_coverage.py <lcov>` |
| 7 | DRY | 产品源码零改动 → **沿 v1.17 登记 37 ≤ 41**（未复测，源未变） | v1.17 要素② |
| 8 | 产品源码零改动 | pristine cmp EXIT=0 | `diff -rq .work/tmp/pristine/src src` |

### 对账口径
- **2393 对账**：1784（v1.17 基线，逐目标 359/30/11/121/265/263/2/12/241/167/313
  未动）+ 319（ui_backfill 新目标）+ 290（engine_backfill 新目标，含 ucs 守卫 1 条）；
  两新目标内部 = `#[path]` 挂载产品测试的合法副本（挂载树倍增机制，notes/rust.md）
  + 全新加固测试 ≈35 条 + gen-drift 漂移守卫 5 条。既有 1784 测试零改动全绿。
- **变异对账**：定稿清单 1072 点（25 文件：src/ui 全部 547 + src/app 全部 548−evt 30
  + main.rs 32；v1.17 主清单 144 与 evt 30 已收敛不在范围）。**1072 = 1051 caught +
  14 unviable + 7 等价登记**；账本对账 previously_caught.txt 去重 1065 = 1051+14 ✓。
  unviable 14 明细随 iterate 轮次输出目录重建未逐行保留（汇总行
  `1072 mutants tested in 5h: 67 missed, 991 caught, 14 unviable` 为准）；0 timeout。
- **收敛轨迹**：首轮 5h → 991C+67M+14U；iterate 续跑 67→39→11（并行 timing 波动
  杀灭 56）；杀灭链修复后 11 点重测 16m → 4C+7M（全等价）→ 服务等价豁免收敛停机。
- **等价/环境类登记 7 点（零存活口径的书面论证）**：
  1. `main.rs:60 restore_terminal → ()`：进程生命周期（crossterm 终端恢复），无 TTY
     测试环境不可观测，副作用经 `let _ =` 显式吞错（沿 v1.17 环境类先例）。
  2. `main.rs:70 setup_panic_hook → ()`：panic hook 注册的终端恢复路径，同上
     （纯测试进程不触发 main 的 panic 场景）。
  3. `main.rs:362:22 overlay_sig | → ^`：**数学等价**——sig 位域 bit0 恒 1、
     bit1=ck_open（∈{0,1} 左移后无进位），`1|{0,2}` ≡ `1^{0,2}` 真值表相同。
  4. `main.rs:370 run → Ok(())`：主事件循环（EventStream/轮询/渲染）需真实 TTY
     与终端事件流，进程级边界（同 1）。
  5. `main.rs:378:16 != → ==`：run 内覆盖层翻转清屏检测，同 4（测试不可达）。
  6. `app/mod.rs:396:40 global_ul_speed > → >=`：**求和不变量**——upload_speed
     语义非负，`>= 0.0` 仅多计入 0.0 速度任务，对 f64 求和值不变（含 ±0.0 情形）。
  7. `keys.rs:45:76 BackTab - → +`：**数学等价**——FILTERS.len()==2 时
     `(filter+len-1)%len ≡ (filter+len+1)%len`（±1 mod 2 同余）。
- **杀灭链缺陷与修复（本轮核心教训，宪法第三章经验）**：首轮 67 missed 的加固
  测试全绿，但 iterate 重测暴露 11 点顽固存活——**测试绿 ≠ 杀灭力**：
  ① `update_conn_stats` 3 点：测试经剥头副本直测（副本永不变异），杀灭必须由
  gen-drift 守卫感知产品变异——守卫漏登记该函数提取式副本 → 补
  `gen_ucs_fixture_matches_product_source`（锚点提取片段逐字比对）；
  ② `delete.rs:77:24`：金样断言错位——rows[10] 实为标题行（不受该位点控制），
  提示行 rows[13] 未断言 → 补 rows[13] 逐字金样。修复后 **known-answer 校准**：
  注入 `&&→||` 与 `+→-` 两个 mutant 实测两套件均红 → pristine 还原 cmp EXIT=0 →
  重测 4 点全部 caught，杀灭链闭环实证。

### 四要素①：本地验证命令清单（按序）
```bash
export PATH="/home/z/swarm-ezr2/.tools/bin:$HOME/.cargo/bin:$PATH"
cd project/ezr
cargo test --workspace            # 全量 2393P/0F（13 目标）
cargo test --test hardening_engine_backfill --test hardening_ui_backfill   # 609P
cargo clippy --all-targets        # 0 警告
cargo fmt --check                 # 0 偏差
bash scripts/arch_check.sh        # 12 规则过
diff -rq ../../.work/tmp/pristine/src src   # 产品源码零改动 EXIT=0
```

### 四要素②：变异复现（1072 点收敛口径）
```bash
export PATH="/home/z/swarm-ezr2/.tools/bin:$HOME/.cargo/bin:$PATH"
cd project/ezr
cargo mutants --iterate -j 2 --baseline skip -t 240 --build-timeout 420 \
  --minimum-test-timeout 220 -o ../../.work/tmp/mutants-resume \
  -f 'src/ui/*.rs' -f 'src/app/*.rs' -f 'src/app/dialog_keys/*.rs' \
  -f 'src/app/engine/mod.rs' -f 'src/app/engine/tick.rs' -f 'src/main.rs'
# 环境变量：CARGO_PROFILE_DEV_DEBUG=0（去 DWARF 缩盘 2.9G→1.1G，行为不变）
# 现账本（previously_caught.txt 去重 1065）天然跳过已收敛点，仅剩 7 登记点
# missed；预期输出末轮 `Found 7` 全部 missed（等价登记，handoff 上表逐条论证）
# 长跑建议：mini-services/mutant-runner（bun 托管 + 等价豁免收敛 + 磁盘看门狗）
```

### 四要素③：上游交接核对
- v1.17 handoff 遗留待办「920 点续跑」：本轮以 1072 点审核超集收官（受限项 3 关闭）；
  基线 1784P/0F 逐目标一致后开工，终态 2393P/0F。
- v1.17 CRAP 登记 74 项（范围 60）：本轮复测 55 项（范围 48），comp 锚点 13/7
  逐点一致，口径连续。
- 软 Gherkin：feature 清单未变（product 源零改动），空操作口径维持，适配器重建
  要求沿 v1.17 登记。

### 四要素④：环境自重建说明
- 工具链：rustup nightly 1.101.0-nightly(1d81eb4ad)（minimal + rustfmt + clippy +
  llvm-tools-preview）→ `~/.cargo/bin`；cargo-mutants 27.1.0（源码编译
  `cargo install cargo-mutants --version 27.1.0 --locked --root ../../.tools`）+
  rustfilt 0.2.1 + cargo-llvm-cov 0.9.1（同法）→ `.tools/bin`。
- 会话级状态（.work/tmp/）不随归档流转：pristine 备份、resume 账本、清单、CRAP
  三件套均可按上述命令从零重建（账本重建 = 全量重跑，~6.5h）。
- 磁盘预算：变异期 worker tmp + 主 target 峰值 ~6G，CRAP 复测前清空主 target
  （llvm-cov 独立 target 目录带 debuginfo=2 需 ~2G）；<800MB 看门狗停机保护。
