use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::pad_right;
use super::{panel_border_style, panel_title_style, ACCENT, FG, FOCUS_BORDER};
use crate::app::{App, SbKey};

const HELP_W: u16 = 70;
const HELP_H: u16 = 24;

fn help_lines() -> Vec<Line<'static>> {
    fn sec(t: &str) -> Line<'static> {
        Line::from(Span::styled(
            format!("  【{t}】"),
            Style::default()
                .fg(FOCUS_BORDER)
                .add_modifier(Modifier::BOLD),
        ))
    }
    fn kd(k: &str, d: &str) -> Line<'static> {
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                pad_right(k, 16),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(d.to_string(), Style::default().fg(FG)),
        ])
    }
    vec![
        sec("全局"),
        kd("Tab / BackTab", "切换面板（任务列表 → 详情 → 并发连接）"),
        kd("鼠标左键", "点击面板聚焦；点击任务/连接选中明细"),
        kd("页脚命令", "A/D/M/Q/H 图例可点击：按压高亮，松开执行"),
        kd("H", "打开 / 关闭本帮助窗口"),
        kd("O", "打开 / 关闭关于窗口（About）"),
        kd("G", "收起/展开 任务详情 + 并发连接面板"),
        kd("Esc", "关闭弹窗"),
        kd("Q", "退出程序（主界面；弹窗内先关窗）"),
        sec("页签"),
        kd("] / [", "右/左切换页签（正在下载 ⇄ 已完成）"),
        kd("鼠标点击页签", "切换到该页签"),
        kd("T", "循环切换页签（与 ] 等效）"),
        sec("任务列表"),
        kd("↑ / ↓", "选择上/下一个任务"),
        kd("PgUp / PgDn", "翻页选择（Home / End 跳首/尾）"),
        kd("鼠标滚轮", "滚动列表（视口自由浏览全表）"),
        kd("A", "打开命令窗口（输入后分流添加）"),
        kd("M / D", "修改 / 删除任务（BT 任务 M 开修改任务：BT）"),
        kd("Space / R", "暂停·继续 / 重试"),
        kd("U / J", "上移 / 下移任务"),
        kd("C", "清除已完成任务"),
        kd("P", "查看分块状态（仅分块下载任务）"),
        sec("命令窗口（A）"),
        kd("输入内容", "http(s) URL / 本地 .torrent 路径"),
        kd("Enter", "确认分流（= 触发 [确定] 按钮 click；Ctrl+Enter / Ctrl+J 同效）"),
        kd("Shift+Enter", "文本框内换行（需终端支持 kitty 键盘协议；普通终端用 Alt+Enter）"),
        kd("Tab", "文本框 ⇄ [确定] 切换焦点（按钮焦点下 Enter = 确认）"),
        kd("Shift+Tab", "补全光标前的本地路径（readline 式，支持 ~）"),
        kd("↑↓ / ←→", "上/下可视行（含 wrap 分段行，列保持，短行落行尾）/ 行内移动光标"),
        kd("鼠标滚轮", "文本区滚动（word wrap 自动换行；滚动条在文本框边框上，可拖拽/点轨道/箭头）"),
        kd("Esc / [✗]", "关闭命令窗口"),
        sec("BT 对话框（添加/修改：BT）"),
        kd("↑↓ / 点击", "文件树移动光标 / 勾选（目录复选框 = 整目录 全选⇄全不选）"),
        kd("← / →", "收拢 / 展开光标目录（默认收拢；点击目录名同效）"),
        kd("Space", "勾选 / 取消光标行（文件单选；目录整目录勾选）"),
        kd("A / N / I", "列表焦点下：全选 / 全不选 / 反选"),
        kd("[全选] / [全不选]", "文件列表框右上角按钮（悬停高亮，点击作用于全部文件与目录）"),
        kd("Tab / BackTab", "字段焦点循环（种子·目录·列表·并发·代理·trackers·按钮）"),
        kd("Enter", "确认（添加 = 立即下载；修改 = 保存文件勾选；trackers 焦点下同效）"),
        kd("Alt+Enter", "trackers 文本框内换行（kitty 协议终端 Shift+Enter 同效）"),
        kd("鼠标滚轮", "悬停文件树 / trackers 滚动；滚动条在各自 panel 边框上（与任务列表 ScrollPanel 同构），可拖拽 / 点击轨道 / 箭头"),
        sec("任务详情"),
        kd("鼠标点击字段名", "校验 / URL / 失败 / 路径 复制对应值"),
        kd("鼠标点击字段值", "打开完整值弹窗（悬停显示下划线）"),
        sec("字段值弹窗"),
        kd("↑↓ / PgUp·PgDn", "滚动内容（Home / End 跳首/尾）"),
        kd("Y", "复制窗口全部内容（完整字段值）"),
        kd("鼠标拖拽 / 滚轮", "移动窗口 / 滚动内容"),
        kd("鼠标点击 [✗]", "关闭窗口（右上角按钮，悬停高亮）"),
        kd("Esc / q", "关闭窗口"),
        sec("并发连接"),
        kd("↑ / ↓", "选择连接明细（视口跟随滚动）"),
        kd("← / →", "横向滚动明细列（客户端/协议等超宽列）"),
        kd("鼠标滚轮", "悬停并发面板 = 滚动连接明细（±2/格）"),
        kd("B", "阻塞并移除选中连接（仅 BT 任务）"),
        sec("滚动条（通用组件）"),
        kd(
            "拖拽滑块 ═║",
            "按住双线滑块拖动 = 比例滚动内容（浮层内同样滚内容）",
        ),
        kd("点击 ↑↓←→", "步进滚动 1 格；按住不放自动连发"),
        kd("点击轨道", "朝点击方向整页滚动（可按住连发）"),
        kd("方向键", "面板聚焦时 ↑↓ 选择/纵向滚动、←→ 横向滚动（并发）"),
        sec("弹窗（添加/修改/删除/pieces/帮助/关于/字段值）"),
        kd("Tab / ↑↓", "切换字段/焦点（下拉框内选择选项）"),
        kd("Enter", "确认"),
        kd("Esc", "取消/关闭"),
        kd("鼠标拖拽", "按住弹窗内空白处拖动移动窗口（滑块/边框除外）"),
        kd("鼠标点击 [✗]", "关闭当前弹窗（右上角按钮，悬停高亮）"),
        sec("pieces 分块弹窗"),
        kd("↑↓ / PgUp·PgDn", "滚动分块网格（Home / End 跳首/尾）"),
        kd("Esc / P / [✗]", "关闭弹窗（右上角按钮可点击，悬停高亮）"),
    ]
}

