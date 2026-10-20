#!/usr/bin/env python3
"""verify_scrollbar_universal.py — 通用滚动条组件（m3）端到端专项验证。

覆盖 §4.1.3 通用滚动条契约在全部实例上的一致行为：

  P1-P8   字形与几何：垂直 ╮↑│║│↓╯（右列，箭头紧贴角与轨道、不留白）、
          水平 ╰空白←空白═══───空白→空白╯（底行，箭头前后各 1 格空白）
  P9      ↓ 箭头单击 = 步进 1 行
  P9b     ↓ 箭头按压高亮：按下 = 前景滑块同色加粗（无背景）、松开后
          400ms 余辉、过期复原
  P10     ↓ 箭头按住连发（0.4s 起振 + 0.12s 周期，1.7s 位移 > 单击）
  P10b    按住期间保持高亮；拖出箭头 = 取消高亮
  P11-P12 滑块拖拽 = 比例滚动（拖到轨道顶 → 回到首行、滑块贴顶）
  P13     轨道点击 = 朝点击方向整页滚动
  P14-P15 键盘 →/← = 并发面板横向滚动（IP 列移出/移回，累计上传移入）
  P16     水平滑块拖到最左 = 横向偏移归零
  Q1-Q5   pieces 弹窗：同组件同行为——↓ 点击滚动网格、拖滑块窗口不动
          （修复：拖滚动条误移窗口）、滑块拖到底 = 末页吸附

用法: python3 scripts/verify_scrollbar_universal.py [二进制路径]

注意 pyte 宽字符陷阱：宽字符首 cell 有数据、次 cell 为空——join 字符串
下标 ≠ 真实列。所有几何/命中判定一律走 col_at/slice_cols（按 cell 真实列
访问，与 test_focus_style.py row_cells 口径一致）。
"""
import os
import pty
import select
import struct
import sys
import termios
import time

import pyte

BIN = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "target", "release", "ezr-tui-demo"
)

RESULTS = []


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond), detail))
    print(("PASS" if cond else "FAIL") + f": {name}"
          + (f" -- {detail}" if detail and not cond else ""))


