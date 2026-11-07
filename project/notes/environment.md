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
- 第二层空间：跑完全量后 `target/debug/deps` 里的测试可执行文件（无扩展名、单个
  20M~数百M，全仓 ~3.8G）可安全删除——clippy 只 check 不链接，后续若需重跑测试仅
  重新链接（秒级）不重编译。全量测试期间设 `CARGO_INCREMENTAL=0` 防增量缓存回填。
- **build 命令同受小盘约束**（config 废弃轮复现）：`cargo build` 漏带
  `CARGO_INCREMENTAL=0` 时 incremental 会话目录写入同样爆盘（os error 28）——
  测试命令带了、build 命令漏带照样翻车；所有 cargo 调用统一带。

## harness 的 tracker/seeder 是会话级临时进程：BT 手测前先起夹具
- **现象**：操作者报告「BT 连不上 tracker、无 peer」；实测 e2e 三连 PASS、产品无回归。
- **判定路径**：`ss -tlnp/-ulnp` 查 17901/17902 无监听 + `~/.ezr/state/registry.json`
  为空 + 用户侧成功时间窗与上一会话 e2e 运行窗重合 → 根因是上一会话起的临时
  tracker/seeder 进程已死，种子里的 `127.0.0.1:17901/17902` 指向空端口。
- **用户可见表现**（死 tracker）：任务停在「下载中 0.0%」约 4 分钟后才报「全部
  tracker 失败」——UDP 首轮 announce 要 15+30+60+120s 重试耗尽，HTTP 秒败但只计
  1/2 个 tracker，二者都败才报失败（engine.rs 的 tracker_fail_count 判定）。
- **自愈/自证**：按 `project/harness/README.md` 重起夹具（tracker 17901/17902 +
  seeder 17903）再跑 ezr；或直接 `python3 swarm_e2e.py --ezr-bin <release/ezr>` 全自动
  验证。夹具进程不随 complete 存活，每轮手测前需重启。

## 终端显示伪影：`#[m` 序列会被吞成疑似损坏
- 本沙箱 Bash 通道回显中 `#[must_use]`、`[http]` 等含 `#[`/`[h`/`[m` 的文本会显示为
  `#ust_use]`、`ttp]` 等残缺形态，极易误判为文件损坏（`cat`/`git show`/python repr 的
  回显同样被吞）。处置：不要凭终端回显改文件；核实用**布尔判定探针**——
  `python3 -c "print('[http]' in open(f).read())"` 输出 True/False，不回显可疑序列本身，
  不受伪影影响（bug12 轮实证：config_fallback.rs 的 `[http]` 三次被误读为 `ttp]`，
  布尔探针证实文件完好）。

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

## 小盘 100% 满在本轮再次复现（2026-10-28 二次确认）
- 全量回归中途再次 `os error 28`，同族处置生效：`rm -rf target/debug/incremental` +
  `find target/debug/deps -maxdepth 1 -type f -executable -size +5M -delete`（一次回收
  ~6.7G，df 100%→46%）；全量测试全程 `CARGO_INCREMENTAL=0`。多套件顺序连跑时每两个
  套件查一次 df，runtime 期间编译假失败（error 28）按本条处置后单套件重跑即可，不是
  产品回归。

## rustup 安装镜像选型：rsproxy 文件下载最快但 dist 清单缺键，工具链走 TUNA dist server
- **现象**：空沙箱装 rustup 时，对 rsproxy.cn/tuna/ustc/sjtu/aliyun 测速——rustup-init
  二进制下载 rsproxy 14.2MB/s（1.5s 拉完 21MB）显著最快、ustc 1.2MB/s、tuna/sjtu/aliyun
  404 或拒连；但用 rsproxy 作 RUSTUP_DIST_SERVER 装 nightly 报
  `Malformed manifest date: ""`——`/rustup/dist/channel-rust-nightly.toml` 返回的是
  HTTP 200 的 OSS NoSuchKey XML 错误页，rustup 当清单解析后 date 字段为空即 panic
  （ustc 同族同源，同样缺该键）。
- **处置**：rustup-init 二进制本体取 rsproxy（快），工具链 dist server 换
  `RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup`（清单 200 正常）→
  `rustup-init -y --default-toolchain nightly --profile minimal --component rustfmt
  --component clippy --no-modify-path` 一次装齐；cargo registry 配 rsproxy sparse
  （~/.cargo/config.toml replace-with）。
- **根因**：镜像的「文件下载速度」与「dist 清单完备性」是两个独立维度——测速只证明带宽，
  装工具链前先 curl 一次 `channel-rust-nightly.toml` 验证内容是 TOML 而非错误页。

