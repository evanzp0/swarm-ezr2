# Rust 专项注意事项

> 归属规则：跨语言通则见 `packs/_common/engineering.md`；本文件只收 Rust 语言、工具链
> 与生态专项的问题与解法（会话复盘沉淀，操作者维护）。含 Rust 的角色开工前先读本文件。
> 吸收合并原 `rust-lessons.md` 与 `rust-constraint.md`（配置模板为强制约束）。

## 1. 工具链与环境

- **PATH 注入不跨会话持久**：会话恢复/新 shell 中 `cargo` 会 not found（工具链本体
  完好，丢的是注入）。任何 cargo 命令前先 `command -v cargo` 探测；失败则
  `export PATH="$HOME/.cargo/bin:$PATH"` 补注入，再 `cargo --version` 复核在位。
  若项目构建同时依赖本地符号链接目录（见下条 ld.lld 桥接），PATH 还需含
  `~/.local/bin`。工具链的安装位置与版本写入 `project/handoff.md` 交接。
- **nightly 工具链漂移会在新会话引入新 lint 警告**：上轮 clippy 0 的代码，本轮
  nightly 可能新增 lint（如 `bool_assert_comparison` 收紧到 `clippy::all`）——
  接手会话开工先跑一次 `cargo clippy --all-targets` 复核 0 警告基线再动工；
  警告属测试断言写法类时机械修正（如 `assert_eq!(x, false)` → `assert!(!x)`）
  属基线恢复，不计行为改动面，但改动要计入本轮产物清单。
- **区分「丢注入」与「本体丢失」**：环境核验先探针安装目录（如 `ls ~/.cargo/bin`）——
  目录在位是丢 PATH 注入（export 即恢复）；目录不存在是全新沙箱/本体丢失，需完整重装
  （rustup minimal profile + clippy + rustfmt），交接记录的安装位置只对同机有效。
- **无 lld 环境的零改动桥接**：rustfmt/clippy 约束模板配套的 `-fuse-ld=lld` 在无 lld
  且无 root 装包权限的环境会挂掉一切链接。解法：用工具链自带 rust-lld 做符号链接——
  `ln -sf ~/.rustup/toolchains/<tc>/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld
  ~/.local/bin/ld.lld`（gcc 按 PATH 找 `ld.lld`），不改基线配置；该符号链接目录必须
  在 PATH 中，且零改动的增量构建会静默跳过链接阶段掩盖其缺失，验证见
  engineering.md「环境不跨会话持久」的强制重做条款。
- **交互式安装器的首次运行提示是长验证挂死的隐形根因**：`cargo llvm-cov` 首跑会交互
  询问是否安装 `llvm-tools-preview`，非交互会话表现为"600s 超时零产出"。装工具阶段先
  `rustup component add llvm-tools-preview`，或统一 `< /dev/null` 暴露错误
  （engineering.md「失败先归因」的挂起类形态）。
- **`rust-toolchain.toml` 钉 `channel = "nightly"` 时无法用日期版工具链做别名桥接**：
  `rustup toolchain link nightly <已装目录>` 被拒绝（`nightly` 是保留通道名），装好
  `nightly-<日期>` 后项目内 cargo 命令仍会按 toolchain 文件自动安装 floating
  `nightly`（最新版）。收敛工具链漂移时直接以「当前沙箱解析到的 nightly」为收敛目标
  逐项归零警告/偏差并记录确切版本（`rustc --version`），不在别名桥接上花功夫。

## 2. 强制配置模板

Rust 项目实现时，**必须**在项目创建时同步生成三个配置文件并逐字套用以下模板
（工作流的开发和测试中都使用），不留到事后补：

- `Cargo.toml`：`[lints.rust]` / `[lints.clippy]`（workspace 项目定义于
  `[workspace.lints]`，各成员经 `[lints] workspace = true` 继承）
- `clippy.toml`
- `rustfmt.toml`

```toml
# Cargo.toml
[lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
too_many_lines = "warn"
cognitive_complexity = "warn"
too_many_arguments = "warn"
cargo = { level = "warn", priority = -1 }
```

```toml
# clippy.toml
cognitive-complexity-threshold = 4
too-many-arguments-threshold = 4
type-complexity-threshold = 200
single-char-binding-names-threshold = 4
enum-variant-size-threshold = 200
literal-representation-threshold = 16384
```

