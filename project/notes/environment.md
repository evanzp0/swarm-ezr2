# 环境经验

## cargo 构建走 rsproxy 镜像
- ~/.cargo/config.toml 配 `rsproxy-sparse`（sparse+https://rsproxy.cn/index/）替换
  crates-io；rustup 安装同理设 `RUSTUP_DIST_SERVER`/`RUSTUP_UPDATE_ROOT` 为 rsproxy。
  直连 crates.io 在本沙箱会长时间卡在下载阶段。

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
