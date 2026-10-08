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
- **模块拆分隐式断掉测试模块的 `use super::*` 间接路径**：把 `xxx.rs` 目录化为
  `xxx/mod.rs` + 子模块（如把自由函数下沉 `xxx/routing.rs`）时，原来定义在 mod.rs
  顶层、被测试模块经 `use super::*` 间接消费的 `use` 导入（如常量）会随产品路径收敛
  而失效——非 test 编译 0 警告（该导入确实无人用），`--all-targets` 下测试编译才报
  E0425。防法：拆分后先跑 `cargo test`（不是只 `cargo build`），测试模块对拆分前
  依赖的符号**显式补导入**，不依赖 glob 隐式传递。挂载兼容性本身无忧：`#[path]`
  挂载 mod.rs 时，`mod 子模块;` 对 `xxx.rs` 与 `xxx/mod.rs` 两种形态解析等价。
- **重复失败分支提取为 `&mut self` 方法会暴露 NLL 的路径敏感借用**：内联的
  `Err(e) => { self.set_toast(..); return; }` 型重复块（`&self` 借用在错误路径上
  提前 return 终止，NLL 按控制流路径判定不冲突）提取成共用方法后，`&mut self`
  变成主路径无条件借用，与同函数内尚在存活的 `&self` 长借用撞 E0502。防法：方法
  化前先把调用点后续要用的借用字段**读出为局部值**（收窄借用窗口），再调用方法；
  这是机械等价变换，不改变行为。
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
- **单文件升级为目录模块（mod.rs + 子文件）的三个机械点**（比同文件分模块多出的坑）：
  ① `#[path]` 挂载测试壳经父模块 `mod engine;` 解析时 `engine.rs` 与 `engine/mod.rs`
  两形态等价（见上文挂载兼容条），目录内新增子模块文件对挂载壳透明——既有挂载测试
  无需改动；② 跨文件 impl 块的私有方法可见性 = 定义模块的子树——兄弟文件互调
  （如 tick.rs 调 evt.rs 的 on_evt）需 `pub(super)`（= pub(in 父模块)，父层不可见，
  窄接口不破坏）；③ 留在 mod.rs 的 cfg(test) 测试模块经 `use super::*` 消费的符号若
  产品构建无消费者（如 Evt/SpeedWindow 仅测试用），导入要 `#[cfg(test)]` 门控防
  产品构建 unused 警告。按行号切段重装时，先 `grep -n` 锁定结构边界（impl 开闭/
  函数 doc 行）再切——凭记忆的行号在会话内既有编辑后必然漂移，切段截断函数体的
  症状是 E0433/E0624 连环报，先核对切段区间再改代码。
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
  正常语义（非调用失败）。另有两个坑：① **报告文件清单 ≠ 全部拷贝清单**——同一 tile
  多份拷贝时报告只列部分配对（实测 16 份相同拷贝只报 2 对），"几处重复"以独立 grep 计数
  为准；② **Rust 词法器在 `/* */` 块注释内遇全角字符报 Lexical error（exit 5）**，
  `//` 行注释免疫——Rust 代码注释一律用行注释可保 CPD 可跑；exit 5（词法失败）与
  exit 4（发现重复）语义不同，先看 `[ERROR]` 行再读结果。
- **lint 姿态块去重：宏/include! 提取是死路，正确路径是「验证冗余 → 删除 → 单源 crate 根」**：
  模块级 `#![allow(...)]` 姿态块多拷贝时，提取共享文件的两条路都不可行——`include!()` 与
  `macro_rules!` 均不能拼接内部属性（探针实证：两者都报 "an inner attribute is not
  permitted in this context"）。先验证冗余性再动手：把 crate 级 posture（bin 入口 crate
  根）与各实现子模块块的 allow 清单逐一对照，子模块块全部被 crate 根覆盖时（含 pedantic/
  nursery 传递覆盖的子 lint）直接删除即可，clippy --all-targets 0 即闭环实证；只把子模块
  块中 crate 根没有的个别 lint（及依据注释）收敛回 crate 根。注意 bin-only crate 的
  `src/bin/*` 是独立编译单元，其 crate 根姿态不被主入口覆盖，不属冗余；带具体依据注释的
  定点 allow（如单文件复杂度豁免）是窄作用域豁免，不属姿态块，保留。