```toml
# rustfmt.toml
unstable_features = true
max_width = 100
edition = "2024"
fn_single_line = false
imports_granularity = "Module"
group_imports = "StdExternalCrate"
```

- **rustfmt unstable 选项必须配 nightly 工具链钉**（`imports_granularity` /
  `group_imports` / `fn_single_line` 仅 nightly 全量生效，stable 下警告提示但忽略）：
  项目根放 `rust-toolchain.toml`（`channel = "nightly"` +
  `components = ["rustfmt", "clippy"]`）使全项目 cargo 命令统一走 nightly，下游角色
  无需手工 `+nightly`；rustfmt/clippy 模板文件仍逐字保留。切换后必须整树回归：
  nightly rustc/clippy 可能引入新 lint（如 nightly 默认 `cargo::unused_dependencies =
  warn` 会揪出死依赖），按警告清零纪律处置；nightly 为移动目标，行为差异以测试
  全绿为准。

## 3. 编译、lint 与磁盘

- **`#[path]` 收编产品源的测试 crate 必须镜像产品 bin 的 crate 级 lint 姿态**：产品入口
  （main.rs）的 crate 级 allow（pedantic/nursery/cast 系等）不随 `#[path]` 模块收编进入
  测试 crate，同一份产品源在测试编译下会重燃产品面本已为 0 的警告；挂载机制还固有产生
  三类告警：`dead_code`（部分收编的产物项在该测试 crate 无消费者）、`duplicate_mod`
  （挂载壳与直挂同文件）、`unused_imports`（产品跨模块 re-export 在部分挂载下无消费者）。
  三分法处置：① 机制固有告警在测试 crate 根部 allow 并注明依据（产品源零改动）；
  ② 测试文件自身写法类警告机械修正；③ 测试文件真实死代码以编译器警告为证据删除。
  盲目 `clippy --fix` 或放任告警都会破坏「产品面 0 + 测试面 0」双口径。

- **生成代码要自带 lint allow，否则警告噪音淹没真实信号**：生成的测试/代码若函数名
  不符 `snake_case` 等约定，会一次产生大量 lint 警告。入口生成器必须在生成文件头部加
  `#![allow(non_snake_case)]`（或按需精确 allow）——排查失败时警告噪音是实打实的成本。
- **中断的重跑靠增量编译低成本恢复**：cargo 的 target 缓存使"测试跑到一半被取消"后
  重跑只重编改动部分；配合 engineering.md「预告 + tee 落盘日志 + 增量重跑」使用，
  长测试被交互消息打断的损失可控制在分钟级。
- **构建目录膨胀会写满磁盘并挂死构建**：长会话多轮增量构建后 `target/` 可膨胀至数 GB。
  构建前查 `df -h /`（余量 <1.5G 先 `cargo clean`，确认无在跑构建）；持续带
  `CARGO_PROFILE_DEV_DEBUG=0` 压制 debuginfo；长会话内每完成一个重构建阶段即复查一次。
  注意：删除 `target/` 后孤儿进程可能仍存活（运行中 inode 未释放），按「先杀进程、
  再删状态」次序处置（engineering.md「测试执行纪律」）。

## 4. 编码实践

- **`let _ = x;` 是通配符丢弃、不移动值**：`_` 模式不绑定所有权，`let _ = s;`（String）后
  `s` 仍可用。它贴在"下方实际在用"的变量上时是无操作死语句（多为历史未用抑制器残留），
  直接删除即可——不要因 E0382 直觉误判而不敢删；真正的未用抑制要用 `_var` 或改签名。
- **bencode/二进制协议手工编码一律走 builder 辅助，不手拼长度前缀**：手写字节串长度
  前缀极易出错，且报错位置远离笔误点。统一用"编码辅助函数 + 解析 round-trip 断言"
  （编码→解析→比对原文），笔误当场现形。

## 5. 平台与 IPC

- **`tokio::net::UnixListener`/`UnixStream` 仅 `cfg(unix)`，Windows 上编不过**：
  Windows 10 1803+ 内核虽有原生 AF_UNIX，但 tokio 未在 Windows 暴露该 API。跨平台
  本机 IPC 的 Rust 现实选型：Unix 用 UDS、Windows 用
  `tokio::net::windows::named_pipe`，或用 `interprocess` crate 抹平。
