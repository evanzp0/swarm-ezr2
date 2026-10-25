//! mouse — 鼠标事件：点击选中/聚焦/滚轮/对话框按钮命中

use std::time::Instant;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::dialog_keys::text_field_at;
use super::{App, DialogKind, Panel, SbKey, ITEM_HEIGHT};
use crate::model::checksum::CK_NONE;
// 点击列 → 字段文本字符偏移（v1.34 三批 bug 修订：鼠标点击在点击处落
// 光标）——实现已收口为输入框组件单源 [`TextInput::click_offset`]（映射 =
// 渲染层发布的 [`FieldWin`] 累积列，与显示取窗同源，不在鼠标层重算宽度）。
use crate::textfield::TextInput;

impl App {
    /// 鼠标事件入口（architect v116：分发核保持行为拆分——对话框态与主界面
    /// 命中各自成函数，消除早退 return 穿越两分支的控制流混杂；分支语义
    /// 不变：浮层打开时一切鼠标事件仅作用于最上层浮层。
    /// v1.32/FR-01-111：关于窗口最上层（关于 > 帮助 > pieces > 对话框 >
    /// 主界面））
    pub fn on_mouse(&mut self, m: MouseEvent) {
        // 指针位置跟踪（v1.34/FR-01-113：任意鼠标事件均携带坐标，供详情字段值
        // 悬停下划线与 [✗] 悬停高亮判定；TUI 无法感知指针离开终端，悬停态
        // 以最后一次上报位置为准，对齐 ezr-demo 重构版 on_mouse）
        self.pointer = Some((m.column, m.row));
        if self.val_open {
            self.on_mouse_val(m);
            return;
        }
        if self.about_open {
            self.on_mouse_about(m);
            return;
        }
        if self.help_open {
            self.on_mouse_help(m);
            return;
        }
        if self.pieces_open {
            self.on_mouse_pieces(m);
            return;
        }
        if self.dialog.is_some() {
            self.on_mouse_dialog(m);
        } else {
            self.on_mouse_main(m);
        }
    }

