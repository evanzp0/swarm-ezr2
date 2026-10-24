#!/usr/bin/env python3
"""verify_footer_pad_scroll.py — 两项修订的端到端验证。

A. footer 双行化（本轮修订：命令图例 chips 恢复 + toast 保留）：页脚总高 4
   （边框 2 + 内容 2），行 1 = 五枚命令 chips（A/D/M/Q/H，鼠标点击与键盘同
   口径），行 2 = toast（◆）；左右各 2 列内边距（§4.3 S2，panel/弹窗同宽）；
   chips 点击回归：点击 H 开帮助、点击 Q 退出（命中矩形随双行 + 2 列
   padding 平移）。
B. 并发列表滚动到底即停：滚到底后继续大量下滚，面板数据区不出现空白行
   （闪烁 bug 的特征是越界窗口渲染出底部空白行又被钳回）。

注意 pyte 宽字符陷阱：宽字符首 cell 有数据、次 cell 为空——必须按 cell 真实列
建立映射（同 scripts/test_focus_style.py row_cells 口径），否则 CJK 后的
索引左移、点击列算错。
"""
import os
import pty
import os
import re
import select
import sys
import time

import pyte

BIN = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "target", "release", "ezr-tui-demo")
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

        # ---------- A: footer 双行化（chips 恢复 + toast 保留，本轮修订） ----------
        fr1 = ROWS - 3  # 页脚内容行 1（chips；总高 4：边框 2 + 内容 2）
        fr2 = ROWS - 2  # 页脚内容行 2（toast）
        lines = render()
        left_border_x = find_in_row(fr1, "│")
        right_border_x = COLS - 1 - lines[fr1][::-1].index("│")
        check("A1 footer 总高 4 行（上边框为圆角 ╭）",
              lines[ROWS - 4].startswith("╭") and lines[ROWS - 1].startswith("╰"),
              f"top={lines[ROWS - 4][:3]!r} bot={lines[ROWS - 1][:3]!r}")
        check("A2 chips 行边框内侧 2 列均为空白（左 padding = 2 列，§4.3 S2）",
              screen.buffer[fr1][left_border_x + 1].data == " "
              and screen.buffer[fr1][left_border_x + 2].data == " ",
              f"got {screen.buffer[fr1][left_border_x + 1].data!r} "
              f"{screen.buffer[fr1][left_border_x + 2].data!r}")
        foot_text = lines[fr1]
        check("A3 行 1 五枚命令 chips 全部可见（A 添加/D 删除/M 修改/Q 退出/H 帮助）",
              all(s in foot_text for s in ("A 添加", "D 删除", "M 修改", "Q 退出", "H 帮助")),
              f"row={foot_text!r}")
        check("A4 行 2 启动欢迎 toast 可见（◆ 前缀，4s 内；行 1 无 ◆）",
              find_in_row(fr2, "◆") is not None and find_in_row(fr1, "◆") is None,
              f"row2={lines[fr2]!r}")

        # ---------- B 前置：定位并发连接面板（保持原口径） ----------
        title_y = next((y for y in range(ROWS) if find_in_row(y, "并发连接") is not None), None)
        check("B1 前置：找到并发连接面板", title_y is not None)
        data_top = data_bot = bottom_first = init_first = None
        if title_y is not None:
            bottom_y = next((y for y in range(title_y + 1, ROWS) if find_in_row(y, "╰") is not None), ROWS - 1)
            data_top = title_y + 2          # 标题行 + 表头行之下
            data_bot = bottom_y - 1
            panel_l = left_border_x
            panel_r = right_border_x
            # 并发面板左列 = 标题行 ╭（右列 42% 起点列）；滚轮与切片都落在
            # 本面板内（旧版用整行/全屏中点，滚轮落左列任务列表——修复）
            right_panel_l = find_in_row(title_y, "╭ 并发连接")
            if right_panel_l is None:
                right_panel_l = panel_l

            def data_blank_rows(ls):
                return [y for y in range(data_top, data_bot + 1)
                        if not ls[y][right_panel_l + 1: panel_r].strip()]

            check("B2 前置：面板初始无空白数据行", len(data_blank_rows(lines)) == 0,
                  f"blanks={data_blank_rows(lines)}")

            # 面板首行判定只取首列序号（速度/累计值为实时模拟数据，
            # 逐秒跳动不可比对；序号 = 连接池槽位号，稳定）
            def panel_first(ls):
                m = re.match(r"\s*(\d+)",
                             ls[data_top][right_panel_l + 1: panel_r].strip())
                return m.group(1) if m else ""

            init_first = panel_first(lines)
            cx = (right_panel_l + 1 + panel_r) // 2
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
            bottom_first = panel_first(lines4)
            check("B4 触底后窗口已移动（面板首行 ≠ 初始首行）",
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
            check("B7 上滚回顶：面板首行恢复初始连接", panel_first(lines6) == init_first,
                  f"now={panel_first(lines6)!r} expect={init_first!r}")

        # ---------- A5-A7: chips 点击回归（命中矩形随双行 + 2 列 padding 平移） ----------
        hx = find_in_row(fr1, "帮助")
        check("A5 前置：footer 行 1 可见「帮助」chip", hx is not None)
        if hx is not None:
            sgr_click(fd, hx, fr1)
            time.sleep(0.8)
            read_all(fd, 0.6)
            lines2 = render()
            # 帮助窗口出现 = 页脚行之外的区域出现「帮助」字样（页脚 chip 排除）
            opened = any(find_in_row(y, "帮助") is not None for y in range(0, ROWS - 5))
            check("A6 点击 H 打开帮助窗口", opened)
            if opened:
                send(fd, b"q")  # 帮助窗口内 q = 关窗（不退出）
                time.sleep(0.6)
                read_all(fd, 0.5)

        # ---------- A7: 点击 Q 退出（双行页脚命中矩形回归） ----------
        lines_now = render()
        qx = find_in_row(fr1, "退出")
        if qx is not None:
            sgr_click(fd, qx, fr1)
            time.sleep(1.0)
            dead = False
            try:
                wpid, _ = os.waitpid(pid, os.WNOHANG)
                dead = wpid == pid
            except ChildProcessError:
                dead = True
            check("A7 点击 Q 退出进程（双行页脚命中矩形正确）", dead)
        else:
            check("A7 前置：找到「退出」chip", False)
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
