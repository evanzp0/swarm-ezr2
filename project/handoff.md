# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（v1.3 后复核批次，第二轮架构评审）　会话：architect-20261004

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.3 大纲）+ `project/mission/phase-01.md`（v1.3 详述）。
- **v1.3 修订（coder 轮，操作者定案）**：①`.ezr` 根目录可经环境变量 `EZR_HOME` 重定位
  （FR-01-70/71 修订，D16）；②新增 FR-01-84 终端哨兵——任意原因终止（含 kill -9）后
  运行 ezr 的终端必须自恢复；③AC-9 同步扩展。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier / coder / cleaner 组 / QA（首轮至四轮） | 详见历史归档 `swarm-20261002-062546` ~ `091047` | 已交付 | 208 测全绿基线 |
| coder（缺陷修复） | 明细表重复表头 + 闪烁两缺陷修复 | 已交付 | 210 测全绿 |
| specifier（三轮修订） | FR-01-81 修订二：并发分块明细表整体移除（三层同步） | 已交付 | 归档 specifier-20261002c |
| coder（三轮修订实现） | 明细表 UI 段落 + 连接级展示机制整体移除 | 已交付 | 206 测全绿、clippy 0、fmt 清零 |
| cleaner（清理批次） | 保持行为清理 3 处 + 覆盖率/CRAP/DRY/变异点四项度量与补测 15 条 | 已交付 | 221 测全绿、clippy 0、fmt 0 |
| architect（首轮） | 四阶段架构评审 + 4 项保持行为改造 + 2 项裁决 + proptest 属性测试 12 条 + arch_check.sh | 已交付 | 233 测全绿、clippy 0、fmt 0、arch_check EXIT=0 |
| coder（v1.3 修订） | 需求文档三层同步 + `EZR_HOME` 重定位 + 终端哨兵（`src/sentinel.rs`）+ main.rs 接线 | 已交付 | 240 测全绿、clippy 0、fmt 0、arch_check 6/6、pty 端到端 A/B/C/D 通过 |
| cleaner（v1.3 后清理批次） | 保持行为清理 2 处 + `parse_speed` 表驱动改造 + sentinel 补测 6 条 + 四项度量 | 已交付 | 246 测全绿、clippy 0、fmt 0、arch_check 6/6 |
| **architect（本轮，第二轮复核）** | 四阶段架构评审（新增面全过）+ doc 口径修正 1 处 + proptest +12 条（24 条）+ arch_check 规则 7/8 新增（阳性对照生效）+ CRAP>6 拆分裁决 | 已交付 | 258 测全绿（详见验证证据）、clippy 0、fmt 0、arch_check 8/8 |

### 遗留受限项（当前有效）

- CPD 余 3 块 `#![allow]` lint 配置头跨文件同形（既往登记，无新增）：model/engine 层
  两个子集家族 + app/mod.rs 与 ezr-proxy.rs 交互层豁免头。Rust 逐文件豁免惯用法，
  登记接受，无需再报。
- CRAP 为受限近似口径（llvm-cov 行覆盖 + BRDA 分支计数代入标准公式；偏差方向：`?` 操作符
  无 BRDA → 入口胶水 comp 低估，测试代码 assert! 宏展开 → 测试函数虚增，门禁排除测试模块）；
  hardender 期以变异结果为准。**早期批次遗留 CRAP>6 共 54 项**，拆分裁决（本轮，见当前节）
  ：四命名函数均不拆，交 hardender 变异后针对性处置。
- 工具链（本轮沙箱全新重建）：rustup stable 1.99.0 + nightly（rustfmt/clippy/
  llvm-tools-preview），`~/.cargo/bin`；lld 桥接 `~/.local/bin/ld.lld` → rust-lld
  （PATH 需含两目录）。度量工具：cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1
  （GitHub 预编译直装），`~/.cargo/bin`；PMD 7.28.0 在 `.tools/pmd-bin-7.28.0/`
  （`pmd cpd` 子命令，`-l rust` 必带）。
- QA-TD-15 端到端 runner 实现与 9 套整体回归仍欠（QA 节点，承前）；
  features/01-persistence-config 场景 10/11 的 QA runner 一并待 QA 节点补齐。
- 终端哨兵边界（承前）：以 `exec ezr` 方式令 ezr 自任会话首时，SIGKILL 会触发内核对控制
  终端的 hangup，复原序列写入将 EIO（termios 仍由哨兵还原）；常规「shell + 前台作业」
  场景已完整覆盖。
- pty 端到端脚本（e2e_v13.py）为 coder 会话临时件未随归档保留：本轮未复跑（本轮产品行为
  零变更；本轮验证面 = 单测 258 全绿 + 承前 pty A/B/C/D 证据）。
- **单实例锁无 home 回退分支（本轮登记，行为未改）**：`main.rs` `acquire_instance_lock`
  在 `state_dir()` 为 None（无 HOME/USERPROFILE 且未设 EZR_HOME）时仅 `File::create`
  temp 锁文件、**不做 flock**——该环境下并发实例互不排除。属既有行为（v1.3 前已存在），
  是有意降级还是缺陷未定：改动即行为变更，architect 不越权；待操作者裁决后由 coder
  落实（若认定缺陷）或 QA 补语义测试（若认定降级）。

## 二、当前产出情况（architect → 交付）

### 本会话产物清单

