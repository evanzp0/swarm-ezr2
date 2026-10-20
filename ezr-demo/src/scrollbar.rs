//! scrollbar.rs — 通用滚动栏组件（垂直 ↑║↓ / 水平 ←═→，全系统共用）
//!
//! 所有 panel / 弹窗的滚动条都由本模块渲染与命中测试（§4.1.3 通用滚动条）：
//!
//! * **字形组合**：滑块用双线字符 `═`（水平）/ `║`（垂直）——**可滑动部分**，
//!   在与面板边框同粗细的**单线轨道** `─` / `│` 上滑动，两端配 `←`/`→`、
//!   `↑`/`↓` 箭头；滑块加粗，颜色 = 所属面板边框色（聚焦面板/弹窗绿、
//!   未聚焦面板暗），轨道/箭头弱化（DIM2/DIM）。
//! * **位置与留白（分轴向规则）**：垂直画在面板**右边框列**、水平画在
//!   **下边框行**。水平箭头**前后两侧**各留 **1 格空白**——外侧靠边框线
//!   （角）、内侧靠轨道：`╰ 空白 ← 空白 ═══─── 空白 → 空白 ╯`；垂直箭头
//!   **紧贴**角与轨道、**不留空白**：`╮↑│║│↓╯`——垂直边框列必须连续
//!   （中断的竖线在视觉上等同边框破损），且垂直空间远比水平金贵，去掉
//!   四格留白后轨道显著更长。
//! * **滑块行程**：滑块在箭头之间的整段轨道上滑动；留白恒定，与滚动位置
//!   无关（水平滑块永不接触箭头，箭头也不接触边框线）。
//! * **显示条件**：`total > page` 且轨道 ≥ 2 格（水平面板宽 ≥ 10 = 2 角
//!   + 2×2 留白 + 2 箭头 + 2 轨道；垂直面板高 ≥ 6 = 2 角 + 2 箭头 +
//!   2 轨道）；轨道过小的极矮/极窄面板不绘制（滚轮/键盘仍可滚动），
//!   避免滑块填满轨道、无法移动的退化态。
//! * **交互契约**（全部实例同一口径，鼠标 + 键盘双通道）：
//!   - 滑块 `═`/`║` 按住拖拽 = 比例滚动内容（浮层内同样滚内容而非移动窗口）；
//!   - 箭头 `↑↓←→` 单击 = 步进 1 单位；**按住不放自动连发**
//!     （0.4s 起振、0.12s 周期，App::tick 驱动）；
//!   - 箭头按压**高亮反馈**：按下/按住 = 前景色换成**滑块同款**
//!     （thumb_color 加粗，无背景色——视觉上箭头短暂变成滑块的样子）；
//!     按住期间指针拖出箭头 = 取消高亮（连发不中断），拖回恢复；
//!     松开后 400ms 余辉（快速单击也有可见点击效果）；
//!   - 轨道单击 = 朝点击方向整页滚动（可按住连发）；
//!   - 键盘方向键 = 面板聚焦时 ↑↓ 纵向（A 列表为选择跟随）、←→ 横向
//!     （并发连接面板）。
//!
//! 渲染与命中测试共用同一份 [`Bar`] 几何（ui 层每帧回填给 App），保证
//! 「画在哪里就能点哪里」，布局变化零延迟。

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    Frame,
};

use crate::ui::{DIM, DIM2};

/// 水平箭头**外侧**（靠边框线/角）的留白格数。垂直方向**不留白**——
/// 垂直边框列保持连续（`╮↑│║│↓╯`），中断的竖线在视觉上等同边框破损，
/// 且垂直空间金贵（§4.1.3 分轴向规则）。
const EDGE_PAD: u16 = 1;

/// 水平箭头**内侧**（靠轨道）的留白格数：箭头前后两侧各 1 格空白——外侧
/// 靠边框（[`EDGE_PAD`]）、内侧靠轨道（本常量）。垂直方向同样不留白。
const ARROW_PAD: u16 = 1;