- **`#[path]` 挂载测试与产品 crate 的模块私有化联动**：产品侧把引擎实现子模块私有化
  （`pub mod` → `mod` + 门面 re-export）会断掉挂载同一 mod.rs 的测试 crate 的内部路径
  （E0603）。解法：产品 mod.rs 加 `#[cfg(test)] pub use` 门面 re-export——挂载测试 crate
  整体在 cfg(test) 下编译（re-export 生效），产品非测试构建不产生该路径（窄接口不破坏）；
  产品 crate 自身测试编译不消费该路径时的 `unused_imports` 属挂载机制固有告警，按三分法 ①
  在 re-export 处精确 allow 并注明依据。re-export 不可超越项自身可见性（E0364）：
  `pub(super)` 项要先在定义处升 `pub`（私有模块内不外泄）再 `#[cfg(test)] pub(crate) use`。
- **`#[path]` 挂载 crate 会连带执行产品源里的 `#[cfg(test)]` 模块，全量计数按编译上下文倍增**：
  加固类测试 crate 挂载产品 mod.rs 时，产品文件内联测试模块随挂载编译执行——在产品文件里
  新增 N 个单测，`cargo test` 总数增加 N ×（产品 bin 1 份 + 挂载该模块的 crate 数），
  不是 N。对账按各测试二进制 "test result" 行复算自洽（同名测试在多 crate 各出现一次属
  机制固有，`duplicate_mod` allow 已在挂载壳声明）；判断计数异常先数编译上下文再找测试。
- **纯文本工具的属性测试模板**（显示宽度表/格式化函数，CJK 混排终端 UI 高发）：
  宽度表函数锁「n ≤ w(s) ≤ 2n」界 + 截断/填充幂等（truncate 二次调用恒等、pad 恰达
  max(w, width)）；时长/日期格式化锁 parse-back 往返（格式化→按格式解析→原值）与
  Option 包装同口径；字节/速度格式化锁后缀封闭域（单位后缀只取固定集合）+「单位绑定
  总量」（配对展示的单位只由 total 决定）+ 负零归一。宽度界属性注意 `max=0` 边界：单
  字符省略号使 w(truncate(s,0))=1>0 属既有契约，属性域排除该点并在 doc 注明。
- **proptest 生成域与被测代码的资源分配维度必须解耦**：被测入口按某输入维度分配
  内存（如 `Blocks::new` 按块数分配 written 向量）时，对该维度用独立随机大区间采样
  （如 piece、total 各自随机到 2^20/2^40）会生成「极小块 × 极大总量」的天文块数组合——
  表象是属性测试挂死/OOM 而非断言失败（无 panic 栈可归因，与测试逻辑无关）。正确形态：
  把资源维度直接作为有界采样参数（如 `n_blocks in 1..=4096`），其余输入由它推导
  （`total = (n-1)*piece+1`，顺带锁定块数构造口径），属性语义不变而内存可控。
- **字符串解析属性的生成字母表要与「修剪/卫生」口径对齐**：被测函数先 trim 或过滤
  控制字符再判空时（如 Content-Disposition 文件名提取），字母表含空白/`\0`/`\u{b}`
  等可修剪字符会使「非空输入必提取成功」假红——反例全是「产品正确拒绝了生成器以为的
  合法输入」（trim 剥空后为空 → None，两次缩字母表才收敛）。要么字母表收窄到非空白
  可打印字符，要么把卫生分支并入 oracle（None ⟺ trim 后为空 ∪ 含拒绝字符）。与
  「属性字母表要排除可被解析器吞掉的词」（饱和 cast 条）同族：生成域 ⊆ 规格域。
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
- **`#[path]` 收编源的 cfg(test) 代码按测试 crate 解析 crate 路径**：产品源经 `#[path]`
  挂载进独立测试 crate 时，其 cfg(test) 模块里的 `crate::xxx` 引用按**该测试 crate** 的根
  解析，而非产品 bin 的根——挂载壳必须提供同名路径点（如在测试侧的 `mod model` 壳内
  `#[path]` 再挂同一个工具模块），漏一处就是「只在部分测试目标编译失败」。首个暴露信号
  常是 `cargo fmt` 报 `failed to resolve mod`，而非 cargo test。
