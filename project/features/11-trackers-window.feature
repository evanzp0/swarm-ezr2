# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   11-trackers-window-01 t 键门控与拒绝臂（未选中 / HTTP 任务）
#   11-trackers-window-02 窗口形态与布局（94×22、qBt 式列结构、URL 换行、数据源真实 tracker 池状态）
#   11-trackers-window-03 状态列三态（工作中 / 失败退避 / 待运行）与 Peers/Seeds/消息 列
#   11-trackers-window-04 滚动与鼠标（滚动条、键盘/滚轮滚动、拖拽移动、点击外部无操作）
#   11-trackers-window-05 关闭通道与模态（t/Esc/q/Q/✗、浮层集合、Ctrl+C 直通）
#   11-trackers-window-06 t 页签切换废除与 tracker 协议白名单（页签键收敛 ]/[；v1.13 反转：ws(s) 同权入池，白名单外 scheme 三来源分流处置，FR-02-31 v1.15）
# 期号 02（phase-02，操作者 bug18 轮指令五项功能之四「't' 快捷键弹出 trackers 窗口」）。
# 需求依据 project/mission/phase-02.md FR-02-31 与 phase-02 D67。
# 原页签 t 切换方式废除（keys.rs cycle_filter 的 t/T 臂退役；]/[ 保留）；
# 只支持 http 与 udp tracker；wss://（及 ws://）不受支持：
#   引擎侧按非法 URL 口径拒绝入池（既有忽略 + 警告）；
#   对话框侧添加/修改 BT trackers 输入确认时拒绝（toast + 焦点回 trackers + 不落库）。
# v1.10（bug18 追轮，操作者三条 UI 反馈）：
#   窗口 74×16 → 90×22（更大）；列参考 qBittorrent Trackers 列：
#   # / URL / 状态 / Peers / Seeds / 消息（类型并入状态口径，不再单列）；
#   URL 超宽由截尾改为 36 列宽列内自动换行（续行仅 URL 列，参与滚动显示行口径）；
#   键盘 ↑↓/PgUp·PgDn/Home·End 与滚轮滚动、右缘滚动条按折行后显示行计数。
#   演示口径（ezr-demo）：新增 BT 任务未手输 trackers 时按「种子元数据自带」预置
#   4 条默认 tracker（http/https/udp）入库，保证 T 弹窗与面板内容一致非空。
# v1.11（bug20 追轮，操作者六条 UI 反馈）：
#   ①相邻明细行之间以 ─ 通栏细线分隔（首条前/末条后不画，同条折行续行间不画）；
#   ②URL 折行续行与首行同色（同 FG，不再降暗）；
#   ③http(s)/www URL 渲染层反终端自动链接（VS15 U+FE0E 零宽注入 + 相邻 span ±1
#     微差色交替，打断终端 hover 自动下划线识别；udp 行不注入；显示/列宽/滚动/
#     点击映射不变；同口径适用于添加/修改 BT 对话框 trackers 面板）；
#   ④内容区左右 padding 对称各 2 列（右缘 = 左缘）；
#   ⑤底部提示行全部删除（内容区延伸至底边框，可视行数 = 内框高 − 3）；
#   ⑥URL 列宽 36 → 44 列（状态/Peers/Seeds/消息 列右移：49/62/69/76，消息列
#     预算 8 列与右 padding 齐平）。
# v1.12（bug24 轮批准归档，specifier；D67⑤ v1.11 修订）：窗口 90×22 → 94×22（内框
#   92×20、内容区宽 88；列起点不变 #/URL 44/状态 49/Peers 62/Seeds 69/消息 76）；
#   末列「消息」预算 8 → 12 列——「下轮 Ns / 重试 NNNNs」3–6 位秒数完整显示，
#   无 '…' 截断（修复前缺陷：预算 8 列 < 「下轮 120s」9 列，三位秒数被截断）。
# v1.13（操作者修订轮指令「tracker 统一 + ws(s) 支持 + BT WebRTC + config 严格校验」；
#   phase-02 v1.15，FR-02-31 v1.15/D70；Inversion 阶段 1 受限退化 chat 输入选择题答案
#   1a/2b/3a/4a/5a，题目录重建登记见 phase-02 v1.15 注记与 handoff 待批）：
#   tracker 支持面反转——scheme 白名单扩为 {http, https, ws, wss, udp}（大小写不敏感），
#   ws:// 与 wss:// 同权入池照常显示与 announce（原「wss/ws 不支持」口径退役）；
#   白名单外 scheme 三来源分流处置：config trackers 键来源 = 启动失败并报错（FR-02-35）；
#   .torrent announce-list 来源 = 该行忽略 + 警告（非 config 条目不入 fail-fast 面）；
#   对话框手输来源 = 确认时 toast "⚠ 不支持的 tracker 协议（仅 http(s)/ws(s)/udp）：
#   <url 截尾>"（截尾 = 首 44 显示字符硬截尾）、焦点回 trackers、不落库。
#   旧口径「引擎按非法 URL 忽略 + 启动警告」仅对 announce-list 来源保留。
Feature: 11-trackers-window trackers 弹窗（t）

  期号 02。任务列表选中任务按 t 弹出与该任务关联的 trackers 窗口：只读查看
  该任务 tracker 池（URL / 状态 / Peers / Seeds / 消息）；编辑入口仍走 M 修改
  对话框（不新增）。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 BT fixture 已就绪（HTTP/UDP tracker + seeder peer + 预置 .torrent 文件目录 /tmp/qatorrents）
    And 终端尺寸 120×44

  Scenario: 11-trackers-window-01 t 键门控与拒绝臂
    When 任务列表按 t（含 T）
    Then 逐臂判定（顺序固定，toast 且弹窗不打开）：<arm>

    Examples:
      | arm                                                              |
      | 未选中任务 → toast "未选中任务，无法查看 trackers"                  |
      | 选中 HTTP 任务 → toast "该任务没有 trackers（仅 BT 任务支持）"      |
      | 选中 BT 任务（下载中/做种中/暂停均可）→ 打开 trackers 窗口          |

  Scenario: 11-trackers-window-02 窗口形态与布局
    Given 选中 BT 任务（tracker 池 = <n> 条，config/announce-list/手输三来源合并）
    When 按 t 打开 trackers 窗口
    Then 出现 94×22 圆角模态浮层居中显示、标题 " Trackers "、顶边框行右端 [✗]（v1.12：D67⑤ 修订，90×22 退役）
    And 布局 = 任务名行（白色加粗，超宽截尾）/ 表头行（# / URL / 状态 / Peers / Seeds / 消息）/ tracker 显示行（URL 折行后逐行占位）；底部无提示行（v1.11 全部删除，内容区延伸至底边框）；内容区左右 padding 对称各 2 列
    And 相邻 tracker 明细行之间以 ─ 通栏细线分隔（首条之前与末条之后不画，同一 tracker 的折行续行之间不画）
    And 每行首行 = #（右对齐 2 位）/ tracker URL（44 列宽列内自动换行，续行仅回填 URL 列，前缀列留空，续行与首行同色同 FG）/ 状态 / Peers / Seeds / 消息
    And 末列「消息」预算 12 列（起点 76，内容区宽 88 − 76，v1.12）：「下轮 <s>s」与「重试 <s>s」3–6 位秒数完整显示，无 '…' 截断（bug24）
    And http(s)/www 形态 URL 渲染层注入 VS15（U+FE0E 零宽）+ 相邻 span ±1 微差色交替——反终端 hover 自动下划线识别（udp 行不注入；显示文本、列宽/滚动/换行计算不变）
    And 显示行数（含折行续行与分隔细线）> 可视行数时右缘滚动条（FR-01-109 通用滚动条口径，滑块恒绿）

    Examples:
      | n  |
      | 3  |

  Scenario: 11-trackers-window-03 状态列三态与 Peers/Seeds/消息
    Given 选中 BT 任务运行中且其 tracker 池含 <kind> tracker
    When 按 t 打开 trackers 窗口
    Then 该行显示 状态 "<status>"（数据源 = 引擎 tracker 池真实状态，非伪造）
    And 消息列倒计时秒数（含 3 位及以上，如 120s / 3600s）在 12 列预算内完整显示无 '…' 截断（v1.12，bug24）

    Examples:
      | kind   | status                                        |
      | HTTP   | 工作中（Peers/Seeds 计数；消息 = 下轮 <s>s）    |
      | UDP    | 工作中（Peers/Seeds 计数；消息 = 下轮 <s>s）    |
      | HTTP   | 失败（Peers/Seeds = —；消息 = 重试 <s>s）      |
      | HTTP   | 待 announce（任务未运行；Peers/Seeds/消息 = —）|

  Scenario: 11-trackers-window-04 滚动与鼠标
    Given trackers 窗口打开且 tracker 显示行数（含 URL 折行续行）超一屏
    When ↑↓ 逐行滚动 / PgUp·PgDn 翻页 / Home·End 跳首尾 / 滚轮滚动（每滚 2 行）
    Then 视口滚动且行内容完整（选择无关——本窗口只读无行选择，滚动即视口滚动；折行续行、分隔细线与首行同为滚动单位）
    And 左键按住窗口空白处拖拽移动（位置钳制终端内、关闭不保留）；点击窗口外部 = 无任何操作

    Examples:
      | trackers |
      | 12       |

  Scenario: 11-trackers-window-05 关闭通道与模态
    Given trackers 窗口打开
    When 按 t / T / Esc / q / Q 或点击 [✗]
    Then 窗口关闭回主界面（q/Q 先关窗不退出；Ctrl+C 退出优先级不变）
    And 窗口打开期间底层面板降级暗框（FR-01-108 浮层集合含 trackers 窗口）、其余快捷键被窗口消费

  Scenario: 11-trackers-window-06 t 页签切换废除与 tracker 协议白名单（v1.13：ws(s) 入池，FR-02-31 v1.15）
    Given 主界面（无浮层）
    When 按 t/T
    Then 不再切换过滤页签（原 cycle_filter 的 t/T 臂退役；过滤页签切换仅 ]/[ 承担；帮助【页签】组 T 条目同步移除、帮助【任务列表】组新增 T 条目、帮助【trackers 弹窗（T）】组含列口径与滚动条目）
    And 任务 tracker 来源含 "<ws_url>"（ws:// 或 wss:// 形态，大小写不敏感）时同权入池并照常显示（v1.13 反转，原「wss/ws 拒入池」臂退役；窗口行、Peers/Seeds/消息列与非 ws 行同口径，状态真实数据源不变）
    And 来源含白名单外 scheme "<bad_url>"（ftp:// 形态）时三来源分流：config trackers 键来源 = 启动失败并报错（FR-02-35）；announce-list 来源 = 该行忽略 + 警告（trackers 窗口与任务 tracker 列表均不出现）；对话框手输来源 = 确认时 toast "⚠ 不支持的 tracker 协议（仅 http(s)/ws(s)/udp）：<url 截尾>"、焦点回 trackers、不落库

    Examples:
      | ws_url                   | bad_url                   |
      | wss://tracker.example/ws | ftp://tracker.example/ann |
      | ws://tracker.example/ws  | ftp://tracker.example/ann |