/// 箭头按压态（渲染高亮用）：Head = 上/左箭头，Tail = 下/右箭头。
/// 与 [`Hit::Head`]/[`Hit::Tail`] 同语义，但不携带轨道/滑块负载——
/// App 层把「按住的箭头」回传给渲染层即可。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrow {
    /// 上/左箭头
    Head,
    /// 下/右箭头
    Tail,
}

/// 滚动条命中区域（Down 事件分类；渲染与命中同一几何）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    /// 上/左箭头：单击或按住 = 反向步进
    Head,
    /// 下/右箭头：单击或按住 = 正向步进
    Tail,
    /// 轨道空白（值 = 轨道格下标）：单击 = 朝点击方向整页滚动
    Track(usize),
    /// 滑块（值 = 抓取点在滑块内的格偏移）：按住拖拽 = 比例滚动
    Thumb(usize),
}

/// 通用滚动条几何（ui 层每帧构建；渲染、命中、拖拽反解共用）
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    /// 所属面板/弹窗完整矩形（含边框；滚动条画在其边框线上）
    pub panel: Rect,
    pub vertical: bool,
    /// 内容总单位数（任务条目/连接行/网格行/文本行/内容列）
    pub total: usize,
    /// 可视单位数（一屏能看到的单位数；水平 = 内框宽度列数）
    pub page: usize,
    /// 当前滚动偏移（渲染前钳制到 0..=max_pos）
    pub pos: usize,
}

impl Bar {
    /// 垂直构造器
    pub fn vertical(panel: Rect, total: usize, page: usize, pos: usize) -> Self {
        Self { panel, vertical: true, total, page, pos }
    }

    /// 水平构造器
    pub fn horizontal(panel: Rect, total: usize, page: usize, pos: usize) -> Self {
        Self { panel, vertical: false, total, page, pos }
    }

    /// 最大滚动偏移（末页起点）
    pub fn max_pos(&self) -> usize {
        self.total.saturating_sub(self.page)
    }

    /// 本轴箭头**外侧**留白格数：水平 = [`EDGE_PAD`]（1），垂直 = 0（紧贴）
    fn edge_pad(&self) -> u16 {
        if self.vertical {
            0
        } else {
            EDGE_PAD
        }
    }

    /// 本轴箭头**内侧**留白格数：水平 = [`ARROW_PAD`]（1），垂直 = 0（紧贴）
    fn arrow_pad(&self) -> u16 {
        if self.vertical {
            0
        } else {
            ARROW_PAD
        }
    }

    /// 箭头占用区间端点（含）：垂直 = 行号范围，水平 = 列号范围。
    /// 两端箭头与同侧角（上/下、左/右）之间各空本轴 [`Self::edge_pad`]
    /// 格留白——水平 1 格、垂直 0 格（紧贴，§4.1.3 分轴向规则）。
    fn arrow_span(&self) -> (u16, u16) {
        let pad = self.edge_pad();
        if self.vertical {
            (
                self.panel.y + 1 + pad,
                self.panel.y + self.panel.height.saturating_sub(2 + pad),
            )
        } else {
            (
                self.panel.x + 1 + pad,
                self.panel.x + self.panel.width.saturating_sub(2 + pad),
            )
        }
    }

    /// 滑块/轨道区间起点（第一轨道格的全局坐标；= 头箭头 + 内侧留白之后）
    fn track_origin(&self) -> u16 {
        self.arrow_span().0 + 1 + self.arrow_pad()
    }

    /// 轨道格数（扣除两端箭头与箭头内侧留白；外侧留白在箭头与边框线之间）
    pub fn track_len(&self) -> usize {
        let (a, b) = self.arrow_span();
        // 闭区间 [a, b] 共 b-a+1 格，扣除 2 格箭头 + 两端各 arrow_pad 格留白
        ((b.saturating_sub(a) + 1) as usize)
            .saturating_sub(2 + 2 * self.arrow_pad() as usize)
    }