- **嵌套 mod 文件内的 `#[path]` 相对该文件所在目录解析**：`tests/a/b/mod.rs` 里的
  `#[path = "../../src/x.rs"]` 实际解析为 `tests/a/src/x.rs`（相对 `b/` 上跳两级），不是
  相对 crate 根——层级按 mod 文件自身位置数。tests 根处 `#[path = "../src/..."]` 只上跳
  一级，两者极易混淆，解析失败先数层数再改路径。

- **llvm-cov lcov 节内 FN 行全在 FNDA/BRDA/DA 之前，流式挂"当前函数"会把全文件
  数据挂到最后一个函数**：lcov 每节顺序是 SF → 全部 FN:<起始行>,<符号> → 全部
  FNDA → 全部 BRDA → 全部 DA → end_of_record——逐行流式解析时若在 FN 行上切换
  "当前函数"再收 DA/BRDA，整个文件的数据都会落到该文件最后一个 FN 记录上
  （症状：函数数 ≈ 文件数、单函数 DA 行数等于全文件行数、其余函数全部"零覆盖"）。
  正确两段式：先按文件收集 FN 清单与文件级 DA/BRDA 映射，再按「起始行 ≤ 数据行 <
  下一起始行」归属；解析后先做阳性对照（已知被测函数 FNDA>0 且 cov 合理）再采信。
- **async fn 的多份单态化副本按逻辑名聚合（剥后缀、按文件归并、取极值）**：
  async fn 与闭包在 llvm-cov 里裂成多个编译实例（符号名带 `s<n>_`、`0B<n>_` 等
  后缀，跨挂载 crate 再各成一份，crate-disambiguator 哈希不同）——FNDA=0 的副本
  常是"另一份被调用"的单态化噪音，不是真缺口。按（文件, 逻辑名）聚合：逻辑名取
  mangled 串中最后一个「长度前缀小写标识符」并循环剥尾部 `s<n>_/_<m>`/`0B<n>_`
  片段；聚合口径 FNDA=max、comp=max、cov=min（取极值）；阳性对照：已知被测函数
  聚合后 FNDA>0。与「符号级输出不可直接精确匹配」（engineering.md）同源。
- **BRDA 计数近似圈复杂度时 `&&`/`||` 短路链每项记 2 个分支**：BRDA 是"分支存在"
  的静态计数（每条短路项产生真/假两个分支记录），据此算出的 comp 是真圈复杂度的
  上界近似（N 条件的守卫链 comp 虚高约一倍）——CRAP 门禁判定（comp≤6）对 3–4 条件
  守卫链会出现"BRDA comp=13 但源码 comp≈6"的假阳性；以源码决策点数（if/match 臂/
  短路项）复核后再登记，不据此硬拆守卫链。

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

- **proptest 的 `prop_union` 要求两侧同型**：`A.prop_union(B)` 签名为
  `other: Self`——枚举键 `select` 策略与 `any::<char>().prop_map(KeyCode::Char)`
  这类不同型策略不能直接 union（E0308：`Select<T>` ≠ `Map<CharStrategy, _>`）。
  正确形态：两侧各自 `.boxed()` 统一为 `BoxedStrategy<T>` 后经
  `prop::strategy::Union::new_weighted(vec![(w, a), (w, b)])` 合并（顺带控制
  键类占比，如导航键 8 / 字符键 2）。

- **两处以上克隆同一参数表的调用点收敛为「参数对象 + 方法」**：N 参函数在
  多个调用点逐参 clone（worker spawn 表、批次提交表）时，把共享句柄收进
  `#[derive(Clone)] struct`，调用点只构造一次结构体、spawn 经方法克隆整包；
  被调函数以结构体为参、函数体首解构（`let Struct { field, mut rx, .. } = deps`）——
  镜像参数表消失、`too_many_arguments` 豁免随之删除；字段清单唯一登记于
  结构体定义，构造点与消费点的漂移由编译器锁定（新增字段漏初始化/漏消费均
  编译错）。watch::Receiver 等句柄均 Clone，逐字段 clone 与整包 clone 语义
  等价，行为保持。

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