- **"本机 IPC"各平台惯用机制不同**：Linux/macOS 用 UDS；Windows 用命名管道；Android
  跨 App 走 Binder/AIDL（UDS 受沙盒 UID 隔离）；iOS 不允许常驻 daemon，惯用做法是把
  引擎做成 lib（`src/lib.rs`）经 FFI/JNI 嵌入。面向多平台的产品在规格期就要把平台
  矩阵摆上桌面（engineering.md「端点语义」通则）。
- **传输层伴生设计随选型整套变化，不是换一个 bind 调用**：UDS 方案通常包含"socket
  文件可连接 ⇒ 已有实例"的单实例探测与路径约定；改 TCP loopback 则单实例要换端口
  探测/锁文件、路径换平台惯例目录（用 `dirs` crate 而非手读 `HOME` 环境变量）。
  选型问题在规格期解决（engineering.md「端点语义」通则），实现期换选型 = 返工整套
  伴生机制。

## 6. 网络与异步

- **reqwest 关闭环境变量代理**：`ClientBuilder::proxy(p)` 或 `.no_proxy()` 任一调用都会
  关闭 `auto_sys_proxy`（不再读 `http_proxy`/`HTTPS_PROXY` 等环境变量）。需求为
  「代理仅经配置文件」时显式 `.proxy(cfg_proxy)` 即可，无需自行清洗环境变量。
- **`adler` crate（Adler-32）API 口径**：切片直算用 `adler::adler32_slice(data)`；
  流式累积用 `Adler32::new()` + `write_slice(&buf[..n])` + `checksum()`——
  该 crate 没有 `push_slice`/`adler32_by_chunk`，按记忆写 API 名会编译失败。
- **tokio 并发写同一文件的正确姿势**：每个 worker 独立 `OpenOptions::open` 打开
  同一路径 + `AsyncSeekExt::seek` 定位写入（不 truncate）。不要 clone `tokio::fs::File`
  句柄后各自 seek——std/tokio `File::try_clone` 共享游标，并发写会互相错位。
  预分配用先 `create(true)`（`truncate(false)` 保护续传场景）再 `set_len(total)` 稀疏分配。
- **令牌桶 acquire(n) 大于桶容量会死循环**： refill 被 `.min(capacity)` 钳制时，
  `n > capacity` 的请求永远凑不齐令牌。正确做法是分批取走（`remaining.min(tokens)`
  循环直到取足），而不是等单次凑满。
- **`Cargo.toml` 的 `[lints]` 表与 `publish = false`**：本地交付项目设 `publish = false`
  可豁免 clippy cargo 组的 metadata 类警告（license/keywords/categories）；
  依赖树固有的多版本重复用文件级 `#![allow(clippy::multiple_crate_versions)]` 豁免
  （入口 bin 顶部），不降低 `[lints]` 模板本身。
- **`cargo fix` 会误删测试专用导入**：cfg(test) 代码使用的符号在非 test 编译下视为
  未使用，`cargo fix` 跑完可能删掉 `use` 导致测试编译失败——跑完 fix 必须再跑一次
  `cargo test` 兜底（fix 的 diff 也要过目）。
- **rustc 1.87+ 的 `u64::is_multiple_of`** 可替换 `x % m == 0`（clippy manual_is_multiple_of
  会提示）；`checked_div` 用于除数可能为 0 的展示算术。

## 7. 测试与变异度量

- **cargo-mutants 的每文件变异点数与文件行数近似线性（约 0.2 点/行）**：千行级交互层
  文件单文件即可产出 300+ 变异点（scan 模式 `cargo mutants --list` 直接统计，无需跑变异；
  27.x 起 `--list` 输出 `文件:行:列: 变异描述` 文本清单，需附 `--line-col=true`，旧
  `--line-counts` 已移除，按行首文件名聚合即得每文件计数）。
  SKILL 的「>100 变异点做保持行为拆分」在模块边界上依赖 architect 裁决、验证依赖 e2e 套件
  （cleaner 不运行）时，不要强行拆——把每文件计数登记 handoff 移交，hardender 全量变异前
  由 architect 先收模块边界（`--file` 可按文件分块跑变异，拆分收益在 hardender 兑现）。