    /// 滑块长度（按可视占比取整，至少 1 格，占比基准 = 整段轨道）。
    /// **行程保证**：`max_pos > 0` 时滑块至多占 track−1 格——内容可滚则滑块
    /// 必可动（行程 ≥ 1），防止取整后滑块填满轨道的退化态；箭头两侧留白
    /// 恒定，滑块永不接触箭头/边框（视觉上始终与箭头隔 1 格空白）。
    pub fn thumb_len(&self) -> usize {
        let track = self.track_len();
        if track == 0 {
            return 0;
        }
        let raw = (((self.page as f64) / (self.total as f64)) * track as f64).round() as usize;
        let mut len = raw.clamp(1, track);
        if self.max_pos() > 0 && track >= 2 {
            len = len.min(track - 1);
        }
        len
    }

    /// 滑块行程（轨道长 − 滑块长）
    pub fn travel(&self) -> usize {
        self.track_len().saturating_sub(self.thumb_len())
    }

    /// 滑块在轨道内的起点（0 基）。顶/底端吸附：pos==0 贴头侧留白内侧、
    /// 末页贴尾侧留白内侧，中间按比例取整（round 保证拖拽正反解一致）。
    /// 水平/垂直同一公式。
    pub fn thumb_pos(&self) -> usize {
        let max = self.max_pos();
        let travel = self.travel();
        if max == 0 || travel == 0 {
            return 0;
        }
        if self.pos >= max {
            return travel;
        }
        ((self.pos as f64 / max as f64) * travel as f64).round() as usize
    }

    /// 滑块拖拽的反函数：滑块轨道位置 → 滚动偏移（[`Bar::thumb_pos`] 的逆）
    pub fn pos_from_thumb(&self, thumb_at: usize) -> usize {
        let max = self.max_pos();
        let travel = self.travel();
        if max == 0 || travel == 0 {
            return 0;
        }
        let at = thumb_at.clamp(0, travel);
        ((at as f64 / travel as f64) * max as f64).round() as usize
    }

    /// 命中测试：(col, row) → [`Hit`]；不在滚动条上返回 None。
    /// 垂直只认右边框列、水平只认下边框行，且限定在箭头/轨道区间内——
    /// 箭头两侧的留白格不可点（纯视觉留白，点击无效果）。
    pub fn hit(&self, col: u16, row: u16) -> Option<Hit> {
        if self.total <= self.page || self.page == 0 || self.track_len() < 2 {
            return None;
        }
        let (a, b) = self.arrow_span();
        let along = if self.vertical {
            if col != self.panel.x + self.panel.width.saturating_sub(1) {
                return None;
            }
            if row < a || row > b {
                return None;
            }
            row
        } else {
            if row != self.panel.y + self.panel.height.saturating_sub(1) {
                return None;
            }
            if col < a || col > b {
                return None;
            }
            col
        };
        if along == a {
            return Some(Hit::Head);
        }
        if along == b {
            return Some(Hit::Tail);
        }
        // 箭头内侧留白（a+1..a+1+arrow_pad、b-arrow_pad..b）不可点（垂直无留白）
        let org = self.track_origin();
        if along < org || (along - org) as usize >= self.track_len() {
            return None;
        }
        let t = (along - org) as usize; // 轨道下标 0..track_len
        let tp = self.thumb_pos();
        let tl = self.thumb_len();
        if t >= tp && t < tp + tl {
            Some(Hit::Thumb(t - tp))
        } else {
            Some(Hit::Track(t))
        }
    }

    /// 指针在轨道内的下标（拖拽/翻页方向判定用；越界钳制到轨道内）
    pub fn track_index_at(&self, col: u16, row: u16) -> usize {
        let along = if self.vertical { row } else { col };
        let len = self.track_len();
        (along.saturating_sub(self.track_origin()) as usize).min(len)
    }
}