- **reqwest 代理是 client 级、请求不可逐个指派**：`reqwest::Proxy` 只能在
  `ClientBuilder` 上设置，`RequestBuilder` 无 per-request 代理 API。「任务/会话级
  代理」的标准实现 = client 缓存池：`Arc<Mutex<HashMap<ProxyKey, Client>>>`（key =
  代理 url + 账号三元组；None key = 直连 client），按请求前查池 get_or_build；
  切换代理 ⇒ 下一次查池即新 client，在途请求持旧 client 自然完成（无中断语义免费
  获得）。注意 `Client` 构建成本不低且池无淘汰，key 必须把账号算进指纹，否则同
  url 不同密码会撞池；构建失败回退直连 + toast 与既有全局代理口径一致。
- **socks 代理的认证只能走 url userinfo，`Proxy::basic_auth` 对 socks 代理静默无效**：
  `Proxy::basic_auth(user, pass)` 仅在 HTTP 代理场景注入 `Proxy-Authorization: Basic`
  头；socks 代理不经过 HTTP 头协商，该调用对 `socks5://` 代理不产生任何认证效果
  （reqwest 0.12 源码实证）——表象是「socks5 配了账号密码却认证被拒/静默失效」，且无
  警告可循。正确形态：把凭证编码进代理 url 的 userinfo（`socks5://user:pass@host:port`），
  reqwest 解析 socks url 时对 userinfo percent-decode 后走 RFC 1929 user/pass 子协商。
  配置层若将凭证与 url 分字段存放（无凭证 url 用于展示/落盘，避免凭证明文进界面与日志），
  必须在引擎构建 client 时对 socks 型内部拼装认证 url，不要把无凭证 url 直接喂给
  `Proxy::all` 了事；`basic_auth` 只留给 http 型代理。
- **https 型代理 = TLS 包裹的 HTTP 代理；IPv6 字面量进 url 必须加方括号**：代理 url 取
  `https://host:port` 形态时 reqwest `Proxy::all` 原生支持——客户端与代理之间整体走 TLS，
  HTTP 代理语义（CONNECT 隧道、`Proxy::basic_auth` 产生 Proxy-Authorization）在 TLS 会话内
  照常生效，无需按 scheme 写分支或特殊处理。ip 与 url 分字段存放、由 type+ip+port 拼装代理
  url 的配置模型中，ip 为 IPv6 字面量（含 `:`）时必须拼成 `scheme://[::1]:port`（带 userinfo
  同理 `scheme://user:pass@[::1]:port`），否则 url 解析失败或主机歧义——方括号收口在单一
  url 构造函数即可三类型（http/https/socks5）全覆盖。

- **HTTP 探测响应体不读会钉死串行服务端**：`GET Range: bytes=0-` 探测只读头部，
  响应体（= 整文件流）若不释放，reqwest Response 存活期间连接一直挂着——串行
  accept 的服务端（测试 fixture 常见形态）会阻塞在写体上，后续连接全部排队；
  小文件（KB 级，未塞满 socket 缓冲）完全无感，MB 级测试文件才暴露。修法：
  捕获最终 URL 后立即 `drop(resp)`（reqwest drop → 关闭连接 → 服务端写失败
  脱阻塞）。单流降级路径例外：探测响应体本身就是下载流，必须移交续读。
  推广纪律：凡是"读头部拿元数据、body 另起连接取"的下载器模式，探测响应
  必须显式释放，且 e2e 至少留一个 MB 级用例兜底。

- **多 target 目录的磁盘归属要先辨再清**：`cargo llvm-cov`、`cargo mutants` 各自使用
  独立 target 目录（`target/llvm-cov-target/` 等），主 `target/debug/build/`（build
  script 产物，常达 GB 级）只服务主 target——删它不影响 llvm-cov/mutants 的增量性，
  反之删 llvm-cov-target 则下次需 2.5G+ 全量重建、磁盘紧张沙箱直接放不下。清缓存前
  按「下一步要跑什么工具」决定归属，别按目录名直觉。

