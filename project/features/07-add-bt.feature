# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   07-add-bt-01 对话框布局与预填（78×29；字段预填；文件树按来源解析；trackers 占位提示；v1.38：+限速两字段；v1.39：限速字段无占位；v1.51：+按顺序下载复选项）
#   07-add-bt-02 文件树交互（目录展开收拢 / 三态勾选 / 统计联动 / [全选][全不选] / A·N·I）
#   07-add-bt-03 trackers 多行输入框（占位提示 / Alt+Enter 换行 / 无 wrap 双向滚动 / 64 行上限 / Enter = 确认）
#   07-add-bt-04 确认校验链（空来源 / 0 选中 / 重复任务 / 种子解析失败；v1.42 修订：判重谓词改「同来源」——与保存目录无关，bug27 轮；v1.41 修订：重复任务 toast 「⚠ 任务已存在」退役 → 弹出「错误」窗口（D56）消息「该链接的任务已存在」，bug25 轮）
#   07-add-bt-05 .torrent 确认成功创建任务（total = 选中合计 / toast / BT 徽标）
#   07-add-bt-06 仅添加（⏸ 已暂停不占槽位 / 空格开始）
#   07-add-bt-07 [退役] 磁力任务元数据获取与确认——留空不重排（D53：phase2 不处理磁力链，磁力任务 phase-03）
#   07-add-bt-08 小终端自适应收缩（列表框不低于 1 可视行）
#   07-add-bt-09 种子来源路径归一（v1.37 新增，D63）
#   07-add-bt-10 限速两字段预填 config 任务级默认值且随任务创建落库（v1.38 新增，FR-02-29；v1.39 修订：二进制单位 + 输入即规范化显示）
#   07-add-bt-11 trackers scheme 校验与默认 trackers（v1.9.1：wss 等拒绝不落库 + 留空默认 4 条入库；v1.43 反转：ws(s) 同权放行、白名单外 ftp 等拒绝、toast 文案更新，FR-02-31 v1.15）
#   07-add-bt-12 按顺序下载复选项（v1.51 新增，FR-02-39/D76：默认不勾选 / Space·Enter·点击切换 / 确认随任务创建落库）
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
# 判重（v1.42/bug27：同来源即命中——任务按 URL 唯一、与保存目录无关）用归一后路径比对。
# v1.37 修订注（操作者 bug14 轮指令，D63；FR-02-02 v1.5/FR-02-09③ 同步）：新增场景 09「种子
# 来源路径归一」——种子行取值（预填或手输）在确认时归一为绝对路径存入任务 url（~ 与 ~/ 展开
# 主目录、相对路径按创建时进程工作目录转绝对；与保存到归一 D62① 同族，复用同一 expand_tilde
# 口径）；判重（同来源）用归一后路径比对；预填层不改写（所见即所输）；命令窗口分流臂②解析前
# 同口径归一（FR-02-01 v1.5）。修 bug：相对路径/`~` 形式的 .torrent 路径此前字面存入 url。
# v1.38 修订注（操作者 bug18 轮指令五项功能之二「任务级上传/下载速度限制」；FR-02-29，D67）：
# ①添加任务：BT 对话框 78×26 → 78×28（布局 = 顶 padding / 种子 / 保存到 / 文件列表框 /
# 并发 / 下载限速 / 上传限速 / 代理 / trackers 输入框 / 按钮前空行 / 按钮行 / 底 padding；
# 两限速字段插在并发之后、代理之前；焦点表 0–8 → 0–10，字段行 y +1、按钮行 y +2）；
# ②限速字段 = 文本输入框：预填 config [common] task_max_download_speed /
# task_max_upload_speed 解析后的整数 B/s（0 显示 0），接受整数 B/s 或十进制单位字符串；
# 留空或非法确认时回退 config 默认（同 HTTP 添加对话框口径，01-add-task-20）。
# v1.39 修订注（操作者 bug21 修订轮批准；FR-02-29 v1.10/D67② 修订，细则同 01-add-task v1.40 注记）：
# ①BT 对话框两限速字段保留（预填/留空/非法口径不变），单位口径十进制改二进制 + 键入/粘贴即
# 规范化显示（如 2m → 2 MiB/s，前导零剥离，退格不回写）+ 占位退役（空态纯空白）；②并发字段
# 满容量重启录入口径同 01-add-task-21（HTTP/BT 四对话框同口径）。
# v1.40 修订注（操作者 bug23 轮批准；FR-02-29 v1.11，D67②）：①限速字段预填/回显改 fmt
# 规范化显示（GiB→MiB→KiB→B 整除链、非 1024 整倍数显裸 N B/s、0 显 0；fmt→parse 恒等；
# 原「预填整数 B/s」口径退役）；②裸数字输入确认恒按 B/s（512 → 512 B/s）；③config 键
# task_max_* 旧写法 KB/MB/GB 十进制废弃不再识别（含旧写法的值 = 非法 → 回退 0 → 预填 0）;
# 留空回退 config 默认口径不变。场景 10 随修订。
# v1.43 修订注（操作者修订轮指令「tracker 统一 + ws(s) 支持 + BT WebRTC + config 严格校验」；
# phase-02 v1.15，FR-02-31 v1.15/D70；Inversion 阶段 1 受限退化 chat 输入选择题答案 1a/2b/3a/4a/5a，
# 题目录重建登记见 phase-02 v1.15 注记与 handoff 待批）：场景 11 反转——tracker scheme 白名单
# 扩为 {http, https, ws, wss, udp}（大小写不敏感），ws:// 与 wss:// 行同权放行入库
#（原 wss/ws 拒绝臂退役，FR-02-31 v1.15）；白名单外 scheme（如 ftp://）整单拒绝且 toast 文案
# 钦定为 "⚠ 不支持的 tracker 协议（仅 http(s)/ws(s)/udp）：<url 截尾>"（截尾 = 首 44 显示字符
# 硬截尾，无省略号后缀；旧文案「仅 http/https/udp」退役）。留空默认 4 条（http/https/udp）不变。
# v1.51 修订注（操作者修订轮指令 20261113「添加和修改任务窗口，增加'按顺序下载' 选项，默认不
# 勾选，随任务持久化，策略参考 qtorrent 的 'Download in sequential order'，确认后应立即生效」；
# phase-02 v1.21，FR-02-39/D76；Inversion 受限退化沿 v1.45–v1.52 先例）：①添加任务：BT 对话框
# 78×28 → 78×29（新增「按顺序下载」复选项行，插在代理之后、trackers 之前；焦点表 0–10 → 0–11
# ——按顺序下载 = 焦点 7，trackers 7→8、立即下载/仅添加/取消 8/9/10 → 9/10/11；字段行 y +1、
# trackers 框与按钮行 y +1）；②复选项形态 = 标签「按顺序下载」+ 复选框 ☑（勾选）/ ☐（默认不勾选），
# Space/Enter 切换、点击切换；确认随任务创建落库（任务元数据持久化，09-bt-engine-24 引擎语义）。
Feature: 07-add-bt 添加任务：BT 对话框

  期号 02。命令窗口分流打开（仅 .torrent 路径，D53）：种子来源 + 保存到 + 文件树勾选
  （选择下载文件）+ 并发 + 代理 + trackers 多行输入；确认创建 BT 任务。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 BT fixture 已就绪（HTTP/UDP tracker + seeder peer + 预置 .torrent 文件目录 /tmp/qatorrents）
    And 终端尺寸 120×44

  Scenario: 07-add-bt-01 对话框布局与预填
    When 经命令窗口输入 "<source>" 并确认打开「添加任务：BT」对话框
    Then 出现 78×29 圆角模态浮层居中显示、标题 " 添加任务：BT "、顶边框行右端 [✗]（v1.38/FR-02-29：78×26 → 78×28 新增限速两字段；v1.51/FR-02-39：78×28 → 78×29 新增「按顺序下载」复选项行）
    And 布局自上而下 = 顶 padding 1 行 / 种子 / 保存到 / 文件列表框（高 9 = 边框 2 + 7 可视行）/ 并发 / 下载限速 / 上传限速 / 代理 / 按顺序下载 / trackers 输入框（高 7 = 边框 2 + 5 可视行）/ 按钮前空行 / 按钮行 / 底 padding
    And 种子行预填 "<source>"（可编辑，300 字符上限）、保存到预填默认下载目录、并发预填 = [bt] 表 bt_concurrency 配置值（v1.35 三大表重构：缺省回退代码常量 20；越界钳制 1–64）、下载限速预填 = config task_max_download_speed 解析值（整数 B/s）、上传限速预填 = config task_max_upload_speed 解析值（v1.38/FR-02-29；限速两字段无占位提示，v1.39）、代理预填默认项（D48：配置 download_dir 缺省链，非 demo 的 /srv/downloads）
    And 文件列表框标题 " 选择下载文件 "、右上角 "[全选]" "[全不选]" 按钮、右下角统计 "已选 <sel_files> 个文件，共 <sel_size>"
    And 文件树按 "<source>" 元数据生成（目录默认收拢 ▸、默认全选——D49）、复选框列 [x] 绿色
    And trackers 输入框标题 " trackers "、空缓冲占位提示 "可输入多行 traker，使用 ALT+ENTER 换行"（DIM）
    And http(s)/www 形态 URL 行渲染层反终端自动链接（VS15 U+FE0E 零宽注入 + 相邻 span ±1 微差色交替——打断终端 hover 自动下划线识别；udp 行不注入；仅影响显示，不改缓冲内容与光标/点击映射、列宽与滚动计算；bug20 追轮，与 trackers 弹窗 URL 列同口径）
    And 「按顺序下载」复选项行 = 标签 "按顺序下载" + 复选框 ☐（默认不勾选）/ ☑（勾选；v1.51/FR-02-39，qBittorrent 'Download in sequential order' 同源）
    And 按钮行 = "[ 立即下载 ]  [ 仅添加 ]  [ 取消 ]"、初始焦点在种子行（焦点表 0–11：种子/保存到/文件列表/并发/下载限速/上传限速/代理/按顺序下载/trackers/立即下载/仅添加/取消——v1.51 焦点环扩展：trackers 7→8、按钮 8/9/10→9/10/11）

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
    When 先添加 "<source>" 任务成功，再经命令窗口以同来源（保存目录同或异）再次打开对话框并确认
    Then 弹出「错误」窗口（D56 几何/交互口径）且消息为 **"该链接的任务已存在"**（v1.42/bug27：判重谓词由「同来源 + 同保存目录」改判「同来源」——任务按 URL 唯一、与保存目录无关，归一后路径比对；v1.41/bug25：原 toast "⚠ 任务已存在" 承载退役；错误窗口弹出时添加对话框保持打开，关闭错误窗口后回到对话框可修正或取消；不出现 "暂不支持该类型的链接" 文案——D53 文案仅属命令窗口分流臂③，与重复任务判定互斥）、列表数量不变
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
    And 任务 url = 种子来源归一后绝对路径（v1.37/D63：详情「URL」字段与注册表来源为绝对路径，~ 展开与相对转绝对同保存到口径）
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

  Scenario: 07-add-bt-09 种子来源路径归一（v1.37 新增，D63）
    Given 预置种子文件 <torrent_name> 位于 <base_dir>
    When 经命令窗口输入 "<input>" 打开「添加任务：BT」对话框并确认创建
    Then 任务创建成功且任务 url = "<expected_url>"（详情「URL」字段与注册表来源一致）
    And 对话框种子行预填保持原样输入（预填层不改写，所见即所输）
    And 以同来源再次创建同一任务时判重命中（**v1.42/bug27：同来源即命中——与保存目录无关**；v1.41/bug25：弹出「错误」窗口消息「该链接的任务已存在」——原 toast 「⚠ 任务已存在」承载退役；判重用归一后路径比对口径不变）

    Examples:
      | torrent_name      | base_dir           | input                                   | expected_url                            |
      | multi-file.torrent | /tmp/qatorrents    | /tmp/qatorrents/multi-file.torrent      | /tmp/qatorrents/multi-file.torrent      |
      | multi-file.torrent | /tmp/qatorrents    | ~/qatorrents/multi-file.torrent         | <home>/qatorrents/multi-file.torrent    |
      | multi-file.torrent | /tmp/qatorrents    | qatorrents/multi-file.torrent           | <cwd>/qatorrents/multi-file.torrent     |
      | multi-file.torrent | /tmp/qatorrents    | ./qatorrents/multi-file.torrent         | <cwd>/qatorrents/multi-file.torrent     |


  # v1.38 新增（操作者 bug18 轮指令，FR-02-29 + D67）：BT 添加对话框限速两字段。
  # v1.39 修订（FR-02-29 v1.10）：二进制单位 + 即时规范化显示 + 占位退役（两字段保留）。
  # v1.40 修订（FR-02-29 v1.11）：预填 fmt 规范化显示 + 裸数字 B/s + config 旧写法废弃回退。
  Scenario: 07-add-bt-10 限速两字段预填 config 任务级默认值且随任务创建落库（v1.39，FR-02-29；v1.40：预填 fmt 规范化 + 裸数字 B/s）
    Given 配置文件 [common] 表 task_max_download_speed = "<cfg_down>"、task_max_upload_speed = "<cfg_up>"
    When 经命令窗口打开「添加任务：BT」对话框
    Then 下载限速字段预填 "<down_prefill>"、上传限速字段预填 "<up_prefill>"（config 解析后 fmt 规范化显示：KiB/MiB/GiB 整倍数带对应单位、非 1024 整倍数显裸 N B/s、0 显 0，v1.40；插在并发与代理之间；无占位提示，v1.39）
    And 在下载限速字段输入 "<input>"
    Then 字段即时规范化显示 "<display>"（二进制单位族；前导零剥离；退格不回写）
    And 确认创建
    Then 任务任务级下载/上传限速落为 "<result_down>"/<up_raw>（注册表落盘；BT 引擎按任务级与全局 min 合成生效，01-throttle-proxy-12）
    And 留空或非法输入确认时回退 config 默认（即预填值对应解析值，同 HTTP 添加对话框口径）

    Examples:
      | cfg_down  | cfg_up  | down_prefill | up_prefill | input | display  | result_down | up_raw   |
      | 2 MiB/s   | 1 MiB/s | 2 MiB/s      | 1 MiB/s    | 2m    | 2 MiB/s  | 2097152     | 1048576  |
      | 512 KiB/s | 0       | 500 KiB/s    | 0          | 留空  | 500 KiB/s | 512000     | 0        |
      | 0         | 512 KiB/s | 0          | 500 KiB/s  | 512   | 512      | 512         | 512000   |
      | 2 MB/s    | 0       | 0            | 0          | abc   | abc      | 0           | 0        |


  # v1.9.1 新增（操作者 bug18 追轮 UI 反馈；FR-02-31）：trackers scheme 校验与默认 trackers。
  Scenario: 07-add-bt-11 trackers scheme 校验与默认 trackers（v1.9.1，FR-02-31；v1.43 反转：ws(s) 同权放行）
    When 经命令窗口打开「添加任务：BT」对话框，trackers 输入 <input> 后确认
    Then 含白名单外 scheme 行（http/https/ws/wss/udp 五种之外，大小写不敏感，如 ftp://）时整单拒绝：toast "⚠ 不支持的 tracker 协议（仅 http(s)/ws(s)/udp）：<url 截尾>"、焦点回 trackers、对话框保持打开、任务不创建（<url 截尾> = 首 44 显示字符硬截尾、无省略号后缀；旧文案「仅 http/https/udp」退役）
    And 输入含 ws:// 或 wss:// 行确认时同权放行（v1.43 反转，原 wss/ws 拒绝臂退役）：该行与 http(s)/udp 行一并入库，任务 tracker 池、T 弹窗、修改面板预填均含该行（scheme 大小写不敏感——WS://、Ws:// 与 ws:// 等价）
    And trackers 留空确认时任务创建成功：按种子元数据口径预置 4 条默认 tracker（http/https/udp）入库，T 弹窗与修改面板预填一致非空（v1.9.1 回归：面板预填不入库导致弹窗空的缺陷已修）

    Examples:
      | input                     |
      | ftp://tracker.example/ann |
      | wss://tracker.example/ws  |
      | WS://tracker.example/ws   |
      | （留空）                  |

  # v1.51 新增（FR-02-39/D76）：「按顺序下载」复选项交互与落库。
  Scenario: 07-add-bt-12 按顺序下载复选项（v1.51 新增，FR-02-39/D76）
    When 打开「添加任务：BT」对话框
    Then 「按顺序下载」复选项初始为 ☐ 未勾选（默认不勾选，FR-02-39）
    When Tab 焦点环切至按顺序下载复选项（或 ↑↓ 移动焦点至此；或点击复选项行）
    Then 复选项获得焦点高亮（标签亮色，同字段族口径）
    When 按 Space（或 Enter，或点击复选框）
    Then 复选框翻转 ☐ ⇄ ☑、对话框不关闭不确认
    When 勾选 ☑ 后按 Enter 确认（或点 [ 立即下载 ]）
    Then 任务创建且「按顺序下载」随任务落库（任务元数据持久化；09-bt-engine-24 引擎语义生效）
    And 重启 ezr 后该任务保持按顺序下载（恢复链路读取任务元数据，FR-02-22 机制）
    When 未勾选（☐）直接确认创建任务
    Then 任务创建且「按顺序下载」= 不启用（与既有任务一致；既有任务无该字段默认不勾选）

    Examples:
      | sequential_default |
      | false              |
