# harness — BT 互操作测试夹具（外部第三方实现）

本目录是 ezr BT 引擎的**外部互操作测试夹具**：用非 ezr 生态的第三方 tracker 与 BT
客户端组成真实 swarm，检验 ezr 的 BEP 3/15 与 peer-wire 实现在真实世界下的兼容性。
ezr 自带 fixture（`ezr-fixture`）属于同源实现，测不出"别人家客户端"才能暴露的问题，
本夹具与其互补。

BT 对照客户端**只用 Python 的 libtorrent**（官方绑定的事实标准实现），曾是附加观察项
的 gallexis/PyTorrent 因产物残缺于 2026-10-28 按操作者指令移除。

## 目录结构

```text
harness/
  tracker/
    pybtracker/            第三方 elektito/pybtracker（BEP 15 UDP tracker，纯 Python asyncio）
    pybtracker_server.py   pybtracker 启动包装器（tuple 绑定 + asyncio.run）
    mini_http_tracker.py   自研极简 BEP 3 HTTP tracker（stdlib-only，JSONL announce 日志）
  swarm_e2e.py             主编排：造种 → 双 tracker → seeder → ezr pty 驱动 → leecher 同时下载 → 对账
  bench_multipeer.py       多 peer 速度基准：N seeder + M 并行 leecher 对照（限速/中途加入/侧车采样速度曲线）
  README.md                本文件
  .ref-py-http-tracker/    不可用候选存档（同 IP 自过滤 + 需预注册 infohash）
  .ref-trondhumbor-tracker/ 不可用候选存档（Flask 1.1.2 死依赖）
  .ref-pieces/             不可用候选存档（eliasson/pieces，无 seed，未启用）
```

## 环境安装

只需 Python 3.10+ 与三个包：`torf`（造 .torrent）、`libtorrent`（对照 BT 客户端）、
`pyte`（终端仿真，用于 pty 驱动 ezr TUI）：

```sh
# 沙箱内 PyPI 走阿里云镜像（清华镜像实测 403）：
pip install -i https://mirrors.aliyun.com/pypi/simple/ torf libtorrent pyte
```

**libtorrent 说明**（本夹具唯一的第三方 BT 客户端，同时充当 seeder 与 leecher）：

- 它是 C++ 事实标准实现库（qBittorrent、Deluge 等知名客户端的内核）的**官方 Python
  绑定**，PyPI 包名 `libtorrent`，manylinux wheel 免编译；import 名同为 `libtorrent`
  （惯例 `import libtorrent as lt`）。
- 本轮验证版本 **2.1.1**；脚本按 2.x 的 settings-pack / alert API 编写，1.2.x 未测。
- 安装后验证：

```sh
python3 -c "import libtorrent; print(libtorrent.version)"
# 期望输出形如 2.1.1.0
```

- 若 pip 源里没有对应平台的 wheel（少见，如部分老 glibc），退路是系统包
  `apt install python3-libtorrent`（版本可能较旧，跑通 2.x API 需自行确认）。

## 选型记录（2026-10-28）

| 角色 | 选择 | 状态 | 说明 |
|---|---|---|---|
| UDP tracker | elektito/pybtracker ★59 | ✅ 启用 | 纯 Python asyncio，BEP 15 connect/announce 正确；注意：seeder 计数按 event=completed 而非 left=0；periodic announce 有 interval 频控（过早重 announce 回 error） |
| HTTP tracker | 自研 mini_http_tracker.py | ✅ 启用 | 用户提名的 tinytracker/Prosperous 均非 BT tracker（GitHub 搜索证实不存在）；现有开源纯 Python HTTP tracker 三个候选全部不可用（见 .ref-*），按 ezr-fixture 同类口径自研兜底 |
| seeder/leecher 对照组 | libtorrent 2.1.1（Python 绑定） | ✅ 启用（唯一 BT 客户端） | 官方 manylinux wheel，功能完整，互操作金标准；一实例播种（17903）、一实例同时下载（17904） |
| ~~纯 Python leecher~~ | ~~gallexis/PyTorrent~~ | ❌ 已移除 | 可运行可 announce 但下载产物不完整（bitrot）；2026-10-28 操作者指令移除，判据不变 |

