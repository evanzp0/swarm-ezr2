# 环境经验

## cargo registry 走 rsproxy；rustup dist 走 TUNA
- ~/.cargo/config.toml 配 `rsproxy-sparse`（sparse+https://rsproxy.cn/index/）替换
  crates-io（registry 用 rsproxy 实测正常）。直连 crates.io 在本沙箱会长时间卡在下载阶段。
- rustup dist（工具链组件下载）不用 rsproxy，改用 TUNA（见下节实测）。

## 后台进程会被回收：长构建分块前台跑
- nohup/setsid 起的 cargo 后台进程在工具调用间隙被沙箱回收（日志停更、进程消失）。
  处置：长构建用前台 `timeout <分块时长>` 分块执行，靠 cargo 增量编译跨块推进；
  测试过滤参数注意 `-j` 属 cargo 侧，须放在 `--` 之前（放 `--` 后会传给测试二进制报
  Unrecognized option）。

## 9.9G 小盘：磁盘满先清 target/debug/incremental
- 全量回归+clippy 阶段磁盘 100% 满（`No space left on device (os error 28)`）。
  `target/debug/incremental` 是最大可再生空间（本仓 ~2.5G），删除不影响既有测试二进制
  的指纹有效性；`~/.cargo/registry/src`（解包源 ~344M）可再生可删。项目交接后
  `bin/swarm complete` 会自动清理 target/ 等构建缓存。

## 终端显示伪影：`#[m` 序列会被吞成疑似损坏
- 本沙箱 Bash 通道回显中 `#[must_use]` 等含 `#[m` 的文本会显示为 `#ust_use]`，极易误判
  为文件损坏。处置：用 `od -c`、`md5sum` 或文件读取工具核实原始字节再下结论，不要凭
  终端回显改文件。

## pip 直连 PyPI/清华镜像均不可用：走阿里云镜像
- `pip config set global.index-url https://mirrors.aliyun.com/pypi/simple/`；清华镜像
  simple 页可读但 packages 403，直连 files.pythonhosted 长时间卡死。libtorrent 2.1.1
  有 cp312 manylinux wheel（`pip install libtorrent`），无需编译。

## rustup dist 镜像选型：TUNA 快、rsproxy 慢到不可用（2026-10-28 实测）
- rust-toolchain.toml 钉 nightly（为 rustfmt unstable 选项）。rsproxy 拉 nightly 组件
  实测 ~20KB/s，且 rustup **无断点续传**（前台分块每块中断即清零重来，8 分钟零进展）。
- 镜像 8MB range 探针结果：**TUNA 13.7MB/s**、USTC 403、SJTU 不通、BFSU 404。换 TUNA
  一次装成 minimal+rustfmt+clippy（命令）：
  `export RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup`
  后 `rustup toolchain install nightly --profile minimal --component rustfmt,clippy`
- 注意：TUNA 的 PyPI packages 403 但 rustup dist 正常——同一镜像站不同服务可达性不同，
  逐服务探测，不要一票否决。
- nightly 装成后补跑 `cargo fmt --check`：全仓仅 src/bt/engine.rs 有 diff（上轮新增
  结构体字面量换行风格），`cargo fmt` 套用后干净；bin ezr 748P/0F、hardening_g_engine
  563P/0F、hardening_engine_backfill 651P/0F、clippy --bins --tests 0 警告
  （全部在 nightly 工具链下验证）。