    /// 关于窗口命中（v1.32/FR-01-111，对齐 demo on_mouse about 分支）：
    /// 左键按住窗口内任意位置拖拽移动；点击窗口外部 = 无任何操作
    ///（模态统一口径：不关闭、不穿透到主界面；Esc/O/q/Q 才是关闭通道）。
    /// 无滚动条无滚轮内容。
    fn on_mouse_about(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // 模态接管：清除页脚 chip 按压态（页脚不可达）
                self.footer_press = None;
                // 右上角 [✗]（v1.34/FR-01-114）：按下高亮（原地松开关闭窗口）
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if pointer_in(&self.about_area, m.column, m.row) {
                    // 开始拖拽：记录指针相对窗口左上角的抓取偏移
                    if let Some(r) = self.about_area {
                        self.about_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
                // 点击窗口外部 = 无任何操作（模态统一口径）
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 拖拽移动：窗口左上角 = 指针 - 抓取偏移（渲染层钳制在终端内）
                if let Some((dx, dy)) = self.about_drag {
                    self.about_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.about_drag = None;
                // [✗] 原地松开 = 关闭关于窗口；拖出热区松开 = 取消（FR-01-114）
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_about();
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    /// 帮助窗口命中（v1.30/FR-01-107；v1.32 修订对齐 ezr-demo m3）：滚轮滚动
    /// 内容（每滚 1 行，帮助步长 ±1）；Down **先查滚动条命中**（滚动条优先于
    /// 窗口拖拽——滑块拖拽滚内容不移窗，m3 修复，FR-01-109 ⑥），未命中时
    /// 窗口内按下 = 开始拖拽移动；点击窗口外部 = 无任何操作（模态统一口径：
    /// 不关闭不穿透；Esc/H/q/Q 才是关闭通道）。
    fn on_mouse_help(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => self.help_scroll += 1, // 渲染层钳制
            MouseEventKind::Down(MouseButton::Left) => {
                // 模态接管：清除页脚 chip 按压态（页脚不可达）
                self.footer_press = None;
                // 右上角 [✗]（v1.34/FR-01-114）：按下高亮（原地松开关闭窗口）
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                // 滚动条命中优先（FR-01-109 ⑥：滑块/箭头/轨道交互，不误移窗口）
                if let Some(bar) = SbKey::Help.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::Help, &bar, hit);
                        return;
                    }
                }
                if pointer_in(&self.help_area, m.column, m.row) {
                    // 开始拖拽：记录指针相对窗口左上角的抓取偏移
                    if let Some(r) = self.help_area {
                        self.help_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
                // 点击窗口外部 = 无任何操作（v1.32 模态统一口径，原「点外关闭」退役）
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 按住的箭头拖出/拖回高亮跟随（连发不中断，FR-01-109 ⑥）
                self.sb_arrow_follow(m.column, m.row);
                if self.sb_drag.is_some() {
                    // 滑块拖拽 = 比例滚动内容（不移窗口，m3 修复）
                    self.sb_drag_move(m.column, m.row);
                } else if let Some((dx, dy)) = self.help_drag {
                    // 窗口拖拽移动：左上角 = 指针 - 抓取偏移（渲染层钳制在终端内）
                    self.help_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.help_drag = None;
                // [✗] 原地松开 = 关闭帮助窗口；拖出热区松开 = 取消（FR-01-114）
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.help_open = false;
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    /// pieces 弹窗命中（FR-01-106 ⑦；v1.32 修订对齐 ezr-demo m3）：滚轮滚动
    /// 网格（每滚 2 行）；Down **先查滚动条命中**（滚动条优先于窗口拖拽——
    /// 滑块拖拽滚网格不移窗，m3 修复，FR-01-109 ⑥），未命中时窗口内按下 =
    /// 开始拖拽；点击窗口外部 = 无任何操作（v1.32 模态统一口径：不关闭、
    /// 不穿透任务列表；关闭通道 = P/Esc/q/Q，原「点外关闭」退役）；
    /// 拖拽移动弹窗（位置钳制在渲染层）；松开结束。
    fn on_mouse_pieces(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.pieces_scroll_by(-2),
            MouseEventKind::ScrollDown => self.pieces_scroll_by(2),
            MouseEventKind::Down(MouseButton::Left) => {
                // 模态接管：清除页脚 chip 按压态（页脚不可达）
                self.footer_press = None;
                // 右上角 [✗]（v1.34/FR-01-114）：按下高亮（原地松开关闭弹窗）
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                // 滚动条命中优先（FR-01-109 ⑥：滑块/箭头/轨道交互，不误移窗口）
                if let Some(bar) = SbKey::Pieces.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::Pieces, &bar, hit);
                        return;
                    }
                }
                if pointer_in(&self.pieces_area, m.column, m.row) {
                    // 开始拖拽：记录指针相对弹窗左上角的抓取偏移
                    if let Some(r) = self.pieces_area {
                        self.pieces_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
                // 点击弹窗外部 = 无任何操作（v1.32 模态统一口径）
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 按住的箭头拖出/拖回高亮跟随（连发不中断，FR-01-109 ⑥）
                self.sb_arrow_follow(m.column, m.row);
                if self.sb_drag.is_some() {
                    // 滑块拖拽 = 比例滚网格（不移窗口，m3 修复）
                    self.sb_drag_move(m.column, m.row);
                } else if let Some((dx, dy)) = self.pieces_drag {
                    // 窗口拖拽移动：左上角 = 指针 - 抓取偏移（渲染层钳制在终端内）
                    self.pieces_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.pieces_drag = None;
                // [✗] 原地松开 = 关闭分块弹窗；拖出热区松开 = 取消（FR-01-114）
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_pieces();
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    /// 字段值弹窗命中（v1.34/FR-01-113，L6 最上层模态）：滚轮滚动内容（±1 行）、
    /// Down 先查滚动条命中（滑块拖拽滚内容不移窗）→ [✗] 按下高亮 → 窗口内
    /// 按下 = 开始拖拽移动；点击窗口外部 = 无任何操作（模态统一口径）；
    /// Esc/q/Q/[✗] 关闭（关闭即清位置，同 pieces 口径）。内容为纯静态文本，
    /// 点击内容不触发复制。
    fn on_mouse_val(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.sb_scroll(SbKey::Val, -1),
            MouseEventKind::ScrollDown => self.sb_scroll(SbKey::Val, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                // 模态接管：清除页脚 chip 按压态（页脚不可达）
                self.footer_press = None;
                // 滚动条命中优先（FR-01-109 ⑥：滑块/箭头/轨道交互，不误移窗口）
                if let Some(bar) = SbKey::Val.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::Val, &bar, hit);
                        return;
                    }
                }
                // 右上角 [✗]（FR-01-114）：按下高亮（原地松开关闭弹窗）
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if pointer_in(&self.val_area, m.column, m.row) {
                    // 开始拖拽：记录指针相对窗口左上角的抓取偏移
                    if let Some(r) = self.val_area {
                        self.val_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
                // 点击窗口外部 = 无任何操作（模态统一口径）
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 按住的箭头拖出/拖回高亮跟随（连发不中断，FR-01-109 ⑥）
                self.sb_arrow_follow(m.column, m.row);
                if self.sb_drag.is_some() {
                    // 滑块拖拽 = 比例滚动内容（不移窗口，m3 修复）
                    self.sb_drag_move(m.column, m.row);
                } else if let Some((dx, dy)) = self.val_drag {
                    // 窗口拖拽移动：左上角 = 指针 - 抓取偏移（渲染层钳制在终端内）
                    self.val_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.val_drag = None;
                // [✗] 原地松开 = 关闭字段值弹窗；拖出热区松开 = 取消（FR-01-114）
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_val_popup();
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    /// 对话框态命中（仅左键生效；下拉浮层优先，其次字段行/按钮行）。
    /// v133/D38：窗口内非交互区（边框/标题/空白——未命中字段行与按钮）
    /// 左键按下 = 开始拖拽，Drag 随动移动窗口（渲染层钳制终端内），Up
    /// 结束；字段/按钮/下拉项点击语义不变，点击窗口外无动作（对话框
    /// 仅经按钮/`Esc`/`Q` 关闭，与帮助/pieces 的点外关闭口径不同）。
    fn on_mouse_dialog(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let kind = self.dialog.as_ref().map(|d| d.kind);
                let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);
                // 模态接管：清除页脚 chip 按压态（v1.31/FR-01-107①）
                self.footer_press = None;
                // 下拉展开时点击 [✗] 位置先收起下拉（FR-01-114 ③：下拉浮层
                // 不设 [✗]，热区被浮层覆盖；与下拉「点外收起」同一优先级）
                if (ck_open || proxy_open) && pointer_in(&self.popup_close_rect, m.column, m.row) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                        d.proxy_open = false;
                    }
                    return;
                }
                // 右上角 [✗]（v1.34/FR-01-114）：按下高亮（原地松开 = 取消对话框，
                // 与 Esc 同口径）
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                let hit = |rects: &[(Rect, usize)]| rect_hit(rects, m.column, m.row);
                if ck_open {
                    let ck_rects = self.dlg_ck_rects.clone();
                    if let Some(i) = hit(&ck_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.ck_type = i;
                        d.ck_sel = i;
                        // v1.32/FR-01-01：选中 NONE（点击）→ 校验码实时清空
                        //（与键盘选择同口径）
                        if d.ck_type == CK_NONE {
                            d.ck_value.clear();
                        }
                        d.ck_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                    }
                    return;
                }
                // 代理下拉（v1.5/FR-01-86）：点击选项选择；点外关闭
                if proxy_open {
                    let proxy_rects = self.dlg_proxy_rects.clone();
                    if let Some(i) = hit(&proxy_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.proxy_sel = i;
                        d.proxy_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.proxy_open = false;
                    }
                    return;
                }
                let field_rects = self.dlg_field_rects.clone();
                let btn_rects = self.dlg_btn_rects.clone();
                if let Some(i) = hit(&field_rects) {
                    // 下拉行下标：Add 校验=3 / Modify 校验=1；代理行恒为末二
                    //（Add 5 / Modify 3）
                    let ck_i = if kind == Some(DialogKind::Add) { 3 } else { 1 };
                    let proxy_i = if kind == Some(DialogKind::Add) { 5 } else { 3 };
                    let d = self.dialog.as_mut().unwrap();
                    // 焦点切换单源（focus_field）：光标随迁字段尾（点击聚焦
                    // 惯例；命中下拉行时随后展开下拉覆盖）
                    d.focus_field(i);
                    // v1.34 三批 bug 修订：文本字段点击 = 光标落点击处
                    //（渲染层窗口几何在位时列→字符偏移映射；非文本行
                    // win=None → 维持 focus_field 迁尾语义）
                    if let Some(win) = self.dlg_field_wins.get(i).and_then(|w| w.as_ref()) {
                        if let Some(f) = text_field_at(d.kind, i) {
                            d.field_mut(f)
                                .set_cursor(TextInput::click_offset(win, m.column));
                        }
                    }
                    if i == ck_i {
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    } else if i == proxy_i {
                        d.proxy_open = true;
                    }
                } else if let Some(btn) = hit(&btn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus_field(btn);
                    }
                    match kind {
                        Some(DialogKind::Add) => self.dlg_activate_add(btn),
                        Some(DialogKind::Modify) => self.dlg_activate_modify(btn),
                        Some(DialogKind::Delete) => self.dlg_activate_delete(btn),
                        None => {}
                    }
                } else if pointer_in(&self.dlg_area, m.column, m.row) {
                    // 非交互区按下 = 开始拖拽：记录指针相对窗口左上角的抓取偏移
                    if let Some(r) = self.dlg_area {
                        self.dlg_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 拖拽移动：窗口左上角 = 指针 - 抓取偏移（渲染层钳制在终端内）
                if let Some((dx, dy)) = self.dlg_drag {
                    self.dlg_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.dlg_drag = None;
                // [✗] 原地松开 = 取消对话框（关闭通道与各实例 Esc 同口径，
                // FR-01-114 ③）；拖出热区松开 = 取消按压不关闭
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        // 取消对话框（与各实例 Esc 同口径：直接关闭不执行）
                        self.dialog = None;
                    }
                }
            }
            _ => {}
        }
    }

    /// 主界面命中（页签点击切换 / 详情热区复制与聚焦 / 并发面板聚焦与选行 /
    /// 列表点击聚焦与选中 / 滚轮只移视口不动选中，v1.30/FR-01-108/109）。
    /// architect v151 分发核保持行为拆分（on_mouse v116 同法在主界面命中内
    /// 复用）：Down/Up/滚轮各自成函数，分支语义不变——按下命中链 = 页脚 →
    /// 页签 → 详情热区 → 滚动条 → 并发面板 → 列表（顺序即优先级，与拆分前
    /// 逐 if 落空穿过完全一致）。
    fn on_mouse_main(&mut self, m: MouseEvent) {
        // 指针命中并发连接面板（FR-01-96：明细非空时面板接收滚轮）→ 滚动明细；
        // 空明细 / 未选中任务时 conns_area 为 None → 滚轮穿透滚动任务列表
        let over_conns = pointer_in(&self.conns_area, m.column, m.row);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => self.on_mouse_main_down(&m),
            MouseEventKind::Up(MouseButton::Left) => self.on_mouse_main_up(&m),
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.on_mouse_main_scroll(&m, over_conns);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 按住的箭头拖出/拖回高亮跟随（连发不中断，v1.32/FR-01-109 ⑥）；
                // 滑块拖拽 = 比例滚动内容（主面板无窗口拖拽语义）
                self.sb_arrow_follow(m.column, m.row);
                self.sb_drag_move(m.column, m.row);
            }
            _ => {}
        }
    }

    /// 主界面左键按下命中链（architect v151 自 [`Self::on_mouse_main`] 拆出）：
    /// 各段命中即消费（返回 true = 事件已处理）；顺序 = 命中优先级。
    fn on_mouse_main_down(&mut self, m: &MouseEvent) {
        if self.main_down_footer(m) {
            return;
        }
        if self.main_down_tab(m) {
            return;
        }
        if self.main_down_detail_hits(m) {
            return;
        }
        // 通用滚动条命中（v1.32/FR-01-109 ⑥：按下滚动条 = 交互同时
        // 聚焦所属面板；滑块拖拽/箭头步进连发/轨道翻页，sb_press_at
        // 依实例序检查 列表 → 并发垂直 → 并发水平；留白格不可点）
        if self.sb_press_at(m.column, m.row) {
            return;
        }
        if self.main_down_conns(m) {
            return;
        }
        self.main_down_list(m);
    }

    /// 页脚命令 chips 命中（v1.31/FR-01-107①）：Down 按压反色高亮
    ///（点击效果 + 400ms 余辉）；返回 true = 命中（原地松开时执行，见
    /// [`Self::on_mouse_main_up`]）。未命中时清除按压态（按压后拖到
    /// 别处再按下 = 取消上一次按压，与拆分前逐 if 穿越语义一致）。
    fn main_down_footer(&mut self, m: &MouseEvent) -> bool {
        // 命中矩形不含分隔符、ui 层每帧重建（footer_cmd_rects）
        if let Some(i) = rect_hit(&self.footer_cmd_rects, m.column, m.row) {
            self.footer_press = Some(i);
            self.footer_flash = Some((i, Instant::now() + std::time::Duration::from_millis(400)));
            return true;
        }
        self.footer_press = None;
        false
    }

    /// 页签命中（ui 层每帧回填 tab_rects）：切换到该页签 + 聚焦列表
    ///（v1.30/FR-01-80：鼠标点击页签切换）
    fn main_down_tab(&mut self, m: &MouseEvent) -> bool {
        let tab_rects = self.tab_rects.clone();
        if let Some(i) = rect_hit(&tab_rects, m.column, m.row) {
            if i != self.filter {
                self.filter = i;
                self.scroll_to_selection();
            }
            // 页签归属任务列表面板：点击页签同时聚焦列表
            self.focus = Panel::List;
            return true;
        }
        false
    }

    /// 详情面板热区命中（字段名点击复制 ×4 / 字段值点击弹窗 / 面板聚焦）。
    /// FR-01-101：字段名点击复制——热区由 ui 层每帧回填（None = 无热区：
    /// 无校验行 / 面板收起 / 窄终端）；面板本身无提示信息，反馈仅在底部
    /// toast（文案照录 demo 原文，D26 同口径）；点击复制同时聚焦详情面板
    ///（v1.30/FR-01-108 ⑤）。
    fn main_down_detail_hits(&mut self, m: &MouseEvent) -> bool {
        // D27 v1.28：复制详情展示值 = 原始 URL 单值（任务级最终 URL
        // 已退役，FR-01-14 v1.28；「复制所见值」锚定不变）
        if pointer_in(&self.detail_url_rect, m.column, m.row) {
            let v = self.sel_task().map(|t| t.url.clone());
            self.copy_detail_value(v, "已复制 url");
            return true;
        }
        if pointer_in(&self.detail_ck_rect, m.column, m.row) {
            let v = self
                .sel_task()
                .and_then(|t| t.checksum.as_ref().map(|c| c.value.clone()));
            self.copy_detail_value(v, "已复制 校验码");
            return true;
        }
        // v1.34/FR-01-101 四项扩展：失败/路径 字段名点击复制
        //（toast 描述被复制的值，D42 钦定文案）
        if pointer_in(&self.detail_fail_rect, m.column, m.row) {
            let v = self
                .sel_task()
                .map(|t| t.error.clone().unwrap_or_else(|| "未知错误".to_string()));
            self.copy_detail_value(v, "已复制 失败原因");
            return true;
        }
        if pointer_in(&self.detail_save_rect, m.column, m.row) {
            let v = self.sel_task().map(|t| t.target_path());
            self.copy_detail_value(v, "已复制 保存路径");
            return true;
        }
        // v1.34/FR-01-113：字段值点击打开完整值弹窗（字段名点击仍是复制，
        // 两种热区互不重叠；热区由 ui 层每帧回填 detail_val_rects）
        if let Some(field) = rect_hit(&self.detail_val_rects, m.column, m.row) {
            self.open_val_popup(field);
            return true;
        }
        // 详情面板任意处点击 = 聚焦（FR-01-108 ⑤）
        if pointer_in(&self.detail_area, m.column, m.row) {
            self.focus = Panel::Detail;
            return true;
        }
        false
    }

    /// 详情字段名复制的共用收口（architect v151 单源：四个字段命中臂同形
    /// 尾段——聚焦详情 + 有值才复制 + toast；无值仍聚焦，与拆分前一致）。
    fn copy_detail_value(&mut self, v: Option<String>, toast: &str) {
        self.focus = Panel::Detail;
        if let Some(v) = v {
            self.copy_to_clipboard(&v);
            self.set_toast(toast);
        }
    }

    /// 并发连接面板点击 = 聚焦 + 选行（表头 1 行之下为数据行，
    /// v1.30/FR-01-108 ⑤）
    fn main_down_conns(&mut self, m: &MouseEvent) -> bool {
        if let Some(a) = self.conns_area {
            let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
            let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
            if in_x && in_y {
                self.focus = Panel::Conns;
                let data_top = a.y + 1; // 表头行之下
                if m.row >= data_top {
                    let idx = (m.row - data_top) as usize;
                    if idx < self.visible_conns_rows {
                        let n = self.conn_count();
                        let ci = self.conns_scroll + idx;
                        if ci < n {
                            self.conns_sel = ci;
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    /// 列表面板点击 = 聚焦列表 + 选中任务（FR-01-108 ⑤）
    fn main_down_list(&mut self, m: &MouseEvent) {
        if let Some(a) = self.list_area {
            let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
            let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
            if in_x && in_y {
                self.focus = Panel::List;
                let idx = (m.row - a.y) as usize / ITEM_HEIGHT as usize;
                let ti = self.scroll + idx;
                if ti < self.filtered().len() {
                    self.selected = ti;
                }
            }
        }
    }

    /// 主界面左键松开（architect v151 自 [`Self::on_mouse_main`] 拆出）：
    /// 通用滚动条松开收尾 + 页脚命令 chip 原地松开执行；其余 Up 事件
    /// 主界面无语义（v1.31/FR-01-107①）
    fn on_mouse_main_up(&mut self, m: &MouseEvent) {
        // 通用滚动条：松开 = 结束拖拽/连发/按压高亮（任何位置，
        // v1.32/FR-01-109 ⑥）；页脚命令 chip：按压后原地松开 = 执行
        // 对应命令（与键盘快捷键同口径，P5 双通道等价）；拖到别处
        // 松开 = 取消
        self.sb_release();
        if let Some(pressed) = self.footer_press.take() {
            let still_on = self.footer_cmd_rects.iter().any(|(r, i)| {
                *i == pressed
                    && m.column >= r.x
                    && m.column < r.x.saturating_add(r.width)
                    && m.row >= r.y
                    && m.row < r.y.saturating_add(r.height)
            });
            if still_on {
                self.run_footer_cmd(pressed);
            }
        }
    }

    /// 主界面滚轮（architect v151 自 [`Self::on_mouse_main`] 拆出）：
    /// 滚轮只移视口不动选中项（v1.30/FR-01-109，自由浏览全表）——指针在
    /// 并发面板上滚明细（活跃行数钳制，v1.34/FR-01-96 ⑦），否则滚任务列表
    fn on_mouse_main_scroll(&mut self, m: &MouseEvent, over_conns: bool) {
        match m.kind {
            MouseEventKind::ScrollUp if over_conns => {
                self.conns_scroll = self.conns_scroll.saturating_sub(2);
            }
            MouseEventKind::ScrollDown if over_conns => {
                let vis = self.visible_conns_rows.max(1);
                // v1.34/FR-01-96 ⑦：明细只显示活跃并发——滚动上限按活跃行数钳制
                let maxlen = self.active_conn_ids().len();
                let maxs = maxlen.saturating_sub(vis);
                self.conns_scroll = (self.conns_scroll + 2).min(maxs);
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(2);
            }
            _ => {
                let vis = self.visible_rows.max(1);
                let maxs = self.filtered().len().saturating_sub(vis);
                self.scroll = (self.scroll + 2).min(maxs);
            }
        }
    }
}

/// 指针是否落在区域内（None 区域恒不命中）
fn pointer_in(area: &Option<Rect>, x: u16, y: u16) -> bool {
    area.is_some_and(|a| {
        x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height)
    })
}

/// 矩形列表命中单源（dialog/main 命中闭包去重；返回载荷下标）：
/// 返回首个包含 (x, y) 的矩形所附载荷（列下标或 [`DetailField`]）
fn rect_hit<T: Copy>(rects: &[(Rect, T)], x: u16, y: u16) -> Option<T> {
    rects
        .iter()
        .find(|(r, _)| {
            x >= r.x
                && x < r.x.saturating_add(r.width)
                && y >= r.y
                && y < r.y.saturating_add(r.height)
        })
        .map(|(_, v)| *v)
}

#[cfg(test)]
mod pieces_mouse_tests {
    //! FR-01-106 ⑦ 弹窗鼠标单测：滚轮滚动网格（±2 行）且任务列表不动、
    //! 左键内按下记抓取偏移 / Drag 随动 / Up 结束、点外关闭且不选中任务。

    use crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;

    use super::*;
    use crate::model::config::Config;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> crate::app::App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-pmouse-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        crate::app::App::new(Config::default(), reg)
    }

    fn dl_task(id: u32, name: &str) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = 5_000_000;
        t.block_size = 1_000_000;
        t
    }

    fn popup_area() -> Rect {
        Rect {
            x: 10,
            y: 5,
            width: 64,
            height: 21,
        }
    }

    fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[tokio::test]
    async fn wheel_scrolls_grid_not_task_list() {
        let mut app = make_app("wheel");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(popup_area());
        app.on_mouse(mev(MouseEventKind::ScrollDown, 30, 10));
        assert_eq!(app.pieces_scroll, 2, "滚轮下滚 2 行（FR-01-106 ⑦）");
        app.on_mouse(mev(MouseEventKind::ScrollDown, 30, 10));
        assert_eq!(app.pieces_scroll, 4);
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        assert_eq!(app.pieces_scroll, 2, "滚轮上滚 2 行");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        assert_eq!(app.pieces_scroll, 0, "上滚钳制 0");
        assert_eq!(app.scroll, 0, "任务列表不滚动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn drag_inside_moves_window_up_ends_drag() {
        let mut app = make_app("drag");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_pos = Some((10, 5)); // 模拟渲染层回填
        app.pieces_area = Some(popup_area());
        // 按下弹窗内 (20, 8) → 抓取偏移 = 指针 - 左上角 = (10, 3)
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 20, 8));
        assert_eq!(app.pieces_drag, Some((10, 3)), "记抓取偏移");
        // 拖到 (18, 6) → pos = 指针 - 偏移 = (8, 3)（渲染层负责钳制终端内）
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 18, 6));
        assert_eq!(app.pieces_pos, Some((8, 3)), "拖拽随动");
        assert!(app.pieces_open, "拖拽不关闭弹窗");
        // 松开 → 拖拽结束、弹窗仍在
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 18, 6));
        assert_eq!(app.pieces_drag, None);
        assert!(app.pieces_open);
        // 无拖拽中的 Drag 事件不移动窗口
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 40, 20));
        assert_eq!(app.pieces_pos, Some((8, 3)), "无抓取偏移不随动");
        app.shutdown().await;
    }

    /// v1.32 模态统一口径（FR-01-106 ⑦ 修订）：点击弹窗外部 = 无任何操作
    ///（不关闭、不穿透任务列表）；弹窗打开期间滚轮仍由弹窗分支消费
    #[tokio::test]
    async fn click_outside_is_noop_without_task_selection() {
        let mut app = make_app("out");
        app.tasks.push(dl_task(1, "p.bin"));
        app.tasks.push(dl_task(2, "q.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(popup_area());
        app.list_area = Some(Rect {
            x: 0,
            y: 5,
            width: 40,
            height: 9,
        });
        // 点击弹窗外（任务列表区 (3, 9) 即第 1 条）→ 无任何操作（不关闭、
        // 不选中、不穿透——关闭通道 = P/Esc/q/Q，v1.32 统一模态口径）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 9));
        assert!(
            app.pieces_open,
            "点外不关闭弹窗（v1.32 修订，原「点外关闭」退役）"
        );
        assert_eq!(app.selected, 0, "该次点击不选中任何任务");
        assert_eq!(app.pieces_drag, None, "窗外按下不起拖拽");
        // 弹窗打开期间滚轮同样由弹窗分支消费（列表不动）
        app.on_mouse(mev(MouseEventKind::ScrollDown, 3, 9));
        assert_eq!(app.pieces_scroll, 2, "滚轮被弹窗分支消费");
        assert_eq!(app.scroll, 0, "任务列表滚动不动");
        app.shutdown().await;
    }

    /// 对话框分支与弹窗分支互斥：弹窗打开时对话框不可能存在（键路由保证），
    /// 弹窗分支先行消费一切鼠标事件
    #[tokio::test]
    async fn pieces_branch_consumes_all_events() {
        let mut app = make_app("consume");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(popup_area());
        // 非左键按下、非滚轮事件全部消费且无副作用
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Right), 30, 10));
        assert!(app.pieces_open);
        assert_eq!(app.pieces_drag, None, "右键不开始拖拽");
        app.on_mouse(mev(MouseEventKind::Moved, 30, 10));
        assert!(app.pieces_open);
        let _ = KeyModifiers::NONE;
        app.shutdown().await;
    }
}