/// 渲染通用滚动条（画在 `bar.panel` 的右边框列 / 下边框行上）。
///
/// * `thumb_color`：滑块颜色 = 所属面板边框色（聚焦绿加粗 / 未聚焦暗）；
///   箭头 DIM、单线轨道 DIM2（轨道 `─`/`│` 与面板边框同字形同粗细）。
/// * `pressed`：按压中的箭头（Some = 前景色换成滑块同款——thumb_color
///   加粗，无背景色；None = 全部箭头常规态 DIM 前景）。
/// * 滑块 = 双线字符 `═`/`║`（加粗）在单线轨道上滑动；水平箭头前后两侧
///   各画 [`EDGE_PAD`]/[`ARROW_PAD`] 格**空白**（真留白，覆盖原边框线/
///   轨道区），垂直箭头紧贴角与轨道不留白——分轴向规则见模块文档。
pub fn render(f: &mut Frame, bar: &Bar, thumb_color: Color, pressed: Option<Arrow>) {
    if bar.page == 0 || bar.total <= bar.page {
        return;
    }
    // 轨道至少 2 格：水平面板宽 ≥ 10（2 角 + 2×2 留白 + 2 箭头 + 2 轨道）、
    // 垂直面板高 ≥ 6（2 角 + 2 箭头 + 2 轨道，不留白）——轨道 1 格时滑块
    // 必然填满（无法移动），不绘制，退化为滚轮/键盘滚动
    let min_len = if bar.vertical {
        2 + 2 + 2
    } else {
        2 + 2 * EDGE_PAD + 2 + 2 * ARROW_PAD + 2
    };
    let span_ok = if bar.vertical {
        bar.panel.height >= min_len && bar.panel.width > 0
    } else {
        bar.panel.width >= min_len && bar.panel.height > 0
    };
    if !span_ok {
        return;
    }
    let (a, b) = bar.arrow_span();
    let arrow_style = Style::default().fg(DIM);
    let track_style = Style::default().fg(DIM2);
    let thumb_style = Style::default().fg(thumb_color).add_modifier(Modifier::BOLD);
    // 按压高亮 = 箭头前景色换成滑块同款（thumb_color + 加粗），无背景色
    let pressed_style = thumb_style;
    let blank_style = Style::default();
    // 滑块 = 双线字符（═/║，可滑动），轨道 = 单线字符（─/│，与面板边框同粗细）
    let (head, tail, track_glyph, thumb_glyph) = if bar.vertical {
        ("↑", "↓", "│", "║")
    } else {
        ("←", "→", "─", "═")
    };
    let tp = bar.thumb_pos();
    let tl = bar.thumb_len();
    let fixed = if bar.vertical {
        bar.panel.x + bar.panel.width.saturating_sub(1)
    } else {
        bar.panel.y + bar.panel.height.saturating_sub(1)
    };
    // 绘制范围 = 箭头区间 [a, b] + 两端各本轴 edge_pad 格留白（覆盖原边框线；
    // 垂直不留白 = 恰为箭头区间）
    let ep = bar.edge_pad();
    let lo = a.saturating_sub(ep);
    let hi = b.saturating_add(ep);
    let org = bar.track_origin();
    let track_end = org + bar.track_len() as u16; // 首个超出轨道的坐标
    for p in lo..=hi {
        let (sym, style) = if p < a || p > b {
            (" ", blank_style) // 箭头外侧留白（箭头 ↔ 边框线；仅水平）
        } else if p == a {
            (
                head,
                if pressed == Some(Arrow::Head) {
                    pressed_style
                } else {
                    arrow_style
                },
            )
        } else if p == b {
            (
                tail,
                if pressed == Some(Arrow::Tail) {
                    pressed_style
                } else {
                    arrow_style
                },
            )
        } else if p < org || p >= track_end {
            (" ", blank_style) // 箭头内侧留白（箭头 ↔ 轨道；仅水平）
        } else {
            let t = (p - org) as usize; // 轨道下标
            if t >= tp && t < tp + tl {
                (thumb_glyph, thumb_style)
            } else {
                (track_glyph, track_style)
            }
        };
        let (col, row) = if bar.vertical {
            (fixed, p)
        } else {
            (p, fixed)
        };
        if let Some(cell) = f.buffer_mut().cell_mut((col, row)) {
            cell.set_symbol(sym);
            cell.set_style(style);
        }
    }
}
