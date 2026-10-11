//! mouse — 鼠标事件：点击选中/滚轮/对话框按钮命中

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{App, DialogKind, ITEM_HEIGHT};

impl App {
    /// 鼠标事件入口（architect v116：分发核保持行为拆分——对话框态与主界面
    /// 命中各自成函数，消除早退 return 穿越两分支的控制流混杂；分支语义
    /// 不变：对话框打开时一切鼠标事件仅作用于对话框）
    pub fn on_mouse(&mut self, m: MouseEvent) {
        if self.dialog.is_some() {
            self.on_mouse_dialog(m);
        } else {
            self.on_mouse_main(m);
        }
    }

    /// 对话框态命中（仅左键生效；下拉浮层优先，其次字段行/按钮行）
    fn on_mouse_dialog(&mut self, m: MouseEvent) {
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
    }

    /// 主界面命中（详情热区复制 / 列表点击选中 / 明细与列表滚轮）
    fn on_mouse_main(&mut self, m: MouseEvent) {
        // 指针命中并发连接面板（FR-01-96：明细非空时面板接收滚轮）→ 滚动明细；
        // 空明细 / 未选中任务时 conns_area 为 None → 滚轮穿透滚动任务列表
        let over_conns = pointer_in(&self.conns_area, m.column, m.row);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // FR-01-101：详情面板字段名点击复制（URL → url 展示值、校验 → 校验码值）。
                // 热区由 ui 层每帧回填（None = 无热区：无校验行 / 面板收起 / 窄终端）；
                // 面板本身无提示信息，反馈仅在底部 toast（文案照录 demo 原文，D26 同口径）
                if pointer_in(&self.detail_url_rect, m.column, m.row) {
                    // D27：复制详情展示值（final_url 优先回退原始 url，FR-01-14 口径）
                    let url = self
                        .sel_task()
                        .map(|t| t.final_url.clone().unwrap_or_else(|| t.url.clone()));
                    if let Some(url) = url {
                        self.copy_to_clipboard(&url);
                        self.set_toast("已复制 url");
                    }
                    return;
                }
                if pointer_in(&self.detail_ck_rect, m.column, m.row) {
                    let v = self
                        .sel_task()
                        .and_then(|t| t.checksum.as_ref().map(|c| c.value.clone()));
                    if let Some(v) = v {
                        self.copy_to_clipboard(&v);
                        self.set_toast("已复制 校验码");
                    }
                    return;
                }
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
            MouseEventKind::ScrollUp if over_conns => {
                self.conns_scroll = self.conns_scroll.saturating_sub(2);
            }
            MouseEventKind::ScrollDown if over_conns => {
                let vis = self.visible_conns_rows.max(1);
                let maxlen = self
                    .sel_task()
                    .filter(|t| t.state.shows_conns())
                    .map_or(0, |t| t.connections.len());
                let maxs = maxlen.saturating_sub(vis);
                self.conns_scroll = (self.conns_scroll + 2).min(maxs);
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
}

/// 指针是否落在区域内（None 区域恒不命中）
fn pointer_in(area: &Option<Rect>, x: u16, y: u16) -> bool {
    area.is_some_and(|a| {
        x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height)
    })
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

    /// FR-01-101：详情字段名点击复制——URL/校验热区命中（toast 文案照录 demo 原文、
    /// 复制内容含 final_url 口径与完整校验码）、无校验任务无热区、对话框分支消费优先
    #[tokio::test]
    async fn detail_label_click_copies_and_toasts() {
        let mut app = make_app("copy");
        let mut t = task(1, "copy.bin");
        t.final_url = Some("http://final.example/copy.bin".to_string());
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

        // 点击「URL」字段名 → 复制详情展示值（final_url 优先）+ toast
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert_eq!(app.toast.as_deref(), Some("已复制 url"), "URL 点击 toast");
        assert_eq!(
            app.last_copied.as_deref(),
            Some("http://final.example/copy.bin"),
            "复制 final_url 展示值（D27）"
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
}