- **PMD 7 的 cpd 子命令用 `--dir`（`--files` 已废弃）**：`pmd cpd -l rust
  --minimum-tokens 50 --dir <src>`；退出码 4 = 发现重复（正常语义，不是失败），
  0 = 无重复。重复块计数口径随目录参数变化，src-only 与 src+tests 是两套数字
  （对账纪律见 engineering.md「度量数字对账先核口径」）。

- **llvm-cov 的 lcov 输出是标准 lcov 格式（冒号+逗号混合分隔）**：记录行为
  `FN:<line>,<name>`、`BRDA:<line>,<block>,<branch>,<taken>`、`DA:<line>,<count>`——
  自写解析器（如 CRAP 近似脚本）时 `FN:`/`BRDA:` 先以冒号切前缀再按逗号切字段，
  直接 `split(":")` 取三段会在标准格式上崩；名字段本身可含逗号，只切第一个逗号。

- **llvm-cov JSON 同源函数跨 crate 实例化必须剥 crate-disambiguator 再聚合**：
  同一源函数被多个测试 crate（`#[path]` 挂载树）编译时，每个实例化的 mangled
  name 带 crate hash（`Cs<hash>` 段），粗规范化会把同一函数拆成多个键、零覆盖
  实例在「取极值」聚合下冒充未覆盖——函数级覆盖率/CRAP 度量先把 `Cs[0-9A-Za-z]+`
  归一为 `Cs` 再提取可读名，按（文件, 规范化名）合并后取「任一实例覆盖即覆盖」。
  阳性对照：某已知被测函数（如 UI 渲染助手）在未剥 hash 的报告里常显示 cov=0。

- **对账时挂载树内的 cfg(test) 测试会在每个挂载它的测试 crate 重复注册（合法）**：
  `#[path]` 收编的产品源里 `#[cfg(test)]` 测试随挂载树进入每个引用它的测试
  crate，全量 `cargo test` 计数比「新增测试数」多出「新增测试 × 挂载该源的
  crate 数」——增量对账按 cargo test 输出逐目标列出并按挂载关系解释，勿与
  「重复 #[test] 属性虚增」（engineering.md 条款）混淆：后者是同 crate 内双注册，
  前者是跨 crate 合法副本（`--list | sort | uniq -d` 在跨 crate 场景会误报）。

- **`&mut self` 方法调用与字段级不相交借用不可混用（事件臂改造高发）**：在同一函数里
  持有 `self.tasks.iter_mut().find()` 返回的 `&mut Task` 期间，对同结构体**其他字段的
  直接访问**（`self.windows.remove(..)`）因字段不相交而合法；但换成 **`&mut self` 方法**
  （如 `self.clear_stats(id)`）会借走整个 self，与仍在存活期的 `t` 冲突（E0499）。改造
  事件处理臂时，收口型 helper 调用要么放在任务借用的最后一次使用之后，要么在方法内
  部改用字段级访问。判别：报错行是方法调用、且同块内另有 `&mut` 迭代借用存活。
- **ratatui TestBackend 断言的两个坑（CJK 跳过单元 / Cell 字段访问）**：①缓冲区里宽字符
  （CJK）占两格——第二格是空符号的跳过单元，逐格拼行文本会在宽字符间混入空格，
  `contains("并发")` 类断言必须先把行文本 `replace(' ', "")` 再匹配（顺序断言用压缩后
  下标）；`TestBackend::to_string()`（buffer_view）自动跳过续格，全文 `contains` 可直接
  用 CJK 连续文本。②`Cell` 的前景色是**公开字段 `fg`**（0.29 无 `fg()` 方法），取格用
  `buffer.cell((x, y)) -> Option<&Cell>`（`Buffer::get` 已 deprecated），配色断言遍历
  用 `cell(..).expect("界内")` 解包。配色断言**只锚定宽字符首格**——续格 fg=Reset、
  修饰为空（视觉由首格承载）；逐格配色遍历遇 Reset 即续格，跳过即可。
