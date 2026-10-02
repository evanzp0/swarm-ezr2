# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-tui-display-01 三区布局与结构完整
#   01-tui-display-02 列表行三行结构与信息行文案
#   01-tui-display-03 任务详情字段全集
#   01-tui-display-04 并发分块明细表与待命态
#   01-tui-display-05 状态配色与协议徽标
#   01-tui-display-06 导航快捷键
#   01-tui-display-07 功能快捷键与页签
#   01-tui-display-08 鼠标交互
#   01-tui-display-09 toast 反馈自动消失
#   01-tui-display-10 CJK 宽度对齐
#   01-tui-display-11 窄屏降级布局
#   01-tui-display-12 头部统计真实语义
#   01-tui-display-13 模拟器移除与空态引导
#   01-tui-display-14 标题栏版本号随期号递进
#   01-tui-display-15 速度展示平滑节奏与归零
#   01-tui-display-16 连接级速度平滑节奏与归零
Feature: 01-tui-display TUI 展示与交互（基线沿用 + 真实数据）

  期号 01。依据 project/mission/phase-01.md FR-01-80~83。布局、状态机、配色、快捷键、
  对话框与文案以 ezr-tui-demo 定稿为基线；数据全部来自真实下载状态。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 终端尺寸为 120×34

  Scenario: 01-tui-display-01 三区布局与结构完整
    When 添加若干任务并观察主界面
    Then 头部为全局统计栏、中部为任务队列栏（含「正在下载/已完成」两页签）与任务列表+详情+Sparkline、页脚为快捷键提示栏
    And 各区块边框与标题样式与 demo 基线一致

  Scenario: 01-tui-display-02 列表行三行结构与信息行文案
    Given 状态为 "<state>" 的任务 "<file>"
    Then 该任务列表行为三行结构且信息行匹配 "<info_pattern>"

    Examples:
      | state   | file        | info_pattern                                        |
      | 等待中  | queue1.bin  | 排队第 N 位 · 等待空闲下载槽位 · <size>             |
      | 下载中  | active1.bin | ↓ <speed> <downloaded>/<total> 剩余 <eta>           |
      | 已暂停  | paused1.bin | ↓ - <downloaded>/<total> 剩余 -                     |
      | 已失败  | failed1.bin | 重试 N/5 · Ns 后重试（或「不自动重试」「已达上限」）|
      | 已完成  | done1.bin   | （校验情况 · 如「SHA-256 校验成功」或「无校验」）   |

  Scenario: 01-tui-display-03 任务详情字段全集
    Given 下载中的任务 "<file>"
    When 查看右侧详情面板
    Then 字段包含：状态、排队（仅等待中）、ID、添加时间、类型（含并发数与断点续传支持）、
      大小（已下载/总大小）、失败原因（仅已失败）、速度与剩余、保存路径、URL、
      分块 x/y · 1 MB/块
    And 默认配置下分块行块大小显示为 "1 MB/块"

    Examples:
      | file       |
      | detail.bin |

  Scenario: 01-tui-display-04 并发分块明细表与待命态
    Given 任务 "<file>" 以并发 4 下载中（文件共 2 块时场景成立）
    Then 分块明细表逐连接显示当前持有块号（块 k/y）与状态
    And 状态含传输中与「待命」（临近结束时多余连接显示待命）

    Examples:
      | file       |
      | chunky.bin |

  Scenario: 01-tui-display-05 状态配色与协议徽标
    Given 状态为 "<state>" 的任务 "<file>"（协议 <scheme>）
    Then 该任务进度条填充与百分比/徽标文字颜色为 "<color>"
    And 协议徽标 [<scheme>] 为黄色

    Examples:
      | state   | file        | scheme | color |
      | 等待中  | color1.bin  | HTTP   | 黄色  |
      | 下载中  | color2.bin  | HTTPS  | 淡蓝  |
      | 已暂停  | color3.bin  | HTTP   | 白色  |
      | 校验中  | color4.bin  | HTTP   | 淡蓝  |
      | 已失败  | color5.bin  | HTTP   | 红色  |
      | 已完成  | color6.bin  | HTTP   | 绿色  |

  Scenario: 01-tui-display-06 导航快捷键
    Given 任务列表含 <tasks> 个任务
    When 依次按 ↑ / ↓ / Home / End / PgUp / PgDn
    Then 选中项按列表导航规则移动且详情面板随选中切换

    Examples:
      | tasks |
      | 12    |

  Scenario: 01-tui-display-07 功能快捷键与页签
    When 按 A
    Then 弹出「添加下载任务」对话框（Esc 可取消）
    When 按 D
    Then 弹出「删除任务」三选对话框（仅删除任务/删除任务和文件/取消）
    When 按 Tab
    Then 页签在「正在下载」与「已完成」间切换
    When 按 G
    Then 右侧速度图表面板显示/隐藏切换
    When 按 C
    Then 已完成任务全部清除（页签计数归零）
    When 按 U / J
    Then 选中任务在列表中上移/下移一位

  Scenario: 01-tui-display-08 鼠标交互
    When 鼠标左键点击列表中的任务
    Then 该任务被选中且详情切换
    When 滚轮滚动列表
    Then 列表视口滚动
    When 对话框打开后点击按钮
    Then 等价于对应按键操作（确认/取消/选项直选）

  Scenario: 01-tui-display-09 toast 反馈自动消失
    When 触发一个 toast 提示（如保存已暂停任务继续但槽位满）
    Then toast 在界面出现并清晰可读
    And 数秒后自动消失且不残留遮挡

  Scenario: 01-tui-display-10 CJK 宽度对齐
    Given 任务文件名含中文（如 "报告文件 最终版.zip"）
    When 观察列表与详情渲染
    Then 各列边界对齐无错位（CJK 按显示宽度两格计算）

  Scenario: 01-tui-display-11 窄屏降级布局
    Given 终端尺寸调整为 <cols>×34
    When 观察主界面
    Then 布局按基线降级（<cols> < 100 时隐藏右侧详情栏）且无溢出错乱

    Examples:
      | cols |
      | 120  |
      | 80   |

  Scenario: 01-tui-display-12 头部统计真实语义
    Given 多任务下载中
    Then 头部 ↓ 为真实全局下载速度（与列表速度汇总一致）
    And 头部 ↑ 恒为 0（BT 二期启用字段）
    And 并发线程 = 当前活跃连接数
    And 会话已下载 = 本次运行累计下载字节
    And 任务数三个计数（总数/正在下载/已完成）与真实任务状态一致

  Scenario: 01-tui-display-13 模拟器移除与空态引导
    When 以干净环境启动且不添加任何任务
    Then 任务列表为空且显示引导文案「按 A 添加下载任务」
    And 空置运行期间无任何任务自行出现或状态自行流转（无伪造数据）

  Scenario: 01-tui-display-14 标题栏版本号随期号递进
    When 观察标题栏
    Then 版本号显示 "v0.1.0-01"（期号 01）

  Scenario: 01-tui-display-15 速度展示平滑节奏与归零
    Given 一个下载中的任务 "<file>"（fixture 慢速门控维持下载观察窗）
    When 连续采样任务信息行速度与头部全局速度 10 秒
    Then 速度数值每秒最多变化一次
    And Sparkline 速度图新增采样点 ≤ 10（每秒至多一个）
    When 暂停该任务（Space）
    Then 该任务展示速度立即归零（信息行进入暂停态文案，无平滑拖尾）
    And 头部全局速度同步扣除该任务份额
    And 恢复下载后速度经平滑爬升而非瞬时跳至峰值

    Examples:
      | file        |
      | three-m.bin |

  Scenario: 01-tui-display-16 连接级速度平滑节奏与归零
    Given 一个下载中的任务 "<file>"（并发 "<concurrency>"，fixture 慢速门控维持下载观察窗）
    When 连续采样并发明细表各行速度 10 秒
    Then 传输中各行速度数值每秒最多变化一次
    And 各行速度与任务速度同拍更新（同一 1 秒节拍，无独立变拍）
    When 暂停该任务（Space）
    Then 明细表各行速度立即归零（无平滑拖尾，状态列不再显示传输中）
    And 恢复下载后各行速度经平滑爬升而非瞬时跳至峰值

    Examples:
      | file         | concurrency |
      | twelve-m.bin | 4           |
