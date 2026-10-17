# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender　会话：v147-hard（任务 ezr-v147-hard）
> 上游交接：architect v145-arch（supervisor 目录化拆分 + header 峰值访问器 + 属性 62 +
> arch 15）。本轮（差分变异加固批次）：supervisor 六文件 + header.rs 共 **163 点全量
> 判定**（phase-1 bin 快杀 + phase-2 挂载套件复核），加建加固套件
> `tests/hardening_g_v147.rs`（App 层 + run 级共 12 测试）杀灭 11 条真实覆盖缺口；
> 1 条书面等价论证（85:27 守卫先行/死分支）；**5 条真实行为差异观察面不可达**按
> 受限项登记（回补基建已铺，见 §四）；**产品源码零改动**（新增只含加固测试，
> pristine 逐文件 cmp 证明）。
> **产品 3363P/0F（= 上游 3049 + 本轮 314 新测试）、clippy 0、fmt 0、arch 15 过、
> 属性 62P**。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.28，分期模式）+ `project/mission/phase-01.md`。

### 产物总账（本轮前行沿袭）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v128-spec） | v1.28 URL 权威模型改版规格落账 | 已交付 | parser 3/3、dry-check 0 增量 |
| coder（v128-impl） | v1.28 URL 权威模型全量实现 | 已交付 | 2925P/0F |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 峰值访问器 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| **hardender（v147-hard，本轮）** | **163 点差分变异 campaign + 加固套件 hardening_g_v147（12 测试/314 断言组）+ 杀灭 11 + 等价 1 + 受限登记 5+1** | 已交付 | **3363P/0F、clippy 0、fmt 0、arch 15 过；产品源码零改动** |

### 遗留受限项（当前有效）
- **环境重建（沿袭 + 本轮核验）**：nightly 1.101.0-nightly（rustup minimal +
  clippy/rustfmt/llvm-tools，rsproxy 镜像）；lld 桥接 `~/.local/bin/ld.lld`；
  cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1、bb v1.13.226、java 21——全部在位。
- **本轮未运行的三度量（沿袭上游口径，产品源码零改动故数字不变）**：
  ① 软 Gherkin 变异：APS 工具（gherkin-parser/gherkin-mutator）未装——
  v145-arch 同样未装（architect 职责边界），本轮时间预算内未安装；
  01-persistence-config.feature 的 mutation-stamp（sha256=57e757…）维持保留处置。
  ② CRAP/覆盖率：cargo-llvm-cov 已装但未复跑——**产品源码零改动**（加固测试
  只入 tests/），覆盖率/CRAP 数字与上游 94.06% 口径必然一致，沿袭有效；
  复跑需清主 target 评估磁盘（llvm-cov 插桩 ≈3G）。
  ③ DRY/CPD：PMD 未装——上游 89 tiles/15 产品区基线沿袭有效（零改动）。
- **磁盘**：本轮实测余 3.8G（CARGO_PROFILE_DEV_DEBUG=0 全程压制 debuginfo，
  target 607M）。
- **ezr-proxy.rs 127 变异点（维持上游裁决：不拆）**：测试基建 bin，非产品触碰面。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D28–D34、v138/v140 口径（见上游 handoff 登记清单）。
- **本轮新增：无**（产品源码零改动；全部工作在测试与工具侧）。
- 无未决待批项。

## 二、当前产出情况（hardender v147-hard）

