# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   08-modify-bt-01 M 键分流与布局（BT 任务 → 修改任务：BT 78×26；HTTP 任务 → 原修改框；已完成拒绝；v1.5：+限速两字段）
#   08-modify-bt-02 预填（任务名静态行 / 文件勾选预填 / 并发 / 限速两字段 / 代理 / trackers 预填）
#   08-modify-bt-03 修改文件勾选确认（total 重算 / 已下载钳制 / 立即生效 toast / 续传口径）
#   08-modify-bt-04 确认校验（0 选中保留 toast）
#   08-modify-bt-05 并发/代理热调立即生效（v1.3 修订：引擎不重启；下调即时收缩同 FR-01-87 v1.16/D30）
#   08-modify-bt-06 修改 trackers 确认后立即生效（v1.2：热更新 tracker 池，无需停止再启动）
#   08-modify-bt-07 任意参数组合修改确认后立即生效且任务不中断（v1.3：FR-02-26 总规则）
#   08-modify-bt-08 修改限速两字段立即生效（v1.5 新增，FR-02-29/FR-02-26）
#   08-modify-bt-09 trackers scheme 校验确认拒绝不落库（v1.9.1：wss 等拒绝 + 焦点回 trackers）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-03/04/26。
# v1.4 修订（操作者 bug17 轮指令，FR-02-27 连接生命周期礼仪 + D66）：场景 05 补充收缩连接的
# 关闭礼仪判据 = 优雅关闭（对端 EOF 无 RST）+ 自关重拨冷却（不记对端失败、冷却内不重拨），
# 细则归口 features/09-bt-engine.feature 场景 15（FR-02-27），本文件只作路径级引用。
# v1.5 修订（操作者 bug18 轮指令五项功能之二「任务级上传/下载速度限制」；FR-02-29，D67）：
# ①修改任务：BT 对话框 78×24 → 78×26（布局 = 顶 padding / 任务名 / 文件列表框 / 并发 /
# 下载限速 / 上传限速 / 代理 / trackers / 按钮行 / 底 padding；两限速字段插在并发之后、
# 代理之前；焦点表 0–5 → 0–7）；②限速字段预填任务当前值（存量任务缺失 = 0）；输入接受
# 整数 B/s 或十进制单位字符串；留空 = 保持当前值；非法 = 不生效并聚焦提示；确认后立即生效
# （限速热调随 Reconfigure 载体，引擎不重启——08-modify-bt-08）。
# v1.3 修订（操作者 bug16 轮指令，FR-02-26 总规则 + D65）：修复前缺陷 = 并发/total 变化走引擎
# 全量重启（CancelBt+StartBt 拆除重建+启动重校验），感知为「改并发未生效、需暂停重启才生效」；
# 修订后 = 并发热调（引擎不重启、不重新排队、无启动重校验窗，上调/下调同 FR-01-87 v1.16/D30），
# 并新增场景 07（任意参数组合确认后立即生效且任务不中断，QA 严格检查）。
# UI 基线 = ezr-demo phase2 源码（specifier 逐条核证：app/dialogs.rs open_modify_bt_dialog /
# dlg_confirm_modify_bt、ui/dialogs.rs draw_bt_dialog ModifyBt 臂；焦点表 0–5、
# 按钮 [ 确定 ][ 取消 ]、toast 文案照录源码）。
# M 键守卫（D45）= state != Completed（demo 核证）——「做种中」BT 任务同样可打开修改；仅「已完成」拒绝。
Feature: 08-modify-bt 修改任务：BT 对话框

  期号 02。M 键对 BT 任务分流进入：任务名静态行 + 文件列表框（预填当前勾选）+ 并发 +
  代理 + trackers（预填任务 tracker 列表）；确认后文件选择/并发/代理立即生效；trackers
  修改亦立即生效（v1.2：确认后热更新引擎 tracker 池，无需停止再启动任务——修复前
  仅改模型、需停止再启动才生效的缺陷，操作者 bug11 轮指令）。
  v1.3 修订（操作者 bug16 轮指令，FR-02-26 任务窗口参数改动确认后立即生效总规则）：
  确认后全部参数热生效——引擎不重启（无全量重挂、无重新排队、无启动重校验窗），任务
  保持下载中/做种中；并发热调口径同 FR-01-87 v1.16/D30。修复前缺陷：并发/total 变化
  走引擎全量重启（拆除重建），感知为「改并发未生效、需暂停重启才生效」。

  Background:
    Given ezr 以干净环境启动
    And 本地 BT fixture 已就绪（HTTP/UDP tracker + seeder peer）
    And 已有一个下载中的多文件 BT 任务（.torrent 添加、共 <total_files> 个文件、已完成部分 piece）

  Scenario: 08-modify-bt-01 M 键分流与布局
    When 选中 BT 任务按 M
    Then 打开 " 修改任务：BT " 对话框（78×26 圆角模态、居中、[✗] 关闭按钮；v1.5/FR-02-29：78×24 → 78×26 新增限速两字段）而非「修改任务：HTTP(S)」
    And 布局自上而下 = 顶 padding 1 行 / 任务 「name」静态行 / 文件列表框（高 8 = 边框 2 + 6 可视行）/ 并发 / 下载限速 / 上传限速 / 代理 / trackers 输入框（5 可视行）/ 按钮前空行 / 按钮行 / 底 padding
    And 按钮行 = "[ 确定 ]  [ 取消 ]"、初始焦点在文件列表（焦点表 0–7：文件列表/并发/下载限速/上传限速/代理/trackers/确定/取消）
    And 文件列表框标题 " 选择下载文件 "、右上角 "[全选]" "[全不选]"、右下角统计 "已选 <k> 个文件，共 <z>"、树交互与添加对话框完全同口径
    When 选中 HTTP 任务按 M
    Then 打开的仍是「修改任务：HTTP(S)」对话框（原口径不变）
    When 选中已完成任务按 M
    Then toast 显示 "⚠ 已完成任务不可修改"（无对话框打开）
    When 选中做种中 BT 任务按 M（D45：守卫 = state != Completed，demo open_modify_dialog 核证）
    Then 正常打开 " 修改任务：BT " 对话框（做种中可修改文件勾选/并发/代理；不改做种计时）

    Examples:
      | total_files |
      | 9           |

  Scenario: 08-modify-bt-02 预填
    When 打开「修改任务：BT」对话框
    Then 任务名静态行显示 "任务 「<name>」"（白色加粗、超宽截断）
    And 文件列表框预填任务当前文件勾选状态（选中 = [x]、未选 = [ ]；按扁平 BtFile 重建树、目录默认收拢）
    And 并发预填任务当前并发值、下载限速/上传限速预填任务当前任务级限速值（整数 B/s；存量任务缺失按 0）、代理预填当前代理（不在选项表内时追加兜底项）
    And trackers 输入框预填任务 tracker 列表（每行一条，含 config 默认与来源解析与手输合并结果）
    And http(s)/www 形态 URL 行渲染层反终端自动链接（VS15 U+FE0E 零宽注入 + 相邻 span ±1 微差色交替——打断终端 hover 自动下划线识别；udp 行不注入；仅影响显示，不改缓冲内容与光标/点击映射、列宽与滚动计算；bug20 追轮，与 trackers 弹窗 URL 列同口径）

    Examples:
      | name                         |
      | ubuntu-24.04.2-desktop-amd64 |

  Scenario: 08-modify-bt-03 修改文件勾选确认
    When 取消 <cancel> 个未完成文件的勾选后按 Enter 确认
    Then 对话框关闭且 toast 逐字显示 "✓ BT 任务已更新：<k>/<n> 文件 · 共 <size>（立即生效）"
    And 任务 total 重算为选中文件大小合计、已下载量钳制 ≤ 新 total、列表进度即时更新
    And 取消选中的文件数据不再推进（未完成部分退出下载缺口）
    When 重新打开修改对话框勾回该文件后确认
    Then 该文件重新纳入下载缺口并继续下载（断点保留：已落盘 piece 不重下）

    Examples:
      | cancel | k | n |
      | 1      | 7 | 9 |

  Scenario: 08-modify-bt-04 确认校验
    When 取消全部文件勾选后按 Enter 确认
    Then toast 显示 "⚠ 请至少保留一个要下载的文件（空格勾选）"、焦点回文件列表、对话框未关闭
    And 按 Esc 或 [✗] 或 [ 取消 ] 关闭时任务保持原状（无任何修改生效）

  Scenario: 08-modify-bt-05 并发/代理热调立即生效（v1.3 修订：引擎不重启）
    Given 任务下载中且多连接在传
    When 把并发改为 "<conns>" 并切换代理为 "<proxy>" 后按 Enter 确认
    Then 并发/代理立即生效且任务保持下载中不中断：引擎不重启（无全量重挂/重新排队/
      启动重校验窗，进度与速度不闪断归零）、既有连接不被整体拆除
    And 上调（<conns> > 原并发）：新连接在下一调度周期加入至新并发数
    And 下调（<conns> < 原并发）：超额连接即时收缩（在传块请求立即中断、块内已写字节保留、
      约 1 进度帧内连接行消失，同 FR-01-87 v1.16/D30 口径；分块进度不减、已下载字节单调不减）
    And 代理切换立即对新发起的连接生效（在途连接按旧代理完成，同 FR-01-87/D19 口径）
    And 头部「并发」与并发连接面板徽标随新并发数刷新
    And 下调收缩的连接按连接生命周期礼仪收尾（v1.4/FR-02-27）：对端读到 EOF 的优雅关闭
      （无 ConnectionReset/RST）、被收缩候选不计对端失败且自关冷却内不重拨该候选
      （细则与严格检查归口 09-bt-engine-15 / QA-BE-15）

    Examples:
      | conns | proxy |
      | 8     | 直连   |

  Scenario: 08-modify-bt-06 修改 trackers 确认后立即生效（v1.2）
    Given 任务下载中且任务 tracker 池含 tracker-A（fixture 日志可观察其周期 announce）
    When 按 M 打开修改对话框，在 trackers 输入框追加一行 tracker-B（另一台 fixture tracker）
      并删除一行 tracker-C（fixture tracker）后按 Enter 确认
    Then 对话框关闭且 toast 照旧显示 "✓ BT 任务已更新：<k>/<n> 文件 · 共 <size>（立即生效）"
    And 任务保持下载中不中断：已建立连接、进度与速度不受影响（不重挂引擎、无重新排队、
      并发明细不闪断）
    And tracker-B 立即被 announce（fixture 日志可证：确认后收到该任务 info_hash 的 announce，
      无需停止再启动）且其返回 peers 纳入候选
    And tracker-C 不再收到后续 announce（fixture 日志停止增长）
    And 修改对话框重开时 trackers 预填 = 确认后的新列表（修改结果持久）
    When 做种中任务同样修改 trackers 确认
    Then 同口径立即生效（做种 announce 按新列表继续）

  # v1.3 新增（操作者 bug16 轮指令，FR-02-26 总规则 + D65）：任意参数组合修改确认后立即
  # 生效且任务不中断——本场景为组合严格检查口径：任何参数组合都不得触发引擎全量重启，
  # 多项修改互不阻塞、互不重置对方已生效状态；QA 套件按本场景做严格检查（QA-MB-07）。
  Scenario: 08-modify-bt-07 任意参数组合修改确认后立即生效且任务不中断（v1.3）
    Given 任务下载中且多连接在传（观察窗 ≥ 10 进度帧）
    When 在同一确认中同时修改：文件勾选（勾回与取消各 1 个）、并发改为 24、下载限速改为 2 MB/s、
      代理切换为直连、
      trackers 追加 1 条并删除 1 条，后按 Enter 确认
    Then 场景 03/05/06/08 的全部判据在同一确认中同时成立（文件缺口调整与 total 重算、
      并发热调上调/下调、代理新连接生效、tracker 池热更新——新增 tracker 立即 announce、
      被删 tracker 停止 announce），且各项互不阻塞、互不重置对方已生效状态
    And 任务保持下载中不中断：引擎不重启（判据同场景 05：无重新排队、无启动重校验窗、
      进度与速度不闪断归零、既有连接不整体拆除）


  # v1.5 新增（操作者 bug18 轮指令，FR-02-29/FR-02-26 + D67）：修改限速两字段立即生效。
  Scenario: 08-modify-bt-08 修改限速两字段立即生效（v1.5，FR-02-29/FR-02-26）
    Given 任务下载中（或做种中）且多连接在传
    When 按 M 将下载限速/上传限速改为 <after> 并确认
    Then 引擎不重启（无 CancelBt+StartBt、无重新排队、无启动重校验窗），任务保持运行
    And 下载速度/上传速度（做种与互惠合计）约一进度帧内向 <after> 收敛（与全局取 min 合成，01-throttle-proxy-12）
    And toast 沿既有 "✓ BT 任务已更新…（立即生效）" 口径且注册表落盘（重开对话框预填新值）
    When 限速字段输入非法值（如 "abc"）后确认
    Then 不生效并聚焦非法字段提示（不关闭对话框）

    Examples:
      | after       |
      | 1 MB/s / 0  |
      | 0 / 1 MB/s  |


  # v1.9.1 新增（操作者 bug18 追轮 UI 反馈；FR-02-31）：trackers scheme 校验拒绝不落库。
  Scenario: 08-modify-bt-09 trackers scheme 校验确认拒绝不落库（v1.9.1，FR-02-31）
    Given 任务 tracker 池 <before> 条（全部 http/https/udp）
    When 按 M 在 trackers 追加 <input> 行并按 Enter 确认
    Then toast "⚠ 不支持的 tracker 协议（仅 http/https/udp）：<url 截尾>"、焦点回 trackers、对话框保持打开
    And 任务 tracker 池保持 <before> 条不变（不落库、引擎池无变更、无 announce 变化；修改面板重开预填原 <before> 条）

    Examples:
      | before | input                    |
      | 2      | wss://tracker.example/ws |
