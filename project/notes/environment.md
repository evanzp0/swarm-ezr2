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
