# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   04-ui-conns-01 头部仪表面板通栏 5 行定稿布局
#   04-ui-conns-02 头部字段单行与峰值图例
#   04-ui-conns-03 页签行并入头部与槽位内联
#   04-ui-conns-04 │ 分隔符与图区留白
#   04-ui-conns-05 单色面积流量图形态
#   04-ui-conns-06 头部字段真实语义
#   04-ui-conns-07 主体带定稿布局与详情定高
#   04-ui-conns-08 并发连接面板 HTTP 三列结构
#   04-ui-conns-09 连接速度与累计量口径
#   04-ui-conns-10 明细可见性状态门槛
#   04-ui-conns-11 Ctrl+↑↓ 明细选择
#   04-ui-conns-12 Ctrl+B 断开守卫链
#   04-ui-conns-13 滚轮命中明细与穿透
#   04-ui-conns-14 G 键紧凑布局与页脚提示
#   04-ui-conns-15 窄终端降级
#   04-ui-conns-16 详情类型行不展示并发数
#   04-ui-conns-17 详情字段名链接样式
#   04-ui-conns-18 点击字段名复制与 toast
Feature: 04-ui-conns UI 对齐 ezr-demo 定稿（头部仪表带 + 单色面积流量图 + 并发连接面板）

  期号 01。依据 project/mission/phase-01.md v1.14（FR-01-94…101、D26/D27，操作者第十/十一批指令）。
  权威参照：ezr-demo/docs/UI交互需求文档-定稿.md v1.4 与 ezr-demo 源码（八轮定稿 + 源码修订-3/5/6/7，提交 5492c7c）。
  端到端口径：全部断言作用于伪终端用户可见画面，不进入项目内部 API。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 终端尺寸为 120×34

  Scenario: 04-ui-conns-01 头部仪表面板通栏 5 行定稿布局
    When 添加若干任务并观察主界面
    Then 头部仪表面板为通栏全宽 5 行（标题边框行 + 3 内容行 + 底边框）且右缘与屏幕右缘重合
    And 标题行为 "◆ EZR Downloader" 加期号版本号且峰值不出现在标题行边框上
    And 全屏不存在「任务队列」面板标题（面板已撤销，FR-01-94）
    And 任务列表自头部正下方起且占满左列全高

  Scenario: 04-ui-conns-02 头部字段单行与峰值图例
    When 观察头部内容行 1
    Then 字段按固定顺序单行排列：并发 → 会话已下载 → 全局 ↓↑ → 峰值 ↓↑
    And 「峰值」写在面板内容里且「峰值 ↓」为下载淡蓝（与流量图曲线同色兼作图例）
    And 「峰值 ↑」为品红

  Scenario: 04-ui-conns-03 页签行并入头部与槽位内联
    When 观察头部内容行 3
    Then 行内容为 正在下载 (n) │ 已完成 (n) │ 下载槽位 x/y
    And 下载槽位内联左对齐、紧跟「已完成」页签并以 │ 分隔（不右对齐行尾）
    And 下载槽位已满时该段黄色加粗（等待任务需排队）
    And Tab 键仍在这两个页签间切换且活动页签黑字青底加粗

  Scenario: 04-ui-conns-04 │ 分隔符与图区留白
    Given 图表处于开启状态（宽终端默认）
    When 观察头部内容区
    Then 字段区与图区之间有一条贯通头部内容区全高的 │ 竖线分隔符
    And 分隔符左侧（字段区侧）与右侧（图区侧）各有 1 列空白
    And 图区右缘再有 1 列空白
    And 头部内容行 2 的横向分割线 ─ 贯通字段区宽、止于 │ 分隔符且图区内不画线

  Scenario: 04-ui-conns-05 单色面积流量图形态
    Given 一个下载中的任务 "<file>"（fixture 慢速门控维持下载观察窗）
    When 连续观察头部右侧流量图 <secs> 秒
    Then 流量图无边框无标题、占头部内容区全高（3 行）、宽为内容区 20%（±2 列）
    And 图区每列均有面积块字符（▁▂▃▄▅▆▇█ 之一）且无断列、全图无盲文点阵字符
    And 全图所有非空格字符为同一颜色（下载淡蓝 RGB(122,185,242)）
    And 顶线随速度变化呈现部分块亚字符精度的连续曲线（顶部含部分块字符）
    And 上传流量不绘制（无独立上行序列）

    Examples:
      | file        | secs |
      | wave-m1.bin | 12   |

  Scenario: 04-ui-conns-06 头部字段真实语义
    Given 多任务下载中
    Then 并发 = 处于下载中（与做种中预留态）任务的连接总数（数据面口径）
    And 会话已下载 = 本次运行累计下载字节（与任务进度增量一致，不积分 EMA 速度）
    And 全局 ↓ = 各下载中任务展示速度之和、全局 ↑ 恒为 0 B/s（01 期）
    And 峰值 ↓/↑ = 会话内速度历史最大值（与速度历史采样一致）

  Scenario: 04-ui-conns-07 主体带定稿布局与详情定高
    When 观察主体带
    Then 左列为任务列表（宽 58%）且占主体带全高
    And 右列顶部为任务详情面板（定高 11 行 = 9 行内容 + 上下边框）
    And 详情面板正下方为并发连接面板且占右列余下全部高度
    And 详情字段与文案仍符合 FR-01-80 v1.11/v1.12 口径（无状态行/速度行、大小行无剩余后缀）

  Scenario: 04-ui-conns-08 并发连接面板 HTTP 三列结构
    Given 下载中的任务 "<file>"（并发 <conns>）
    When 查看并发连接面板
    Then 面板标题为「并发连接」且标题右侧显示活跃 x（活跃 = 速度 > 0 的连接数）
    And 表头为 序号 / 下载速度 / 累计下载 三列（数据列宽 8 右对齐、序号列宽 4、单空格分隔）
    And 数据行按序号 1 基升序且序号与连接 id 一致
    And 面板底部提示行恒定显示 "Ctrl+↑↓ 选择 · Ctrl+B 断开（仅 BT）"
    And 待命连接（速度为 0）下载速度列显示 "-"

    Examples:
      | file       | conns |
      | conns4.bin | 4     |

  Scenario: 04-ui-conns-09 连接速度与累计量口径
    Given 下载中的任务 "<file>"（并发 <conns>，fixture 慢速门控）
    When 连续观察该任务明细 <secs> 秒
    Then 各连接下载速度经平滑显示（无逐帧跳变；采样窗内变化次数 ≤ 采样秒数）
    And 各连接累计下载随传输单调递增且为该连接自本次开始下载起累计字节
    And 连接完成当前块领取新块后累计量保留且速度不归零闪烁
    When 暂停该任务后直接继续（服务器支持断点续传）
    Then 恢复下载后各连接累计量在原值基础上延续累计（不清零）
    When 将该任务置于失败后按 R 重试
    Then 重试开新一次下载后各连接累计量从零重新累计

    Examples:
      | file        | conns | secs |
      | cum-m2.bin  | 3     | 8    |

  Scenario: 04-ui-conns-10 明细可见性状态门槛
    Given 状态为 "<state>" 的任务 "<file>"
    When 查看并发连接面板
    Then 明细为空且面板居中显示「（无并发连接）」且活跃计数为 0
    And 面板不拦截滚轮（滚轮穿透滚动任务列表）

    Examples:
      | state   | file        |
      | 等待中  | gate-q.bin  |
      | 已暂停  | gate-p.bin  |
      | 校验中  | gate-v.bin  |
      | 已失败  | gate-f.bin  |
      | 已完成  | gate-c.bin  |

  Scenario: 04-ui-conns-11 Ctrl+↑↓ 明细选择
    Given 下载中的任务 "<file>"（并发 <conns>）且处于 "<state>" 状态的另一任务 "<ofile>" 也被观察
    When 在明细可见任务上按 Ctrl+↓ ×3 再按 Ctrl+↑
    Then 选中行下移 3 格后上移 1 格且选中行以深青背景 + 首列白字高亮
    And 选中行越出可视范围时明细自动跟随滚动保持可见
    And 连续按 Ctrl+↑/↓ 至边界时选择钳制不越界
    When 切换选中另一任务后再切回
    Then 明细选择已自动复位到首行
    When 在 "<state>" 状态任务上按 Ctrl+↓
    Then toast 显示 "当前状态无并发明细（仅下载中/做种中可选）"

    Examples:
      | file        | conns | state  | ofile       |
      | sel-m4.bin  | 4     | 已暂停 | sel-paused.bin |

  Scenario: 04-ui-conns-12 Ctrl+B 断开守卫链
    Given 下载中的 HTTP 任务 "<file>"（并发 <conns>）与状态为 "<state>" 的任务 "<ofile>"
    When 在下载中任务上按 Ctrl+B
    Then toast 显示 "仅 BT 任务支持断开并发连接（Ctrl+B）" 且明细行数不变
    When 在 "<state>" 状态任务上按 Ctrl+B
    Then toast 显示 "当前状态无可断开的并发连接（仅下载中/做种中）"（状态门槛 toast 先于协议 toast）
    And 对话框打开时 Ctrl+↑↓ 与 Ctrl+B 均不响应且 Ctrl+C 退出不受影响
    And BT 任务的断开实际执行属 phase-02 范围（01 期无 BT 任务，D26）

    Examples:
      | file        | conns | state  | ofile      |
      | disc-m5.bin | 3     | 已暂停 | disc-p.bin |

  Scenario: 04-ui-conns-13 滚轮命中明细与穿透
    Given 下载中的任务 "<file>"（并发 <conns>）且明细行数超出面板可视高度
    When 指针悬停在并发连接面板上滚动滚轮
    Then 明细视口滚动且任务列表不滚动
    When 明细为空（未选中任务或状态门槛生效）时滚动滚轮
    Then 滚轮穿透滚动任务列表

    Examples:
      | file        | conns |
      | wheel64.bin | 16    |

  Scenario: 04-ui-conns-14 G 键紧凑布局与页脚提示
    When 按 G
    Then 任务详情与并发连接面板收起且任务列表占满整行
    And 头部流量图仍显示（不受 G 影响）
    And 页脚快捷键行 G 项提示为「面板」
    When 再按 G
    Then 右栏两面板恢复显示且头部布局不变

  Scenario: 04-ui-conns-15 窄终端降级
    Given 终端尺寸调整为 <cols>×34
    When 观察主界面
    Then 头部仪表面板无图（分割线与页签行贯通全宽）且任务列表满宽
    And 并发明细面板不渲染且布局无溢出错乱
    And 页脚快捷键提示按 <cols> 逐级精简

    Examples:
      | cols |
      | 80   |

  Scenario: 04-ui-conns-16 详情类型行不展示并发数
    Given 下载中的任务 "<file>"（并发 <conns>）与状态为「已暂停」的任务 "<pfile>"
    When 逐个选中并观察详情面板类型行
    Then 两任务的类型行内容均为 协议名 · 支持断点续传/不支持断点续传 且不含「并发」字样与并发数值
    And 下载中任务有活跃连接时类型行同样无并发数（并发信息仅由头部字段与并发连接面板承载）
    And 协议名为黄色加粗且续传说明为普通前景色

    Examples:
      | file        | conns | pfile      |
      | type-m6.bin | 4     | type-p.bin |

  Scenario: 04-ui-conns-17 详情字段名链接样式
    Given 提供了校验码的任务 "<file>" 被选中
    When 观察详情面板字段名配色（逐单元格前景色与下划线修饰断言）
    Then 「校验」与「URL」字段名文字为下载淡蓝 RGB(122,185,242) 且带下划线
    And 下划线仅覆盖字段名文字部分（字段名后的对齐填充空格无下划线）
    And 其余字段名（排队/ID/类型/大小/保存/分块）仍为暗灰且无下划线

    Examples:
      | file         |
      | link-m7.bin  |

  Scenario: 04-ui-conns-18 点击字段名复制与 toast
    Given 提供了校验码 "<ck>" 的任务 "<file>"（详情展示 URL 为 "<url>"）被选中且无对话框
    When 左键点击详情面板「URL」字段名
    Then 复制内容为详情展示的 URL 值 "<url>" 且 toast 显示 "已复制 url"
    When 左键点击「校验」字段名
    Then 复制内容为完整校验码 "<ck>"（非 10 位截断展示值）且 toast 显示 "已复制 校验码"
    When 选中无校验码任务 "<nock>" 并点击详情面板原校验行字段名区域
    Then 该处无热区（点击无动作、剪贴板与 toast 均不变）
    When 按 G 收起右栏两面板后在原「URL」字段名位置左键点击
    Then 点击无动作（热区已随面板收起清空）
    When 打开任意对话框后点击「URL」字段名
    Then 点击被对话框分支消费且剪贴板与 toast 均不变

    Examples:
      | file        | ck                                   | url                                | nock          |
      | copy-m7.bin | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 | http://127.0.0.1:<port>/copy-m7.bin | copy-nock.bin |
