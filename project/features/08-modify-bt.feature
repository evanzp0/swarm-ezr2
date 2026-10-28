# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   08-modify-bt-01 M 键分流与布局（BT 任务 → 修改任务：BT 78×24；HTTP 任务 → 原修改框；已完成拒绝）
#   08-modify-bt-02 预填（任务名静态行 / 文件勾选预填 / 并发 / 代理 / trackers 预填）
#   08-modify-bt-03 修改文件勾选确认（total 重算 / 已下载钳制 / 立即生效 toast / 续传口径）
#   08-modify-bt-04 确认校验（0 选中保留 toast）
#   08-modify-bt-05 并发/代理热调与分块连接重建（保速度续传）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-03/04。
# UI 基线 = ezr-demo phase2 源码（specifier 逐条核证：app/dialogs.rs open_modify_bt_dialog /
# dlg_confirm_modify_bt、ui/dialogs.rs draw_bt_dialog ModifyBt 臂；焦点表 0–5、
# 按钮 [ 确定 ][ 取消 ]、toast 文案照录源码）。
# M 键守卫（D45）= state != Completed（demo 核证）——「做种中」BT 任务同样可打开修改；仅「已完成」拒绝。
Feature: 08-modify-bt 修改任务：BT 对话框

  期号 02。M 键对 BT 任务分流进入：任务名静态行 + 文件列表框（预填当前勾选）+ 并发 +
  代理 + trackers（预填任务 tracker 列表）；确认后文件选择/并发/代理立即生效。

  Background:
    Given ezr 以干净环境启动
    And 本地 BT fixture 已就绪（HTTP/UDP tracker + seeder peer）
    And 已有一个下载中的多文件 BT 任务（.torrent 添加、共 <total_files> 个文件、已完成部分 piece）

  Scenario: 08-modify-bt-01 M 键分流与布局
    When 选中 BT 任务按 M
    Then 打开 " 修改任务：BT " 对话框（78×24 圆角模态、居中、[✗] 关闭按钮）而非「修改任务：HTTP(S)」
    And 布局自上而下 = 顶 padding 1 行 / 任务 「name」静态行 / 文件列表框（高 8 = 边框 2 + 6 可视行）/ 并发 / 代理 / trackers 输入框（5 可视行）/ 按钮前空行 / 按钮行 / 底 padding
    And 按钮行 = "[ 确定 ]  [ 取消 ]"、初始焦点在文件列表（焦点表 0–5：文件列表/并发/代理/trackers/确定/取消）
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
    And 并发预填任务当前并发值、代理预填当前代理（不在选项表内时追加兜底项）
    And trackers 输入框预填任务 tracker 列表（每行一条，含 config 默认与来源解析与手输合并结果）

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

  Scenario: 08-modify-bt-05 并发/代理热调与分块连接重建
    When 把并发改为 "<conns>" 并切换代理为 "<proxy>" 后按 Enter 确认
    Then 并发/代理立即生效（热调口径同 FR-01-87）
    And total 或并发变化时按既有续传口径重建分块连接且各连接速度保留（保速度续传，不重新排队）
    And 头部「并发」与并发连接面板徽标随新并发数刷新

    Examples:
      | conns | proxy |
      | 8     | 直连   |
