# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（本轮已完成）→ 建议下一节点 hardender
> 会话：ezr-architect-round（2026-11-06，architect 轮：四阶段架构评审 + 保持行为架构
> 改进 2 项 + 属性测试 +3 条 + arch_check 白名单收口与阳性对照复验）
> 上游交接：project/handoff.md（cleaner 轮交接 = bug25 UI 修正轮保持行为清理 +
> ezr-demo clippy 基线清零 + 925P/0F 基线）
> 环境注记：本轮运行于全新沙箱（工具链零起点重建：rustup nightly 1.101.0 前台装齐，
> 镜像口径见 notes/environment.md architect 轮），工作区已迁移 /home/z/swarm-ezr2。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器。需求权威 `project/mission.md`
  （分期模式 v1.42）+ `project/mission/phase-01…04.md`。
- 本轮为 **architect 架构轮**：四阶段架构评审（UI/核心分离、依赖规则、信息隐藏、局部
  代码质量）+ 保持行为架构改进 + 属性测试支持 + 自动化架构检查复核。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier/coder/cleaner（历轮） | bug25 三项规格 + 实现 + demo 同构 + 保持行为清理 + 覆盖率/CRAP/DRY/变异点度量 | 已交付 | 沿袭全绿（cleaner 轮 925P/0F） |
| **architect（本轮）① 四阶段评审** | 评审入口 = scripts/arch_check.sh（前次 architect 轮交付物，16 规则）；①UI/核心分离全过（规则 11/12/13/16）；②依赖规则 **2 组违规**（规则 8 env 直读 + 规则 14 白名单漂移）；③信息隐藏全过（规则 6/7/9/15，bt 门面 pub 面合理）；④局部代码质量无新增（cleaner 轮已收敛） | 已交付 | 评审报告见本轮 handoff 第二节；arch_check 基线 14 OK+2 FAIL |
| **architect（本轮）② 架构改进 2 项** | ①`app/cmd.rs` HOME 直读收口 `crate::model::config::home_dir()`（展开算法逐字保留、"/" 回退保留；home_dir 的 USERPROFILE 回退随收口层语义，Linux 行为逐字等价）；②`scripts/arch_check.sh` 规则 14 白名单扩展（cmd_sb/cmd_cursor_screen/trackers_scroll/trackers_sb/dlg_bt_sb/dlg_trackers_sb/dlg_trackers_h_sb 共 4 组 12 行存量回填收口 + 注释登记判例与消费点证据） | 已交付 | clippy 0 + fmt 净 + 全量测试全绿；行为面零变化（改动仅环境源替换与检查脚本白名单） |
| **architect（本轮）③ 属性测试 +3** | `src/property_tests.rs`：prop_bencode_round_trip（任意 BValue 树编码→解析恒等 + 再编码逐字节同形）、prop_bencode_decode_contract（decode 消费整段 + 尾部垃圾拒绝）、prop_piece_map_conservation（覆盖集有序唯一/片长界/写盘计划结构界/全局字节守恒/pieces_of_file span 公式/选中文件交叉一致） | 已交付 | 独立命令 74P/0F（71 基线 + 3）；属性迭代 2 轮揭穿**测试侧**假不变量（实现零改动），实录见 notes/testing.md |
| **architect（本轮）④ arch_check 复核** | 16 规则全绿 + 阳性对照三连（规则 8 注入 env 读取→红、规则 14 注入 app.selected 写→红、还原→16 OK 全绿） | 已交付 | EXIT 0；检查有效性确认（零发现结论经阳性对照采信） |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（历轮登记，全部不变）：死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；
  QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态观察项；
  leech 判据无 e2e 自动化；bad_piece 归属口径（D67 允许）；QA-TD-23 与
  QA-UC-07/17/18 环境预存 FAIL 待 QA 轮复验；`periodic_flush_writes_sidecar_mid_download`
  HTTP 滴流 flake 单跑绿沿例；同 IP 拓扑残余启动段；leech 封禁按裸 IP 连坐面；
  ezr 不发 KeepAlive。
- 沿袭（cleaner 轮登记，全部不变）：变异实跑（cargo-mutants 跑模式）属 QA/mutants 轮
  预算（scan 计数已登记：ezr 全库 5710 / bug25 触碰 7 文件 437）；CRAP 登记口径
  dlg_finish_add / dlg_confirm_add_bt comp≈14 拆分需重构轮授权；g_v147 quota flake
  间歇态沿例（本轮全量跑绿）。
- **本轮新增受限项（架构观察，移交 hardender）**：engine↔bt 模块级双向类型引用
  （bt/engine.rs use crate::engine ↔ engine/mod.rs/supervisor use crate::bt）为既定
  协调枢纽模式——EngineShared/BtTaskCmd/Evt/BtTunables/BtConnView/TorrentMeta 契约
  类型，Rust 可见性已强制窄接口（engine 实现子模块 pub(super) 对外不可见）；拆分属
  「依赖共享可变状态的长分派拆分」不安全类，登记移交：hardender 全量变异暴露具体
  高复杂度函数后再针对性拆，本轮不强行做。
- 本轮改动面零新增变异点：cmd.rs 改动为环境源替换（±0 变异点），property_tests.rs
  为测试代码（不进产品变异面）。

### 实现定义值登记（累计口径，本轮无新增）
- 本轮未新增任何实现定义值；既有登记值全部不变（面板层速度格式化/判重目录解析/
  并发列宽/客户端收缩基线/重复任务错误窗口等，见 cleaner 轮交接同节）。
- arch_check.sh 规则 14 白名单构成变化（回填白名单 +4 组字段）属检查器判例登记，
  非运行时行为值，登记于脚本注释（含消费点证据与本轮阳性对照实录）。