- `project/ezr/src/property_tests.rs`：
  - 模块 doc 单测基线数字引用修正（过期「221」→ 引用 handoff 对账口径；doc-only）；
  - **新增属性测试 12 条**（总量 12 → 24），覆盖此前未及的纯逻辑面：
    `checksum` validate_value 完全特征化 + parse_cli_x 往返 + algo_index_by_name 规范查找；
    `config::parse_speed` "/s" 后缀不变性 + 单位乘数十进制口径 + 大小写/空白归一 + 非法归零
    + 单调性；`config::from_toml` 输出域不变量（块大小/槽位恒正、并发钳 [1,64]、重试 ≥1、
    0 回退）；`slots` allocate 幂等封顶 + 列表序 + enforce 幂等与状态语义；`timefmt`
    分钟取整不变 + 定宽形状 + 字典序单调；`consistency` 判据特征化（等同 Valid、
    MissingStamp、URL/大小差异必失效、确定性）；`sidecar` JSON 往返全字段保真 +
    stamp 投影；`registry` from_tasks→JSON→load 保真 + next_id 不变量 + 快照投影往返。
- `project/ezr/scripts/arch_check.sh`：规则 6 → 8 条——
  - **规则 7【适配器边界】**：sentinel 终端适配器仅入口 main.rs 可引用，业务层
    （app/ui/engine/model）零依赖（v1.3 新增面 sentinel.rs 的边界规则化）；
  - **规则 8【环境变量收口】**：`std::env::var(_os)` 读取收口于适配缝
    （model/config.rs、sentinel.rs、入口），业务/机制层直读即违规；
  - 两条均经阳性对照实测（注入违规 → 非零退出 → 移除，对照记录在脚本尾注）。
- `packs/_common/engineering.md`：+1 条（GitHub API 限流时用 `releases/expanded_assets/<tag>`
  页读取真实资产清单，资产文件名可能不含版本号）。
- `packs/_common/notes/rust.md`：+2 条（proptest 多参数长度耦合与测试体内不可采样；
  f64→整数 `as` 饱和语义与「非法输入归零」属性的字母表排除纪律）。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | 258P / 0F（217+30+11） | `cargo test` |
| 属性测试（独立计数） | 24P / 0F（+12 本轮） | `cargo test property_tests::` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| 格式化 | 通过 | `cargo fmt --check` |
| 架构边界 | 8/8 规则通过（本轮 6 → 8，新增两条阳性对照生效） | `bash scripts/arch_check.sh` |

### 对账口径

- 上游口径：全量 246 = 205+30+11，其中主二进制 205 含 12 条属性测试（193 单测 + 12 属性），
  属性测试独立计数不入单测对账（承前纪律）。
- 本轮：**非属性单测 193+30+11 = 234 不变**（产品代码零改动，仅 doc 与测试基建）；
  属性测试 12 → 24（+12）；全量 258 = 217+30+11，可由 `cargo test | grep '^test result'`
  复算自洽。
- 架构改造面：仅 property_tests.rs doc 与 arch_check.sh（测试基建），产品源码零改动；
  行为不变裁决成立（全量测试前后同绿、数字口径如上）。

### 本轮评审结论与裁决（供 hardender/QA 使用）

- **四阶段评审全过**：①UI/核心分离——model 纯逻辑零 IO、engine 事件面纯数据、sentinel
  核心决策（`sentinel_core` 泛型 R/W + restore 闭包）与 IO 外壳分离可单测、config 的 env
  读取属薄适配缝；②依赖规则——单向分层维持、Connection 构造点均合规、无导入环；③信息
  隐藏——句柄私有字段 + 幂等 wake 契约、EngineHandle 窄接口、边界 DTO 带文档；④局部质量
  ——parse_speed 表驱动改造与原 else-if 链逐项等价（顺序 mb→kb→b、乘数一致、既有测试钉住）。
- **CRAP>6 拆分裁决（54 项中的四个命名函数，均不拆）**：`app/dialog_keys.rs add_dialog_char`
  与 `app/mouse.rs on_mouse` 为绑定 App 共享可变状态的交互分派核心（拆分 = 参数列表膨胀 +
  状态跨模块漂移，SKILL 不安全类）；`ui/dialog.rs draw_add_dialog` 为 ratatui 渲染函数；
  `bin/ezr-fixture emit_throttle` 属 QA fixture 基建不在产品门禁面。留 hardender 变异测试
  暴露具体缺陷后针对性处置；每文件变异点数既往扫描均 ≤100，无强制拆分前提。

### 待办与待批（未决项）

1. QA-TD-15 端到端 runner + 9 套整体回归 + 场景 10/11 runner（QA 节点，承前）。
2. hardender 期：变异加固全量执行（工具本轮已在位）；早期批次 CRAP>6 的 54 项以变异
   结果为准复核（本轮拆分裁决见上）。
3. **待操作者裁决**：单实例锁无 home 回退分支的 flock 缺失（遗留受限项末条）——有意
   降级或缺陷？裁决后由 coder（修）或 QA（钉语义）落实。
4. 无其他待批事项。

### 移交建议

- 运行 `bin/swarm complete` 打包归档（先 `git add -A`，packaging.md 口径；target/ 由
  complete 自动清理）。
- 后续如继续演进，按 six-pack 角色链顺序建议：**six-pack/hardender**（变异加固：工具在位、
  拆分裁决已给、`--file` 可按文件分块跑）→ **six-pack/QA**（补 QA-TD-15 与场景 10/11
  runner，端到端收口 v1.3）。
