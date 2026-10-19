#!/usr/bin/env python3
"""verify_footer_pad_scroll.py — 两项修订的端到端验证。

A. footer 左右内边距：内容行首列（边框内侧）为空白，chips 从第 2 内容列开始；
   新几何下点击 H 打开帮助、点击 Q 退出（命中矩形随 padding 平移回归）。
B. 并发列表滚动到底即停：滚到底后继续大量下滚，面板数据区不出现空白行
   （闪烁 bug 的特征是越界窗口渲染出底部空白行又被钳回）。

注意 pyte 宽字符陷阱：宽字符首 cell 有数据、次 cell 为空——必须按 cell 真实列
建立映射（同 scripts/test_focus_style.py row_cells 口径），否则 CJK 后的
索引左移、点击列算错。
"""
import os
import pty
import select
import sys
import time

import pyte

BIN = "/home/z/my-project/swarm-ezr2/ezr-demo/target/release/ezr-tui-demo"
COLS, ROWS = 110, 26  # 小高度：强制并发面板溢出（8 连接 > 可视数据行）

screen = pyte.Screen(COLS, ROWS)
stream = pyte.ByteStream(screen)

RESULTS = []


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond), detail))
    print(("PASS" if cond else "FAIL") + f": {name}" + (f" -- {detail}" if detail and not cond else ""))


def read_all(fd, budget=1.0):
    deadline = time.monotonic() + budget
    while time.monotonic() < deadline:
        r, _, _ = select.select([fd], [], [], 0.1)
        if not r:
            break
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        stream.feed(chunk)


def render():
    return ["".join(screen.buffer[y][x].data for x in range(COLS)) for y in range(ROWS)]


def cells(y):
    """行 y 的非空 cell 序列 [(真实列, 字符)]（跳过宽字符次 cell）。"""
    return [(x, screen.buffer[y][x].data) for x in range(COLS) if screen.buffer[y][x].data]


def find_in_row(y, needle):
    """needle 在行 y 的真实起始列；找不到返回 None。"""
    seq = cells(y)
    s = "".join(ch for _, ch in seq)
    i = s.find(needle)
    return None if i < 0 else seq[i][0]


def send(fd, payload: bytes):
    os.write(fd, payload)


def sgr_click(fd, col0, row0):
    send(fd, f"\x1b[<0;{col0 + 1};{row0 + 1}M".encode())
    time.sleep(0.12)
    send(fd, f"\x1b[<0;{col0 + 1};{row0 + 1}m".encode())


def sgr_wheel(fd, col0, row0, down=True):
    code = 65 if down else 64
    send(fd, f"\x1b[<{code};{col0 + 1};{row0 + 1}M".encode())