- **交互层覆盖率可以在单测内打开（TestBackend + 桩服务器），不必依赖 PTY e2e**：ratatui 的
  draw_* 系函数用 `ratatui::backend::TestBackend`（0.29 无 feature 门控）直接渲染应用全状态
  并断言缓冲文本；事件处理用「非法输入 → 事件泵至错误终态」与「桩服务器 → 事件泵至成功
  终态」两条真实事件流覆盖（事件泵循环带谓词提前收束，勿跑满 deadline）。注意 TestBackend
  的 `Display` 会把宽字符半格单元隐藏并附 `Hidden by multi-width symbols` 注记——断言锚定
  不被 CJK 截断的稳定文本（如 ASCII 名称、选中行标记），不要断言与 CJK 相邻的边框/标题。
- **输出管道会吞 `#[m` 形态字符**：日志采集/展示层按 CSI 转义清洗 `[m` 等序列时，
  源码中的 `#[must_use]` 会显示成 `#ust_use]`（可复现、可误导"文件损坏"判断）。
  审计源码一律以 `od -c`/`xxd` 字节为准，不信显示层。
- **双向拷贝代理（io::copy 两线程对拷）的单测必须显式关闭客户端写侧**：`shutdown(Write)`
  后代理的 client→upstream 拷贝得到 EOF、两个 copy 线程才能收尾、socket 才会 drop；
  否则 read_to_end 与 copy 线程互等，测试永久挂起（真实客户端通常天然关写侧，故线上不显）。
  桩上游的"读一次→回一包"契约要与被测流量方向对齐（CONNECT 隧道：客户端先发一笔
  再读回包），否则双向互等死锁。
- **拆大文件（>100 变异点/行）时先机械分模块再提纯函数，两步各自全绿**：先按职责把
  impl 块切到子模块（同 crate 多 impl 合法、子模块可见父私有项），用 `git diff --stat`
  与全量测试确认"纯移动"；再做表驱动等数据化的纯函数抽取。切分脚本
  对 doc 注释/属性边界的 off-by-one 高发——每刀后先 `cargo build` 再继续，孤儿 doc 与
  双重 impl 头都是必踩点。
- **`cargo llvm-cov --lcov` 的 FNDA 行只有两字段（命中数, 符号名），不含行号**：做函数级
  覆盖率/受限近似 CRAP 分析时，行号必须从对应 `FN:<行>,<符号>` 记录取，再按"函数起始行
  ≤ 数据行 ≤ 下一起始行"归属 DA 行；`--hide-instantiations` 只合并泛型实例化，嵌套闭包
  仍各自独立成 FN 记录且与外层函数共享行区间——按起始行归属时闭包的 DA 会同时计入
  外层函数与闭包，函数级 CRAP 口径要把闭包记录与外层视为同一逻辑函数（取极值）再计算，
  否则分派器类函数的复杂度被重复放大（engineering.md「符号级输出不可直接精确匹配」的
  lcov 维度延伸）。
- **`cargo llvm-cov --lcov` 默认不输出 BRDA（分支数据整条缺失）**：需要分支计数口径的
  受限近似（如 CRAP 的 comp 代入）必须加 `--branch` 重新生成 lcov；"零发现"前先对已知
  多分支低覆盖函数做阳性对照（BRDA>0 才算数据在位），否则全 crate 函数 comp 都会
  塌缩成 1、CRAP 全绿假象。
- **BRDA 分支计数做 comp 近似的两个偏差方向**：① `?` 操作符不产生 BRDA——早退密集的
  入口胶水（main/run_tui 类）comp 被低估、CRAP 假性达标，登记偏差方向留 hardender 以
  变异为准；② 测试代码 `assert!` 宏展开为每个断言生成分支记录——测试函数 comp 虚增，
  CRAP 门禁按 mangled 名含 `testss_` 排除测试模块（带前缀的 `cli_parse_tests` 等测试
  模块同样命中，勿只匹配 `mod tests`）。
- **PMD 7.x 的获取与调用**：Maven Central 上 `pmd-dist` 只有 maven jar（无 bin zip），
  bin 发行包在 GitHub releases（tag 形如 `pmd_releases/<版本>`，资产
  `pmd-dist-<版本>-bin.zip`）；GitHub API 限流时按已知 tag 拼直链下载，勿据 404 误判
  "未发布"。CPD 无独立启动器，用 `pmd cpd` 子命令（`-l rust` 必带，见 engineering.md
  「静默空输出」条）；输入用位置参数（7.28 无 `--files` 选项），发现重复时 exit 4 属
  正常语义（非调用失败）。