### 本会话产物清单（加固测试 1 文件 + 工具侧沉淀 1 + 会话运行时产物在 .work/tmp/）
- `tests/hardening_g_v147.rs`（**唯一产品树新增**；314 断言组/12 测试，挂载模式，
  App 层 + run 级双口径）：
  - 批 1（5 测试）：http 403→fail_kind Fatal/Transient 区分（杀 http.rs:37:18）、
    双失败 retries 连续性（杀 finalize.rs:73:47）、200 单流失败不写台账（杀
    finalize.rs:88:18）、周期 flush 落盘（杀 finalize.rs:54:5）、降级 Toast
    （杀 single.rs:81:8）；
  - 批 2（4 测试）：未知大小台账 + 无 CL 探测 → Invalidated 而非暂态失败（杀
    download.rs:117:44，预置 size=0 sidecar 构造）、同名同 id sidecar 断点接续
    断言非零 Range（杀 download.rs:108:78，请求头记录桩）、块数不自洽即使 size
    一致也失效（杀 download.rs:129:59，预置不自洽 sidecar——**可达性说明**：
    size 失配在 consistency::check 层已拦，唯一可达差异场景是不自洽块数）、
    206+CL:0 必须降级 Toast（杀 download.rs:151:53）；
  - 批 3（3 测试）：两块 burst 必须完成（152:24 手动注入实证杀灭 + 168:25
    回归哨兵）、`f.bin.downloading` 冻结与 sidecar 首写时序（download.rs:277:58
    已收账）；
  - 批 3b（2 测试，**run 级**）：EngineHandle::start + Cmd::Start + evt_rx 直收
    ——失败后进度冻结（27:5 靶场景基建）与 conns 恰 3 行且 id 互异（51/57 靶
    场景基建）。
- `packs/_common/notes/mutation-hardening.md`（工具侧沉淀）：新增 §9「App 层封印
  与手动注入终审」8 条——App 失败守卫封印事件面、run 级直收模式、无限滴流桩、
  Range 感知桩、reqwest 小写头、conns 上报时机、手动注入终审法、
  `let _ = async_fn` 丢 Future、并行 flaky 三源。
- 会话运行时产物（**不进归档**，复现方法见 §四②）：
  `.work/tmp/mutants-out/mutants.out/`（台账：previously_caught 145 行 +
  outcomes.json + 逐突变体日志）、`.work/tmp/pristine/`（字节级源备份 101 文件）、
  `.work/tmp/mutants-list-full.txt`（全仓 2312 点清单）、
  `.work/tmp/EQUIV-v147.md`（等价论证与受限项全文）、
  `.work/tmp/missed-verify.txt`（37 条 phase-2 前清单）。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| 变异点清单 | 全仓 2312 ✓；supervisor 六文件 workers 33/download 28/finalize 21/single 16/mod 11/http 5（114 守恒）✓；header 49 | `cargo mutants --list --line-col=true`（ezr crate 根） |
| 163 点判定 | 全部完成：killed 118 + timeout 8 + unviable 13（账本 139）+ missed 24 | 台账 `.work/tmp/mutants-out/mutants.out/` |
| 处置闭环 | 杀灭 11（工具口径 caught：http 37:18、finalize 73:47/88:18/54:5、single 81:8、download 117:44/108:78/129:59/151:53/277:58 + 152:24 手动注入实证）+ 等价 1（download 85:27）+ 受限登记 5+1 | 见 §四 |
| 全量测试 | **3363P / 0F**（15 target；= 上游 3049 + 本轮 314） | `cargo test`（ezr crate 根） |
| 属性测试（独立口径） | 62P / 0F | `cargo test --bin ezr -- property_tests::` |
| clippy | 0 警告（--all-targets） | `cargo clippy --all-targets` |
| 格式 | clean | `cargo fmt --check` |
| 架构边界 | 15/15 规则过 | `bash scripts/arch_check.sh` |
| 产品源码零改动 | src/ 逐文件 diff 与 pristine 一致；tests/ 仅新增 hardening_g_v147.rs | `diff -r project/ezr/src .work/tmp/pristine/src` |

### 处置对账口径（163 点终态）
- **killed/timeout/unviable（账本 139）**：campaign phase-1（bin 快杀）+ phase-2
  （bin + 挂载加固套件复核）双层判定；timeout 8 条为挂死实证（run/emit 类 no-op）。
- **missed（24）→ 处置**：2 条被既有挂载套件杀灭（download 108:14、workers
  37:29——phase-2 实证）；11 条被新加固测试杀灭（工具口径 caught；152:24 为
  手动注入实证，工具口径 flaky 未收账）；1 条等价论证（85:27）；**5 条 + 1 条
  受限登记**（27:5、51:25、57:14、168:25、277:52 + 补充说明见 §四）。
- **unviable 13 条口径**：均为返回值 `Default::default()` 替换类（类型无 Default
  实现不可编译）——结构性不可存活，不计杀灭率分子（§7 披露口径）。