## rustup 镜像 2026-10-31（bug13 轮）复测：rsproxy 官方双变量整链路可用且最快
- **现象**：同机新沙箱复测，rsproxy 官方双变量（`RUSTUP_DIST_SERVER=https://rsproxy.cn` +
  `RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`）下 rustup-init.sh 前台一次装齐 nightly
  minimal + rustfmt + clippy；init 二进制 ~16MB/s，channel-rust-nightly.toml 返回正常 TOML
  （旧「dist 清单缺键/Malformed manifest」本轮未复现——另注意安装脚本 rustup-init 的
  rustup-init 二进制取自 **UPDATE_ROOT**/dist/…，UPDATE_ROOT 误写成 …/rustup/rustup 双层
  路径必 404，报错文案是「installer not found」，别据此改判清单缺键）。
- **口径更新**：空沙箱首选 rsproxy 官方双变量一条命令装齐；crates.io 仍配 rsproxy sparse。
  TUNA/USTC 作为后备（各轮可达性与清单完备性会漂移，装前先 curl 清单验证）。
- **bug15 轮新沙箱复测（2026-10-31）**：8MB 探针 rsproxy 单路径 `/rustup/dist/<host>/rustup-init`
  **19.3MB/s** 最快（tuna/ustc/aliyun 单路径 404 属既有双层路径陷阱）；直链下载 rustup-init
  二进制 + 双变量前台 `-y --profile minimal --default-toolchain nightly --component
  rustfmt,clippy` 一次装齐（cargo 1.101.0-nightly，EXIT=0，免 init.sh 双跳；直链单路径
  `/rustup/dist/…` 与 DIST_SERVER 同层，不触发 UPDATE_ROOT 双层陷阱）

## bug6 轮环境：USTC 全套镜像选型（rustup dist + cargo sparse index）
- 本轮沙箱 Rust 工具链再次缺位（会话隔离）。镜像实测：延迟 USTC 0.52s < TUNA 0.71s <
  rsproxy 1.53s（官方 static.rust-lang.org 8s 超时）；5MB 吞吐 USTC 3MB/s > TUNA 2MB/s。
- **本轮口径**：rustup 用 `RUSTUP_DIST_SERVER/RUSTUP_UPDATE_ROOT = mirrors.ustc.edu.cn/rust-static`
  前台安装（操作者要求前台、不走后台）；nightly minimal + rustfmt + clippy 一次装齐。
  cargo 另配 `~/.cargo/config.toml`：`source.ustc = sparse+https://mirrors.ustc.edu.cn/crates.io-index/`
  替换 crates-io（上轮 rsproxy 清单缺键问题本轮 USTC 未复现，冷构建 4m01s 全绿）。
- 工具链钉版走 rust-toolchain.toml（nightly + rustfmt + clippy），cargo 命令自动走
  nightly，无需手工 `+nightly`。
- **工具陷阱**：`bin/swarm begin` 的输出被管道截断（如 `| head`）时，脚本会因 SIGPIPE
  在写 session 状态前中断，`complete` 报「没有进行中的会话」——begin/complete 输出
  重定向到文件再查看，不要用管道截头。
- **begin 会清空 `.work/tmp/`**（config 废弃轮发现）：begin 入口
  `find "$TMPD" -mindepth 1 -exec rm -rf {} +`——重定向目标放 `.work/tmp/begin.log`
  会被脚本自己删掉（表象 = redirect 成功但 cat 不到了，会话已正常建立）；
  begin 输出日志放 `.work/` 根或直接看 stdout，不入 tmp/。

## harness 速度基准的 piece 粒度压测口径（--piece-kib）
- `swarm_e2e.py` 新增 `--piece-kib`（默认 256 不变）：速度回归必须补一组小 piece
  压测（如 `--payload-mb 256 --piece-kib 16` → 16384 pieces）。引擎逐块成本随
  pieces 数放大，256KiB piece 的千级 pieces 基准全绿不代表真实大种子不塌缩
  （bug9 轮实证：16k pieces 下旧实现慢 25%）。

## rustup 镜像选型本轮实测（2026-10-30，TUNA 整链路最快且可用）
- 测速口径：rustup-init 二进制 rsproxy 17.4MB/s > TUNA 12.8MB/s（注意 TUNA 的
  rustup-init 在**双层路径** `/rustup/rustup/dist/<target>/rustup-init`）> USTC
  8.2MB/s（`/rust-static/rustup/dist/...`）；SJTU/aliyun 404。
- dist 清单：TUNA（`/rustup/dist/channel-rust-nightly.toml`）与 USTC
  （`/rust-static/dist/...`）正常；**rsproxy/ustc 的 `/rustup/dist/` 路径仍是
  NoSuchKey XML 错误页（第三次复现，测速快 ≠ 能装）**。