## 二、当前产出情况

### 本会话产物清单
- **ezr 产品端（2 文件）**：`src/app/cmd.rs`（HOME 读取收口 config::home_dir，
  ~ 展开算法逐字保留）、`src/property_tests.rs`（+3 属性测试 +2 生成器
  +3 行导入）。
- **ezr 检查器（1 文件）**：`scripts/arch_check.sh`（规则 14 白名单扩展 + 判例
  注释登记；16 规则全绿）。
- **project/notes/**：architecture.md / testing.md / environment.md 各追加 architect
  轮条目 + INDEX.md 三行同步（白名单漂移修复模式 / proptest 内联捕获陷阱与属性
  归因法 / 全新沙箱工具链重建实录）。
- **.work/logs/**：基线复核 / archcheck 三态 / 阳性对照 / 属性测试三轮 / 全量验证
  日志 20 份（01_fetch…20_demo_test）。
- **ezr-demo**：本轮零触碰（demo 44P/0F + clippy 0 沿袭复核通过）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr bins 单元测试 | **928P/0F**（925 基线 +3 属性，精确对账） | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bins` |
| ezr 集成测试（12 组） | **全绿 6201P**（142+828+726+721+2+16+202+695+451+709+858+851） | `CARGO_INCREMENTAL=0 cargo test --test <套件名>` 逐组 |
| ezr 属性测试（独立命令） | **74P/0F**（71+3，与 bins 计数口径独立对账） | `cargo test property_tests::` |
| ezr clippy / fmt | **0 警告 / 干净** | `cargo clippy --all-targets` / `cargo fmt --check` |
| arch_check.sh | **16 规则全绿 EXIT 0**（基线 14 OK+2 FAIL → 改造后全绿） | `bash scripts/arch_check.sh` |
| 阳性对照（规则 8/14） | 注入即红 / 还原即绿（3 连实测） | 注入 `std::env::var("X")` 于 engine/mod.rs、`app.selected = 1;` 于 ui/list.rs |
| ezr-demo 测试 / clippy | **44P/0F / 0 警告**（沿袭复核） | `cd ezr-demo && cargo test` / `cargo clippy --all-targets` |

### 对账口径
- bins 计数链：925（cleaner 基线）→ +3（本轮属性测试）= **928**；其他 bin
  （mktorrent 30 / proxy 1 / fixture 15）不变。
- 集成套件 12 组与 cleaner 轮全绿口径一致；**g_v147 实测 709P**（cleaner 轮交接记
  708——按 engineering「以实测为准」登记偏差：708 疑为 flake-FAIL 运行行
  （708P/1F，总 709）转写，本轮全量绿 709P/0F 为该套件满编数）。
- cmd.rs 收口改动为环境源 1:1 替换：HOME 存在时逐字等价；HOME 缺失时旧
  `env::var` Err → "/" 回退，新 `home_dir()` None → "/" 回退等价（差异仅在
  HOME 缺失且 USERPROFILE 存在的非典型环境，随收口层语义，Linux 测试面零影响）。
- 属性测试 2 轮失败均为**测试侧断言过界**（proptest shrink 最小输入指认）：
  ①逐 piece 写盘守恒 → 真不变量为全局守恒（未选文件贡献 piece 网格但无计划条目、
  边界 piece 部分相交）；②「未选文件零区间」→ pieces_of_file 按 size>0 返回 span
  与 selected 无关（仅 size==0 返回 (0,0)，该函数为 #[allow(dead_code)] 预留 API）。
  实现零改动，契约以实现源码为准。
- 白名单扩展逐字段核实消费点：cmd_sb/trackers_sb/dlg_*_sb → app/scrollbar.rs
  SbKey 映射 + app/mouse.rs 命中测试；cmd_cursor_screen → ui/mod.rs:226 光标绘制
  消费；trackers_scroll → app 键盘/滚动写（popups.rs trackers_scroll_by、
  scrollbar.rs 拖拽置位）+ ui 渲染期钳制——全部落入脚本既有判例形态。

### 待办与待批
- 无待批。移交待办（沿袭）：①变异实跑属 QA/mutants 轮预算；②dlg_finish_add/
  dlg_confirm_add_bt 拆分（CRAP 登记口径，需重构轮授权）；③QA 轮复验
  QA-TD-23 与 QA-UC-07/17/18 环境预存 FAIL 面、场景 R PTY 复验。
- 本轮新增移交：④engine↔bt 协调枢纽拆分裁决移交 hardender（变异暴露后针对性拆，
  见遗留受限项）。
- 操作者修订建议（工具侧 packs/，按宪法操作者维护约定不自行改动）：
  `packs/_common/notes/rust.md` proptest 条款可补一句「内联捕获失败同样适用于
  prop_assert!（concat 展开为格式串，一律显式位置参数）」——完整实录已入
  project/notes/testing.md。
- 无阻塞。

### 移交建议
- 下一步：`six-pack/hardender`（SKILL 建议顺序；变异预算 + CRAP 拆分裁决 + 本轮
  engine↔bt 观察项在其预算内兑现）。
- 工具链交接：本沙箱全新安装 rustup 1.29.3 + nightly 1.101.0（2026-11-02，
  PATH=~/.cargo/bin，minimal profile + rustfmt + clippy 组件）；cargo registry
  rsproxy.cn sparse（~/.cargo/config.toml）；rustup 镜像 TUNA（清单级实测 7MB/s
  最快，安装实录见 notes/environment.md architect 轮）；无 lld 桥接需求（项目无
  .cargo/config.toml 链接器配置）。swarm 工作区位置：/home/z/swarm-ezr2（自 /tmp
  迁移，会话无损）。