## 用法

前置：`bin/swarm complete` 会按宪法清理构建缓存（`target/` 整目录删除），因此**每个
会话收尾后首次跑 e2e 前需重建二进制**：

```sh
cd project/ezr
cargo +stable build --release --bin ezr   # nightly 工具链就绪后可直接 cargo build --release
```

然后：

```sh
cd project/harness
python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150
```

`--ezr-bin` 传相对路径也可以：脚本会在内部将其解析为绝对路径（pty 子进程会 chdir
到隔离 HOME，相对路径必须在 chdir 前解析）。

常用参数：`--payload-mb`（payload 大小，默认 4 MiB）、`--timeout`（整体下载窗口秒数）、
`--workdir`（指定工作目录）、`--keep`（保留全部中间产物供检查）。

编排步骤（全自动）：确定性伪随机 payload（seed 0xE27）→ torf 生成 256KB piece 的
.torrent（announce-list 含 http://127.0.0.1:17901/announce + udp://127.0.0.1:17902/announce）
→ 起双 tracker（HTTP 17901 / UDP 17902）→ libtorrent seeder（17903）→ pty 驱动 ezr TUI
（A → 种子路径 → Enter → Enter 立即下载）→ libtorrent leecher（17904）同时下载 →
等全部完成 → SHA-256 三方对账（原始 payload / ezr 产物 / libtorrent leecher 产物）。

端口占用：17901/17902/17903/17904（夹具专用，避开 QA fixture 端口段；跑前确保无残留，
见「已知坑」）。

工作目录默认 `../../.work/tmp/swarm-e2e/`（宪法临时文件纪律），保留证据文件：
`result.json`（机器可读结果）、`http_announces.jsonl`（每条 announce 的
peer_id/event/left/uploaded/downloaded）、`udp_tracker.log`（pybtracker 日志）、
`ezr_screen_final.txt`（TUI 终帧）。`--keep` 保留全部中间产物。

判据：ezr 到达完成/做种态，且 ezr 产物 SHA-256 == 原始 payload == libtorrent leecher 产物。

## 多 peer 速度基准（bench_multipeer.py）

```sh
cd project/harness
python3 bench_multipeer.py --ezr-bin ../ezr/target/release/ezr \
  --seeders 2 --leechers 0 --payload-mb 128 --seeder-upload-kbps 2560 \
  --workdir ../../.work/tmp/bench-x
```

- `--seeders N` 初始 libtorrent seeder 数（各占 17903+i 端口）；`--leechers M` 与 ezr
  同时下载的对照 leecher 数（17920+i）。
- `--seeder-upload-kbps` 对每个 seeder 设 libtorrent 上传限速（`torrent_handle.
  set_upload_limit` + session `upload_rate_limit` 双保险），用于复现「单 seeder
  2.5MB/s」的真实局域网口径；0 = 不限速（回环页缓存全开 ~50MB/s，测不出多 peer 效应）。
- `--late-seeders N --late-seeder-delay T`：T 秒后追加 seeder（绑定 127.0.0.2+ 以区分
  IP；ezr 靠 re-announce ≤30s 内发现）。
- 进度采样：ezr 的 `.ezr` sidecar（引擎 1s tick 落盘的 done_pieces JSON），完成后
  sidecar 被删除视为完成。产物：`result.json` + `speed_curve.json`（t/done 采样，
  其中 `t<0` 的负值行是事件标记：`-100-N` 并发数、`-1-N` late seeder 加入点）。
- 判据：finished + sha256_match。速度结论看 `avg_speed_Bps` 与曲线拐点。

## 结果记录

- **2026-10-28 首轮（三连 PASS）**：ezr 2.2s 完成下载并转做种；ezr / libtorrent-leecher /
  原始 payload 三方 SHA-256 一致；HTTP(BEP 3) 与 UDP(BEP 15) 双协议 announce 证据齐备
  （peer_id `-EZ2000-` 形态、started→periodic 序列、left/uploaded/downloaded 真实演进）。
  当时 PyTorrent 作为附加观察产物不完整（不阻断）。
