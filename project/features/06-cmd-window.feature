# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   06-cmd-window-01 命令窗口打开与布局关闭
#   06-cmd-window-02 Enter 确认分流（http URL / 本地 torrent 路径 / 空输入 / 多行取首行）
#   06-cmd-window-03 换行与行数上限（Shift+Enter / Alt+Enter；64 行上限）
#   06-cmd-window-04 Ctrl+Enter / Ctrl+J / [确定] 按钮确认与焦点循环
#   06-cmd-window-05 Shift+Tab 本地路径补全（唯一 / 目录尾随 / 多候选 / 无匹配 / 非 path token / ~ 展开）
#   06-cmd-window-06 多行编辑与鼠标（可视行导航列保持 / 点击定位 / 滚轮 / 粘贴分行）
#   06-cmd-window-07 入口分流与文案（A 键 = 命令窗口；页脚 A 命令 chip；空态提示）
#   06-cmd-window-08 错误窗口（非 http 且非 .torrent path / .torrent 解析失败 → 66×7 错误浮层；磁力链拒绝文案）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-01/04/05、D44/D53/D55/D56/D59。
# UI 基线 = ezr-demo phase2 源码（specifier 逐条核证：app/cmd.rs / ui/cmd.rs / multiline.rs /
# app/keys.rs / ui/popups.rs；78×16、9 可视行、按钮行 = 内框底 −2 无空行——设计文档
# 「与文本框间空 1 行」与源码不符，以源码为准）。
# 分流三分（D44）：http → HTTP 框、.torrent path 解析成功 → BT 框、
# 其余（含磁力链）→ 关窗 + 「错误」窗口；.torrent 解析失败 → 错误窗口（不再开 BT 框）。
# 帮助【命令窗口】「输入内容」条目 = http(s) URL / 本地 .torrent 路径（D59 去磁力）。
Feature: 06-cmd-window 命令窗口（A 键新入口）

  期号 02。A 键与页脚 A chip 打开命令窗口：大多行文本框 + [确定] 按钮；
  Enter 确认三分流（D44）——http(s) URL → 「添加任务：HTTP(S)」；本地 .torrent 路径
  解析成功 → 「添加任务：BT」并预填种子来源、解析失败 → 「错误」窗口；其余任何输入
  （含磁力链，D53 phase2 不处理磁力链）→ 关窗 + 「错误」窗口；Shift+Tab readline 式路径补全。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 终端尺寸 120×44（使命令窗口 78×16 完整居中显示）

  Scenario: 06-cmd-window-01 命令窗口打开与布局关闭
    When 按 A 打开命令窗口
    Then 出现 78×16 圆角模态浮层居中显示、标题 " 请输入命令或链接 "（旧标题「命令窗口」退役不出现）
    And 内部大多行文本框宽 72（x = 内框 + 2）高 11（边框 2 + 9 可视行）、无小标题、空缓冲无任何占位提示
    And 文本框下方紧随水平居中的 "[ 确定 ]" 按钮（按钮行 = 内框底 − 2，文本框与按钮间无空行）、底部留 1 行 padding
    And 顶边框行右端有 [✗] 关闭按钮（悬停红加亮）
    When 按 Esc 关闭后再按页脚 chip "A 命令" 打开
    Then 命令窗口再次打开且缓冲已清空（关闭即清，位置不跨次记忆）
    When 文本框内输入字符后把焦点 Tab 到按钮，再按 q
    Then 命令窗口关闭（q/Q 仅按钮焦点下关窗；文本框焦点内 q 为普通字符）
    When 点击 [✗] 原地松开
    Then 命令窗口关闭

  Scenario: 06-cmd-window-02 Enter 确认分流
    When 按 A 打开命令窗口并输入 "<input>"
    And 按 Enter 确认
    Then 命令窗口关闭且按输入类型分流 "<dialog>"
    And 对应对话框的来源字段预填 "<input>"（HTTP(S) = URL 字段并发自动回填 4；BT = 种子字段并发预填 20）
    When 按 A 打开命令窗口并逐行输入首行 "<first>" 与另外 <extra_lines> 行非空内容后按 Enter
    Then 命令窗口关闭并 toast "⚠ 仅处理第一行输入，其余 <extra_lines> 行已忽略"
    And 对应对话框预填种子来源 "<first>"
    When 按 A 打开命令窗口且不输入任何内容后按 Enter
    Then 命令窗口关闭且无对话框打开、无任务创建（空输入确认 = 仅关窗）

    Examples:
      | input                                                            | dialog        | first                                                            | extra_lines |
      | http://fixture.local/files/big-one.bin                           | 添加任务：HTTP(S) | http://fixture.local/files/big-one.bin                           | 0           |
      | /tmp/qatorrents/ubuntu-24.04.2-desktop-amd64.torrent             | 添加任务：BT    | /tmp/qatorrents/ubuntu-24.04.2-desktop-amd64.torrent             | 0           |
      |   /tmp/qatorrents/padded.torrent                                 | 添加任务：BT    | /tmp/qatorrents/padded.torrent                                   | 2           |

  Scenario: 06-cmd-window-03 换行与行数上限
    When 按 A 打开命令窗口
    And 按 Shift+Enter 后继续输入（kitty 协议终端）
    Then 文本框内在光标处换行（光标后内容移到新行）且确认前共 2 个逻辑行
    When 按 Alt+Enter 后继续输入（任何终端降级键位）
    Then 文本框内再次换行（Alt+Enter 与 Shift+Enter 同效）
    And 单行输入 400 字符不设上限、按显示宽 word wrap 折行展示（空格优先断行、超长 token 硬断行）
    When 连续换行至第 65 行
    Then toast 显示 "⚠ 命令窗口最多 64 行"（行数上限 64）

  Scenario: 06-cmd-window-04 Ctrl+Enter / Ctrl+J / [确定] 按钮确认与焦点循环
    When 按 A 打开命令窗口并输入本地种子路径
    And 按 Ctrl+J 确认
    Then 命令窗口关闭并打开 "添加任务：BT"（Ctrl+J = 确认，任何终端；Ctrl+Enter kitty 上报同效）
    When 按 A 重新打开、输入 http URL 后按 Tab
    Then 焦点切换到 [确定] 按钮（反色高亮）
    When 按钮焦点下按 Enter
    Then 命令窗口关闭并打开 "添加任务：HTTP(S)"（按钮焦点下 Enter = 确认，与 [确定] click 同分支）
    And 再按 Tab 焦点切回文本框且光标到行尾

  Scenario: 06-cmd-window-05 Shift+Tab 本地路径补全
    When 按 A 打开命令窗口并输入 "<typed>"
    And 按 Shift+Tab
    Then 文本框内容补全为 "<completed>" 且 toast 为 "<toast>"（readline 式：光标前 whitespace 分隔 token，仅对像本地路径的 token 生效）

    Examples:
      | typed                              | completed                                    | toast                              |
      | /tmp/qato                          | /tmp/qatorrents/                             | 补全目录：/tmp/qatorrents/          |
      | /tmp/qatorrents/ub                 | /tmp/qatorrents/ubuntu-24.04.2-desktop-amd64.torrent | （无 toast——唯一文件候选直接补全）   |
      | ~/down                             | /home/qa/downloads/                          | 补全目录：/home/qa/downloads/        |
      | /tmp/zzz无匹配前缀                  | /tmp/zzz无匹配前缀                            | 无匹配：/tmp/zzz无匹配前缀           |
      | https://example.com/ann            | https://example.com/ann                      | （无 toast——非 path token 不响应）  |

  Scenario: 06-cmd-window-06 多行编辑与鼠标
    When 按 A 打开命令窗口并粘贴含换行的多行文本（bracketed paste）
    Then 粘贴文本按 \n 自动分行入编辑器（段内控制字符剔除）
    When 在多行缓冲中按 ↑ / ↓
    Then 光标在相邻可视行间移动（含 word wrap 分段行）且列保持——目标行当前列有字符停该列、无字符落行尾
    When 点击文本框内某字符格
    Then 文本框聚焦且光标落在点击处（窄字符格点其前；宽字左半格在前、右半格在后；点行尾空白 = 到段尾）
    And 滚轮悬停文本区 = 滚动可视行视口、滚动条在文本框右边框上（滑块拖拽 / 箭头连发 / 轨道翻页可用）
    And 按住窗口空白处拖拽 = 移动窗口、点击窗口外部 = 无任何操作

  Scenario: 06-cmd-window-07 入口分流与文案
    When 启动后按 A
    Then 打开的是命令窗口而非「添加任务：HTTP(S)」对话框（A 直接开 HTTP 添加框路径退役）
    And 页脚命令 chips 显示 "A 命令 · D 删除 · M 修改 · Q 退出 · H 帮助"
    And 任务列表空态（此页签下没有任务时）显示 "（此页签下没有任务，按 A 打开命令窗口添加任务）"
    And 帮助窗口（H）包含【命令窗口（A）】组（9 条目）与【BT 对话框（添加/修改：BT）】组（10 条目）
    And 帮助【命令窗口（A）】「输入内容」条目 = "http(s) URL / 本地 .torrent 路径"（D59 文案去磁力；demo 原文含「/ 磁力链接」不采用）

  Scenario: 06-cmd-window-08 错误窗口（非法输入与解析失败）
    When 按 A 打开命令窗口并输入 "<input>" 后按 Enter（或 [确定] 按钮 / Ctrl+Enter）
    Then 命令窗口先关闭且无任何添加对话框打开、无任务创建
    And 出现 66×7 圆角模态浮层居中显示、标题 " 错误 "、顶边框行右端 [✗]
    And 消息行居左显示红色文字 "<message>"
    And "[ 确定 ]" 按钮水平居中（按钮行 = 内框 y+3）
    When 按 Esc（或 Enter / q / 点 [确定] / 点 [✗]）
    Then 错误窗口关闭且无残留（点击窗口外部无操作；可拖拽、位置关闭即清）

    Examples:
      | input                                                                                             | message                |
      | magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=ubuntu-24.04.2-desktop-amd64.iso | 暂不支持该类型的链接   |
      | ftp://fixture.local/files/big-one.bin                                                             | 暂不支持该类型的链接   |
      | 随手一行普通文本                                                                                    | 暂不支持该类型的链接   |
      | /tmp/qatorrents/missing.torrent                                                                   | 无法读取种子文件       |
      | /tmp/qatorrents/corrupt.torrent                                                                   | 无法读取种子文件       |