#[cfg(test)]
mod mouse_tests {
    use crossterm::event::KeyModifiers;

    use super::*;
    use crate::app::DetailField;
    // FieldWin 仅测试消费（产品经 dlg_field_wins 类型间接使用），从组件
    // 单源引入（与 app 门面 re-export 同一类型）
    use crate::app::FieldWin;

    fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn make_app(tag: &str) -> crate::app::App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-mouse-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        crate::app::App::new(crate::model::config::Config::default(), reg)
    }

    fn task(id: u32, name: &str) -> crate::model::Task {
        crate::model::Task::new_queued(
            id,
            name.to_string(),
            crate::model::Protocol::Http,
            format!("http://example.com/{name}"),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            5,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        )
    }

    fn dlg(kind: DialogKind, focus: usize) -> super::super::Dialog {
        crate::app::testutil::dialog(kind, focus)
    }

    /// v1.34 三批：点击列 → 字符偏移映射（渲染层 FieldWin 累积列单源）——
    /// 逐字符定位、宽字符左右半格分置、越界钳制窗口首/尾、左缘 … 指示
    /// 扣减、空字段恒 0
    #[test]
    fn click_cursor_offset_maps_columns_to_char_indices() {
        // 窄字符 3 个（cols 末位 = 窗口总宽 3）
        let narrow = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0, 1, 2, 3],
            win_lo: 0,
            win_hi: 3,
        };
        assert_eq!(TextInput::click_offset(&narrow, 35), 0, "首字符列 → 前");
        assert_eq!(TextInput::click_offset(&narrow, 36), 1, "第 2 字符列 → 前");
        assert_eq!(TextInput::click_offset(&narrow, 37), 2, "第 3 字符列 → 前");
        assert_eq!(TextInput::click_offset(&narrow, 38), 3, "越末 → win_hi");
        assert_eq!(
            TextInput::click_offset(&narrow, 60),
            3,
            "右侧 padding → win_hi"
        );
        assert_eq!(TextInput::click_offset(&narrow, 34), 0, "label 区 → 窗口首");
        // CJK 宽字符（cols [0,2,4]）：左半格 = 前、右半格 = 后
        let wide = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0, 2, 4],
            win_lo: 0,
            win_hi: 2,
        };
        assert_eq!(TextInput::click_offset(&wide, 35), 0, "宽字左半格 → 其前");
        assert_eq!(TextInput::click_offset(&wide, 36), 1, "宽字右半格 → 其后");
        assert_eq!(
            TextInput::click_offset(&wide, 37),
            1,
            "第 2 宽字左半格 → 其前"
        );
        // 超宽尾窗（左缘 … 指示占 1 列扣减；win_lo = 7）
        let cut = FieldWin {
            value_x: 35,
            left_cut: true,
            cols: vec![0, 1, 2, 3],
            win_lo: 7,
            win_hi: 10,
        };
        assert_eq!(TextInput::click_offset(&cut, 35), 7, "… 指示列 → 窗口首");
        assert_eq!(TextInput::click_offset(&cut, 36), 7, "窗口首字符列");
        assert_eq!(
            TextInput::click_offset(&cut, 37),
            8,
            "第 2 可见字符列 → 7+1"
        );
        // 空字段（占位提示，cols = [0]）：任意点击 → 0
        let empty = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0],
            win_lo: 0,
            win_hi: 0,
        };
        assert_eq!(TextInput::click_offset(&empty, 35), 0, "空字段 → 0");
        assert_eq!(TextInput::click_offset(&empty, 50), 0, "空字段任意列 → 0");
    }

    /// v1.34 三批：对话框文本字段点击 = 聚焦 + 光标落点击处（列 → 字符
    /// 映射消费渲染层发布的窗口几何）；非文本行（win=None）维持
    /// focus_field 迁尾语义；字段热区夹具形态 = 120×40 真实帧回填几何
    #[tokio::test]
    async fn dialog_field_click_positions_cursor() {
        let mut app = make_app("dlgcur");
        app.dialog = Some(dlg(DialogKind::Add, 2));
        {
            let d = app.dialog.as_mut().unwrap();
            d.url.set("https://example.com/path"); // 24 字符
            d.conns.set("4");
            d.focus_field(2); // 焦点先在并发（光标随迁 conns 尾 = 1）
        }
        // 渲染层每帧回填形态（dlg_field_wins 与行下标对齐；非文本行 None）
        let win_url = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: (0..=24).collect(),
            win_lo: 0,
            win_hi: 24,
        };
        let win_conns = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0, 1],
            win_lo: 0,
            win_hi: 1,
        };
        app.dlg_field_wins = vec![
            Some(win_url),   // 0 URL
            None,            // 1 保存到
            Some(win_conns), // 2 并发
            None,            // 3 校验
            None,            // 4 校验码
            None,            // 5 代理
        ];
        app.dlg_field_rects = vec![
            (
                Rect {
                    x: 26,
                    y: 16,
                    width: 68,
                    height: 1,
                },
                0,
            ),
            (
                Rect {
                    x: 26,
                    y: 18,
                    width: 68,
                    height: 1,
                },
                2,
            ),
        ];

        // 点击 URL 值第 5 字符列（"https" 之后）→ 聚焦 URL + 光标 5
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 40, 16));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 0, "点击聚焦 URL 行");
        assert_eq!(d.url.cursor, 5, "光标落点击列对应字符偏移（: 前）");

        // 点击 URL 值区右侧 padding → 光标钳到字段尾
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 90, 16));
        assert_eq!(app.dialog.as_ref().unwrap().url.cursor, 24, "越末 → 字段尾");

        // 点击并发行值区首列（未聚焦字段）→ 聚焦 + 光标落点击处（0）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 35, 18));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 2, "点击聚焦并发行");
        assert_eq!(d.conns.cursor, 0, "点击处映射覆盖迁尾（值首列 = 0）");
    }

    /// FR-01-101：详情字段名点击复制——URL/校验热区命中（toast 文案照录 demo 原文、
    /// 复制内容 = 原始 URL 单值（D27 v1.28）与完整校验码）、无校验任务无热区、
    /// 对话框分支消费优先
    #[tokio::test]
    async fn detail_label_click_copies_and_toasts() {
        let mut app = make_app("copy");
        let mut t = task(1, "copy.bin");
        t.checksum = Some(crate::model::Checksum {
            algo: "SHA-256",
            value: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        });
        app.tasks.push(t);
        // 模拟 ui 层热区回填（内框 (1,1) 起；校验行 idx=4、URL 行 idx=6 的形态不限，
        // 点击臂只看 rect 命中）
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });
        app.detail_ck_rect = Some(Rect {
            x: 1,
            y: 5,
            width: 9,
            height: 1,
        });

        // 点击「URL」字段名 → 复制详情展示值（原始 URL 单值，v1.28）+ toast
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert_eq!(app.toast.as_deref(), Some("已复制 url"), "URL 点击 toast");
        assert_eq!(
            app.last_copied.as_deref(),
            Some("http://example.com/copy.bin"),
            "复制原始 URL 展示值（D27 v1.28 单值收敛）"
        );

        // 点击「校验」字段名 → 复制完整校验码（非 10 位截断）+ toast
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 5));
        assert_eq!(
            app.toast.as_deref(),
            Some("已复制 校验码"),
            "校验点击 toast"
        );
        assert_eq!(
            app.last_copied.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            "复制完整校验码"
        );

        // 无校验码任务：校验热区 None（ui 层回填口径）→ 点击原位置无动作
        app.tasks.push(task(2, "nock.bin"));
        app.selected = 1;
        app.detail_ck_rect = None;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 5));
        assert_eq!(
            app.last_copied.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            "无热区点击不改变复制内容"
        );
        assert_eq!(app.toast.as_deref(), Some("已复制 校验码"), "toast 不变");
        app.shutdown().await;
    }

    /// D27 v1.28 修订（FR-01-80/D27，规格 01-download-engine-08）：详情「URL」
    /// 展示与点击复制收敛为**原始 URL 单值**——任务级最终 URL 退役（FR-01-14
    /// v1.28），「复制所见值」锚定不变，所见即原始 URL。旧口径 final_url 优先臂
    /// 退役：即便会话内曾探测到跳转落点，复制的也必须是原始 URL。
    #[tokio::test]
    async fn detail_url_click_copies_original_url_single_source() {
        let mut app = make_app("copyorig");
        // v1.28：任务模型不再持有跳转落点——复制值只能来自原始 URL 单源；
        // 本用例锚定「所见即所复制」：详情展示值（原始 URL）与点击复制值一致
        app.tasks.push(task(1, "orig.bin"));
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert_eq!(app.toast.as_deref(), Some("已复制 url"), "URL 点击 toast");
        assert_eq!(
            app.last_copied.as_deref(),
            Some("http://example.com/orig.bin"),
            "复制原始 URL（v1.28 单值收敛，final_url 优先臂退役）"
        );
        app.shutdown().await;
    }

    /// FR-01-101：G 收起（热区 None）点击无动作；对话框打开时点击被对话框分支
    /// 先行消费（复制与 toast 均不触发）
    #[tokio::test]
    async fn detail_click_guards() {
        let mut app = make_app("copyg");
        app.tasks.push(task(1, "g.bin"));
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });

        // 热区清空（面板收起/窄终端回填口径）：点击原位置 → 无动作
        app.detail_url_rect = None;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert!(app.last_copied.is_none(), "热区 None 点击无复制");

        // 热区在位但对话框打开：对话框分支先行消费，点击不触发复制
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert!(app.last_copied.is_none(), "对话框内点击不触发复制");
        assert!(app.dialog.is_some(), "对话框不被详情点击关闭");
        app.shutdown().await;
    }

    /// 无对话框：列表点击选中（ITEM_HEIGHT 行距）、列表外点击不动、
    /// 滚轮边界（空滚动钳制）、其他事件类型忽略
    #[tokio::test]
    async fn click_selects_and_scroll_bounded() {
        let mut app = make_app("list");
        app.list_area = Some(Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 9,
        });
        app.tasks.push(task(1, "a.bin"));
        app.tasks.push(task(2, "b.bin"));
        app.visible_rows = 6;

        // 点击第 0 条（row 1 → idx (1-1)/4 = 0）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 1));
        assert_eq!(app.selected, 0, "点击首条选中");
        // 点击第 1 条（row 5 → idx 1）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 5));
        assert_eq!(app.selected, 1, "点击次条选中");
        // 列表区域外（y=0）不改变选中
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 0));
        assert_eq!(app.selected, 1, "列表外点击不选中");
        // x 越界同理
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 50, 5));
        assert_eq!(app.selected, 1, "列外点击不选中");

        // 滚动：条目少于可视行 → 下滚钳制 0；上滚 saturating 0
        app.on_mouse(mev(MouseEventKind::ScrollDown, 5, 5));
        assert_eq!(app.scroll, 0, "无可滚动余量");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 5, 5));
        assert_eq!(app.scroll, 0, "顶部上滚钳制");

        // 非点击/滚轮事件忽略
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 5, 5));
        assert_eq!(app.selected, 1, "拖拽事件忽略");
        app.shutdown().await;
    }

    /// 对话框打开：字段点击聚焦/展开下拉、下拉选项点选与点外关闭、
    /// 按钮点击激活、空白点击无动作、非左键与滚轮全部消费
    #[tokio::test]
    async fn dialog_click_routes() {
        let mut app = make_app("dlgclick");
        app.dialog = Some(dlg(DialogKind::Add, 0));
        // 字段矩形：0=URL、3=算法下拉行、5=代理下拉行；按钮：6=立即下载 7=仅添加 8=取消
        // （v1.15/FR-01-103 三钮）
        app.dlg_field_rects = vec![
            (
                Rect {
                    x: 2,
                    y: 2,
                    width: 30,
                    height: 1,
                },
                0,
            ),
            (
                Rect {
                    x: 2,
                    y: 5,
                    width: 30,
                    height: 1,
                },
                3,
            ),
            (
                Rect {
                    x: 2,
                    y: 7,
                    width: 30,
                    height: 1,
                },
                5,
            ),
        ];
        app.dlg_btn_rects = vec![
            (
                Rect {
                    x: 2,
                    y: 12,
                    width: 10,
                    height: 1,
                },
                6,
            ),
            (
                Rect {
                    x: 14,
                    y: 12,
                    width: 10,
                    height: 1,
                },
                7,
            ),
            (
                Rect {
                    x: 26,
                    y: 12,
                    width: 10,
                    height: 1,
                },
                8,
            ),
        ];

        // 点击算法下拉行 → 聚焦 3 并展开算法下拉
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 5));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 3, "算法行点击聚焦");
        assert!(d.ck_open, "算法行点击展开下拉");

        // 算法下拉选项 1 点击 → 选择并收起
        app.dlg_ck_rects = vec![(
            Rect {
                x: 2,
                y: 6,
                width: 30,
                height: 1,
            },
            1,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 6));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.ck_type, 1, "点选算法项");
        assert_eq!(d.ck_sel, 1);
        assert!(!d.ck_open, "选择后收起");

        // 再展开算法下拉，点击下拉外 → 仅收起不改选择
        app.dialog.as_mut().unwrap().ck_open = true;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 20));
        let d = app.dialog.as_ref().unwrap();
        assert!(!d.ck_open, "点外收起");
        assert_eq!(d.ck_type, 1, "未改变选择");

        // 点击代理下拉行 → 展开代理下拉
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 7));
        assert!(app.dialog.as_ref().unwrap().proxy_open);
        // 代理下拉选项 0 点击 → 选择并收起
        app.dlg_proxy_rects = vec![(
            Rect {
                x: 2,
                y: 8,
                width: 30,
                height: 1,
            },
            0,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 8));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.proxy_sel, 0, "点选代理项");
        assert!(!d.proxy_open, "选择后收起");

        // 普通字段点击 → 仅聚焦；空白点击 → 无动作
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 2));
        assert_eq!(app.dialog.as_ref().unwrap().focus, 0, "URL 行点击聚焦");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 20));
        assert_eq!(app.dialog.as_ref().unwrap().focus, 0, "空白点击无动作");

        // 点击取消按钮（8，第三钮 x=26）→ 关闭对话框
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 16, 12));
        assert!(app.dialog.is_some(), "仅添加钮（空 URL）被校验拒绝不关闭");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 28, 12));
        assert!(app.dialog.is_none(), "取消按钮关闭");

        // 非左键点击与滚轮：对话框打开时全部消费、无动作
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Right), 5, 2));
        assert!(app.dialog.is_some(), "右键不触发路由");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 5, 2));
        assert!(app.dialog.is_some(), "滚轮被对话框分支消费");
        app.shutdown().await;
    }

    /// Modify/Delete 对话框按钮点击臂：Modify 取消关闭、Delete 取消关闭、
    /// Modify 确认（无任务上下文）不生效不 panic
    #[tokio::test]
    async fn dialog_click_modify_delete_buttons() {
        let mut app = make_app("mdbtn");

        // Modify：取消按钮（5）点击 → 关闭
        app.dialog = Some(dlg(DialogKind::Modify, 4));
        app.dlg_btn_rects = vec![(
            Rect {
                x: 14,
                y: 12,
                width: 10,
                height: 1,
            },
            5,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 16, 12));
        assert!(app.dialog.is_none(), "Modify 取消按钮关闭");

        // Modify：确认按钮（4）点击、task_id 缺失 → 不生效不 panic
        app.dialog = Some(dlg(DialogKind::Modify, 4));
        app.dlg_btn_rects = vec![(
            Rect {
                x: 2,
                y: 12,
                width: 10,
                height: 1,
            },
            4,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 4, 12));
        assert!(app.dialog.is_some(), "确认不生效（无任务上下文）");

        // Delete：取消按钮（2）点击 → 关闭
        app.dialog = Some(dlg(DialogKind::Delete, 0));
        app.dlg_btn_rects = vec![(
            Rect {
                x: 20,
                y: 12,
                width: 10,
                height: 1,
            },
            2,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 22, 12));
        assert!(app.dialog.is_none(), "Delete 取消按钮关闭");
        app.shutdown().await;
    }

    /// v133/D38 对话框拖拽：非交互区按下起拖（抓取偏移 = 指针 − 窗口左上）、
    /// Drag 随动更新 dlg_pos、Up 结束；字段行按下不起拖（点击语义优先）；
    /// 窗外按下无动作（对话框不点外关闭）；再次打开重新居中。
    #[tokio::test]
    async fn dialog_drag_moves_window() {
        let mut app = make_app("dlgdrag");
        app.open_add_dialog();
        app.dlg_area = Some(Rect {
            x: 3,
            y: 5,
            width: 74,
            height: 11,
        });
        // 字段行（URL 行）按下 → 不起拖
        app.dlg_field_rects = vec![(
            Rect {
                x: 4,
                y: 6,
                width: 30,
                height: 1,
            },
            0,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 10, 6));
        assert_eq!(app.dlg_drag, None, "字段行按下不起拖");

        // 非交互区（标题行）按下 → 起拖，抓取偏移 = (10−3, 5−5)
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 10, 5));
        assert_eq!(app.dlg_drag, Some((7, 0)), "非交互区按下起拖");

        // Drag 随动：dlg_pos = 指针 − 抓取偏移
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 13, 3));
        assert_eq!(app.dlg_pos, Some((6, 3)), "拖拽随动窗口左上角");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 20, 5));
        assert_eq!(app.dlg_pos, Some((13, 5)));

        // Up 结束；后续 Drag 不再移动
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 20, 5));
        assert_eq!(app.dlg_drag, None, "松开结束拖拽");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 30, 15));
        assert_eq!(app.dlg_pos, Some((13, 5)), "非拖拽态 Drag 不移动");

        // 窗外按下：无动作（不关闭、不起拖）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 0, 0));
        assert_eq!(app.dlg_drag, None, "窗外按下无动作");
        assert!(app.dialog.is_some(), "对话框不因点外关闭");

        // 关闭再打开 → 重新居中（位置/拖拽记忆清空，D38）
        app.dialog = None;
        app.open_add_dialog();
        assert_eq!(app.dlg_pos, None, "再次打开重新居中");
        assert_eq!(app.dlg_drag, None);
        app.shutdown().await;
    }

    /// about 弹窗夹具：打开 + 回填窗口区与 [✗] 热区（重开场景复用，避免
    /// 脚手架同形克隆自配对 CPD tile）
    fn open_about_with_rects(app: &mut crate::app::App) {
        app.open_about();
        app.about_area = Some(Rect {
            x: 30,
            y: 8,
            width: 40,
            height: 7,
        });
        app.popup_close_rect = Some(Rect {
            x: 66,
            y: 8,
            width: 3,
            height: 1,
        });
    }

    /// 关于窗口命中臂（v1.32/FR-01-111 + v1.34/FR-01-114）：窗口内按下起拖 /
    /// Drag 随动 / [✗] 按下高亮 + 原地松开关闭 / 拖出热区松开取消 /
    /// 点外无操作（模态统一口径）；页脚 chip 按压态被模态接管清除。
    #[tokio::test]
    async fn about_window_mouse_paths() {
        let mut app = make_app("abmouse");
        open_about_with_rects(&mut app);
        app.footer_press = Some(0);

        // 窗口内按下 → 起拖（抓取偏移 = 指针 − 左上角）+ 页脚按压清除
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 35, 10));
        assert_eq!(app.about_drag, Some((5, 2)), "窗口内按下起拖");
        assert_eq!(app.footer_press, None, "模态接管清除页脚按压态");

        // Drag 随动：about_pos = 指针 − 抓取偏移
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 38, 12));
        assert_eq!(app.about_pos, Some((33, 10)), "拖拽随动窗口左上角");

        // [✗] 按下 → 高亮（x_press），不再起拖
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 67, 8));
        assert!(app.x_press, "[✗] 按下高亮");
        assert_eq!(app.about_drag, Some((5, 2)), "[✗] 按下不改拖拽态");

        // [✗] 原地松开 → 关闭关于窗口
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 67, 8));
        assert!(!app.about_open, "[✗] 原地松开关闭");
        assert!(!app.x_press, "松开复位按压态");

        // 重新打开：拖出热区松开 = 取消按压不关闭；点外无操作
        open_about_with_rects(&mut app);
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 67, 8));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 40, 20));
        assert!(!app.x_press, "拖出热区松开复位按压");
        assert!(app.about_open, "拖出热区松开不关闭");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 2));
        assert!(app.about_open, "点外不关闭（模态统一口径）");
        assert_eq!(app.about_drag, None, "窗外按下不起拖");
        app.shutdown().await;
    }

    /// 字段值弹窗命中臂（v1.34/FR-01-113 + FR-01-114）：窗口内按下起拖 /
    /// Drag 随动 / [✗] 原地松开关闭且位置即清 / 点外无操作 / 滚轮消费。
    #[tokio::test]
    async fn val_popup_mouse_paths() {
        let mut app = make_app("valmouse");
        app.tasks.push(task(1, "val.bin"));
        app.selected = 0;
        app.open_val_popup(DetailField::Url);
        assert!(app.val_open, "有选中任务时 URL 字段弹窗打开");
        app.val_area = Some(Rect {
            x: 20,
            y: 6,
            width: 48,
            height: 12,
        });
        app.popup_close_rect = Some(Rect {
            x: 64,
            y: 6,
            width: 3,
            height: 1,
        });

        // 窗口内按下 → 起拖；Drag 随动
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 10));
        assert_eq!(app.val_drag, Some((10, 4)), "窗口内按下起拖");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 25, 8));
        assert_eq!(app.val_pos, Some((15, 4)), "拖拽随动窗口左上角");

        // 滚轮：消费且钳制（无滚动内容 max=0）
        app.on_mouse(mev(MouseEventKind::ScrollDown, 30, 10));
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        assert_eq!(app.val_scroll, 0, "无内容滚动钳制 0");

        // [✗] 按下高亮 → 原地松开关闭且位置即清（同 pieces 口径）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 6));
        assert!(app.x_press, "[✗] 按下高亮");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 65, 6));
        assert!(!app.val_open, "[✗] 原地松开关闭");
        assert_eq!(app.val_pos, None, "关闭位置即清");
        assert_eq!(app.val_drag, None);

        // 再开：点外无操作（不关闭、不起拖）
        app.open_val_popup(DetailField::Url);
        app.val_area = Some(Rect {
            x: 20,
            y: 6,
            width: 48,
            height: 12,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 2));
        assert!(app.val_open, "点外不关闭（模态统一口径）");
        assert_eq!(app.val_drag, None, "窗外按下不起拖");
        app.shutdown().await;
    }

    /// 帮助窗口命中臂（v1.30/FR-01-107 + v1.34/FR-01-114）：滚轮滚动内容
    /// ±1 行、窗口内按下起拖、Drag 随动、[✗] 原地松开关闭、拖出热区取消。
    #[tokio::test]
    async fn help_window_mouse_paths() {
        let mut app = make_app("helpmouse");
        app.help_open = true;
        app.help_area = Some(Rect {
            x: 25,
            y: 3,
            width: 70,
            height: 20,
        });
        app.popup_close_rect = Some(Rect {
            x: 89,
            y: 3,
            width: 3,
            height: 1,
        });

        // 滚轮：帮助步长 ±1（FR-01-109）
        app.on_mouse(mev(MouseEventKind::ScrollDown, 50, 10));
        assert_eq!(app.help_scroll, 1, "下滚 1 行");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.help_scroll, 0, "上滚 1 行");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.help_scroll, 0, "上滚钳制 0");

        // 窗口内按下起拖 + Drag 随动
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 8));
        assert_eq!(app.help_drag, Some((5, 5)), "窗口内按下起拖");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 28, 6));
        assert_eq!(app.help_pos, Some((23, 1)), "拖拽随动窗口左上角");

        // [✗] 按下高亮 → 原地松开关闭
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 90, 3));
        assert!(app.x_press, "[✗] 按下高亮");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 90, 3));
        assert!(!app.help_open, "[✗] 原地松开关闭");

        // 重新打开：拖出热区松开 = 取消按压不关闭
        app.help_open = true;
        app.popup_close_rect = Some(Rect {
            x: 89,
            y: 3,
            width: 3,
            height: 1,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 90, 3));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 50, 10));
        assert!(!app.x_press, "拖出热区松开复位按压");
        assert!(app.help_open, "拖出热区松开不关闭");
        app.shutdown().await;
    }

    /// 对话框 [✗] 关闭臂（v1.34/FR-01-114 ③）：按下高亮 → 原地松开取消对话框
    /// （与 Esc 同口径）、拖出热区松开不关闭；下拉展开时点击 [✗] 位置先收起
    /// 下拉（不进入 [✗] 按压）。
    #[tokio::test]
    async fn dialog_close_btn_cancel_paths() {
        let mut app = make_app("dlgx");
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.popup_close_rect = Some(Rect {
            x: 70,
            y: 5,
            width: 3,
            height: 1,
        });

        // [✗] 按下高亮 → 原地松开 = 取消对话框
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 71, 5));
        assert!(app.x_press, "[✗] 按下高亮");
        assert!(app.dialog.is_some(), "按下不关闭");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 71, 5));
        assert!(app.dialog.is_none(), "[✗] 原地松开取消对话框");
        assert!(!app.x_press, "松开复位按压态");

        // 拖出热区松开 = 取消按压不关闭
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 71, 5));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 30, 9));
        assert!(!app.x_press, "拖出热区复位按压");
        assert!(app.dialog.is_some(), "拖出热区松开不关闭");

        // 下拉展开时点击 [✗] 位置：先收起下拉（FR-01-114 ③ 守卫臂）
        app.dialog = Some(dlg(DialogKind::Add, 3));
        app.dialog.as_mut().unwrap().ck_open = true;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 71, 5));
        assert!(!app.dialog.as_ref().unwrap().ck_open, "下拉先收起");
        assert!(!app.x_press, "守卫臂不进入 [✗] 按压");
        assert!(app.dialog.is_some());
        app.shutdown().await;
    }

    /// architect v151：主界面按下命中链优先级锁定（on_mouse_main_down 拆分后
    /// 语义基线，为 hardender 变异加固提供分发序锚点）——页脚 → 页签 → 详情
    /// 热区 → 滚动条 → 并发面板 → 列表，命中即消费；页脚未命中清除按压态；
    /// 无值字段名点击只聚焦不复制不 toast。
    #[tokio::test]
    async fn main_down_dispatch_chain_priority() {
        let mut app = make_app("dispatch");
        app.tasks.push(task(1, "chain.bin"));
        app.selected = 0;
        // ① 页脚 chip 与列表面板同格播种：命中页脚即消费——不聚焦、不选中
        app.footer_cmd_rects = vec![(
            Rect {
                x: 2,
                y: 2,
                width: 4,
                height: 1,
            },
            0,
        )];
        app.list_area = Some(Rect {
            x: 2,
            y: 2,
            width: 20,
            height: 10,
        });
        app.focus = Panel::Conns;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 2));
        assert_eq!(app.footer_press, Some(0), "页脚命中：按压记录");
        assert!(app.footer_flash.is_some(), "页脚命中：400ms 余辉");
        assert_eq!(app.focus, Panel::Conns, "页脚命中不改变焦点");
        assert_eq!(app.selected, 0, "页脚命中不选中任务");
        // ② 未命中页脚的按下清除按压态（原地松开才执行——拖走换目标取消）
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 5));
        assert_eq!(app.footer_press, None, "未命中页脚：按压清除");
        // ③ 详情字段名命中（选中越界 → sel_task() None = 无值臂；
        //    tab_rects 未播种，页脚/列表均不覆盖该点）：
        //    聚焦详情、不复制、不 toast（无值臂语义）
        app.detail_url_rect = Some(Rect {
            x: 0,
            y: 0,
            width: 6,
            height: 1,
        });
        app.selected = 99;
        let toast_before = app.toast.clone();
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 0));
        assert_eq!(app.focus, Panel::Detail, "字段名命中聚焦详情");
        assert!(app.last_copied.is_none(), "无值不复制");
        assert_eq!(app.toast, toast_before, "无值不 toast（启动 toast 不变）");
        // ④ 页签命中（与详情热区同格播种）：命中即消费在页签段——不进入详情臂
        //（选中恢复合法值：真实输入通路恒有钳制，scroll_to_selection 不越界）
        app.tab_rects = vec![(
            Rect {
                x: 0,
                y: 0,
                width: 5,
                height: 1,
            },
            1,
        )];
        app.selected = 0;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 0));
        assert_eq!(app.filter, 1, "页签切换（命中链先于详情热区）");
        assert_eq!(app.focus, Panel::List, "页签命中聚焦列表");
        // ⑤ 无任何热区命中：列表面板点击 = 聚焦 + 选中（命中链末端；
        //    第 2 行任务条目 idx 1 = (row−a.y)/ITEM_HEIGHT）
        app.tasks.push(task(2, "chain2.bin"));
        app.filter = 0;
        app.selected = 0;
        app.focus = Panel::Conns;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 6));
        assert_eq!(app.focus, Panel::List, "列表点击聚焦");
        assert_eq!(app.selected, 1, "列表点击按行选中任务");
        app.shutdown().await;
    }
}