- **2026-10-28 移除 PyTorrent 后回归**：PASS（判据路径与首轮一致）。同轮加固两处：
  ① `--ezr-bin` 在 chdir 前解析为绝对路径（相对路径曾因 pty 子进程 chdir 到隔离 HOME
  而指向不存在文件，子进程 127 退出 + tty 回显，外观像“TUI 未就绪”）；② 等 TUI 首屏
  （“命令窗口”提示）与 BT 对话框改用带持续 pump 的 `wait_for`（原为单次 0.6s 定值
  pump，存在启动时序敏感面）。
- **2026-10-30 bug10 轮（上传离环 + 空闲置换修复后回归）**：4MB PASS；256MB 基准 ezr
  **64.2s（4.0MB/s）**——上轮 123.8s 的近 2 倍，原因：标准编排本就带并行 leecher，ezr
  上传路径此前内联在引擎循环，修复后自身下载不再被上传服务拖累；16KiB piece 压测
  129.8s → **69.6s** 同因。多 peer 基准（`bench_multipeer.py`，seeder 限速 2.5MB/s）：
  ①1 seeder 2.39MB/s（修复前后持平）；②2 seeder 从头 4.68MB/s（扩速正常）；③late
  seeder 绑 127.0.0.2 中途加入，re-announce 发现后爬升 4.1–6.3MB/s；④**1 seeder +
  1 并行 leecher（同 127.0.0.1）为「添加 peer 反而降速」的最小复现**：修复前 ezr
  175s/0.77MB/s（首连随机落在 leecher，同 IP 去重把 seeder 永久挡住，leecher 下完转
  做种才恢复），修复后 115.6s/1.16MB/s（60s 空闲置换重拨 seeder 后满速）。

## 由夹具暴露并已修复的 ezr 缺陷（phase-02）

1. **多块 piece 永不拼装**（`bt/engine.rs on_block`）：原实现依赖"整 piece 从磁盘回读"
   合并块，但未完成 piece 从不落盘——piece_len > 16KB（真实世界 256KB 标准件）时块被
   丢弃、死循环重取。自研 fixture 测不出（其测试种子 piece 恰为单块）。修复：内存
   partial 按序拼装 + 校验通过才落盘 + write/flush 失败即中断。
2. **peer 连接帧错位**（`bt/engine.rs peer_pump`）：`tokio::select!` 同时轮询
   `read_msg`（内含非取消安全的 `read_exact`）与出站通道，写分支命中时丢弃已消费字节，
   真实客户端流式连发 block 时 100% 触发「消息超长」。修复：读写分离，reader 独占读半部
   永不取消。
3. **tokio File 写入可见性**：`write_all` 返回 Ok 只代表进入 tokio 内部缓冲，
   必须 `flush().await` 才保证对同进程其他读者可见（BtDone 即刻校验/读盘场景必需）。
4. **引擎任务 panic 静默僵尸**（`engine/mod.rs start_bt_task`）：panic 仅 RST 断连、
   任务永停但 UI 无感。修复：join 监视器把 panic 转 `Evt::BtFailed`（Fatal）。

## 已知坑（写给下次）

- ezr 的 TUI 主任务把 `terminal.draw` 与 `app.tick()` 放在同一个 async 任务里：**pty
  无人读时内核缓冲填满，draw 阻塞 → 整个 TUI 冻结**（引擎任务独立运行不受影响）。
  任何 pty 驱动脚本在等待观察期间必须持续 pump（`ezr.pump(...)`），否则会误判
  "UI 不更新"。
- 端口 17901/17902 残留进程会让新 tracker 绑定失败但 `wait_port_tcp` 仍探活成功
  （探到的是僵尸），表现为 announce 日志 0 行。跑前 `pkill -f mini_http_tracker; pkill -f pybtracker_server`。
- torf 的 `piece_size` 单位是字节且须 16KiB 倍数（256KiB = 262144）。
- pty 子进程里 `execvp` 用相对路径时，它相对于 **chdir 之后的隔离 HOME** 而非调用
  cwd——所有传给被测进程的路径（二进制、.torrent）一律先转绝对路径；子进程 exec
  失败会在 pty 上留下 Python traceback（若被测物是脚本）或静默 127（二进制），
  表层症状都是“按键被回显、TUI 不出现”。