class Tui:
    """pty + pyte 会话：注入键盘/SGR 鼠标事件，逐帧取屏（列精确访问）。"""

    def __init__(self, cols, rows):
        self.ncols, self.nrows = cols, rows
        self.pid, fd = pty.fork()
        if self.pid == 0:
            os.environ["TERM"] = "xterm-256color"
            os.environ["COLORTERM"] = "truecolor"
            os.execv(BIN, [BIN])
            os._exit(1)
        import fcntl
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        self.fd = fd
        self.screen = pyte.Screen(cols, rows)
        self.stream = pyte.ByteStream(self.screen)

    def read_all(self, budget=1.0):
        deadline = time.monotonic() + budget
        while time.monotonic() < deadline:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if not r:
                break
            try:
                chunk = os.read(self.fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            self.stream.feed(chunk)

    def key(self, data, settle=0.25):
        os.write(self.fd, data)
        time.sleep(settle)
        self.read_all(0.3)

    def mdown(self, x, y):
        os.write(self.fd, f"\x1b[<0;{x + 1};{y + 1}M".encode())

    def mup(self, x, y):
        os.write(self.fd, f"\x1b[<0;{x + 1};{y + 1}m".encode())

    def mdrag(self, x, y):
        os.write(self.fd, f"\x1b[<32;{x + 1};{y + 1}M".encode())

    def click(self, x, y, settle=0.4):
        self.mdown(x, y)
        time.sleep(0.12)
        self.mup(x, y)
        time.sleep(settle)
        self.read_all(0.3)

    def col_at(self, x, y):
        """(x, y) 处 cell 字符（宽字符次 cell = ''，列号恒精确）。"""
        return self.screen.buffer[y][x].data

    def row(self, y):
        """第 y 行按列展开的字符列表（下标 = 真实列）。"""
        return [self.col_at(x, y) for x in range(self.ncols)]

    def text(self, y, x0=0, x1=None):
        """第 y 行 [x0, x1) 列的拼接文本（列精确；'' 跳过宽字符尾格）。"""
        x1 = self.ncols if x1 is None else x1
        return "".join(self.col_at(x, y) for x in range(x0, x1))

    def close(self):
        try:
            os.kill(self.pid, 9)
            os.waitpid(self.pid, 0)
        except (ProcessLookupError, ChildProcessError):
            pass
        try:
            os.close(self.fd)
        except OSError:
            pass


def find_panel(t, title, min_col=0):
    """按标题行定位面板 → (title_y, bottom_y, right_x)。列精确。"""
    title_y = next((y for y in range(t.nrows) if title in t.text(y)), None)
    if title_y is None:
        return None, None, None
    bottom_y = next(
        (y for y in range(title_y + 1, t.nrows)
         if max((x for x in range(min_col, t.ncols) if t.col_at(x, y) == "╰"),
                default=-1) > min_col),
        None,
    )
    right_x = max((x for x in range(min_col, t.ncols)
                   if t.col_at(x, title_y) == "╮"), default=None)
    return title_y, bottom_y, right_x


def vbar_cells(t, x_col, y0, y1):
    """滚动条列 (y0..y1) 的字符序列（列精确）。"""
    return "".join(t.col_at(x_col, y) for y in range(y0, y1 + 1))


def conns_first_data(t, title_y, bot_y):
    """并发面板第一条数据行文本（首列 = 掩码 IP），无则 None。"""
    for y in range(title_y + 2, bot_y):
        s = t.text(y, 71, 119).strip()
        if s and (("*." in s) or (":*:" in s) or s[0].isdigit()):
            return s
    return None


def conns_header(t, title_y, bot_y):
    """并发面板表头行文本（含列名；窗口 = 面板内框 71..119 列），无则 None。"""
    for y in range(title_y + 1, bot_y):
        s = t.text(y, 71, 119)
        if "下载速度" in s:
            return s
    return None


def hbar_geom(t, yh):
    """第 yh 行水平滚动条几何 (xa=←, xb=→)，无则 (None, None)。"""
    xa = next((x for x in range(70, t.ncols) if t.col_at(x, yh) == "←"), None)
    xb = next((x for x in range(t.ncols - 2, 70, -1) if t.col_at(x, yh) == "→"), None)
    return xa, xb


def main():
    # ============ 会话 1：主界面 120x33（End 选中 sintel BT 任务） ============
    # 120x33 下并发连接面板高 12：垂直紧贴几何 → 轨道 8 格、滑块 7 格、
    # 行程 1 —— 能同时驱动箭头/滑块/轨道三种交互（面板高 < 6 则组件按
    # 契约不绘制）
    t = Tui(120, 33)
    try:
        time.sleep(1.4)
        t.read_all()
        t.key(b"\x1b[F", 0.35)      # End → sintel（BT，12 连接）
        t.key(b"\t", 0.25)
        t.key(b"\t", 0.5)           # Tab×2 → 聚焦并发连接

        title_y, bot_y, right_x = find_panel(t, "并发连接", min_col=60)
        check("P1 并发连接面板定位", title_y is not None and bot_y is not None,
              f"title={title_y} bottom={bot_y}")
        if title_y is None:
            return summary()

        # ---- 字形与几何（P2-P8） ----
        xa, xb = hbar_geom(t, bot_y)
        check("P2 BT 并发面板底部水平滚动条出现（←═→）",
              xa is not None and xb is not None,
              repr(t.text(bot_y, 68)) if xa is None else "")
        if xa is not None:
            htrack = t.text(bot_y, xa, xb + 1)
            check("P3 水平滚动条含 ─ 轨道 + ═ 滑块",
                  "─" in htrack and "═" in htrack, repr(htrack))
        up_y = next((y for y in range(title_y + 1, bot_y)
                     if t.col_at(right_x, y) == "↑"), None)
        dn_y = next((y for y in range(title_y + 1, bot_y)
                     if t.col_at(right_x, y) == "↓"), None)
        check("P4 垂直 ↑ 紧贴顶角（不留白，╮↓ 相邻）",
              up_y == title_y + 1, f"↑ at {up_y}, corner row {title_y}")
        check("P5 垂直 ↓ 紧贴底角（不留白，相邻行即 ╯）",
              dn_y == bot_y - 1, f"↓ at {dn_y}, corner row {bot_y}")
        if xa is not None:
            left_corner_x = max((x for x in range(t.ncols)
                                 if t.col_at(x, bot_y) == "╰"), default=-1)
            check("P6 水平 ← 距左边框 1 格留白（水平专用；垂直紧贴不留白）",
                  xa == left_corner_x + 2, f"← at {xa}, corner {left_corner_x}")
            check("P7 水平 → 距右边框 1 格留白（水平专用；垂直紧贴不留白）",
                  xb == t.ncols - 3, f"→ at {xb}, right border {t.ncols - 1}")
        vcol = vbar_cells(t, right_x, title_y + 1, bot_y - 1)
        check("P8 垂直滚动条 = ↑│║│↓（║ 滑块在 │ 单线轨道上）",
              "↑" in vcol and "║" in vcol and "↓" in vcol and "│" in vcol,
              repr(vcol))
        # ---- P8b：垂直紧贴（不留白）：↑ 下方、↓ 上方直接是轨道/滑块 ----
        check("P8b ↑ 下方紧贴轨道/滑块（垂直不留白，无空白格）",
              t.col_at(right_x, title_y + 2) in ("│", "║"),
              repr(t.col_at(right_x, title_y + 2)))
        check("P8b2 ↓ 上方紧贴轨道/滑块（垂直不留白，无空白格）",
              t.col_at(right_x, bot_y - 2) in ("│", "║"),
              repr(t.col_at(right_x, bot_y - 2)))
        if xa is not None:
            check("P8c ← 外侧（靠左边框）留白 1 格空白（水平同规则）",
                  t.col_at(xa - 1, bot_y) == " ",
                  repr(t.col_at(xa - 1, bot_y)))
            check("P8c2 ← 内侧（靠轨道）留白 1 格空白（水平同规则）",
                  t.col_at(xa + 1, bot_y) == " ",
                  repr(t.col_at(xa + 1, bot_y)))

        # ---- P9：↓ 箭头单击 = 步进 1 ----
        first0 = conns_first_data(t, title_y, bot_y)
        t.click(right_x, dn_y)
        first1 = conns_first_data(t, title_y, bot_y)
        check("P9 ↓ 箭头单击 = 纵向步进 1 行（首行移动）",
              first0 and first1 and first0[:12] != first1[:12],
              f"{first0!r} → {first1!r}")
        after_click = first1

        # ---- P9b：↓ 箭头按压高亮（按下即亮、松开 400ms 余辉、过期复原） ----
        def hot(c):
            """按压高亮态：前景色 = 滑块同款（聚焦 → 绿）+ 加粗，无背景色。
            crossterm 把基本色发为 256 色（38;5;2），pyte 归一化为十六进制：
            green→00cd00；背景无设置时 bg == "default"。"""
            return (c.fg in ("green", "00cd00") and c.bold
                    and c.bg in ("default", None))

        t.mdown(right_x, dn_y)
        time.sleep(0.15)
        t.read_all(0.2)
        c = t.screen.buffer[dn_y][right_x]
        check("P9b1 按下 ↓ = 高亮（前景滑块同色加粗、无背景）", hot(c), f"fg={c.fg} bg={c.bg} bold={c.bold}")
        t.mup(right_x, dn_y)
        time.sleep(0.1)
        t.read_all(0.15)
        c = t.screen.buffer[dn_y][right_x]
        check("P9b2 松开后余辉内仍高亮（400ms）", hot(c), f"fg={c.fg} bg={c.bg}")
        time.sleep(0.5)
        t.read_all(0.2)
        c = t.screen.buffer[dn_y][right_x]
        check("P9b3 余辉结束复原（非滑块色高亮）", not hot(c), f"fg={c.fg} bg={c.bg}")

        # ---- P10：↓ 按住连发 + P10b 高亮保持/拖出取消 ----
        t.mdown(right_x, dn_y)
        time.sleep(0.5)
        t.read_all(0.2)
        c = t.screen.buffer[dn_y][right_x]
        check("P10b1 按住期间箭头保持高亮", hot(c), f"fg={c.fg} bg={c.bg}")
        t.mdrag(right_x, (title_y + bot_y) // 2)      # 拖离箭头
        time.sleep(0.2)
        t.read_all(0.2)
        c = t.screen.buffer[dn_y][right_x]
        check("P10b2 拖出箭头 = 取消高亮（连发不中断）", not hot(c),
              f"fg={c.fg} bg={c.bg}")
        time.sleep(1.0)             # 0.4s 起振 + ~1.3s 连发
        t.mup(right_x, dn_y)
        time.sleep(0.4)
        t.read_all()
        first2 = conns_first_data(t, title_y, bot_y)
        check("P10 ↓ 按住连发 1.7s 位移大于单击（到底即停）",
              first2 and first2[:12] not in {(first0 or "")[:12], (after_click or "")[:12]},
              f"click→{after_click!r} hold→{first2!r}")

        # ---- P11/P12：滑块拖拽回顶 ----
        thumb_rows = [y for y in range(title_y + 1, bot_y)
                      if t.col_at(right_x, y) == "║"]
        check("P11 前置：垂直滑块可见", len(thumb_rows) > 0, str(thumb_rows))
        if thumb_rows:
            ty = thumb_rows[len(thumb_rows) // 2]
            top_track = title_y + 2   # ↑（紧贴顶角 +1）之下第一轨道格
            t.mdown(right_x, ty)
            steps = 6
            for i in range(1, steps + 1):
                yy = ty + (top_track - ty) * i // steps
                t.mdrag(right_x, yy)
                time.sleep(0.05)
                t.read_all(0.05)
            t.mup(right_x, top_track)
            time.sleep(0.4)
            t.read_all()
            first3 = conns_first_data(t, title_y, bot_y)
            thumb_after = [y for y in range(title_y + 1, bot_y)
                           if t.col_at(right_x, y) == "║"]
            check("P12 滑块拖到轨道顶 = 回到首行（比例滚动 + 贴合头箭头内侧）",
                  first3 and first0 and first3[:12] == first0[:12]
                  and thumb_after and thumb_after[0] == title_y + 2,
                  f"first={first3!r} expect={first0!r} thumb={thumb_after}")

            # ---- P13：轨道点击翻页 ----
            t.click(right_x, up_y)      # 确保滑块贴顶（P12 已回顶，此处幂等），
            before = conns_first_data(t, title_y, bot_y)
            thumb_rows2 = [y for y in range(title_y + 1, bot_y)
                           if t.col_at(right_x, y) == "║"]
            track_below = thumb_rows2[-1] + 1 if thumb_rows2 else None
            if track_below and track_below < bot_y - 1 \
                    and t.col_at(right_x, track_below) == "│":
                t.click(right_x, track_below)
                after = conns_first_data(t, title_y, bot_y)
                check("P13 轨道点击 = 朝点击方向整页滚动（首行大幅移动）",
                      before and after and before[:12] != after[:12],
                      f"{before!r} → {after!r}")
            else:
                check("P13 轨道点击 = 朝点击方向整页滚动", False,
                      f"no track below thumb at {track_below}")

        # ---- P14/P15：键盘 →/← 横向滚动 ----
        t.click(95, (title_y + bot_y) // 2)   # 点面板中部 = 聚焦（清按压态）
        hdr0 = conns_header(t, title_y, bot_y) or ""
        for _ in range(6):
            t.key(b"\x1b[C", 0.06)            # → ×6 = 12 列（钳到 max）
        hdr_r = conns_header(t, title_y, bot_y) or ""
        check("P14 → 键横向滚动：IP 列移出、累计上传列移入",
              "IP" not in hdr_r and "累计上传" in hdr_r,
              f"before={hdr0.strip()[:28]!r} after={hdr_r.strip()[:28]!r}")
        for _ in range(6):
            t.key(b"\x1b[D", 0.06)            # ← ×6 回最左
        hdr_b = conns_header(t, title_y, bot_y) or ""
        check("P15 ← 键横向滚回：IP 列回到视野",
              hdr_b.strip().startswith("IP"), repr(hdr_b.strip()[:16]))

        # ---- P16：水平滑块拖到最左 ----
        if xa is not None:
            t.mdown(xb, bot_y)                # 先按住 → 连发到最右
            time.sleep(1.2)
            t.mup(xb, bot_y)
            time.sleep(0.3)
            t.read_all()
            check("P16a 滑到最右时滑块贴合尾侧留白内侧（→ 内外侧各有留白）",
                  t.col_at(xb - 1, bot_y) == " " and t.col_at(xb + 1, bot_y) == " ",
                  repr((t.col_at(xb - 1, bot_y), t.col_at(xb + 1, bot_y))))
            thumb_cols = [x for x in range(xa + 1, xb)
                          if t.col_at(x, bot_y) == "═"]
            if thumb_cols:
                tx = thumb_cols[len(thumb_cols) // 2]
                t.mdown(tx, bot_y)
                for i in range(1, 7):
                    xx = tx + (xa + 1 - tx) * i // 6
                    t.mdrag(xx, bot_y)
                    time.sleep(0.05)
                    t.read_all(0.05)
                t.mup(xa + 1, bot_y)
                time.sleep(0.4)
                t.read_all()
                hdr_l = conns_header(t, title_y, bot_y) or ""
                check("P16 水平滑块拖到最左 = 横向偏移归零（IP 回视野）",
                      hdr_l.strip().startswith("IP"), repr(hdr_l.strip()[:16]))
            else:
                check("P16 水平滑块拖到最左 = 横向偏移归零", False, "thumb not found")
    finally:
        t.close()

    # ============ 会话 2：pieces 弹窗（拖滑块 = 滚内容、窗口不动） ============
    t2 = Tui(120, 44)
    try:
        time.sleep(1.4)
        t2.read_all()
        t2.key(b"p", 0.8)
        p_title = next((y for y in range(t2.nrows) if "分块状态" in t2.text(y)), None)
        check("Q1 pieces 弹窗打开", p_title is not None)
        if p_title is not None:
            _, p_bot, p_right = find_panel(t2, "分块状态", min_col=60)
            p_top = p_title
            vcol = vbar_cells(t2, p_right, p_top + 1, p_bot - 1)
            check("Q2 pieces 弹窗垂直滚动条 ║ 滑块 + │ 轨道（与主面板同组件）",
                  "↑" in vcol and "↓" in vcol and "║" in vcol and "│" in vcol, repr(vcol))
            grid_y0 = next((y for y in range(p_top + 1, p_bot)
                            if "⬜" in t2.text(y) or "🟩" in t2.text(y)), None)
            grid_text0 = "".join(t2.text(y) for y in range(p_top + 1, p_bot))
            corner_x0 = next((x for x in range(t2.ncols)
                              if t2.col_at(x, p_top) == "╭"), None)
            dn_y = next(y for y in range(p_top + 1, p_bot)
                        if t2.col_at(p_right, y) == "↓")
            t2.click(p_right, dn_y, settle=0.35)
            t2.click(p_right, dn_y, settle=0.35)
            grid_text1 = "".join(t2.text(y) for y in range(p_top + 1, p_bot))
            corner_x1 = next((x for x in range(t2.ncols)
                              if t2.col_at(x, p_top) == "╭"), None)
            title_y1 = next((y for y in range(t2.nrows)
                             if "分块状态" in t2.text(y)), None)
            check("Q3 pieces 滚动条 ↓ 箭头点击生效（网格滚动）",
                  grid_text0 and grid_text1 != grid_text0,
                  "grid region text changed")
            check("Q4 拖/点滚动条不移动窗口（位置不变）",
                  corner_x0 == corner_x1 and p_top == title_y1,
                  f"corner x {corner_x0}→{corner_x1}, y {p_top}→{title_y1}")
            thumb_rows = [y for y in range(p_top + 1, p_bot)
                          if t2.col_at(p_right, y) == "║"]
            if thumb_rows:
                ty = thumb_rows[len(thumb_rows) // 2]
                bot_track = dn_y - 1
                t2.mdown(p_right, ty)
                for i in range(1, 7):
                    yy = ty + (bot_track - ty) * i // 6
                    t2.mdrag(p_right, yy)
                    time.sleep(0.05)
                    t2.read_all(0.05)
                t2.mup(p_right, bot_track)
                time.sleep(0.4)
                t2.read_all()
                thumb_end = [y for y in range(p_top + 1, p_bot)
                             if t2.col_at(p_right, y) == "║"]
                check("Q5 pieces 滑块拖到底 = 末页吸附（贴合尾箭头内侧）",
                      thumb_end and thumb_end[-1] == dn_y - 1,
                      f"thumb bottom {thumb_end[-1] if thumb_end else None}"
                      f" expect {dn_y - 1}")
            else:
                check("Q5 pieces 滑块拖到底 = 末页吸附", False, "thumb not found")
    finally:
        t2.close()

    return summary()


def summary():
    fails = [n for n, ok, _ in RESULTS if not ok]
    print(f"RESULT: {len(RESULTS) - len(fails)}/{len(RESULTS)} PASS"
          + (f"; FAILED: {fails}" if fails else ""))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
