# EZR Downloader（ezr download）— v0.1.0-01

终端高性能多协议下载器。**01 期**：HTTP/HTTPS 真实下载内核与 TUI 正式版。

界面与交互沿用 `ezr-tui-demo` 定稿基线（布局 / 状态机 / 配色 / 快捷键 / 对话框 /
中文文案），下载内核为真实实现：多并发分块（AIR2）、断点续传（sidecar 元数据）、
自动重试与指数退避、完整性校验（7 种算法）、全局限速、HTTP(S) 代理、会话持久化
与崩溃恢复。

## 功能特性（01）

- **下载内核**：HTTP/1.1 + HTTPS（TLS 1.2+ 系统根证书）；重定向跟随（≤10 次）；
  Range 分块并发（默认 1 MB/块，末块吸收余数，连接动态领块、乱序完成，实际并发
  = min(设置并发, 剩余块数)）；不支持 Range / 无 Content-Length 时自动单并发降级。
- **断点续传**：sidecar 元数据文件 `<目标文件>.ezr`（原子写：temp + rename）；
  暂停恢复 / 失败重试 / 程序重启三条路径均从断点续传；续传一致性检查
  （最终 URL / ETag / Last-Modified / 大小任一变化 → 作废断点从头下载，不计失败；
  连续 3 次失效转停等防循环）。
- **失败与重试**：失败分类展示（网络 / HTTP 状态码 / 校验 / 大小不符 / 磁盘 /
  内容持续变化）；重试计数连续性（有进展重置 1、无进展累加；达上限停等）；
  指数退避 8→16→32→60s，`Retry-After` 存在时优先采用（无 60s 上限）；
  HTTP 4xx（除 408/429）与校验失败不自动重试（可 `R` 手动重试）。
- **完整性校验**：MD5 / SHA-1 / SHA-224 / SHA-256 / SHA-384 / SHA-512 / Adler-32
  七种算法；校验值来源 = 添加对话框 / CLI `-x <算法>=<校验码>` 显式提供，或
  伴随文件 `<目标文件>.<算法后缀>`（大小写不敏感，`hex` 或 GNU `hex  文件名`
  格式，位数不符视为无效）。
- **TUI**：三区布局 + 任务列表（3 行条目）+ 详情分块表 + Sparkline 速度图 +
  toast 反馈 + 鼠标（点击选中 / 滚轮 / 对话框按钮）+ CJK 宽度对齐 +
  窄屏降级（<100 列隐藏右栏）；下载中文件名追加 `.downloading` 扩展名防误用，
  完成时移除。
- **对话框粘贴**：终端 bracketed paste（FR-01-06）——添加对话框的 URL / 目录 /
  并发 / 校验码字段整体接收剪贴板粘贴，过滤规则与逐键输入一致（长 URL 粘贴
  无需担心截断或换行混入）。
- **持久化**：任务注册表 `~/.ezr/state/registry.json`（含已完成/已失败历史，
  原子写）；配置 `~/.ezr/config.toml`（键见下）；`.ezr` 根目录可经环境变量
  `EZR_HOME` 重定位（未设置/空 → `~/.ezr`；不同 EZR_HOME 实例隔离，各自锁与数据）；
  崩溃恢复：下载中任务重启后回「等待中」按序排队，sidecar 断点自动合并。
- **终端哨兵**：TUI 启动前武装存活哨兵子进程（同二进制、独立进程组）——任意原因
  终止（含 `kill -9`）后，运行 ezr 的终端自恢复：raw mode 复位（CTRL+C 立即可用）、
  鼠标捕获/bracketed paste 关闭（无转义字符残影）、备用屏幕离开；优雅退出路径不受
  影响（哨兵收到通知即静默退场）。
- **其他**：单实例文件锁保护；`Q`/`Esc`/`Ctrl+C` 优雅退出（停传保留断点 →
  保存注册表 → 恢复终端；panic hook 兜底）。

## 构建与安装

依赖：Rust stable 1.85+（2024 edition 特性使用 stable 子集）。

```sh
cargo build --release          # 产物: target/release/ezr
cargo build --release --bin ezr-fixture
cargo build --release --bin ezr-proxy
```

