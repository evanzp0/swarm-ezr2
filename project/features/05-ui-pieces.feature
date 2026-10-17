# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   05-ui-pieces-01 P 键打开分块弹窗与窗口形态
#   05-ui-pieces-02 四态色块与元信息行口径（非下载中）
#   05-ui-pieces-03 下载中任务的 🟦 在传块与 🟦 优先
#   05-ui-pieces-04 打开定位下载前沿与滚动
#   05-ui-pieces-05 弹窗模态与关闭
#   05-ui-pieces-06 功能门槛拒绝臂
#   05-ui-pieces-07 鼠标滚轮/拖拽/点击外部
#   05-ui-pieces-08 页脚 P 分块 提示
Feature: 05-ui-pieces P 键 pieces 分块弹窗（FR-01-106，对齐 ezr-demo 定稿 §6.3/§6.4）

  期号 01。依据 project/mission/phase-01.md v1.29（FR-01-106、D35，操作者指令两批需求之一）。
  权威参照：ezr-demo/docs/UI交互需求文档-定稿.md v1.4 §6.3/§6.4/§6.5 与 ezr-demo 源码
  （open_pieces / draw_pieces / on_mouse pieces 分支）。
  功能门槛：仅支持分块下载的任务才有此功能——BT 任务恒支持（phase-02 预留，01 期不触发）；
  HTTP(S) 任务需服务器支持断点续传，不支持续传（单流整段下载）按 P toast 拒绝且弹窗不打开。
  端到端口径：全部断言作用于伪终端用户可见画面，不进入项目内部 API。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 终端尺寸为 120×34

  Scenario: 05-ui-pieces-01 P 键打开分块弹窗与窗口形态
    Given 下载中的任务 "<file>"（并发 <conns>，fixture 慢速门控维持观察窗）被选中
    When 按 P
    Then 弹窗打开且为固定 <w>×<h> 圆角边框浮层、在终端内居中
    And 弹窗标题为「分块状态」且加粗青色
    And 弹窗内容自上而下四段：任务名行（超宽截尾白色加粗）/ 元信息行 / 分块网格 / 快捷键提示行
    And 元信息行 = " <protocol> · 块大小 <bsize> · {x}/{y} 块 · 并发 <conns> "（{x}/{y} = 该任务已完成/总块数，与详情面板「分块」行同口径同值）
    And 快捷键提示行为 " ↑↓/PgUp/PgDn 滚动 · Esc/P 关闭"
    And 弹窗下方主界面仍完整渲染（浮层覆盖、打开/关闭帧全量重绘无宽字符残影）
    When 按 Esc 关闭后将终端尺寸调整为 <small_cols>×<small_rows> 再按 P
    Then 弹窗尺寸钳制到终端内且网格行数随内框收缩、四段内容完整不溢出

    Examples:
      | file          | conns | w  | h  | protocol | bsize | small_cols | small_rows |
      | piece-dl8.bin | 4     | 64 | 21 | HTTP     | 1 MB  | 56         | 16         |

  Scenario: 05-ui-pieces-02 四态色块与元信息行口径（非下载中）
    Given 状态为 "<state>" 的任务 "<file>"（并发 <conns>）被选中且已完成 <done> 块、另有 <partial> 个部分块与 0 个在传块
    When 按 P 观察分块网格
    Then 网格每块为一个宽 2 列的色块字符且同状态连续块合并为单段
    And 已完成块显示 🟩（块号 < 已完成块数，全部按字节序排列）
    And 网格内不出现 🟦（任务非「下载中」）
    And 有已落盘零头的未完成块显示 🟨（状态下沉冻结的持块连接）
    And 其余块显示 ⬜
    And 弹窗内无图例计数行、无任务状态行、无分页/总块数提示行

    Examples:
      | state  | file            | conns | done | partial |
      | 已暂停 | piece-pause.bin | 4     | 5    | 2       |
      | 已完成 | piece-done.bin  | 4     | 12   | 0       |

  Scenario: 05-ui-pieces-03 下载中任务的 🟦 在传块与 🟦 优先
    Given 下载中的任务 "<file>"（并发 <conns>，已完成 <done> 块、<active> 个块在传）被选中
    When 按 P 观察分块网格
    Then 在传块（已分配并发下载的块）显示 🟦 且块内零头多少不影响（分配即算）
    And 已完成块显示 🟩、未分配未落盘块显示 ⬜
    And 同一块既有已落盘零头又在传时只显示 🟦（🟦 优先于 🟨）
    When 按空格暂停该任务后再按 P 观察网格
    Then 网格内不再出现 🟦（原在传块按落盘零头转 🟨 或无零头转 ⬜）

    Examples:
      | file          | conns | done | active |
      | piece-act.bin | 4     | 3    | 4      |

  Scenario: 05-ui-pieces-04 打开定位下载前沿与滚动
    Given 下载中的任务 "<file>"（约 <pct>% 进度、总块数超过一屏）被选中
    When 按 P
    Then 打开首帧定位到下载前沿：第一块未完成块所在行滚动至网格垂直居中（上方 🟩、前沿行含 🟦/🟨、下方 ⬜）
    And 块数超一屏时弹窗右缘（右边框列）出现滚动条且滑块位置与滚动偏移一致
    When 连续观察 <secs> 秒下载推进
    Then 网格滚动位置不变（打开后不自动跟随下载前沿）
    When 按 ↓ ×<rows> 再按 PgDn 再按 End
    Then 网格逐行下滚后按页下滚（每页 <page> 行）再跳至末页且滚动上限按实际总行数钳制
    When 按 Home
    Then 网格回到首行
    When 按 PgUp
    Then 网格按页上滚（每页 <page> 行）

    Examples:
      | file          | pct | secs | rows | page |
      | piece-big.bin | 50  | 6    | 3    | 8    |

  Scenario: 05-ui-pieces-05 弹窗模态与关闭
    Given 下载中的任务 "<file>"（并发 <conns>）被选中
    When 按 P 打开弹窗
    And 依次按 Space、R、A、D、M、C、Tab、G、U、J（P/Esc/Q 与弹窗滚动键除外）
    Then 全部按键被弹窗消费为主界面无动作：任务不暂停/不重试/不增删、无对话框打开、页签不切换、右栏不收起、任务列表选择不动
    When 按 q
    Then 弹窗关闭且应用未退出（主界面恢复完整渲染）
    When 再按 P 重新打开后按 Q
    Then 弹窗关闭且应用未退出
    When 再按 P 重新打开后按 Esc
    Then 弹窗关闭且应用未退出
    When 再按 P 打开弹窗后按 Ctrl+C
    Then 应用退出且终端恢复正常（哨兵复原）
    When 重启 ezr 并选中下载中任务后按 Ctrl+↓
    Then Ctrl+↑↓/Ctrl+B 在弹窗未打开时行为不变（FR-01-97 既有口径回归）

    Examples:
      | file          | conns |
      | piece-dl8.bin | 4     |

  Scenario: 05-ui-pieces-06 功能门槛拒绝臂
    When 无任何任务时按 P
    Then toast 显示 "未选中任务，无法查看分块" 且弹窗不打开
    When 等待中未探测的任务 "<ufile>"（列表大小显示「未知」，D35）被选中后按 P
    Then toast 显示 "任务尚未探测，暂无分块信息" 且弹窗不打开
    When 不支持断点续传的任务 "<nfile>"（服务器无 Range 支持，详情类型行「不支持断点续传」）被选中后按 P
    Then toast 显示 "该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息" 且弹窗不打开
    When 探测完成的空内容任务 "<efile>"（total = 0）被选中后按 P
    Then toast 显示 "任务内容为空，无分块信息" 且弹窗不打开
    And 以上各拒绝臂按 P 后主界面均无弹窗浮层出现

    Examples:
      | ufile            | nfile             | efile           |
      | piece-unprob.bin | piece-norange.bin | piece-empty.bin |

  Scenario: 05-ui-pieces-07 鼠标滚轮/拖拽/点击外部
    Given 下载中的任务 "<file>"（总块数超过一屏）被选中
    When 按 P 打开弹窗后指针悬停在弹窗内滚动滚轮向下 <notches> 次
    Then 分块网格向下滚动 <notches>×2 行且任务列表不滚动
    When 左键按住弹窗内任意空白位置拖动 <dx>,<dy> 后松开
    Then 弹窗随指针移动且新位置始终钳制在终端内
    When 按 P 关闭后再按 P 重新打开
    Then 弹窗重新在终端内居中（上次拖拽位置不保留）
    When 左键点击弹窗外部（任务列表区域）
    Then 弹窗关闭且该次点击不选中任何任务（弹窗打开期间鼠标事件全部由弹窗分支消费）

    Examples:
      | file          | notches | dx | dy |
      | piece-big.bin | 3       | 10 | 4  |

  Scenario: 05-ui-pieces-08 页脚 P 分块 提示
    When 观察页脚快捷键行
    Then 快捷键行含 "P" 与 " 分块 " 项且位于「R 重试」与「A 添加」之间（式样与其他项一致：键名青色加粗、说明暗灰）
    When 终端尺寸调整为 80×34
    Then 窄屏精简后 "P 分块" 项仍在快捷键行可见（截断位次口径不变）