- **proptest 多参数的长度必须耦合**：一个属性同时生成「数量 n」与「逐项数据 vec」时，
  两个独立策略（`n in 0..N` + `vec(..., 0..N)`）长度各随机，测试体内按 `[i]` 索引必
  越界 panic（表现为属性随机红，缩小到 `n > vec.len()` 才现形）。正确形态：定长 vec
  （`vec(..., N)`）+ 测试体内切片 `[..n]`，或把数量并入 vec 后取 `len()`。同理，策略
  只能在参数位置声明，不能在测试体内按迭代采样（对 `select(...)` 逐次"抽一个"的
  `lift()` 形态不存在）——需要"N 个随机 X"就用 `collection::vec(select(&[..]), N)`
  作为参数生成。
- **`f64 → 整数` 的 `as` 转换是饱和语义，属性字母表要排除可被解析器吞掉的词**：
  `NaN as u64 = 0`、`inf as u64 = u64::MAX`（Rust 浮点转整数为饱和 cast，非截断回绕）。
  「非法输入 → 0」类属性若输入字母表含 `inf`/`infinity` 这类能被 `f64` 合法解析的词
  （如速度解析 `parse_speed("inf")` 实得 u64::MAX），属性会红——这是口径选择问题而非
  产品缺陷：要么收紧生成字母表（如正则排除 `i`），要么把饱和路径显式纳入规格口径，
  二选一并在属性 doc 注明，不得默默排除后当作"已验证非法面"。

## 8. 架构边界与适配器方向

- **低层模块不得构造高层模块的展示类型（适配方向：high→low）**：低层的内部数据视图应
  暴露纯数据字段，由消费侧的高层适配为展示类型。错误形态：低层提供
  `LowView::to_high(&self) -> high::High` 之类的构造方法——低层反向依赖高层的结构体，
  依赖方向倒置。正确形态：高层在消费侧内联 `High { field: lv.field, ... }` 构造。
  判定法：画依赖箭头时，`低层 → 高层展示类型` 的箭头是否与既有分层方向（高层→低层
  单向依赖）相反——相反即违规。自动化检查：grep 高层类型的字面构造模式于低层目录
  应无命中（跳过 `///` 注释行；类型与目录名按项目替换）。

- **入口（main.rs）不扩展应用主类型的 impl**：`impl App { fn add_task_from_cli }` 写在
  main.rs 会让 main 与应用层互相 reach-in（main 调 app、又在 main 里实现 app 的方法），
  模块边界模糊。正确形态：入口特有逻辑属于应用层职责，放应用层模块（如 `app/cli.rs`）；
  main.rs 只 `app.add_task_from_cli(...)` 调用。自动化检查：
  `grep -nE '^impl App \{' src/main.rs` 应无命中（`App` 换成项目应用主类型名）。这条规则
  在 Rust 项目里尤其高发——Rust 的 `impl` 块可分散在多个文件，但不代表"应"分散到非自身
  模块。

- **需求修订移除展示后，死字段会跨边界流动**：UI 删除某段展示后，底层事件若仍携带
  原展示专用的字段，该字段在适配器里被丢弃，成为跨边界流动的死数据。删除展示时
  穷举其底层数据的全部读取点（含适配器）一并清理，否则规格已删而代码残留成僵尸。

- **`proptest` 作为 Rust 属性测试框架（dev-dependency）**：SKILL 要求 architect 负责
  属性测试支持；Rust 生态标准为 `proptest` crate，作为 `[dev-dependencies]` 引入
  （不进发布物）。属性测试与常规单测**分开**纪律（engineering.md「设计与可测试性」）：
  独立模块 `src/property_tests.rs`（`#[cfg(test)] mod property_tests`），独立运行
  `cargo test property_tests::`；全量 `cargo test` 也跑，但失败不与基线对账
  （对账只计既有单测）。覆盖目标：纯逻辑模块的不变量、守恒性、往返、幂等性、排序、
  解析/格式化稳定性等（按项目自身的纯函数清单选取）。`proptest!` 宏内的 `format_args!`
  不能隐式捕获外层变量——断言消息里的变量必须用 `format!("{}", var)` 显式传参，
  或用 `"msg (var={})"` + `var` 的位置参数形式。

- **`prop_assert_eq!` 按值接参（非 Copy 类型会移动）**：断言后的变量即被 move，
  后续再断言报 E0382——对 String/Vec 等类型要么传 `.clone()`，要么先取引用
  断言再消费；一个属性里对同一值断言多次时首选绑定引用。

