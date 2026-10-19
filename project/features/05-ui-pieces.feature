# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   05-ui-pieces-01 P 键打开分块弹窗与窗口形态（v1.30 修订：64×21→64×19、提示行→图例行）
#   05-ui-pieces-02 四态色块与元信息行口径（非下载中）（v1.30 修订：图例行恒显）
#   05-ui-pieces-03 下载中任务的 🟦 在传块与 🟦 优先
#   05-ui-pieces-04 打开定位下载前沿与滚动（v1.30 修订：滚动条字形 ▲│█▼、滑块恒绿）
#   05-ui-pieces-05 弹窗模态与关闭（v1.30 修订：回归键 Ctrl+↑↓/Ctrl+B 改 ↑↓/B）
#   05-ui-pieces-06 功能门槛拒绝臂（v1.31 修订：未探测臂前置条件细化——!probed && total==0 大小未知态，D35 收窄）
#   05-ui-pieces-07 鼠标滚轮/拖拽/点击外部（v1.31 修订：浮层渲染口径零清屏注记）
#   （08 已随 v1.30 页脚收敛（P3/FR-01-107）退役：页脚 P 分块 提示项不复存在，序号留空不重排）
Feature: 05-ui-pieces P 键 pieces 分块弹窗（FR-01-106，对齐 ezr-demo 重构版 §6.5）

  期号 01。依据 project/mission/phase-01.md v1.29（FR-01-106、D35，操作者指令两批需求之一）；
  v1.30 修订（操作者指令，对齐 ezr-demo 重构版）：窗口 64×21→64×19（底部快捷键提示行改四色
  图例行）、滚动条滑块恒聚焦绿、模态优先级按 FR-01-107（帮助窗口 > pieces > 对话框 > 主界面）。
  权威参照：ezr-demo/docs/ezr-demo UI界面及交互设计规范.md §6.5 与 ezr-demo 源码
  （open_pieces / draw_pieces / on_mouse pieces 分支；v1.29 权威参照 = docs/UI交互需求文档-定稿.md）。
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
    Then 弹窗打开且为固定 <w>×<h> 圆角边框浮层、在终端内居中（v1.30：64×19）
    And 弹窗标题为「分块状态」且加粗青色
    And 弹窗内容自上而下四段：任务名行（超宽截尾白色加粗）/ 元信息行 / 分块网格 / 四色图例行（v1.30：图例行替代原快捷键提示行，P3/FR-01-107）
    And 元信息行 = " <protocol> · 块大小 <bsize> · {x}/{y} 块 · 并发 <conns> "（{x}/{y} = 该任务已完成/总块数，与详情面板「分块」行同口径同值）
    And 图例行为 "🟩 已下载   🟦 正在下载   🟨 部分下载   ⬜ 未下载"（色块字符与网格同源，v1.30）
    And 弹窗下方主界面仍完整渲染（浮层覆盖、打开/关闭/拖拽帧由增量 diff 直接绘制不清屏、无宽字符残影无整屏闪烁——v1.31/FR-01-106⑧ 浮层翻转零清屏）
    When 按 Esc 关闭后将终端尺寸调整为 <small_cols>×<small_rows> 再按 P
    Then 弹窗尺寸钳制到终端内且网格行数随内框收缩、四段内容完整不溢出

    Examples:
      | file          | conns | w  | h  | protocol | bsize | small_cols | small_rows |
      | piece-dl8.bin | 4     | 64 | 19 | HTTP     | 1 MB  | 56         | 16         |

  Scenario: 05-ui-pieces-02 四态色块与元信息行口径（非下载中）
    Given 状态为 "<state>" 的任务 "<file>"（并发 <conns>）被选中且已完成 <done> 块、另有 <partial> 个部分块与 0 个在传块
    When 按 P 观察分块网格
    Then 网格每块为一个宽 2 列的色块字符且同状态连续块合并为单段
    And 已完成块显示 🟩（块号 < 已完成块数，全部按字节序排列）
    And 网格内不出现 🟦（任务非「下载中」）
    And 有已落盘零头的未完成块显示 🟨（状态下沉冻结的持块连接）
    And 其余块显示 ⬜
    And 底部图例行恒显示四色说明（🟩🟦🟨⬜，v1.30：图例行随窗口形态恒在，与状态无关）

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
    And 块数超一屏时弹窗右缘（右边框列）出现滚动条：▲ 顶部箭头（暗灰）、轨道 = 边框线、█ 滑块（恒聚焦绿加粗）、▼ 底部箭头（暗灰），且滑块位置与滚动偏移一致（v1.30/FR-01-109）
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
    Then 弹窗关闭且应用未退出（主界面恢复完整渲染；q/Q 先关窗不退出，v1.30 语义不变）
    When 再按 P 重新打开后按 Q
    Then 弹窗关闭且应用未退出
    When 再按 P 重新打开后按 Esc
    Then 弹窗关闭且应用未退出
    When 再按 P 打开弹窗后按 Ctrl+C
    Then 应用退出且终端恢复正常（哨兵复原）
    When 重启 ezr、选中下载中任务、按 Tab 聚焦并发面板后按 ↓，再选中下载中任务按 B
    Then ↑↓ 明细选择与 B 断开在弹窗未打开时行为不变（v1.30 修订：原 Ctrl+↑↓/Ctrl+B 回归口径随 FR-01-97 v1.30 改键）

    Examples:
      | file          | conns |
      | piece-dl8.bin | 4     |

  Scenario: 05-ui-pieces-06 功能门槛拒绝臂
    When 无任何任务时按 P
    Then toast 显示 "未选中任务，无法查看分块" 且弹窗不打开
    When 等待中未探测的任务 "<ufile>"（列表大小显示「未知」——probed = false 且 total = 0 大小未知态，D35/v1.31 收窄口径）被选中后按 P
    Then toast 显示 "任务尚未探测，暂无分块信息" 且弹窗不打开
    And 恢复重探窗口（有断点任务获槽后 Probed 事件未至，total 已知）按 P 不落入本臂——落入正常三臂可开窗（v1.31 未探测臂收窄：!probed && total == 0，FR-01-106①；窗口过窄由产品单测承载，端到端不驱动）
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
    Then 弹窗重新在终端内居中（上次拖拽位置不保留——pieces 关闭即清位置，与帮助窗口跨次记忆不同，v1.30）
    And 弹窗开关/拖拽/位置变化帧由增量 diff 直接绘制且不清屏、无整屏闪烁（v1.31/FR-01-106⑧：浮层翻转零清屏，边界半格宽字符由 clip_wide_at_edges 缓冲区级清扫）
    When 左键点击弹窗外部（任务列表区域）
    Then 弹窗关闭且该次点击不选中任何任务（弹窗打开期间鼠标事件全部由弹窗分支消费）

    Examples:
      | file          | notches | dx | dy |
      | piece-big.bin | 3       | 10 | 4  |
