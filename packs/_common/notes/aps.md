# APS 验收流水线与 Babashka 专项注意事项

> 用途分类：APS（Acceptance-Pipeline-Specification）工具与 Babashka 运行时的专项
> 问题与解法（会话复盘沉淀，操作者维护）。调用约定、安装位置与受限项规则等通用条款
> 见 `packs/_common/engineering.md`「验收流水线（APS）」「工具启动」。

- **dry-checker 对中文 feature 文本的大批 findings 是分词假阳性**：gherkin-ir-dry-checker
  按字母数字 token 计算 Jaccard 相似度，中文步骤文本退化为空 token 集（score=1），
  near-duplicate / possible-synonym 因此大面积误报。按其 spec「findings 为 advisory、
  synonym 须人工复核后处置」的规则逐条人工复核后判定为假阳性、不修改规格文本即可；
  不要据此改写中文规格或绕开 dry-check。
- **IR 场景行数与 feature 场景名计数口径不同**：gherkin-parser 的 IR 把 Scenario
  Outline 按 Examples 行展开，IR 场景行数 > feature 内场景名个数——对账 pass/skip
  增量时先统一口径（执行行数 vs 场景数），差异属展开口径而非丢场景。
- **分块跑块的 IR 子集目录是会话级产物**：`.work/tmp/aps-*` 跑块目录随会话清除，
  每次新会话需从 `parse.sh` 全量 IR 重建（按 feature 序号取子集拼块）；块目录名与
  分块脚本内引用名要一致，断点续跑只补未完成块。
- **沙箱后台进程不跨工具调用存活，验收块必须前台逐块跑**：setsid/nohup/disown 均
  逃不过会话回收（实测探针分钟级即死）；块跑一半无声消失时优先怀疑此因。网关
  「context canceled」只代表调用回报丢失，runner 常已完赛——以日志 summary 与进程
  表核实现场再决定续跑，勿盲目整块重跑。注意网关故障有两种后果需区分：仅回报
  丢失（日志完整含 `== summary ==`，勿重跑）与前台 shell 被回收连带杀死 runner
  （日志截断无 summary，需补跑该轮；若死在场景中逃还会留下孤儿 daemon——
  `pgrep -af 'ezr __daemon'` 是被杀中逃的铁证，续跑前先清场删 socket）。
- **清场 pgrep/pkill 模式必须匹配真实命令行**：daemon 实例命令行是
  `<bin>/ezr __daemon-worker`（不含 `ezr-daemon` 字样），按 `ezr-daemon` 模式清场
  必漏；判定实例用 `pgrep -af ezr` + 固定 socket（`$HOME/.ezr/ezr.sock`）探测。
  `ezr run` 的实例守卫与 spawn 存在竞态：可能打印「已在运行」却仍拉起 worker——
  协议冒烟类「daemon 应已在运行」的脚本，socket 在位就直接跑，不要依据 ezr run
  的返回信息判断。
- **OS 级 IPC 验收的环境判据（第二用户不可得口径）**：判据三连——`id -u` 非 0、
  `sudo -n true` 失败（需密码）、无 newuidmap（unshare 仅单行映射自身 euid，kuid
  恒等于 socket 属主）⇒ 无法构造另一内核用户的真实 connect。`@requires-second-user`
  场景按 QA 通过准则走 `setup:` 受限门记 S，Skip 文案内嵌环境判据（可审计、可迁移：
  有 root/newuidmap 的环境可转实跑）。
- **UDS 全监听扫描双层口径（f03-04 先例）**：ss 主路径（`-tlnp`/`-ulnp` 按 pid 列
  提取）+ `/proc/net/{tcp,tcp6,udp,udp6}` 与 `/proc/<pid>/fd` 的 `socket:[inode]`
  交集兜底（确定性、不依赖外部工具）；BT/DHT 同端口口径下断言「daemon 名下端口
  集合 ⊆ {bt_listen_port}」+ UDS 连通正对照，构成「无客户端通信 IP 监听」的
  可证伪断言。
- **f16-01#row0 fixture 竞态先例（digest_form HTTP 分支）**：wait_for_status 首见
  downloading 可早于 progress_map.unit 物化（首个分片登记前）→ 形态断言采到空
  unit 假 FAIL。修法与 f17-10 同口径：断言前加有界轮询等稳定态（≤10s @250ms），
  离开目标态/超时交由原断言按真实形态报错——产品零改动，均为 fixture 侧。
- **「中性 tracker slot 未武装返回 127.0.0.1:0 无失败路径」是反模式（f17-10 E-r1 实锤）**：
  ezr 引擎 run_download 是单轮 peer 获取——死 peer（含 port-0 拨号即拒、无重试）使
  worker 即刻退出，join 收尾 `incomplete swarm: 0/N` → 任务毫秒级自败；fixture 的
  stop 与该自败竞速（引擎调度 tick 先插入则自败先发）→ 「停止落 paused」类场景偶发
  假 FAIL，且 error 字段是「daemon 内部错误」极具误导性。修法：中性窗改 sinkhole
  peer（只绑不答的 listener）——拨入成功、握手 reply 无超时挂起，worker 必活至
  kill9，stop 确定性胜出；Given 收尾再重武装真 seeder。凡「构造无 peer 停止态」
  的 fixture 均适用，不要依赖空 peer/死 peer 让任务「稳定停留 downloading」。
- **bb 任务调用式（APS gherkin-parser 实跑配方）**：`.tools/Acceptance-Pipeline-Specification`
  内 `bb` 是源码目录不是二进制，可执行 bb 在 PATH（/usr/local/bin/bb）；任务表在根 `bb.edn`
  （`gherkin-parser`/`gherkin-ir-dry-checker`/`gherkin-mutator`）。正确调用 = **cwd 切到 APS 根
  目录用 `bb run <task> <in> <out>`**（bb 靠 cwd 探测 bb.edn；`bb --project <dir> run ...` 在该版
  本会把 `--project` 当任务参数漏进 CLI 报 File does not exist）。解析器要求恰好 2 参数
  （feature 路径、JSON 输出路径），异常退出码 2=参数错、1=解析失败，输出为 JSON（不是 EDN）。
