# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   07-add-bt-01 对话框布局与预填（78×26；字段预填；文件树按来源解析；trackers 占位提示）
#   07-add-bt-02 文件树交互（目录展开收拢 / 三态勾选 / 统计联动 / [全选][全不选] / A·N·I）
#   07-add-bt-03 trackers 多行输入框（占位提示 / Alt+Enter 换行 / 无 wrap 双向滚动 / 64 行上限 / Enter = 确认）
#   07-add-bt-04 确认校验链（空来源 / 0 选中 / 重复任务 / 种子解析失败）
#   07-add-bt-05 .torrent 确认成功创建任务（total = 选中合计 / toast / BT 徽标）
#   07-add-bt-06 仅添加（⏸ 已暂停不占槽位 / 空格开始）
#   07-add-bt-07 [退役] 磁力任务元数据获取与确认——留空不重排（D53：phase2 不处理磁力链，磁力任务 phase-03）
#   07-add-bt-08 小终端自适应收缩（列表框不低于 1 可视行）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-02/04、D44/D47–D50/D55/D59。
# UI 基线 = ezr-demo phase2 源码（specifier 逐条核证：ui/dialogs.rs draw_bt_dialog /
# app/dialogs.rs dlg_confirm_add_bt / ui/tree.rs / ui/multiline.rs；焦点表 0–8、
# 按钮 [ 立即下载 ][ 仅添加 ][ 取消 ]、全部 toast 文案照录源码）。
# 种子来源仅 .torrent 文件路径（D53——磁力链 phase-03）；
# 确认链①文案 = ⚠ 请输入种子文件路径（去磁力，D59）；07-07 磁力场景退役留空不重排。
# v1.36 修订注（操作者 bug13 轮指令，D62；FR-02-02 v1.4/FR-01-03 v1.38）：场景 05 确认成功链
# 新增「保存到归一」判据——确认时保存到取值（预填或手输）按归一口径统一处理：~ 与 ~/ 前缀
# 展开用户主目录、相对路径（非 / 开头且非 ~ 前缀）按创建时进程工作目录转绝对路径、末尾 /
# 去除；任务落注册表与详情「路径」均为归一后绝对路径；预填层不改写（所见即所输）；重复任务
# 判重（同来源 + 同保存目录）用归一后路径比对。
Feature: 07-add-bt 添加任务：BT 对话框

  期号 02。命令窗口分流打开（仅 .torrent 路径，D53）：种子来源 + 保存到 + 文件树勾选
  （选择下载文件）+ 并发 + 代理 + trackers 多行输入；确认创建 BT 任务。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 BT fixture 已就绪（HTTP/UDP tracker + seeder peer + 预置 .torrent 文件目录 /tmp/qatorrents）
    And 终端尺寸 120×44

  Scenario: 07-add-bt-01 对话框布局与预填
    When 经命令窗口输入 "<source>" 并确认打开「添加任务：BT」对话框
    Then 出现 78×26 圆角模态浮层居中显示、标题 " 添加任务：BT "、顶边框行右端 [✗]
    And 布局自上而下 = 顶 padding 1 行 / 种子 / 保存到 / 文件列表框（高 9 = 边框 2 + 7 可视行）/ 并发 / 代理 / trackers 输入框（高 7 = 边框 2 + 5 可视行）/ 按钮前空行 / 按钮行 / 底 padding
    And 种子行预填 "<source>"（可编辑，300 字符上限）、保存到预填默认下载目录、并发预填 = [bt] 表 bt_concurrency 配置值（v1.35 三大表重构：缺省回退代码常量 20；越界钳制 1–64）、代理预填默认项（D48：配置 download_dir 缺省链，非 demo 的 /srv/downloads）
    And 文件列表框标题 " 选择下载文件 "、右上角 "[全选]" "[全不选]" 按钮、右下角统计 "已选 <sel_files> 个文件，共 <sel_size>"
    And 文件树按 "<source>" 元数据生成（目录默认收拢 ▸、默认全选——D49）、复选框列 [x] 绿色
    And trackers 输入框标题 " trackers "、空缓冲占位提示 "可输入多行 traker，使用 ALT+ENTER 换行"（DIM）
    And 按钮行 = "[ 立即下载 ]  [ 仅添加 ]  [ 取消 ]"、初始焦点在种子行（焦点表 0–8：种子/保存到/文件列表/并发/代理/trackers/立即下载/仅添加/取消）

    Examples:
      | source                                               | sel_files | sel_size  |
      | /tmp/qatorrents/ubuntu-24.04.2-desktop-amd64.torrent | 8         | 6.21 GB   |

  Scenario: 07-add-bt-02 文件树交互
    When 打开「添加任务：BT」对话框（多文件种子、树含目录）
    And 在文件列表焦点按 ↓ / ↑
    Then 光标在可见行间移动（收拢目录跳过子树）、光标行 SEL_BG 整行背景、视口按需跟随
    And 对目录行按 → 展开显示子行（▾）、按 ← 收拢（光标钳制界内）
    When 对文件行按 Space
    Then 该文件勾选翻转（[x] 绿 ⇄ [ ] 暗）、祖先目录三态自底向上重算（[x] 全选 / [-] 黄部分 / [ ] 全不选）
    And 右下角统计实时联动 "已选 <k> 个文件，共 <z>"（目录不计数；收拢目录下已选文件同样计入）
    When 点击目录行复选框列
    Then 整目录 全选 ⇄ 全不选（部分 → 全选）
    When 点击目录行复选框以外区域
    Then 目录展开/收拢切换（光标定位该行）
    And 列表焦点下按 A / N / I = 全选 / 全不选 / 反选；点击 "[全选]" / "[全不选]" 按钮对全树生效（悬停黄色加亮无下划线）
    And 可见行 > 7 时垂直滚动条绘制在列表框右边框上（滚轮悬停列表框滚动；滚动后视口停留不回弹）

  Scenario: 07-add-bt-03 trackers 多行输入框
    When 打开「添加任务：BT」对话框并把焦点切到 trackers 输入框
    Then 标题 " trackers "、5 可视行、逻辑行 1:1 渲染不做 word wrap
    And 空缓冲显示占位提示 "可输入多行 traker，使用 ALT+ENTER 换行"、输入任意内容后消失
    When 输入 "<tracker1>" 后按 Alt+Enter 再输入 "<tracker2>"（kitty 协议终端 Shift+Enter 同效）
    Then trackers 内为 2 个逻辑行（框内换行、不触发确认）
    When 输入超过 5 行或单行超宽
    Then 溢出行由垂直滚动条（右边框）承接、超宽行内由水平滚动条（下边框，箭头步长 2 列）承接、内容未超宽时自动隐藏
    When 连续换行至第 65 行
    Then toast 显示 "⚠ trackers 最多 64 行"
    And trackers 焦点下按 Enter = 确认（与 [ 立即下载 ] click 同义）；↑↓ = 上/下逻辑行（列保持）、←→ 行内移动、Home/End 首/末行；粘贴多行自动分行

    Examples:
      | tracker1                            | tracker2                          |
      | udp://tracker.fixture.local:6969/announce | https://tracker.fixture.local/announce |

  Scenario: 07-add-bt-04 确认校验链
    When 清空种子行后按 Enter 确认
    Then toast 显示 "⚠ 请输入种子文件路径"（D59 文案去磁力）、焦点回种子行、对话框未关闭
    When 种子行有效但取消全部文件勾选（或树无选中文件）后按 Enter
    Then toast 显示 "⚠ 请至少选择一个要下载的文件（空格勾选）"、焦点回文件列表、对话框未关闭
    When 先添加 "<source>" 任务成功，再经命令窗口以同来源 + 同保存目录再次打开对话框并确认
    Then toast 显示 "⚠ 任务已存在"、列表数量不变、对话框未关闭
    When 种子行编辑为不存在的路径 "<bad_path>" 后按 Enter
    Then toast 显示 "⚠ 无法读取种子文件"、对话框未关闭（D55：确认前按当前值重解析）

    Examples:
      | source                                               | bad_path                    |
      | /tmp/qatorrents/ubuntu-24.04.2-desktop-amd64.torrent | /tmp/qatorrents/missing.torrent |

  Scenario: 07-add-bt-05 .torrent 确认成功创建任务
    When 打开「添加任务：BT」对话框（种子 = "<source>"、勾选 <selected> 个文件）后按 Enter 确认
    Then 对话框关闭、任务列表新增任务 "<name>"（重名同目录自动 ".1" 后缀递增）、状态「等待中」或「下载中」
    And toast 逐字显示 "✓ 已添加任务 #<id>: <name>（BT，已选 <selected>/<total> 文件）"
    And 任务 total = 选中文件大小合计、协议徽标 "[BT]"（黄色）、详情类型行 = "BT · 支持断点续传"、详情无校验字段
    And 保存到按归一口径落盘为绝对路径（v1.36/D62：~ 展开用户主目录、相对路径按创建时工作目录转绝对、末尾 / 去除；预填层不改写）
    And 保存目录下按元数据路径结构重建目录结构（多文件）；磁盘随下载推进只落盘选中文件（FR-02-19）

    Examples:
      | source                                               | selected | total | name                              |
      | /tmp/qatorrents/ubuntu-24.04.2-desktop-amd64.torrent | 8        | 9     | ubuntu-24.04.2-desktop-amd64      |
      | /tmp/qatorrents/big-file.torrent                     | 1        | 1     | big-file.bin                      |

  Scenario: 07-add-bt-06 仅添加
    When 打开「添加任务：BT」对话框（种子有效、选中 <selected> 个文件）后点击 "[ 仅添加 ]"
    Then 任务创建且状态「已暂停」、不占下载槽位（下载槽位计数不含该任务）、无下载发生
    And toast 逐字显示 "⏸ 已添加任务 #<id>: <name>（BT，<selected>/<total> 文件，已暂停，按空格开始）"
    And 对该任务按空格后转为「等待中/下载中」并开始下载（重新排队口径同 FR-01-33 v1.21）

    Examples:
      | selected |
      | 8        |

  Scenario: 07-add-bt-07 [退役，留空不重排]
    # 原「磁力任务元数据获取与确认」场景随 D53 退役（操作者裁决：phase2 不处理磁力链；
    # 磁力任务/元数据拉取/40 位 infohash 属 phase-03）。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）；
    # 命令窗口磁力输入的新行为见 features/06-cmd-window.feature 场景 08（错误窗口）。
    # 本场景无可执行步骤：QA 对应用例 QA-AB-07 同步退役（qa/07-add-bt-qa.md 注记）。

  Scenario: 07-add-bt-08 小终端自适应收缩
    When 终端高度不足时打开「添加任务：BT」对话框
    Then 对话框钳制到终端尺寸、文件列表框按剩余高度收缩且不低于 1 可视行（trackers/字段行保留）
    And 内框宽 < 30 或高度低于最低需求时仅渲染边框与标题、不留残影
