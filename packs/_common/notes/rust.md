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
- **区分「丢注入」与「本体丢失」**：环境核验先探针安装目录（如 `ls ~/.cargo/bin`）——
  目录在位是丢 PATH 注入（export 即恢复）；目录不存在是全新沙箱/本体丢失，需完整重装
  （rustup minimal profile + clippy + rustfmt），交接记录的安装位置只对同机有效。
- **无 lld 环境的零改动桥接**：rustfmt/clippy 约束模板配套的 `-fuse-ld=lld` 在无 lld
  且无 root 装包权限的环境会挂掉一切链接。解法：用工具链自带 rust-lld 做符号链接——
  `ln -sf ~/.rustup/toolchains/<tc>/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld
  ~/.local/bin/ld.lld`（gcc 按 PATH 找 `ld.lld`），不改基线配置；该符号链接目录必须
  在 PATH 中，且零改动的增量构建会静默跳过链接阶段掩盖其缺失，验证见
  engineering.md「环境不跨会话持久」的强制重做条款。

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

- **stable 工具链忽略部分 rustfmt unstable 选项**（`imports_granularity` /
  `group_imports` / `fn_single_line` 仅 nightly 全量生效，stable 下警告提示但忽略）：
  配置文件仍按模板原样保留，不为此改模板或切 nightly。

## 3. 编译、lint 与磁盘

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

## 6. 网络与异步（Rust 专项沉淀）

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
