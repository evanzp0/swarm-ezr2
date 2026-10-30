# QA 套件 · 02-named-proxy 命名代理（http/https/socks5）与任务级选择（期号 01 · v1.9/FR-01-86/89/90/93）

> 对应规格：`project/features/02-named-proxy.feature`（18 场景）。
> 端到端 UI 层验证：对话框交互 + fixture 代理日志观察（ezr-proxy `--name` 多实例 +
> `--log` jsonl 逐请求记录）+ 注册表文件观察；不使用项目内部 API。
> （v1.6：旧全局 proxy 键退役（FR-01-90）；新增 type 类型与校验用例（FR-01-89）。
>  v1.7：D24 改判——type 收为 http/socks5 两值（旧 https 收编 http）、矛盾/未知不再作废
>  （以 url 为准 + 警告）；NP-08..10 同步重写。
>  v1.8：FR-01-93 凭证按类型分级——socks5 必填 / http 可选（缺省匿名，半填作废）；
>  新增 NP-11..15；socks5 认证生效判定 = RFC 1929 user/pass 握手（引擎经代理 url
>  userinfo 传递，`Proxy::basic_auth` 对 socks 代理无效的 v1.5 缺陷一并修复）。
>  v1.9：条目字段重构（name/type/ip/port/username/password），url 键退役——NP-01/03/04/07
>  用例字段形态机械同步（ip/port），NP-08..10 重写（type 三值枚举 / type 缺失作废 / type
>  未知作废；内部 url 由 type+ip+port 构造的正确性以「连接以 type 对应协议形态抵达
>  ip:port」为判定），NP-13..15 补 https 相位（https 凭证规则与 http 同级），新增 NP-16..18
>  （ip 校验 / port 校验 / IPv6 字面量）；QA-NP-01..18 连续编号。）
>  v140（桩扩展轮）：ezr-proxy 补齐 https（代理自身 TLS，内嵌自签证书）/ socks5
>  （RFC 1929 强制认证）/ 认证强制 http（407 质询）与 --bind IPv6 监听；NP-04/08/13/15/18
>  下载相位 S→实跑（NP-04 增 407 质询负例、NP-15 增错凭证负例；https 相位证书信任经
>  SSL_CERT_FILE 注入，产品零改动）；环境前置与判据同步改写，无常设 S。）
>  v1.10（config 三大标准表重构）：键名 proxies → proxy 迁入 [common] 表，即 [[common.proxy]]——
>  全部用例的配置写入位置机械同步（[[proxies]] → [[common.proxy]]，条目字段/校验语义不变）；
>  旧 [[proxies]] 表静默忽略的判据由 `qa/01-persistence-config-qa.md` QA-PC-12 承载。

## 环境前置

- fixture 服务器 + ezr-proxy 多实例（`--name proxy-a --log proxy-a.jsonl` 等；含需认证实例）。
- ezr-proxy 支持 http / https（代理自身 TLS 监听，内嵌自签证书）/ socks5（RFC 1929
  user/pass）三形态监听（QA-NP-08/13/15 用；v140 桩扩展交付）。
- https 相位证书信任：套件以 `SSL_CERT_FILE` 指向桩内嵌证书的 PEM 形态
  （`ezr/src/bin/ezr-proxy-certs/ca.pem`）注入产品进程（reqwest
  rustls-tls-native-roots 经 rustls-native-certs 0.8 / openssl-probe 读取；SAN 覆盖
  IP 127.0.0.1 / IP ::1 / DNS localhost）。产品零改动。
- QA-NP-15 的 socks5 监听为**强制认证**形态（无/错凭证回 0x01 拒绝）；https 认证监听
  为 TLS 会话内 basic auth 强制形态（无/错凭证回 407 质询）；NP-11/12 的 socks5 形态
  仅作配置合法性背景，条目作废后无连接尝试。