- dist 组件吞吐：TUNA **28.7MB/s**（16MB range 探针）。新发现：新版清单里组件 URL
  是镜像重写后的**绝对 URL**（含日期戳目录 `dist/<date>/…`），rustup 读取清单后
  自然落在本站，手工探测必须先解析清单拿真实路径，拿旧路径探测会误判 404。
- 本轮结论：TUNA 单镜像整链路（init + 清单 + 组件）最快且可用，前台一次装齐
  nightly minimal + rustfmt + clippy（RUSTUP_DIST_SERVER/RUSTUP_UPDATE_ROOT 均指
  TUNA）。

## rustup 镜像补记（2026-10-30 coder 轮）：单路径探测 TUNA 404 是已知双层路径陷阱，USTC 单路径一站式可用

- 复现：用单路径 `/rustup/dist/<target>/rustup-init` 探测 TUNA/aliyun/nju/huawei 全
  404（TUNA 实际在双层路径 `/rustup/rustup/dist/...`，见上条）；USTC 单路径
  `/rust-static/rustup/dist/...` 直接可用。
- 本轮实测：USTC rustup-init 8.65MB/s（5MB range 探针），前台 `-y --profile minimal`
  一次装齐 stable + nightly（rust-toolchain.toml 钉 nightly 自动补装），全程无需换源。
- crates.io 走 USTC sparse 索引（`~/.cargo/config.toml` replace-with ustc）+ 沙箱无
  外网时依赖拉取全绿；pip 用阿里云镜像装 torf/libtorrent/pyte（harness 依赖）。
- 磁盘 9.9G 小盘：debug 增量缓存两次撑爆（No space left on device）——测试轮用
  `CARGO_INCREMENTAL=0`，必要时清 `ezr-demo/target` 与 `target/debug/incremental`。


## GitHub 访问：raw.githubusercontent 重置不可用，git clone 偶发断连靠重试，历史版本文件用 blob:none+sparse
- raw.githubusercontent.com 直连被重置（Connection reset）；github.com 的 git 操作可用但
  偶发 TLS 断连/超时（约 1/3 概率），失败重试 2–4 次即可成功。
- 取大仓库单文件/历史版本：`git clone --depth 1 --filter=blob:none --sparse` 后
  `git sparse-checkout set <目录>`（按需懒取 blob）；再要历史版本时
  `git fetch --depth 1 origin tag <tag>` + `git show <tag>:<path>`——blob:none 滤镜下
  fetch 元数据很小，跨版本比对源码成本可控（本轮 qBt/lt 四分支比对即此法）。
- `git ls-remote --tags` 一次性拿全标签列表再挑版本，比盲猜 tag 名可靠（qBt 用
  release-X.Y.Z 命名，不是 vX.Y.Z）。
## bug14 轮（specifier）
- GitHub 直连间歇不可用（git clone 与 release 下载同时超时；本会话开头 clone swarm-ezr2 成功过，同会话后段 443 连不通）：gh-proxy.com 前缀镜像可用且快（bb v1.12.209 静态包 + APS 仓库 clone 均经 `https://gh-proxy.com/https://github.com/...` 成功）；ghfast.top/kkgithub 探测不通。rustup/cargo 仍走 rsproxy 不受影响。
- **heredoc python 写文件偶发不持久化**（本轮两次复现：re.sub 36 单元格与 PC-04 改写——脚本打印成功、紧随的读回是旧内容；间隔一次调用后有时又变新内容）：写关键文件一律「写后 fsync + 本进程读回布尔探针 + 外部 rg 复核」三连；仍不符时用行索引重写并再验。

## bug16 轮（specifier）：工具调用输入/输出存在双向文本改写，含方括号序列的锚点匹配必须运行时拼接 + 字节级验证
- 现象：经 chat 工具通道编辑 mission/phase-02.md 时，输入内容中的 `[bt]/[http]` 被改写为
  `[bt]/ttp]`（反向亦有），Edit 工具 old_str 永远匹配不上；工具输出显示的文件内容同样被
  反向改写，肉眼比对「看起来一致」但 `s.count(anchor)==0`。
- 处置：锚点字符串一律运行时拼接构造（如 `'[' + 'http' + ']'`），绕开输入改写；匹配失败时
  用 `unicode_escape` + `bytes.hex()` 对文件段与锚点做字节级比对裁决，不信任何一侧的显示
  文本（含 repr/unicode_escape 输出——输出同样可能被改写，bytes hex 才是终审）。
- 根因：chat 管道存在一层自动「纠错」改写，触发面包含方括号缩写序列；常规 ASCII 锚点
  （`| D64 |`、`Feature:` 行）不受影响，含 `[/]` 形态序列的才中招。