- **私有子模块工具函数的测试可达性用「门面 re-export」而非改调用方**：内部
  纯函数需跨模块供属性测试时，在定义处升为 `pub`（私有模块内，不外泄）+ 父模块
  `#[cfg(test)] pub(crate) use`；直接对 `pub(super)` 项做 `pub(crate) use`
  会报 E0364（re-export 不可超越项自身可见性），以及非测试构建报 unused import——
  `#[cfg(test)]` 门控一并解决。零外部 API 面增量的测试基建手法。

- **`proptest::sample::select` 需要借用或 `Cow<'static, [T]>`**：`select([a, b])`
  会因数组不是 `Cow` 而编译失败；正确形态 `select(&[a, b])`（借用切片）或
  `select(vec![a, b])`（`Vec` 实现 `Into<Cow>`）。自定义枚举类型需 `Clone + Debug + 'static`。

- **轻量架构检查脚本用 grep 即可覆盖大多数分层规则**：Rust 的 `use crate::xxx` 语句
  是依赖方向的显式标记，`grep -rnE "^[[:space:]]*use[[:space:]]+crate::(<高层模块名枚举>)"`
  按"低层目录内不得出现高层 use"过滤即可（模块枚举替换为项目自身的顶层分层名）。跳过
  `///` 注释行（文档可提及类型名，代码不得构造）。脚本放在 `project/<crate>/scripts/arch_check.sh`，CI 集成为 `&&` 链的一环；
  比起 `cargo-deny`/`cargo-modules` 等重量级工具，grep 脚本零依赖、可读、易维护，
  适合"分层依赖方向"这类规则简单的检查（复杂规则如 trait impl 方向才需重量级工具）。

## 9. 终端与 TTY（TUI）

- **TUI 异常终止后的终端自恢复 = 存活哨兵子进程模式**：SIGKILL 进程内无法捕获
  （raw mode 关 ISIG → CTRL+C 失效；鼠标捕获未复位 → 终端持续上报 SGR 鼠标事件
  被 shell 回显成 `32;64;10M` 形态残影；备用屏幕未离开）。方案：TUI 启动前 spawn
  同一二进制的哨兵子进程——①ready 握手防竞态（子进程先捕获 cooked termios、写
  ready 字节后，父进程才 enable_raw_mode）；②子进程阻塞等 wake 管道：读到数据 =
  父进程正常收尾（静默退出），EOF = 父进程死亡（SIGKILL/崩溃/exit 皆触发）→
  还原 termios + 写终端复原序列后退出。零 unsafe 实现口径：termios 还原用
  `stty -g` 可移植编码往返（Linux/macOS 皆内置），管道用 `std::io::pipe` +
  `PipeReader/PipeWriter: Into<Stdio>` 安全转换，无需裸 fd 与 libc。
- **哨兵必须独立进程组（`Command::process_group(0)`）**：会话首死亡时内核向
  **前台进程组**广播 SIGHUP——留在父进程组的哨兵会被同波及死，还原代码无机会执行
  （表象：哨兵已武装、kill -9 后无任何还原、无残留进程）。独立成组后哨兵凭管道
  EOF 而非信号感知死亡，正是设计语义；该坑在带 pty 的端到端验证前完全不可见，
  单测全绿照常通过。
- **`/dev/tty` 必须读写打开（`OpenOptions::new().read(true).write(true)`）**：
  `File::open` 只读，只读 fd 上 `write()` 返回 EBADF，而 `tcsetattr` 类 ioctl
  （如 `stty <saved>`）在只读 fd 上照样成功——"termios 已还原但复原序列写入失败"
  的假象即由此而来。诊断线索：持有中的 fd 突发 EBADF，先怀疑打开权限位而非"fd 被
  谁关了"。
- **pty 测试 harness 的会话首假象**：测试脚本里对被测 TUI 直接 setsid+TIOCSCTTY
  使其自任会话首时，kill 它会触发内核 tty hangup（后续写 /dev/tty 得 EIO）——真实
  场景会话首是操作者 shell（始终存活），不会有 hangup。harness 应用 bash 作会话首、
  被测 TUI 作前台作业（`bash -c "<tui>; echo done"`，尾缀真实命令防止 bash exec
  优化改变进程树形态）。进程识别用「exe 路径 + ppid 拓扑」（同二进制的哨兵与主进程
  exe 相同，按"父是否 ezr 进程"区分；跨轮次 pid 复用会以单 pid 断言制造假失败，
  套件启动先清理上一轮孤儿进程）。
