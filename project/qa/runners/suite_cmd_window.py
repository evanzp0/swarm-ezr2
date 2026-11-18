#!/usr/bin/env python3
"""suite_cmd_window.py — 06-cmd-window-qa.md v1.7 八用例可执行化（six-pack/QA 2026-11-17）。

覆盖 QA-CW-01…08 全部用例（qa/06-cmd-window-qa.md；对应 features/06-cmd-window.feature
8 场景，终端 120×44、命令窗口 78×16、错误窗口 66×7）。

Fixture 组成（每用例独立现场，路径按 QA 工作区实例化，行为判据与规格 Examples 逐条对应）：
- <fixture_root>/qatorrents/：ubuntu-24.04.2-desktop-amd64.torrent（单文件种子，分流臂 +
  唯一文件补全候选）、padded.torrent（多行取首行用例）、corrupt.torrent（非法 bencode）、
  missing.torrent（不预置，不存在路径用例）；
- env.home/downloads（~/down 补全用例；app HOME = env.home，tilde 展开落 env.home）；
- 补全判据按操作者裁决①形态断言：输入框保持 ~/ 形态、toast 补全目录：~/downloads/。

kitty 键盘协议序列：Shift+Enter = \\x1b[13;2u、Ctrl+Enter = \\x1b[13;5u；
Alt+Enter = ESC 前缀 \\x1b\\r（任何终端降级键位）。行为判据以画面观察为准，
序列是否被产品接受由用例结果直接呈现。

时序纪律（engineering.md）：正向探测有界重试窗（wait_for）、负向断言单发短窗、
断言锚定最终态常驻文本（标题/按钮/字段行），toast 只在 3s 窗口内即时断言。
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

QA_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, QA_DIR)

from harness import TMP_ROOT, EzrApp, Suite, Env, assert_in, assert_not_in  # noqa: E402

MKTORRENT = os.path.join(QA_DIR, "..", "..", "ezr", "target", "debug",
                         "ezr-mktorrent")

SHIFT_ENTER = "\x1b[13;2u"
CTRL_ENTER = "\x1b[13;5u"
ALT_ENTER = "\x1b\r"
BACKTAB = "\x1b[Z"

UB_TORRENT = "ubuntu-24.04.2-desktop-amd64.torrent"
URL_EXAMPLE = "http://fixture.local/files/big-one.bin"


def _click(app: EzrApp, col0: int, row0: int) -> None:
    """0 基 buffer 坐标左键点击（SGR 1 基换算；press + release 成对，部分交互只认 release）。"""
    app.send(f"mclick:{col0 + 1}:{row0 + 1}")
    app.send(f"mup:{col0 + 1}:{row0 + 1}")


def _cell(app: EzrApp, y: int, x: int) -> str:
    return app.screen.buffer[y][x].data or ""


def _row_text(app: EzrApp, y: int, x0: int = 0, x1: int | None = None) -> str:
    x1 = app.screen.columns if x1 is None else x1
    return "".join(_cell(app, y, x) for x in range(x0, x1)).rstrip()


def _buf_find_col(app: EzrApp, y: int, needle: str) -> "int | None":
    """buffer 逐格扫描定位 needle 起始单元格列（宽字符 stub 格 data=""，拼串时不补空格）。"""
    chars = []
    for x in range(app.screen.columns):
        d = _cell(app, y, x)
        chars.append(d if d else "")
    i = "".join(chars).find(needle)
    return i if i >= 0 else None


class CmdWindowSuite(Suite):
    def __init__(self):
        super().__init__("06-cmd-window", port_base=44200)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in (1, 2, 3, 4, 5, 6, 7, 8):
            getattr(self, f"_case_{i}")()
        return self.results

    def _snapshot(self, env: Env, case_id: str) -> None:
        if self._cur_app is None:
            return
        try:
            with open(os.path.join(TMP_ROOT, f"{case_id}.screen.txt"), "w",
                      encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    # -- fixture -------------------------------------------------------------

    def _make_fixtures(self, env: Env) -> str:
        qdir = os.path.join(env.fixture_root, "qatorrents")
        os.makedirs(qdir, exist_ok=True)
        payload = os.path.join(env.fixture_root, "payload-ub.bin")
        if not os.path.exists(payload):
            with open(payload, "wb") as f:
                f.write(b"u" * (1024 * 1024))
        for name in (UB_TORRENT, "padded.torrent"):
            out = os.path.join(qdir, name)
            if not os.path.exists(out):
                r = subprocess.run(
                    [MKTORRENT, "--out", out, "--piece-len", "262144", payload],
                    capture_output=True, text=True)
                assert r.returncode == 0, f"mktorrent({name}) 失败: {r.stderr.strip()}"
        with open(os.path.join(qdir, "corrupt.torrent"), "wb") as f:
            f.write(b"this is not bencode at all")
        os.makedirs(os.path.join(env.home, "downloads"), exist_ok=True)
        return qdir

    # -- geometry ------------------------------------------------------------

    def _frame(self, app: EzrApp, title: str) -> tuple[int, int, int, int]:
        """标题所在圆角浮层 → (顶行, 左列, 右列, 底行)（buffer 逐格扫描）。"""
        ty = app.find_row(title)
        assert ty is not None, f"未找到标题「{title}」"
        l = next(x for x in range(app.screen.columns)
                 if _cell(app, ty, x) == "╭")
        r = next(x for x in range(l + 1, app.screen.columns)
                 if _cell(app, ty, x) == "╮")
        by = next(y for y in range(ty + 1, app.screen.lines)
                  if _cell(app, y, l) == "╰")
        return ty, l, r, by

    def _open_cmd(self, app: EzrApp) -> tuple[int, int, int, int]:
        app.send("A")
        assert app.wait_for("请输入命令或链接", 4), "命令窗口应打开"
        return self._frame(app, "请输入命令或链接")

    def _cmd_closed(self, app: EzrApp) -> bool:
        return app.find_row("请输入命令或链接") is None

    def _find_btn_row(self, app: EzrApp, ty: int, by: int) -> int:
        for y in range(ty + 1, by):
            if "[ 确定 ]" in _row_text(app, y):
                return y
        raise AssertionError("未找到 [ 确定 ] 按钮行")

    def _btn_focus_bg(self, app: EzrApp, y: int) -> "str | None":
        """按钮行「确定」字样处的背景色（反色高亮断言用）。"""
        line = _row_text(app, y)
        if "[ 确定 ]" not in line:
            return None
        x = line.index("[ 确定 ]") + 2
        return app.screen.buffer[y][x].bg

    def _row_fg(self, app: EzrApp, y: int, needle: str) -> "str | None":
        line = _row_text(app, y)
        if needle not in line:
            return None
        x = line.index(needle)
        return app.screen.buffer[y][x].fg

    def _chip_click(self, app: EzrApp) -> None:
        """页脚「A 命令」chip 点击重开命令窗口（buffer 逐格定位，CJK 宽字符安全）。"""
        for i in range(app.screen.lines):
            col = _buf_find_col(app, i, "A 命令")
            if col is not None:
                _click(app, col + 1, i)
                assert app.wait_for("请输入命令或链接", 4), "chip 点击应重开命令窗口"
                return
        raise AssertionError("页脚未找到 A 命令 chip")

    # -- cases ---------------------------------------------------------------

    def _box_cols(self, app: EzrApp, box_top: int, box_bottom: int,
                  l: int, r: int) -> "tuple[int, int]":
        """文本框内容区列范围：取中间行首两个 │ 边框内列（工程确定性，不依赖固定偏移）。"""
        mid = (box_top + box_bottom) // 2
        cols = [x for x in range(l + 1, r) if _cell(app, mid, x) == "│"]
        assert len(cols) >= 2, f"文本框边框定位失败（行 {mid}）: {cols}"
        return cols[0] + 1, cols[-1]

    def _box_rows(self, app: EzrApp, box_top: int, box_bottom: int,
                  l: int, r: int) -> "list[str]":
        x0, x1 = self._box_cols(app, box_top, box_bottom, l, r)
        return [_row_text(app, y, x0, x1).strip()
                for y in range(box_top + 1, box_bottom)]

    def _box_content(self, app: EzrApp, box_top: int, box_bottom: int,
                     l: int, r: int) -> str:
        return "\n".join(self._box_rows(app, box_top, box_bottom, l, r))

    def _case_1(self):
        """QA-CW-01 布局（78×16 / 文本框 72×11 / 按钮行内框底−2）+ Esc/重开清缓冲/q/[✗]。"""
        @self.case("QA-CW-01")
        def go(env: Env):
            self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                ty, l, r, by = self._open_cmd(app)
                assert r - l + 1 == 78, f"命令窗口应 78 列（实测 {r - l + 1}）"
                assert by - ty + 1 == 16, f"命令窗口应 16 行（实测 {by - ty + 1}）"
                assert l == (app.screen.columns - 78) // 2, \
                    f"应水平居中（左界 {l}，期望 {(app.screen.columns - 78) // 2}）"
                assert ty == (app.screen.lines - 16) // 2, \
                    f"应垂直居中（顶行 {ty}，期望 {(app.screen.lines - 16) // 2}）"
                title_row = _row_text(app, ty)
                assert_not_in("命令窗口", title_row,
                              "旧标题「命令窗口」退役不出现（标题行）")
                x_close = next((x for x in range(r - 4, r)
                                if _cell(app, ty, x) == "✗"), None)
                assert x_close is not None, "顶边框右端应有 [✗]"
                btn_row = self._find_btn_row(app, ty, by)
                assert btn_row == by - 2, \
                    f"按钮行应为内框底 −2（实测 {btn_row}，底边框 {by}）"
                box_bottom = btn_row - 1
                assert "╰" in _row_text(app, box_bottom, l + 1, r) or \
                    _cell(app, box_bottom, l + 1) in "└╰", \
                    "按钮行上方应紧随文本框下边框（无空行）"
                box_top = box_bottom - 10
                tline = _row_text(app, box_top)
                assert "╭" in tline or "┌" in tline, "文本框应有顶边框"
                bx0, bx1 = self._box_cols(app, box_top, box_bottom, l, r)
                assert bx1 - bx0 == 70, \
                    f"文本框应宽 72（含边框，内容区 70，实测 {bx1 - bx0}）"
                content_rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert not any(s.strip() for s in content_rows), \
                    f"空缓冲应无任何占位提示: {content_rows}"
                app.type_text("probe123")
                app.pump(0.2)
                assert_in("probe123",
                          self._box_content(app, box_top, box_bottom, l, r),
                          "键入应入文本框")
                app.send("esc")
                app.pump(0.3)
                assert self._cmd_closed(app), "Esc 应关闭命令窗口"
                self._chip_click(app)
                assert "probe123" not in app.text(), "重开应缓冲清空（关闭即清）"
                ty2, l2, r2, by2 = self._frame(app, "请输入命令或链接")
                assert (ty2, l2) == (ty, l), "重开位置应重新居中（不跨次记忆）"
                app.type_text("x")
                app.send("tab")
                app.pump(0.2)
                app.send("q")
                app.pump(0.3)
                assert self._cmd_closed(app), "按钮焦点下 q 应关窗"
                self._open_cmd(app)
                _click(app, x_close if x_close is not None else r - 2, ty)
                app.pump(0.3)
                assert self._cmd_closed(app), "点击 [✗] 应关闭"
            finally:
                app.graceful_quit()

    def _case_2(self):
        """QA-CW-02 Enter 确认分流（HTTP / BT / 多行取首行 / 空输入）。"""
        @self.case("QA-CW-02")
        def go(env: Env):
            qdir = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            ub = os.path.join(qdir, UB_TORRENT)
            try:
                self._open_cmd(app)
                app.type_text(URL_EXAMPLE)
                app.send("enter")
                assert app.wait_for("添加任务：HTTP(S)", 4), "http URL 应分流 HTTP 对话框"
                assert self._cmd_closed(app), "命令窗口应先关闭"
                text = app.text()
                m = re.search(r"URL\s*>\s*(\S[^│]*?)(?:\s{2,}|│|$)", text)
                assert m and URL_EXAMPLE in (m.group(1) or ""), \
                    f"URL 字段应预填输入: {text[:400]!r}"
                assert re.search(r"并发\s*>\s*4\b", text), "并发应自动回填 4"
                app.send("esc")
                assert app.wait_gone("添加任务：HTTP(S)", 3), "Esc 应关闭对话框"

                self._open_cmd(app)
                app.type_text(ub)
                app.send("enter")
                assert app.wait_for("添加任务：BT", 5), "torrent 路径应分流 BT 对话框"
                text = app.text()
                assert "qatorrents/" + UB_TORRENT in text.replace("…", ""), \
                    f"种子字段应预填路径（字段 … 头截断，尾部完整可见）: {text[:400]!r}"
                assert re.search(r"并发\s*>\s*20\b", text), "BT 并发应预填 20"
                app.send("esc")
                assert app.wait_gone("添加任务：BT", 3)

                self._open_cmd(app)
                app.type_text(ub)
                app.send(ALT_ENTER)
                app.type_text("second line")
                app.send(ALT_ENTER)
                app.type_text("third line")
                app.send("enter")
                assert app.wait_for("添加任务：BT", 5), "多行首行为 torrent 应分流 BT"
                assert_in("仅处理第一行输入，其余 2 行已忽略", app.text(),
                          "toast 逐字")
                app.send("esc")
                app.pump(0.3)

                self._open_cmd(app)
                app.send("enter")
                app.pump(0.4)
                assert self._cmd_closed(app), "空输入确认应仅关窗"
                text = app.text()
                assert_not_in("添加任务：HTTP(S)", text, "空输入不应打开 HTTP 对话框")
                assert_not_in("添加任务：BT", text, "空输入不应打开 BT 对话框")
                assert_not_in("已添加任务", text, "空输入不应创建任务")
            finally:
                app.graceful_quit()

    def _case_3(self):
        """QA-CW-03 换行（Shift+Enter kitty / Alt+Enter 降级）/ 400 字符 wrap / 65 行上限。"""
        @self.case("QA-CW-03")
        def go(env: Env):
            self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                ty, l, r, by = self._open_cmd(app)
                btn_row = self._find_btn_row(app, ty, by)
                box_top, box_bottom = btn_row - 11, btn_row - 1
                app.send(SHIFT_ENTER)
                app.type_text("S")
                app.pump(0.2)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert rows and rows[0] == "" and any(v == "S" for v in rows), \
                    f"Shift+Enter 应在光标处换行（首行空 + S 落新行）: {rows[:4]}"
                app.send(ALT_ENTER)
                app.type_text("A")
                app.pump(0.2)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert [v for v in rows if v].count("A") == 1, \
                    f"Alt+Enter 应再次换行: {rows[:4]}"
                app.send(*(["bs"] * 8))
                app.pump(0.2)
                app.type_text("w" * 400)
                app.pump(0.5)
                joined = self._box_content(app, box_top, box_bottom, l, r)
                assert joined.count("w") >= 380, \
                    f"400 字符应全量可见（word wrap 展示）: {joined.count('w')}"
                toast_at = None
                for i in range(1, 71):
                    app.send(SHIFT_ENTER, gap=0.03)
                    if "命令窗口最多 64 行" in app.text():
                        toast_at = i
                        break
                assert toast_at is not None, "连续换行应在 64 行上限处 toast"
                assert toast_at >= 40, \
                    f"toast 出现过早（第 {toast_at} 次换行；PTY 丢退格容差下起点行数 ≤3，限界语义 = 第 65 行拒绝）"
            finally:
                app.graceful_quit()

    def _case_4(self):
        """QA-CW-04 Ctrl+J / Ctrl+Enter 确认分流 / Tab 焦点反色 / 按钮焦点 Enter。"""
        @self.case("QA-CW-04")
        def go(env: Env):
            qdir = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            ub = os.path.join(qdir, UB_TORRENT)
            try:
                self._open_cmd(app)
                app.type_text(ub)
                app.send("\x0a")
                assert app.wait_for("添加任务：BT", 5), "Ctrl+J 应确认分流 BT 对话框"
                app.send("esc")
                app.pump(0.3)
                assert self._cmd_closed(app)

                self._open_cmd(app)
                app.type_text(URL_EXAMPLE)
                app.send(CTRL_ENTER)
                assert app.wait_for("添加任务：HTTP(S)", 5), "Ctrl+Enter 应确认分流 HTTP"
                app.send("esc")
                app.pump(0.3)

                self._open_cmd(app)
                app.type_text(URL_EXAMPLE)
                ty, l, r, by = self._frame(app, "请输入命令或链接")
                btn_row = self._find_btn_row(app, ty, by)
                bg0 = self._btn_focus_bg(app, btn_row)
                app.send("tab")
                app.pump(0.3)
                bg1 = self._btn_focus_bg(app, btn_row)
                assert bg1 is not None and bg1 != bg0, \
                    f"Tab 后 [ 确定 ] 应反色高亮（{bg0!r} → {bg1!r}）"
                app.send("enter")
                assert app.wait_for("添加任务：HTTP(S)", 4), "按钮焦点 Enter 应确认"
                app.send("esc")
                app.pump(0.3)
                assert self._cmd_closed(app), "对话框 Esc 后应回主界面（非命令窗口残留）"
            finally:
                app.graceful_quit()

    def _case_5(self):
        """QA-CW-05 Shift+Tab 路径补全五例（目录尾随 / 唯一文件 / ~ 展开 / 无匹配 / 非 path）。"""
        @self.case("QA-CW-05")
        def go(env: Env):
            qdir = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_cmd(app)
                pre = os.path.join(env.fixture_root, "qato")
                app.type_text(pre)
                app.send(BACKTAB)
                assert app.wait_for("补全目录：", 3), "目录候选应出补全 toast"
                whole = app.text()
                assert os.path.join(env.fixture_root, "qatorrents") + "/" in whole, \
                    f"应补全为目录尾随 / 形态: {whole[-600:]!r}"
                app.send("esc")
                assert app.wait_gone("请输入命令或链接", 3)
                assert app.wait_gone("补全目录", 4), "例 1 toast 应在进入例 2 前消退（3s 生存期）"

                self._open_cmd(app)
                app.type_text(os.path.join(qdir, "ub"))
                app.send(BACKTAB)
                assert app.wait_for("24.04.2-desktop-amd64", 4), \
                    "唯一文件候选应直接补全（有界等待渲染到达）"
                app.pump(0.35)
                ty5, l5, r5, by5 = self._frame(app, "请输入命令或链接")
                btn5 = self._find_btn_row(app, ty5, by5)
                box_text = "".join(self._box_rows(app, btn5 - 11, btn5 - 1, l5, r5))
                assert_in("ubuntu-24.04.2-desktop-amd64.torrent", box_text, \
                    f"补全结果应为完整文件名（盒区内容，折行跨行拼接）: {box_text!r}")
                whole = app.text()
                assert_not_in("补全目录", whole, "唯一文件补全无 toast")
                assert_not_in("无匹配", whole, "唯一文件补全无无匹配 toast")
                app.send("esc")
                assert app.wait_gone("请输入命令或链接", 3)

                self._open_cmd(app)
                app.type_text("~/down")
                app.send(BACKTAB)
                assert app.wait_for("补全目录：~/downloads/", 3), \
                    "tilde 目录应补全为 ~/downloads/ 且 toast 保持 ~ 形态"
                assert_in("~/downloads/", app.text(), "输入框应保持 ~/ 形态")
                app.send("esc")
                assert app.wait_gone("请输入命令或链接", 3)

                self._open_cmd(app)
                app.type_text("/tmp/zzz无匹配前缀")
                app.send(BACKTAB)
                assert app.wait_for("无匹配：/tmp/zzz无匹配前缀", 3), \
                    "无匹配应 toast 无匹配：<token>"
                app.send("esc")
                assert app.wait_gone("请输入命令或链接", 3)
                assert app.wait_gone("无匹配", 4), "例 4 toast 应在进入例 5 前消退"

                self._open_cmd(app)
                app.type_text("https://example.com/ann")
                app.send(BACKTAB)
                app.pump(0.5)
                whole = app.text()
                assert_in("https://example.com/ann", whole, "URL token 应保持原样")
                assert_not_in("补全目录", whole, "非 path token 不响应（无补全 toast）")
                assert_not_in("无匹配", whole, "非 path token 不响应（无无匹配 toast）")
            finally:
                app.graceful_quit()

    def _case_6(self):
        """QA-CW-06 粘贴分行 / ↑↓ 列保持 / 行首 Backspace 合并接缝 / 点击定位 / 滚轮 / 拖窗。"""
        @self.case("QA-CW-06")
        def go(env: Env):
            self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                ty, l, r, by = self._open_cmd(app)
                btn_row = self._find_btn_row(app, ty, by)
                box_top, box_bottom = btn_row - 11, btn_row - 1
                bx0, bx1 = self._box_cols(app, box_top, box_bottom, l, r)
                os.write(app.fd, b"\x1b[200~abcdef\nab\x1b[201~")
                app.pump(0.4)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert "abcdef" in rows and "ab" in rows, \
                    f"粘贴应按 \\n 分行: {rows[:5]}"

                app.send("up")
                app.pump(0.15)
                app.type_text("X")
                app.pump(0.2)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert "abXcdef" in rows, \
                    f"↑ 后列应保持（短行下行尾 col=2，上行该列有字符应停该列）: {rows[:5]}"

                app.send("down")
                app.pump(0.15)
                app.type_text("Y")
                app.pump(0.2)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert "abY" in rows, \
                    f"↓ 后短行应落行尾（光标钳到下行尾）: {rows[:5]}"

                app.send("up", "left", "left", "left", "down")
                app.pump(0.2)
                app.send("bs")
                app.pump(0.2)
                app.type_text("Z")
                app.pump(0.2)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert "abXcdefZabY" in rows, \
                    f"行首 Backspace 合并后光标应落接缝（上一行长度处）: {rows[:5]}"

                row_idx = next(i for i, v in enumerate(rows) if "abXcdef" in v)
                _click(app, bx0 + rows[row_idx].index("d"), box_top + 1 + row_idx)
                app.pump(0.2)
                app.type_text("Q")
                app.pump(0.2)
                rows = self._box_rows(app, box_top, box_bottom, l, r)
                assert any("abXcQdef" in v for v in rows), \
                    f"点击应定位光标（窄字符格点其前 = 被点字符之前插入）: {rows[:5]}"

                os.write(app.fd, b"\x1b[200~" + b"\n".join(
                    f"line{n:02d} filler".encode() for n in range(20)) + b"\x1b[201~")
                app.pump(0.5)
                first_bottom = self._first_visible(app, box_top, box_bottom, l, r)
                app.send("mwu:60:20", "mwu:60:20")
                app.pump(0.3)
                first_up = self._first_visible(app, box_top, box_bottom, l, r)
                assert first_up != first_bottom, \
                    f"滚轮上滚应移动可视行视口（{first_bottom!r} → {first_up!r}）"
                app.send("mwd:60:20", "mwd:60:20")
                app.pump(0.3)
                first_back = self._first_visible(app, box_top, box_bottom, l, r)
                assert first_back == first_bottom, \
                    f"滚轮下滚应还原视口（不回弹）: {first_back!r}"

                ty2 = app.find_row("请输入命令或链接")
                assert ty2 is not None
                l2 = next(x for x in range(app.screen.columns) if _cell(app, ty2, x) == "╭")
                app.send(f"mclick:{l2 + 10}:{ty2 + 1}")
                app.pump(0.1)
                app.send(f"mdrag:{l2 + 20}:{ty2 + 6}")
                app.pump(0.15)
                app.send(f"mup:{l2 + 20}:{ty2 + 6}")
                app.pump(0.3)
                ty3 = app.find_row("请输入命令或链接")
                assert ty3 is not None and ty3 != ty2, \
                    f"拖拽标题应移动窗口（顶行 {ty2} → {ty3}）"
            finally:
                app.graceful_quit()

    def _first_visible(self, app: EzrApp, box_top: int, box_bottom: int,
                       l: int, r: int) -> str:
        for y in range(box_top + 1, box_bottom):
            s = _row_text(app, y, l + 3, r - 2).strip()
            if s:
                return s
        return ""

    def _help_all_text(self, app: EzrApp, pages: int = 14) -> str:
        """帮助窗口滚动收集全文本（组条目超一屏，PgDn 逐页收集拼接）。"""
        seen: list[str] = []
        for _ in range(pages):
            seen.append(app.text())
            app.send("pgdn")
            app.pump(0.25)
            new = app.text()
            seen.append(new)
            if new == seen[-2] and len(seen) > 3:
                break
        return "\n".join(seen)

    def _case_7(self):
        """QA-CW-07 A 入口分流 / 页脚 chips / 空态提示 / 帮助两组条目与 D59 文案。"""
        @self.case("QA-CW-07")
        def go(env: Env):
            self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                app.send("A")
                assert app.wait_for("请输入命令或链接", 4), "A 应打开命令窗口"
                assert_not_in("添加任务：HTTP(S)", app.text(), "A 不应直开 HTTP 添加框")
                app.send("esc")
                app.pump(0.3)
                assert_in("A 命令 · D 删除 · M 修改 · Q 退出 · H 帮助", app.text(),
                          "页脚 chips 逐字")
                assert_in("（此页签下没有任务，按 A 打开命令窗口添加任务）",
                          app.text(), "空态提示逐字")
                app.send("H")
                assert app.wait_for("帮助", 4), "帮助窗口应打开"
                help_text = self._help_all_text(app)
                assert_in("命令窗口（A）", help_text, "帮助应含【命令窗口（A）】组")
                assert_in("BT 对话框（添加/修改：BT）", help_text,
                          "帮助应含【BT 对话框】组")
                assert_in("http(s) URL / 本地 .torrent 路径", help_text,
                          "「输入内容」条目 D59 文案（无磁力）")
                assert_not_in("磁力链接", help_text,
                              "帮助输入内容条目不应再含磁力（D59）")
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _is_reddish(self, fg: "str | None") -> bool:
        """红色系判别：ANSI red 或 6 位 hex 且 R 主导（实测 e25c54 形态）。"""
        if fg is None:
            return False
        if fg == "red":
            return True
        if len(fg) == 6 and all(c in "0123456789abcdef" for c in fg.lower()):
            r = int(fg[0:2], 16)
            g = int(fg[2:4], 16)
            b = int(fg[4:6], 16)
            return r >= 0xB0 and r > g + 0x30 and r > b + 0x30
        return False

    def _case_8(self):
        """QA-CW-08 五类非法输入 → 66×7 错误窗口 + 五种关闭通道。"""
        @self.case("QA-CW-08")
        def go(env: Env):
            qdir = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            cases = [
                ("magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10"
                 "&dn=ubuntu-24.04.2-desktop-amd64.iso", "暂不支持该类型的链接"),
                ("ftp://fixture.local/files/big-one.bin", "暂不支持该类型的链接"),
                ("随手一行普通文本", "暂不支持该类型的链接"),
                (os.path.join(qdir, "missing.torrent"), "无法读取种子文件"),
                (os.path.join(qdir, "corrupt.torrent"), "无法读取种子文件"),
            ]
            closes = ["esc", "enter", "q", "btn", "x"]
            try:
                for idx, (bad, msg) in enumerate(cases):
                    self._open_cmd(app)
                    app.type_text(bad)
                    app.send("enter")
                    assert app.wait_for("错误", 4), \
                        f"[{idx}] 非法输入应弹出错误窗口: {bad[:40]}"
                    assert self._cmd_closed(app), "命令窗口应先关闭"
                    text = app.text()
                    assert_not_in("添加任务：HTTP(S)", text,
                                  "错误臂不应打开 HTTP 添加对话框")
                    assert_not_in("添加任务：BT", text,
                                  "错误臂不应打开 BT 添加对话框")
                    ety, el, er, eby = self._frame(app, " 错误 ")
                    assert er - el + 1 == 66, f"错误窗口应 66 列（实测 {er - el + 1}）"
                    assert eby - ety + 1 == 7, f"错误窗口应 7 行（实测 {eby - ety + 1}）"
                    btn_row = self._find_btn_row(app, ety, eby)
                    assert btn_row == ety + 4, \
                        f"[ 确定 ] 应在内框 y+3（实测 {btn_row}，顶边框 {ety}）"
                    msg_row_idx = next(
                        (y for y in range(ety + 1, eby) if msg in _row_text(app, y)),
                        None)
                    assert msg_row_idx is not None, f"应显示消息「{msg}」"
                    fg = self._row_fg(app, msg_row_idx, msg[:2])
                    assert fg is not None and self._is_reddish(fg), \
                        f"消息应红色（实得 fg={fg!r}）"
                    chan = closes[idx % len(closes)]
                    if chan == "btn":
                        btn_col = _row_text(app, btn_row).index("[ 确定 ]")
                        _click(app, btn_col + 1, btn_row)
                        assert app.wait_gone(" 错误 ", 3), \
                            "点击 [ 确定 ] 应关闭错误窗口（有界等待）"
                    elif chan == "x":
                        xc = next(x for x in range(er - 4, er) if _cell(app, ety, x) == "✗")
                        _click(app, xc, ety)
                        assert app.wait_gone(" 错误 ", 3), \
                            "点击 [✗] 应关闭错误窗口（有界等待）"
                    else:
                        app.send(chan)
                        assert app.wait_gone(" 错误 ", 3), \
                            f"关闭通道 {chan} 应关闭错误窗口（有界等待）"
            finally:
                app.graceful_quit()


def main() -> int:
    suite = CmdWindowSuite()
    suite.run()
    p, f, s = suite.summary()
    print(f"\n[06-cmd-window] P={p} F={f} S={s}")
    return 1 if f else 0


if __name__ == "__main__":
    sys.exit(main())
