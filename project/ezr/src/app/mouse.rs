//! mouse — 鼠标事件：点击选中/滚轮/对话框按钮命中

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{App, DialogKind, ITEM_HEIGHT};

impl App {
    /// 鼠标事件入口
    pub fn on_mouse(&mut self, m: MouseEvent) {
        if self.dialog.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                let kind = self.dialog.as_ref().map(|d| d.kind);
                let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);
                let hit = |rects: &[(Rect, usize)]| {
                    rects
                        .iter()
                        .find(|(r, _)| {
                            m.column >= r.x
                                && m.column < r.x.saturating_add(r.width)
                                && m.row >= r.y
                                && m.row < r.y.saturating_add(r.height)
                        })
                        .map(|(_, i)| *i)
                };
                if ck_open {
                    let ck_rects = self.dlg_ck_rects.clone();
                    if let Some(i) = hit(&ck_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.ck_type = i;
                        d.ck_sel = i;
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
                    d.focus = i;
                    if i == ck_i {
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    } else if i == proxy_i {
                        d.proxy_open = true;
                    }
                } else if let Some(btn) = hit(&btn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = btn;
                    }
                    match kind {
                        Some(DialogKind::Add) => self.dlg_activate_add(btn),
                        Some(DialogKind::Modify) => self.dlg_activate_modify(btn),
                        Some(DialogKind::Delete) => self.dlg_activate_delete(btn),
                        None => {}
                    }
                }
            }
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(a) = self.list_area {
                    let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
                    let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
                    if in_x && in_y {
                        let idx = (m.row - a.y) as usize / ITEM_HEIGHT as usize;
                        let ti = self.scroll + idx;
                        if ti < self.filtered().len() {
                            self.selected = ti;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(2);
            }
            MouseEventKind::ScrollDown => {
                let vis = self.visible_rows.max(1);
                let maxs = self.filtered().len().saturating_sub(vis);
                self.scroll = (self.scroll + 2).min(maxs);
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // 任务操作（Space/R/D/C 语义沿用 demo + 真实引擎动作）
    // -----------------------------------------------------------------------
}

#[cfg(test)]
mod mouse_tests {
    use crossterm::event::KeyModifiers;

    use super::*;

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
        // 字段矩形：0=URL、3=算法下拉行、5=代理下拉行；按钮：6=确认 7=取消
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

        // 点击取消按钮（7）→ 关闭对话框
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 16, 12));
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
}