## bug17 轮：沙箱中途重置吞掉整个工作区——归档必须即时落盘
- 现象：bug17 轮 specifier 完成（归档已生成于主目录）+ coder 轮实现过半（bin 864P/0F）
  之际，沙箱文件系统整体重置：/home/z/work（工作区+target）、~/.cargo、主目录归档全部
  丢失，仅 /home/z/my-project 幸存。
- 处置：从 my-project 存活的上轮归档重建工作区；rustup/cargo/bb/APS 按既有口径前台重装
  （TUNA 30MB/s 最快）；规格补丁与代码补丁按上下文中的原始 patch 脚本精确重放；重放后
  全量验证重跑。
- 纪律：**每轮 `bin/swarm complete` 后立即把归档 tar.gz 复制进 /home/z/my-project/**，
  不等多轮攒批；长跑中途的关键产物（如已通过的测试基线记录）随 handoff/notes 落盘，
  不依赖工作区存续。

- bug18 轮：`[/]` 显示层改写与磁盘真实损坏的判别法——输出/回读中 `[h` 开头序列被吞（`[http]`→`ttp]`）是**显示陷阱**（bug16 轮登记的输入/输出双向改写），磁盘可完好；判别 = python bytes 统计 `count(b'[http]')` vs `count(b'ttp]')` 并检查每处 `ttp]` 前缀是否为 `[h`（独立 ttp] = 磁盘真实损坏）。本轮实证：01-config-template 的 ttp] 全为显示假象（磁盘 7 处 [http] 完好），01-persistence-config L195 存在 1 处历史写入侧真实损坏（`+ ttp] 11 键`，本轮修复）。写入含 `[h` 序列的新文本时仍用锚点拼接 + 写后 bytes 终审双保险。
- bug18 轮：gh-proxy.com 镜像复证（APS clone + babashka v1.13.229 静态包下载均秒过；GitHub 直连 GnuTLS -110 复现）——bb 装机口径 = releases 清单先查（资产名随版本漂移）→ gh-proxy 下载 → .tools/bin/bb + PATH。；bug19 轮：沙箱 PTY 对 \x7f（退格）连发投递上限 ≈2 字节/批（30 字节 burst 仅 2 事件达、8×30 分批仅 ~56/240，char 类 burst 无此限制 30/30 全达）——HEAD worktree 对照实验证伪本轮回归后定性环境预存；PTY 级验证脚本大段清空改「合法 URL 直接确认」绕开退格清空；脚本默认 target/release 路径未构建时子进程秒退 → 全屏空白全 FAIL 的假象（用显式路径传 debug 二进制）
- bug21 轮镜像复测（2026-11-03）：GitHub 直连 GnuTLS -110 复现，ghfast.top 前缀镜像本轮可用且快（swarm-ezr2 clone 秒过——更新旧「ghfast.top 不通」口径，各轮可达性漂移属实）；rustup 五镜 8MB 探针 rsproxy 16.7MB/s 最快但其 `/rustup/dist/` 清单第 4 次复现 NoSuchKey XML 错误页（测速快 ≠ 能装再证），TUNA 双层路径清单有效（813KB TOML）+ 8.7MB/s → 本轮 TUNA 整链路前台一次装齐 nightly minimal + rustfmt + clippy（EXIT=0）；USTC 本轮清单同为错误页。装前 curl 清单验 TOML 的纪律持续有效

## bug21 修订轮：chat 工具 Edit/MultiEdit「报失败但部分应用」陷阱
- 现象：MultiEdit 对 qa/08-modify-bt-qa.md 的批量编辑报「No replacement was performed」，
  但头两处编辑实际已落盘（工具声称原子，实测部分应用）；同轮 python 脚本锚点断言也因
  手误锚字失败一次（兑/兜形近字）。
- 处置：关键文档编辑后一律读回布尔探针（rg 关键新串）逐处核验，不信工具返回值；
  锚点复杂（表格长行 + 反引号 + `[/]` 形态序列）时直接走 python 行索引重写
  （bug16 轮「锚点运行时拼接」纪律的延伸——行首 `| QA-XX-NN |` 前缀定位最稳）；
  编辑失败时先 rg 查现状再决定补哪几处，勿盲目重放整批。
- 根因：chat 工具通道对含反引号包裹路径 + 中文括号长串的锚点匹配不稳定
  （与 bug16 轮双向改写同族：显示文本与匹配文本都不可全信，读回探针是终审）。

## bug23 轮镜像复测（2026-11-03）：rsproxy dist 清单本轮正常，直链二进制 + 双变量一次装齐
- 8MB 分段吞吐 rsproxy **16.9MB/s** 唯一真实可用（USTC/TUNA 本轮该路径 404——与前几轮
  结论互换，可达性逐轮漂移再证）；直链下载 rustup-init 二进制 + 官方双变量
  `RUSTUP_DIST_SERVER/RUSTUP_UPDATE_ROOT=https://rsproxy.cn` 前台 `-y --default-toolchain
  nightly --profile minimal -c rustfmt -c clippy` 一次装齐（rustup 1.29.2 + nightly
  1.101.0，EXIT=0），清单缺键问题本轮未复现。装前 curl 清单验 TOML 的纪律仍有效
  （本轮跳过直接成功属运气好，不改变纪律）。

## bug23/bug24 归档轮（specifier）
- chat 选择题面板通道恢复可用：Inversion 阶段 1 六题经面板逐题作答成功（bug13–18 轮「面板不可用」受限退化沿袭解除）——后续 specifier 轮先试面板逐题作答，仍不可用时再退化 chat 输入答案；面板单轮批量出题与「阶段 1 一次问全」口径天然吻合，操作者 chat 追加指令可即时覆盖面板答案（本轮「旧写法废弃」即如此，以 chat 指令为准）。
- babashka 资产名漂移实证：v1.12.209 的 linux-amd64 资产实名 = `babashka-1.12.209-linux-amd64-static.tar.gz`（旧名 .tar.xz 直接 404）——「releases 清单先查」条款的直接实例，清单页经 gh-proxy 同样可达。

## config 废弃轮镜像复测（2026-11-04）：USTC 单路径唯一完整可用
- 六镜 10s 吞吐探针：**USTC 13.6MB/s 唯一真实可用**（21MB 直链拉完，清单
  channel-rust-nightly.toml 900KB 正常 TOML、date 2026-11-03）；官方直连仅
  0.29MB/s；TUNA/rsproxy/aliyun/nju 单路径全 404（TUNA 双层路径陷阱、rsproxy
  `/rustup/dist/` 清单 404 第 6 次观测）——「测速快 ≠ 能装、装前 curl 清单」纪律
  持续有效。
- 本轮口径：USTC 直链 rustup-init + 双变量（RUSTUP_DIST_SERVER/UPDATE_ROOT =
  mirrors.ustc.edu.cn/rust-static）前台 `-y --profile minimal --default-toolchain
  nightly -c rustfmt -c clippy` 一次装齐（EXIT=0，cargo 1.101.0-nightly）；
  cargo registry 配 USTC sparse。可达性逐轮漂移再证，下轮仍需先探针。

标志列轮复测（2026-11-05）：api.github.com releases 清单直连可用（babashka v1.13.229 资产名
  babashka-1.13.229-linux-amd64-static.tar.gz 与 v1.12.209 同族——「清单先查」条款再证）；
  gh-proxy.com 前缀下载 bb 静态包 + clone APS 仓库秒级完成；bb + APS 按既有口径装 .tools/ 成功，
  gherkin-parser/dry-checker 三 feature 全跑通。
- **rustup 镜像复测（本轮）**：rsproxy.cn 可用且最快（rustup-init 直链两次 25.1/17.9MB/s，
  USTC 同轮 12.5/3.7MB/s，官方 static.rust-lang.org 20KB/s 超时）；官方 rustup-init.sh 的
  `RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup/rustup` 双层路径 404（报「installer not
  found」勿误判平台缺支持）——本轮正确组合 = 直链下载
  `https://rsproxy.cn/rustup/dist/x86_64-unknown-linux-gnu/rustup-init` 二进制 +
  `RUSTUP_DIST_SERVER=https://rsproxy.cn`（清单在 /dist/channel-*.toml）+
  `RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`（rustup-init 在 /rustup/dist/<arch>/），
  前台一次装齐 nightly minimal+rustfmt+clippy；cargo registry 沿 rsproxy-sparse。
- **lan_bench/proxy 端口段**：代理监听 17905、seeder 绑 127.0.0.2:17903、tracker 17901
  （only-static 模式，UDP tracker 不起——torrent announce-list 里的 udp:// 会失败退避，
  无碍），与 QA fixture 端口段不冲突。

## bug25 轮：分离长编译进程仍被回收、.work/session 中段消失、rustup 镜像新鲜度判据
- **现象/处置**：①nohup+setsid 双保险分离的 `cargo test --tests` 长编译进程两次被
  环境回收（log 0 字节、进程消失）——集成测试改前台分块（先 `--no-run` 编译、再逐组
  前台跑，单组 ≤2 分钟不触 600s 上限）。②`.work/session` 在会话中段消失（plan.md/
  rag/tmp 存活）——按宪法中断恢复口径凭 plan.md 勾选状态重 begin 同角色后 complete，
  产物无损；git stash pathspec（-- ezr-demo/ project/）不触 .work/，非 stash 所为。
  ③rustup 镜像「最快」判据必须含**新鲜度**：Aliyun rustup dist 实测最快（4.2MB/s）
  但快照停在 2026-05（nightly 1.98，滞后约半年，装旧 nightly 编译依赖有风险）；
  TUNA 新鲜（Last-Modified 2026-11-03，nightly 1.101.0 与历轮同版）1.2MB/s——
  装前 curl dist/channel-rust-nightly.toml 的 Last-Modified 先验，工具链走 TUNA、
  rustup-init 本体可走 Aliyun；镜像结论沿 environment.md 历轮口径继续漂移观察。

## cleaner 轮（2026-11-05）：cargo registry 镜像 404 挂死陷阱与 clippy metadata 噪音
- **TUNA sparse registry 对部分新发布 crate 404**：`windows_x86_64_gnu 0.52.6` 等新包
  TUNA 返回 404（未缓存），cargo 自动 fallback 直连 static.crates.io——沙箱内挂死
  （`cargo fetch` 570s 零 Downloaded 输出、无任何 spurious 重试日志）。判据：fetch/
  clippy 长时间停在 `Downloading crates ...` 且无进展行即此坑。处置：`~/.cargo/
  config.toml` 换 rsproxy.cn sparse（`sparse+https://rsproxy.cn/index/`，与 rustup
  镜像同源），fetch 一次过（单文件 1.5-2MB/s）。
- **source 切换后缓存目录隔离可复用**：cargo 按 `<registry-host>-<hash>` 分缓存目录
  （`.cargo/registry/cache/`），换源即"全量重下"假象——同内容 `.crate` 文件校验和
  一致，可直接 `cp` 复用（276 个/31MB 免重下，cargo 校验通过）。
- **clippy cargo 组 lint 的 metadata 失败噪音**：`clippy::multiple-crate-versions`
  等需读完整 cargo metadata；依赖图缺 crate（404/offline）时报 "could not read cargo
  metadata" 且把 lint 失败本身当警告输出（`generated 1 warning`、`-W clippy::cargo`
  help 尾注、无 `-->` 代码位置）——本轮 17 轮分块跑出的"警告"全是此坑，切 rsproxy +
  cargo fetch 补齐 12 个 crate 后复跑证实 **0 警告**（nightly 11-04 无漂移新 lint）。
  判据：警告无代码位置 + metadata 错误文案 = 环境噪音 ≠ 代码问题，先补全依赖再复跑。

architect 轮环境重建实录（2026-11-06，全新沙箱零工具链）：rustup 镜像测速（清单文件 channel-rust-nightly.toml 计速）TUNA 最快 7MB/s 且 200（rsproxy/USTC/SJTU 该路径 404）——**ghfast.top 取官方 rustup-init.sh + RUSTUP_DIST_SERVER/UPDATE_ROOT 双变量指 TUNA 前台装齐** nightly 1.101.0（minimal profile + rustfmt + clippy 组件），与 bug25 轮记录的 1.101 工具链版本带对齐；cargo registry 按 cleaner 轮口径直接写 rsproxy sparse（cargo fetch 一次过零 404，rsproxy dist 清单缺键问题未复现第 7 次）；本项目无 .cargo/config.toml 链接器配置，无 lld 桥接需求。沙箱 Write 工具限主目录 → swarm 工作区 /tmp 迁 /home/z/swarm-ezr2（bin/swarm 按 BASH_SOURCE 自定位解析路径，begin 后迁移会话无损，status 复核即可）。夜间默认 DEV/TEST DEBUG=0 + CARGO_INCREMENTAL=0 全程压盘，全量 bins+12 套件+clippy 后盘余 4.4G 无清理需求。
QA 轮（2026-11-06）补记：接手被中断会话先清残留进程——上一会话 `timeout 560 python3 suite_*.py` 形态的套件进程可与会话解耦存活（ps 复核 + kill；残留套件与新生 foreground 批并行会争 CPU/fixture 扭曲限速测速）；.work/tmp/qa-run/qa-dl-* qa-fix-* 每用例临时目录随用例结束可清（证据在 screen dump 与日志，不在临时 HOME），700MB 级可回收。TP 套件前台分批节奏实测：QA_TP_ONLY=1,2 ≈5.1min / =3,6 ≈5.1min / =4,5,7,8,9,10 ≈1min，各批独立 timeout 560 内安全；速度断言批必须串行跑（并行限速测量互相干扰）。磁盘：target/debug/build 可膨胀至 4.5G（deps+incremental 才 ~1.1G）、llvm-cov 目标目录另计 ~1.1G、coverage 手工管线需 ~2G 余量——coverage/DRY 步骤前 du 分层清理（demo target 683M / debug/build 4.5G 可安全删，binaries 是独立文件不受 deps 硬链删除影响——同 inode 硬链注意：deps 内同名测试二进制删除会连带顶层 binary 丢失，需要时 cargo build --bins 重建）。

## specifier 守卫轮复测（2026-11-06）：rustup 镜像整链路与 gh-proxy bb/APS 装机
- 全新沙箱工具链重建。镜像测速用**真实要下载的文件**做样本（rustup-init）且先探明各镜像布局：
  TUNA 16MB 样本 37.29MB/s（4MB 样本 19.54；同镜像大小样本差异可达 2 倍，选型以大样本为准）、
  rsproxy 22.53、ALIYUN 4.52、USTC 3.10、NJU/SJTU 不可用、官方 0.20。TUNA 整链路（init 双层
  路径 `/rustup/rustup/dist/…` + dist `/rustup/dist/` 清单 200 + 组件）直链 rustup-init + 双变量
  前台一次装齐 rustup 1.29.3 + nightly 1.101.0 minimal + rustfmt/clippy（与历轮版本带一致）。
- **布局陷阱反向形态（新）**：把 TUNA 的 `$server/rustup/dist/` 清单布局套到 rsproxy 上会误判
  「rsproxy 工具链分发路径不全」——rsproxy 的正确 DIST_SERVER = **裸根** `https://rsproxy.cn`
  （清单在 `/dist/channel-rust-nightly.toml`，本轮验证 200 且 TOML 新鲜 date 2026-11-06）。
  各镜像 DIST_SERVER 布局不同，验证清单前先按该镜像自己的口径定位根，勿跨镜像套布局
  （与「UPDATE_ROOT 双层路径 404」陷阱同族、方向相反）。
- GitHub release 资产直连 0.04MB/s（bb 静态包量级不可行）；NJU github-release 镜像无 babashka
  （404）；**gh-proxy.com 前缀依旧可用**（bb v1.13.229 静态包 25.3MB/54s + APS 仓库 clone 均成）；
  bb 装 `.tools/bin/bb`、APS 装 `.tools/Acceptance-Pipeline-Specification`，gherkin-parser/
  ir-dry-checker 对修订后 feature 跑通（parser 15 场景；dry-check 101 findings 全为中文分词
  假阳性——涉新场景的 33 条互为镜像臂/反义断言，按 advisory 人工复核裁决不改文本）。
- 本轮通道（zai-web IM）无 chat 选择题面板组件：Inversion 直接按受限退化执行（编号清单 +
  操作者单条消息回复 `1A 2A 3A 4A`），一轮收齐；bug23/24 轮「先试面板」口径适用于有面板
  组件的通道，无面板通道直接退化不属违规，受限项照常落盘 handoff。

## specifier 审批改判轮复测（2026-11-06，v1.46）
- 沙箱重置二度发生（会话开局工作区与 `~/.cargo`、`~/.rustup` 全空）：从 my-project 归档
  `tar -xzf` 原样重建工作区一次成功（project/ 267 文件 + bin/packs 全量，`bin/swarm status`
  正常）——bug17「每轮 complete 后归档即时复制 my-project」纪律的直接收益：**归档即备份**。
- 归档不含 `.tools/`（.gitignore 排除）：bb v1.13.229 + APS（github.com/unclebob/Acceptance-
  Pipeline-Specification）按 gh-proxy.com 前缀法重装秒级完成；gherkin-parser/dry-checker 复跑通过。
- **rustup dist 清单路径是架构无关的** `dist/channel-rust-nightly.toml`（架构信息在清单内条目、
  不在路径上）——探针路径带 $ARCH 时 8 镜像全 404 系自伤误判；修正后格局：TUNA 200/date 11-03、
  rsproxy 200/11-06、NJU 200/11-06、USTC/BFSU 404、SJTU 超时、Aliyun 200 但快照停 2026-05-29
  （新鲜度判据排除）。
- **清单内组件 URL 为上游绝对 URL**（`https://static.rust-lang.org/dist/<日期>/…`）——rustup
  安装时自动重写到 DIST_SERVER 前缀；手工 curl 测速须剥上游前缀按镜像布局重拼（不剥则 rsproxy
  422/404 假象）。
- **TUNA dist 组件单次 404 属瞬态**（同 URL 复验两次均 206、速度 37.76/27.49 MB/s）——「测速快
  ≠ 能装」的补充判据：**单次 404 先复验再弃选**；rsproxy dist 307 → CDN（lf3/lf9-static.rsproxy.cn）
  跟跳后 11.45 MB/s（rustup 原生跟跳不受影响）；NJU 8.53。终选 TUNA（init 36.00 MB/s + dist 大样本
  两连 27+ MB/s）前台装齐 nightly 1.101.0（与上轮版本带一致）。
- 审批改判流程先例：操作者 chat 审批即修订裁决（Inversion 不适用，沿 bug25 轮先例）；审批只给
  行为面时，机制面判别载体（本轮 = FR-01-22 三要素探测判同/异文件）由 specifier 合成裁决并随
  phase-01 v1.46 注记登记供否决——透明登记先于自行扩权。

## coder v1.46 轮镜像复测（2026-11-07）：TUNA 双层均最快整链路装齐
- 全新沙箱（~/.cargo、~/.rustup 全空）。rustup-init 直链探针：TUNA 双层路径 44.0MB/s > rsproxy 17.6 > USTC 10.4 > aliyun 8.9 > NJU 404。
- dist 清单（架构无关路径 dist/channel-rust-nightly.toml）：TUNA 200/date 2026-11-03、rsproxy 200/11-06、USTC 200/11-06、aliyun 200 但快照停 2026-05-29（新鲜度判据排除）。
- dist 组件（清单内 rustc tar.xz 剥上游前缀按镜像布局重拼）16MB range 探针：TUNA 56.8MB/s、rsproxy 5.4MB/s、USTC 403——**本轮终选 TUNA 双变量（DIST_SERVER/UPDATE_ROOT 均 = https://mirrors.tuna.tsinghua.edu.cn/rustup，init 双层路径 /rustup/rustup/dist/<arch>/）直链 + 前台 -y --profile minimal --default-toolchain nightly -c rustfmt -c clippy --no-modify-path 一次装齐**（rustup nightly 1.101.0 与历轮版本带一致，EXIT=0）。
- cargo registry 按 cleaner 轮口径直接写 rsproxy sparse（~/.cargo/config.toml）；bb v1.13.229 + APS 经 gh-proxy.com 前缀重装秒级；gherkin-parser 15 场景 exit 0 + dry-checker exit 0（108 findings 与规格轮复核记录同集）。

## bug27 清理轮镜像复测（2026-11-07）：rsproxy 官方双变量整链路最快 + 度量工具三件套装机
- 全新沙箱。8MB 探针：rsproxy `/rustup/dist/<arch>/rustup-init` 9.2MB/s（可用）、USTC 2.5MB/s、
  TUNA/aliyun/官方单路径 404。清单按镜像自身布局验证：rsproxy DIST_SERVER=裸根 `/dist/` TOML 有效
  （date 2026-11-07）、USTC `/rust-static/dist/` 200、rsproxy `/rustup/dist/` 清单 404（双层陷阱第 N 次）。
  **终选 rsproxy 官方双变量（DIST_SERVER=https://rsproxy.cn + UPDATE_ROOT=https://rsproxy.cn/rustup）**
  直链 rustup-init 二进制（改名陷阱：rustup-init 按程序名推断 proxy 名，文件名带 `.`/前缀会报
  `unknown proxy name`——落盘名必须恰为 rustup-init）+ 前台 `-y --profile minimal --default-toolchain
  nightly -c rustfmt -c clippy --no-modify-path` 一次装齐 nightly 1.101.0（与历轮版本带一致）。
  镜像变量必须写入每个新 shell（rustup component add 等后续命令同样走 dist server）。
- **度量工具三件套装机（cleaner 口径）**：cargo-llvm-cov 0.9.1 与 cargo-mutants 27.1.0 用预编译
  二进制（api.github.com 直连查资产清单 → gh-proxy.com 前缀下载 release tar.gz → 解包入
  ~/.local/bin，比 cargo install 编译快一个量级）；llvm-tools-preview 组件随 rustup 装（避开
  llvm-cov 首跑交互安装挂死陷阱）。**DRY 工具 = PMD CPD 7.29.0 `-l rust`**（pmd-dist bin.zip
  134MB 经 gh-proxy 取，装 ~/.tools-pmd；需 java 在位）：jscpd 5.4.2 的 npm 发行版 **rust
  tokenizer 缺失**——阳性对照（除函数名外逐字节相同的双 .rs 文件）rust 格式 0 命中、同内容
  .py 命中 1，判 tokenizer 缺失不可用；PMD CPD 同对照命中 5 行/46 token 克隆，为 Rust 项目
  可用 DRY 扫描器（`pmd cpd --dir src --language rust --minimum-tokens 70`；EXIT=4=有克隆属正常）。
- 清理轮全流程磁盘账：镜像装机+全量基线（bin+12 套件逐目标）+llvm-cov ×3 + 全量复验 ×2 后
  盘余 2.7G（72%）——llvm-cov 每轮 ~0.4G 插桩工件是增量大头；未到清理线（complete 自动清 target/）。