## 使用

```sh
ezr                              # 打开 TUI（空任务列表）
ezr URL... [-d 保存目录] [-c 并发] [-x <算法>=<校验码>] [--max-speed 2 MB/s]
```

`-x` 示例：`ezr https://example.com/file.iso -x sha256=0123abcd…`（算法名 ∈
md5/sha1/sha224/sha256/sha384/sha512/adler32，大小写不敏感；位数不符启动即报错）。

### TUI 快捷键

| 键 | 功能 | 键 | 功能 |
|---|---|---|---|
| `A` | 添加任务 | `Space` | 暂停 / 继续（等待中 = 退出队列） |
| `R` | 重试失败任务（断点续传 / 校验失败 = 重新校验） | `D` | 删除任务（三选） |
| `U` / `J` | 上移 / 下移（调整排队优先级） | `C` | 清理已完成 |
| `G` | 显示/隐藏速度图 | `Tab` | 切换页签（正在下载 / 已完成） |
| `↑↓ PgUp PgDn Home End` | 选择 | `Q` / `Esc` / `Ctrl+C` | 退出 |

### 配置（~/.ezr/config.toml，可缺失，非法值回退默认；`.ezr` 根目录可经环境变量 `EZR_HOME` 重定位）

```toml
download_dir = "/data/downloads"   # 对话框留空时的默认目录（缺省 ~/Downloads）
block_size_http = 1048576          # HTTP 块大小（字节，默认 1 MB）
download_slots = 5                 # 全局下载槽位（默认 5）
max_speed = "2 MB/s"               # 全局限速（0 = 不限）
max_retries = 5                    # 自动重试上限
auto_retry = true                  # 是否自动重试
proxy = "http://127.0.0.1:8118"    # HTTP(S) 代理（仅配置文件，不读环境变量）
default_concurrency = 4            # 对话框留空时的默认并发
```

## 自动化验证

单元测试 + 引擎端到端冒烟（mock HTTP 服务器，覆盖探测/分块/单流降级/校验/
断点续传/一致性失效/状态码分类）：

```sh
cargo test                        # 全部测试（246 个：单测 + 属性测试）
cargo clippy --all-targets        # 零警告门槛
bash scripts/arch_check.sh        # 架构边界检查（6 规则）
```

本地 fixture 服务器（QA 基建，能力对应 qa/ 各套件环境前置节）：

```sh
# 1. 生成标准文件集（含 100MB 稀疏文件、streamy.bin 无 Content-Length、ghost-404.bin）
ezr-fixture gen --root /tmp/ezr-fix

# 2. 启动 fixture（请求头 JSONL 日志供「无块重传」等断言使用）
ezr-fixture serve --root /tmp/ezr-fix --port 8765 --log access.jsonl

# 3. 启动代理 fixture（双实例区分记录；CONNECT 隧道记录）
ezr-proxy serve --port 8766 --name proxy-a --log proxy-a.jsonl
```

fixture 控制参数（叠加在文件 URL 上）：`?norange=1`（关闭 Range）、
`?status=503&retry_after=10`（状态码注入）、`?disconnect=N`（传 N 字节断连）、
`?speed=B/s`（慢速门控）、`?redirect=N`（重定向链）、`?etag=X` / `?noheaders=1`
（一致性头控制）、`?cd=文件名`（Content-Disposition）、`?swapsize=N`（同 URL 换内容）。

## 交付物

- 源码（`src/`）+ 预编译二进制（Linux x86_64，`target/release/ezr`）
- fixture 工具（`ezr-fixture` / `ezr-proxy`）
- 单元测试与端到端冒烟（`cargo test`）
- 本 README（使用说明 + 自动化验证方式）

## 范围说明（01 未含）

BT / 磁力链（02 期）、每任务限速与 SOCKS5（03 期）。
`--no-tui` 无终端模式已从需求中移除（原 FR-01-74，v1.2 操作者定案删除）。
v1.3 修订：`.ezr` 目录支持 `EZR_HOME` 重定位（D16）；新增终端哨兵（FR-01-84，
任意原因终止后终端自恢复）。