def main():
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.environ["COLORTERM"] = "truecolor"
        os.execv(BIN, [BIN])
        os._exit(1)

    import fcntl
    import struct
    import termios
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    try:
        time.sleep(1.5)
        read_all(fd, 1.0)
        lines = render()
        _ = lines

        # ---------- A1-A3: footer padding ----------
        fr = ROWS - 3  # 页脚内容行 1
        left_border_x = find_in_row(fr, "│")
        right_border_x = COLS - 1 - lines[fr][::-1].index("│")
        check("A1 footer 行1 边框内侧第 1 列为空白（左 padding）",
              screen.buffer[fr][left_border_x + 1].data == " ",
              f"got {screen.buffer[fr][left_border_x + 1].data!r}")
        a_col = find_in_row(fr, "A")
        check("A2 chips 起列 = 边框内侧第 2 列（padding=1）", a_col == left_border_x + 2,
              f"A at col {a_col}, expect {left_border_x + 2}")

        # ---------- B 前置：定位并发连接面板 ----------
        title_y = next((y for y in range(ROWS) if find_in_row(y, "并发连接") is not None), None)
        check("B1 前置：找到并发连接面板", title_y is not None)
        data_top = data_bot = bottom_first = init_first = None
        if title_y is not None:
            bottom_y = next((y for y in range(title_y + 1, ROWS) if find_in_row(y, "╰") is not None), ROWS - 1)
            data_top = title_y + 2          # 标题行 + 表头行之下
            data_bot = bottom_y - 1
            panel_l = left_border_x
            panel_r = right_border_x

            def data_blank_rows(ls):
                return [y for y in range(data_top, data_bot + 1)
                        if not ls[y][panel_l + 1: panel_r].strip()]

            check("B2 前置：面板初始无空白数据行", len(data_blank_rows(lines)) == 0,
                  f"blanks={data_blank_rows(lines)}")
            init_first = lines[data_top].strip()[:10]
            cx = (panel_l + 1 + panel_r) // 2
            cy = (data_top + data_bot) // 2

            # 大量下滚（20 次 × 2 行 ≫ 连接总数，必然多次触底）
            for _ in range(20):
                sgr_wheel(fd, cx, cy, down=True)
                time.sleep(0.02)
            time.sleep(0.25)
            read_all(fd, 0.4)
            lines4 = render()
            bl = data_blank_rows(lines4)
            check("B3 触底后连续下滚：无空白数据行（闪烁修复）", len(bl) == 0, f"blank rows={bl}")
            bottom_first = lines4[data_top].strip()[:10]
            check("B4 触底后窗口已移动（首行 ≠ 初始首行）",
                  bottom_first != init_first and bottom_first != "",
                  f"init={init_first!r} now={bottom_first!r}")

            # 再追加 5 次下滚：必须保持满窗（到底即停）
            for _ in range(5):
                sgr_wheel(fd, cx, cy, down=True)
                time.sleep(0.02)
            time.sleep(0.25)
            read_all(fd, 0.4)
            lines5 = render()
            check("B5 再次下滚仍无空白行", len(data_blank_rows(lines5)) == 0,
                  f"blank rows={data_blank_rows(lines5)}")
            same_shape = all(
                (lines4[y].strip() == "") == (lines5[y].strip() == "")
                for y in range(data_top, data_bot + 1)
            )
            check("B6 数据行骨架稳定（无行时有时无）", same_shape)

            # 上滚恢复
            for _ in range(20):
                sgr_wheel(fd, cx, cy, down=False)
                time.sleep(0.02)
            time.sleep(0.25)
            read_all(fd, 0.4)
            lines6 = render()
            check("B7 上滚回顶：首行恢复初始连接", lines6[data_top].strip()[:10] == init_first,
                  f"now={lines6[data_top].strip()[:10]!r} expect={init_first!r}")

        # ---------- A4/A5: 新几何下点击 H 打开帮助 ----------
        hx = find_in_row(fr, "帮助")
        check("A4 前置：footer 行内可见「帮助」chip", hx is not None)
        if hx is not None:
            sgr_click(fd, hx, fr)
            time.sleep(0.8)
            read_all(fd, 0.6)
            lines2 = render()
            # 帮助窗口出现 = 页脚行之外的区域出现「帮助」字样（页脚 chip 排除）
            opened = any(find_in_row(y, "帮助") is not None for y in range(0, ROWS - 5))
            check("A5 点击 H 打开帮助窗口", opened)
            if opened:
                send(fd, b"q")  # 帮助窗口内 q = 关窗（不退出）
                time.sleep(0.6)
                read_all(fd, 0.5)

        # ---------- A6: 点击 Q 退出（新几何回归） ----------
        lines_now = render()
        qx = find_in_row(fr, "退出")
        if qx is not None:
            sgr_click(fd, qx, fr)
            time.sleep(1.0)
            dead = False
            try:
                wpid, _ = os.waitpid(pid, os.WNOHANG)
                dead = wpid == pid
            except ChildProcessError:
                dead = True
            check("A6 点击 Q 退出进程（padding 后命中矩形正确）", dead)
        else:
            check("A6 前置：找到「退出」chip", False)
    finally:
        try:
            os.kill(pid, 9)
        except (ProcessLookupError, ChildProcessError):
            pass
        try:
            os.close(fd)
        except OSError:
            pass

    fails = [n for n, ok, _ in RESULTS if not ok]
    print(f"RESULT: {len(RESULTS) - len(fails)}/{len(RESULTS)} PASS" + (f"; FAILED: {fails}" if fails else ""))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