pub(super) fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    let rect = match app.help_pos {
        Some((px, py)) => {
            let dw = HELP_W.min(area.width);
            let dh = HELP_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, HELP_W, HELP_H);
            app.help_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 帮助 ", panel_title_style(true)));
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Help));
    let inner = sb.render_block(f, rect, block);
    app.help_area = Some(rect);
    app.popup_close_rect = draw_close_btn(f, app, rect, super::text::w(" 帮助 "));

    let lines = help_lines();
    let content = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    let view_h = content.height as usize;
    let max_scroll = lines.len().saturating_sub(view_h);
    app.help_scroll = app.help_scroll.min(max_scroll);
    app.help_max_scroll = max_scroll;
    let start = app.help_scroll;
    let shown: Vec<Line> = lines.iter().skip(start).take(view_h).cloned().collect();
    f.render_widget(Paragraph::new(shown), content);

    sb.vertical(lines.len(), view_h, start);
    app.help_scrollbar = sb.render_bars(f, rect).vertical;
}

#[cfg(test)]
pub(super) fn help_lines_len() -> usize {
    help_lines().len()
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;
    use crate::app::App;

    fn render_help(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.help_open = true;
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term
    }

    #[tokio::test]
    async fn help_window_geometry_and_sections() {
        let mut app = testfx::make_app_in("ezr-help", "geom");
        let term = render_help(&mut app, 120, 40);
        let s = term.backend().to_string();
        assert!(s.contains("帮助"), "标题: {s}");
        for sec in ["【全局】", "【页签】", "【任务列表】"] {
            assert!(s.contains(sec), "组标题 {sec}: {s}");
        }
        assert!(
            s.contains("切换面板（任务列表 → 详情 → 并发连接）"),
            "全局 Tab 条目: {s}"
        );
        assert!(
            s.contains("打开 / 关闭关于窗口（About）"),
            "全局 O 条目（v1.32 新增）: {s}"
        );
        assert!(
            s.contains("右/左切换页签（正在下载 ⇄ 已完成）"),
            "页签条目: {s}"
        );
        assert!(
            s.contains("打开命令窗口（输入后分流添加）"),
            "A 条目 phase-02 文案: {s}"
        );
        assert!(
            s.contains("修改 / 删除任务（BT 任务 M 开修改任务：BT）"),
            "M/D 条目 BT 补注: {s}"
        );
        app.help_scroll = 10;
        let mut term_mid = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term_mid.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let sm = term_mid.backend().to_string();
        assert!(
            sm.contains("查看分块状态（仅分块下载任务）"),
            "P 条目（中间帧）: {sm}"
        );
        assert!(
            sm.contains("【命令窗口（A）】"),
            "phase-02 命令窗口组: {sm}"
        );
        assert!(
            sm.contains("http(s) URL / 本地 .torrent 路径"),
            "命令窗口输入内容条目（D59 无磁力链）: {sm}"
        );
        assert!(
            sm.contains("确认分流（= 触发 [确定] 按钮 click"),
            "命令窗口 Enter 条目: {sm}"
        );
        let needles = [
            "【BT 对话框（添加/修改：BT）】",
            "全选 / 全不选 / 反选",
            "校验 / URL / 失败 / 路径 复制对应值",
            "打开完整值弹窗（悬停显示下划线）",
            "复制窗口全部内容（完整字段值）",
        ];
        for needle in needles {
            let mut found = false;
            for scroll in 0..=super::help_lines_len() {
                app.help_scroll = scroll;
                let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
                t.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
                if t.backend().to_string().contains(needle) {
                    found = true;
                    break;
                }
            }
            assert!(found, "滚动全帧扫描未命中条目：{needle}");
        }
        app.help_scroll = usize::MAX;
        let mut term2 = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term2.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let s2 = term2.backend().to_string();
        assert_eq!(app.help_scroll, 50, "End 钳制 = 70 − 20（末页视窗 50..69）");
        for sec in [
            "【并发连接】",
            "【滚动条（通用组件）】",
            "【弹窗（添加/修改/删除/pieces/帮助/关于/字段值）】",
            "【pieces 分块弹窗】",
        ] {
            assert!(s2.contains(sec), "滚动后组标题 {sec}: {s2}");
        }
        assert!(
            s2.contains("选择连接明细（视口跟随滚动）"),
            "并发连接 ↑↓ 条目 v1.32 文案: {s2}"
        );
        assert!(
            s2.contains("横向滚动明细列（客户端/协议等超宽列）"),
            "并发连接 ←→ 条目（v1.32 新增）: {s2}"
        );
        assert!(
            s2.contains("悬停并发面板 = 滚动连接明细（±2/格）"),
            "并发连接滚轮条目（v1.32 新增）: {s2}"
        );
        assert!(
            s2.contains("按住双线滑块拖动 = 比例滚动内容"),
            "滚动条组条目（v1.32 新增组；尾括注超内框宽被截，断言可见前段）: {s2}"
        );
        assert!(
            s2.contains("阻塞并移除选中连接（仅 BT 任务）"),
            "B 条目照录（D26 预留）: {s2}"
        );
        assert!(
            s2.contains("关闭当前弹窗（右上角按钮，悬停高亮）"),
            "弹窗组 [✗] 条目: {s2}"
        );
        assert!(
            s2.contains("关闭弹窗（右上角按钮可点击，悬停高亮）"),
            "pieces 组 [✗] 条目: {s2}"
        );
        assert!(
            !s2.contains("↑↓/滚轮 滚动本页"),
            "末行 note 滚动提示退役（P10）: {s2}"
        );
        assert_eq!(
            app.help_area,
            Some(ratatui::layout::Rect::new(25, 8, 70, 24)),
            "区域回填"
        );
        assert_eq!(app.help_pos, Some((25, 8)), "首帧居中并记忆");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_scroll_clamps_to_last_page_and_scrollbar() {
        let mut app = testfx::make_app_in("ezr-help", "scroll");
        let total = super::help_lines_len();
        assert_eq!(
            total, 70,
            "phase-02 内容 70 行（十一组 = 11 组标题 + 59 条目；九组 50 行 +1（A/M-D 拆分）+9（命令窗口）+10（BT 对话框））"
        );
        app.help_scroll = usize::MAX;
        let term = render_help(&mut app, 120, 40);
        assert_eq!(
            app.help_scroll,
            total - 20,
            "End 渲染层钳制到末页（内容 {total} 行 - 可视 20 行）"
        );
        let buf = term.backend().buffer();
        let col = 25 + 70 - 1;
        let thumb = (9..31).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "║")
                .unwrap_or(false)
        });
        assert!(thumb, "右缘滚动条滑块出现（内容超窗）");
        assert!(app.help_scrollbar.is_some(), "帮助滚动条几何回填");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_position_memory_across_reopens() {
        let mut app = testfx::make_app_in("ezr-help", "mem");
        app.help_open = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let first = app.help_pos;
        assert!(first.is_some(), "首帧回填位置");
        app.help_open = false;
        app.help_pos = Some((3, 5));
        app.help_open = true;
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.help_pos, Some((3, 5)), "位置跨次记忆（v1.30）");
        assert_eq!(
            app.help_area,
            Some(ratatui::layout::Rect::new(3, 5, 70, 24))
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_clamps_in_small_terminal() {
        let mut app = testfx::make_app_in("ezr-help", "clamp");
        let term = render_help(&mut app, 50, 18);
        let s = term.backend().to_string();
        assert!(s.contains("帮助"), "钳制后仍渲染标题: {s}");
        assert_eq!(
            app.help_area,
            Some(ratatui::layout::Rect::new(0, 0, 50, 18))
        );
        app.shutdown().await;
    }
}