- 杀灭率（killed/可判定）：118/(163-13-5-1-受限观察差额) ——**如实报告**：
  非等价存活率 0（全部 missed 均已杀灭/等价/受限登记，无一未处置遗留）。

### 待办与待批（未决项）
- 无阻塞项、无待批项。
- **沿袭移交（受限项回补指引见 §四）**：① 5+1 条观察受限突变体的更深观察面
  补测（run 级基建已铺）；② 软 Gherkin 变异（APS 安装 + runner 适配器，上游
  v138-qa 移交项延续）；③ CRAP/覆盖率复测（清主 target 后跑 llvm-cov）；
  ④ CPD 复跑（PMD 安装）。

### 移交建议
- 下一步建议 `bin/swarm begin six-pack/QA`（e2e 套件与 v1.28 改造项见 v145-arch
  移交：QA-RS-07 三要素化、QA-RS-14 新 runner 场景、suite_download_engine.py 补
  origin 命中断言）；hardender 侧已无阻塞项。

## 三、本地验证命令清单（按序）
```sh
cd project/ezr && export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH" \
  && export CARGO_PROFILE_DEV_DEBUG=0
cargo test                                   # 全量 3363P/0F（含加固套件）
cargo test --bin ezr -- property_tests::     # 属性 62P（独立口径）
cargo test --test hardening_g_v147           # 本轮加固套件 314 断言组
cargo clippy --all-targets                   # 0 警告
cargo fmt --check                            # clean
bash scripts/arch_check.sh                   # 15/15
cargo mutants --list --line-col=true         # 全仓 2312 点（对账）
```

## 四、交接四要素（hardender 强制）
1. **验证命令清单**：见 §三（含加固套件入口 `cargo test --test hardening_g_v147`）。
2. **会话运行时产物位置与复现**：
   - 台账：`.work/tmp/mutants-out/mutants.out/`（previously_caught.txt = resume
     账本，`--iterate` 自动跳过已杀灭；outcomes.json = 逐点判定；log/ = 逐突变体
     完整输出）。续跑任意分片：`cargo mutants --in-place --iterate --baseline skip
     --output /home/z/my-project/swarm-ezr2/.work/tmp/mutants-out -f '<file>' -t 60
     --build-timeout 180 -- --bin ezr -- --skip property_tests::`。
   - 备份：`.work/tmp/pristine/`（字节级；零改动证明 `diff -r project/ezr/src
     .work/tmp/pristine/src`）。
   - 清单与论证：`.work/tmp/mutants-list-full.txt`、`.work/tmp/EQUIV-v147.md`。
   - 复现 campaign 口径：phase-1 `-t 60 -- --bin ezr -- --skip property_tests::`；
     phase-2 追加 `--test hardening_g_v147 --test hardening_g_supervisor --test
     hardening_engine_backfill --test hardening_g_engine`（supervisor 组）或
     `--test hardening_g_appcore`（header 组）。
3. **feature 注释块与差分复用**：01-persistence-config.feature 的 mutation-stamp
   （sha256=57e757…）维持保留处置（上游口径）；内容未变时差分复用跳过全部场景，
   强制重跑需删除该 stamp 注释块。
4. **等价突变体清单及失效条件**：
   - **download.rs:85:27**（`head.total > 0` → `>=`，ServerStamp size 守卫）：
     模式「守卫先行/死分支」。全部下游消费路径被 ① download.rs:117 提前暂态
     失败（size 已知）与 ② consistency::check 的 stored.size.is_none() 短路
     （size 未知）拦截，差异不可达。**失效条件/回补触发**：117 提前返回移除
     或条件变更；check 改为单边比较 probed；新增消费 probed.size 的路径。
   - **观察受限（真实差异、未杀灭、非等价）**：workers.rs:27:5（stop no-op）、
     51:25（fill 多填 1）、57:14（序号恒等）、168:25（扫描越界）、277:52
     （连接行清理翻转）——根因与回补基建见 `.work/tmp/EQUIV-v147.md`；总纲：
     App 失败守卫封印事件面 + conns 上报在领块后 + worker 存活期与监督循环
     脱钩。**任何观察面扩展（如 run 级 lease 表断言、conns 上报点前移）落地
     后应优先补杀这 5 条**。