- QA-NP-18 需 IPv6 回环（`::1`）监听的 fixture http 代理（ezr-proxy `--bind ::1`，
  v140 起支持）。
- 独立 HOME（每用例独立沙箱目录）；进程管理工具（pgrep/pkill）。
- 日志断言工具（jq/grep 对 jsonl 按目标 host/port 过滤，注意排除探活噪声）。
- v1.5 形态注册表 fixture（QA-NP-06 用；任务代理字段为缺省或 "global" 值）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-NP-01 | 02-named-proxy-01 | 配置 3 个命名代理（`[[common.proxy]]`，name/type/ip/port 各异，其一含认证）打开添加对话框展开代理下拉 | 选项 = 直连 + 3 命名项（按配置顺序）；每项按「名（type）」显示类型标注（三值形态）；选项文案不含账号密码 |
| QA-NP-02 | 02-named-proxy-02 | 任务 A→proxy-a、B→proxy-b、C→直连并行下载 | proxy-a/b 日志各记录对应任务请求、互不串线、均无 C；三任务完成且文件正确 |
| QA-NP-03 | 02-named-proxy-03 | 配置重名/空名条目 + 一个合法条目 | 非法条目不在下拉；合法在位；启动 toast 一次；启动与下载不受阻 |
| QA-NP-04 | 02-named-proxy-04 | http 型认证代理（ip/port + 成对凭证，v140 桩为强制认证形态）下载；桩级探针负例 + 全界面/日志全文检索 | 桩侧：无凭证/错凭证 → 407 + `Proxy-Authenticate: Basic` 质询、正凭证放行；产品：认证通过下载成功；界面与日志均无 username/password 明文（含 407/放行事件记录） |
| QA-NP-05 | 02-named-proxy-05 | 会话 1 引用 gone-proxy 存盘 → 会话 2 删除该条目续传 | 直连完成；toast 提醒一次（引用不存在已按直连） |
| QA-NP-06 | 02-named-proxy-06 | 旧形态注册表（缺省/"global"）启动续传 | 直连完成（旧 "global" 值映射直连，FR-01-90）；不崩溃 |
| QA-NP-07 | 02-named-proxy-07 | 命名代理 ip = "bad host"（无法与 port 构成合法代理地址）添加下载 | 使用该任务时 toast 一次（地址无效已按直连）；直连完成；启动不崩溃 |
| QA-NP-08 | 02-named-proxy-08 | 分别配置 type=http / https / socks5 三代理（ip=127.0.0.1，port 各异）各下载 | 各连接以 type 对应协议形态抵达对应 port（http = 代理请求、https = 代理自身 TLS 会话内 http 形态、socks5 = RFC 1929 握手 + 隧道，日志在案）；下拉按「名（type）」标注；三任务完成且文件正确 |
| QA-NP-09 | 02-named-proxy-09 | 配置条目不写 type 键（ip/port 在位）+ 合法条目 | 条目不在下拉（type 必填，缺失即作废，无从推断）；启动 toast 警告一次；合法条目可用且下载成功 |
| QA-NP-10 | 02-named-proxy-10 | 配置 type="ftp"（三值之外未知值）+ 合法条目 | 条目不在下拉（type 未知即作废）；启动 toast 警告一次；合法条目可用且下载成功 |
| QA-NP-11 | 02-named-proxy-11 | socks5 条目缺 username（password 已填）+ 合法条目 | 作废条目不在下拉；启动 toast 警告一次（凭证缺失被忽略，文案不含凭证值）；合法条目可用且下载成功；启动与下载不受阻 |
| QA-NP-12 | 02-named-proxy-12 | socks5 条目缺 password（username 已填）+ 合法条目 | 同 QA-NP-11 判据（缺 password 即作废） |
| QA-NP-13 | 02-named-proxy-13 | http 与 https 条目均不配 username/password（v140：https 相位经 TLS 监听实跑），添加任务选之下载 | 两相位均无凭证相关警告（匿名合法）；请求经对应代理（日志在案）且代理侧无认证记录；下载成功 |
| QA-NP-14 | 02-named-proxy-14 | http 与 https 条目各跑「只填 username / 只填 password」两相位（4 行参数）+ 合法条目 | 各相位条目不在下拉；启动 toast 警告一次（username 与 password 需成对配置，文案不含凭证值）；合法条目可用且下载成功 |
| QA-NP-15 | 02-named-proxy-15 | socks5 与 https 条目各带正确 username/password（v140：两形态监听实跑），添加任务选之下载 | 各条目在下拉（类型标注与值一致，选项文案不含凭证）；下载成功且认证按类型生效（socks5 = RFC 1929 user/pass 握手、https = TLS 会话内 basic auth，代理侧认证日志可证）；桩级负例：socks5 错凭证 0x01、https 错凭证 407；界面与 ezr 日志无凭证明文 |
| QA-NP-16 | 02-named-proxy-16 | 配置 ip 键缺失 / ip 为空字符串 / ip 仅空白字符（三行参数各跑一轮）+ 合法条目 | 各相位条目不在下拉（ip 必填非空，缺失或空白即作废）；启动 toast 警告一次；合法条目可用且下载成功；启动与下载不受阻 |
| QA-NP-17 | 02-named-proxy-17 | 配置 port 键缺失 / port 非整数 / port=0 / port=65536 / port=-1（各行参数各跑一轮）+ 合法条目 | 各相位条目不在下拉（port 须为 1..=65535 内整数，缺失/非整数/越界即作废）；启动 toast 警告一次；合法条目可用且下载成功 |
| QA-NP-18 | 02-named-proxy-18 | 配置 ip="::1"（IPv6 回环 fixture http 代理，v140 桩 `--bind ::1`）type=http，添加任务下载 | 条目正常加载（下拉「s6-proxy（http）」）；下载成功且经该 IPv6 代理（日志可证，内部 url 方括号构造对用户透明）；无因 ip 含冒号的异常文案或崩溃 |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据（v140 起 NP-04/08/13/15/18 下载相位实跑，无常设
  S；如遇环境异常按实况登记并说明）。
- 代理归属判定以 jsonl 日志逐请求核对为准；并发场景允许探活噪声，按目标端口+时间窗过滤。
- v1.9 起 type 为唯一事实来源、内部代理 url 由 type+ip+port 构造：NP-08 判定「连接以 type
  对应协议形态抵达 ip:port」（代理协议与下载目标协议正交——三类型均须能服务 http 与 https
  下载目标）；NP-09/10/16/17 作废条目判定 = 不在下拉 + 启动警告一次 + 引擎无该条目连接尝试。
- 认证信息不泄漏口径沿原 01-throttle-proxy-11（界面、toast、ezr 日志、代理日志全查）；
  v1.8 起 NP-11..15 的警告/提示文案一并纳入凭证不回显检查（FR-01-93）。
- socks5 经手判定：ezr-proxy socks5 形态日志记录 CONNECT 请求目标（域名/端口）；
  v1.8 认证生效判定：socks5 形态需认证监听侧记录 user/pass 子协商成功（RFC 1929），
  未认证连接被拒（匿名 socks5 不放行，FR-01-93）；v1.9 https 型认证判定：TLS 解密后记录
  Proxy-Authorization: Basic（basic auth 在 TLS 会话内生效，FR-01-93 ②）。
- NP-09/10/11/12/14/16/17 作废条目判定补充：引擎无该条目的连接尝试（无以作废条目为代理的
  请求记录），且启动与合法条目下载不受阻（容错口径同 FR-01-71）。

## 套件脚本

- 可执行套件：`project/qa/runners/suite_named_proxy.py`（QA 会话落，端到端 UI 层；`python3 suite_named_proxy.py` 于 runners 目录执行，依赖 `harness.py` 共享基建与 pyte/wcwidth）。
