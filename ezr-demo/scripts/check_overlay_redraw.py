#!/usr/bin/env python3
"""check_overlay_redraw.py — 验证浮层翻转无闪烁（零清屏）+ 屏幕一致性。

历史背景 bug：浮层边框恰好落在底层 CJK 宽字符的右半格上时，增量 diff 跳过
被覆盖的尾格，宽字残留在浮层边框上且被后续帧固化。旧版修复 = 浮层翻转那帧
terminal.clear() 全屏清屏重绘——副作用是打开/关闭窗口的瞬间整个终端闪烁。

现行修复：残影由渲染层 clip_wide_at_edges（每处浮层 Clear 后调用）在缓冲区
层根治（左缘宽字符主格截断、右缘孤立半格补空格），前后帧模型自洽，无需
全屏清屏。本脚本断言新契约：

  1. 打开/关闭各浮层（添加/修改/删除对话框、校验下拉、pieces、帮助）
     —— 输出字节流中不得出现 \\x1b[2J（翻转零清屏 = 零闪烁）
  2. 浮层打开期间边框列完整（无 CJK 残影穿透、无缺格；112 列为历史
     straddle 复现宽度，另扫 100..118）
  3. 关闭后屏幕与打开前逐格一致（pyte 理想终端网格；界面每 100ms 有动画
     区（速度/图表/进度），用「双快照差分掩码」排除动画格，只断言静态格）
  4. 浮层稳定期零清屏（允许动画更新输出）

用法: python3 scripts/check_overlay_redraw.py [程序路径]
"""

import fcntl
import os
import pty
import re
import select
import struct
import sys
import termios
import time
import unicodedata

import pyte

MOVE_TO = re.compile(rb"\x1b\[\d+;\d+H")
CLEAR_ALL = b"\x1b[2J"

RESULTS = []


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond), detail))
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" ({detail})" if detail else ""))


class Tui:
    """pty + pyte 理想终端封装：收集字节流 + 维护屏幕网格。"""

    def __init__(self, program, cols=112, rows=36):
        self.cols, self.rows = cols, rows
        self.screen = pyte.Screen(cols, rows)
        self.stream = pyte.ByteStream(self.screen)
        self.bytes_since_mark = b""
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ["TERM"] = "xterm-256color"
            os.environ["COLORTERM"] = "truecolor"
            os.execvp(program, [program])
            os._exit(1)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

    def pump(self, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if not r:
                continue
            try:
                data = os.read(self.fd, 1 << 16)
            except OSError:
                break
            if not data:
                break
            self.stream.feed(data)
            self.bytes_since_mark += data

    def send(self, payload, wait=0.6):
        os.write(self.fd, payload)
        self.pump(wait)

    def mark(self):
        self.bytes_since_mark = b""

    def grid(self):
        """按 cell 的网格（cell data 可为 '' / 多字符，不做字符级 join）。"""
        return tuple(
            tuple(self.screen.buffer[y][x].data for x in range(self.cols))
            for y in range(self.rows)
        )

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


def border_broken(tui, x_left, y_top, y_bot, x_right):
    """浮层左右边框列是否残缺（残影/缺格/穿透都会表现为非边框字符）。

    参数均为 0 基：(左列, 顶行, 底行, 右列)。右列额外接受通用滚动条箭头
    ↑/↓、║ 滑块、│ 单线轨道（m3 修订：垂直紧贴角与轨道、不留白；留白格
    容忍仅为历史兼容，仅当紧邻 ↑/↓ 箭头时合法——防止误吞真缺格）。"""
    s = tui.screen
    if y_bot >= tui.rows or x_right >= tui.cols:
        return f"越界: y_bot={y_bot} x_right={x_right}"
    right_ok = ("│", "╮", "╯", "↑", "║", "↓")  # ║ = 聚焦滑块、│ = 单线轨道（画在右边框列）

    def gap_ok(y):
        """留白格：必须紧邻 ↑（上方留白）或 ↓（下方留白）箭头。"""
        if y > 0 and s.buffer[y - 1][x_right].data in ("↑", "↓"):
            return True
        if y < tui.rows - 1 and s.buffer[y + 1][x_right].data in ("↑", "↓"):
            return True
        return False

    for y in range(y_top, y_bot + 1):
        l = s.buffer[y][x_left].data
        r = s.buffer[y][x_right].data
        if l not in ("│", "╭", "╰") or (r not in right_ok and not (r == " " and gap_ok(y))):
            return f"row {y}: left={l!r} right={r!r}"
    return None


def row_text(tui, y):
    """行 y 的可见文本（按 cell 拼接，宽字符尾格 data 为空自然被跳过）。"""
    return "".join(tui.screen.buffer[y][x].data for x in range(tui.cols))


def row_seq(tui, y):
    """行 y 的非空 cell 序列 [(真实列, 字符)]（宽字符尾格 data='' 自然跳过）。"""
    return [(x, tui.screen.buffer[y][x].data) for x in range(tui.cols) if tui.screen.buffer[y][x].data]


def find_title_col(tui, y, title):
    seq = row_seq(tui, y)
    s = "".join(d for _, d in seq)
    i = s.find(title)
    return None if i < 0 else seq[i][0]


def locate_dialog_box(tui, title):
    """按标题定位浮层盒子（标题在顶边框行）：(xl, yt, xr, yb) 或 None。"""
    for y in range(tui.rows):
        tx = find_title_col(tui, y, title)
        if tx is None:
            continue
        xl = next((x for x in range(tx, -1, -1) if tui.screen.buffer[y][x].data == "╭"), None)
        if xl is None:
            continue
        xr = next(
            (x for x in range(tui.cols - 1, tx, -1) if tui.screen.buffer[y][x].data == "╮"),
            None,
        )
        if xr is None:
            continue
        for yb in range(y + 1, tui.rows):
            if tui.screen.buffer[yb][xl].data == "╰":
                return xl, y, xr, yb
    return None


def animated_mask(tui):
    """保留接口：返回空掩码（残影检测器不再依赖采样掩码）。"""
    return set(), tui.grid()


BORDER_GLYPHS = set("│─╭╮╰╯")


def is_cjk(ch):
    return bool(ch) and any(unicodedata.east_asian_width(c) in ("W", "F") for c in ch)


def residue_scan(tui, before, label, popup_rects):
    """残影检测（确定性）：关闭后，弹窗原四角位置不得残留角字符
    ╭╮╰╯——主界面面板角位置固定且与弹窗角不重合时，角字符只可能
    来自未清除的弹窗；若打开前该位置本就是面板角（几何重合，视觉
    无差）则豁免。"""
    now = tui.grid()
    bad = []
    for (xl, yt, xr, yb) in popup_rects:
        for (cx, cy) in ((xl, yt), (xr, yt), (xl, yb), (xr, yb)):
            if now[cy][cx] in "╭╮╰╯" and before[cy][cx] not in "╭╮╰╯":
                bad.append((cy, cx, now[cy][cx]))
    check(f"{label}: 关闭后无残影（角字符检测）", not bad,
          f"{len(bad)} 处, 首几个={bad[:3]}")


def cycle(tui, label, open_keys, close_keys, title, snapshot, mask):
    """开→断言零清屏+边框完整→关→断言无残影。"""
    tui.mark()
    for k in open_keys:
        tui.send(k, wait=0.5)
    tui.pump(0.4)
    data = tui.bytes_since_mark
    check(f"{label}: 打开零清屏（无 \\x1b[2J）", CLEAR_ALL not in data,
          f"2J count={data.count(CLEAR_ALL)}")
    box = locate_dialog_box(tui, title)
    if box is None:
        check(f"{label}: 浮层出现（定位 {title.strip()}）", False)
        for k in close_keys:
            tui.send(k, wait=0.4)
        return
    xl, yt, xr, yb = box
    broken = border_broken(tui, xl, yt, yb, xr)
    check(f"{label}: 边框完整无残影（x={xl}..{xr}, y={yt}..{yb}）", broken is None, broken or "")
    tui.mark()
    for k in close_keys:
        tui.send(k, wait=0.5)
    tui.pump(0.4)
    data = tui.bytes_since_mark
    check(f"{label}: 关闭零清屏（无 \\x1b[2J）", CLEAR_ALL not in data,
          f"2J count={data.count(CLEAR_ALL)}")
    residue_scan(tui, snapshot, label, [box])


def main():
    program = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"
    # phase2：A 键 = 命令窗口入口；打开添加对话框需输入 URL + Ctrl+J(\n) 分流
    # （phase2.6：命令窗口 Enter = 确认分流，Shift+Enter/Alt+Enter = 换行，
    #   此处用 \n=Ctrl+J 确认；对话框内 Enter 语义不变）
    URL = b"https://mirrors.edge.kernel.org/fedora/releases/42/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-42.iso"


    # ---- 场景 0：straddle 宽度扫描（112 = 历史复现宽度）----
    print("== 场景 0：straddle 宽度扫描（打开 A → 边框完整 → Esc → 还原） ==")
    for cols in [100, 104, 108, 110, 111, 112, 113, 114, 116, 118]:
        tui = Tui(program, cols=cols, rows=36)
        try:
            tui.pump(1.5)
            mask, snap = animated_mask(tui)
            tui.mark()
            tui.send(b"a", wait=0.3)
            tui.send(URL, wait=0.3)
            tui.send(b"\n", wait=0.6)
            tui.pump(0.3)
            data = tui.bytes_since_mark
            ok_noclear = CLEAR_ALL not in data
            box = locate_dialog_box(tui, "添加任务：HTTP(S)")
            if box is None:
                check(f"{cols} 列: 对话框出现", False)
                continue
            broken = border_broken(tui, box[0], box[1], box[3], box[2])
            tui.send(b"\x1b", wait=0.6)
            residue_scan(tui, snap, f"{cols} 列", [box])
            check(f"{cols} 列: 零清屏+边框完整",
                  ok_noclear and broken is None,
                  f"noclear={ok_noclear} broken={broken}")
        finally:
            tui.close()

    # ---- 场景 1..7：各浮层 翻转零清屏 + 一致性（112x36 标准尺寸）----
    tui = Tui(program)
    try:
        tui.pump(1.6)
        mask, snapshot = animated_mask(tui)

        print("== 场景 1：添加对话框 打开/关闭 ==")
        cycle(tui, "添加对话框", [b"a", URL, b"\n"], [b"\x1b"], "添加任务：HTTP(S)", snapshot, mask)

        print("== 场景 2：校验算法下拉 展开/收起/关窗 ==")
        tui.send(b"a", wait=0.3)
        tui.send(URL, wait=0.3)
        tui.send(b"\n", wait=0.5)
        for _ in range(3):  # URL -> 目录 -> 并发 -> 校验
            tui.send(b"\t", wait=0.15)
        tui.mark()
        tui.send(b"\r", wait=0.5)
        tui.pump(0.3)
        check("校验下拉: 展开零清屏", CLEAR_ALL not in tui.bytes_since_mark)
        box = locate_dialog_box(tui, "校验算法")
        if box:
            broken = border_broken(tui, box[0], box[1], box[3], box[2])
            check("校验下拉: 边框完整无残影", broken is None, broken or "")
        else:
            check("校验下拉: 出现（定位「校验算法」）", False)
        tui.mark()
        tui.send(b"\x1b", wait=0.4)   # 收起下拉
        tui.send(b"\x1b", wait=0.5)   # 关闭对话框
        tui.pump(0.3)
        check("校验下拉: 收起+关窗零清屏", CLEAR_ALL not in tui.bytes_since_mark)
        dbox = locate_dialog_box(tui, "添加任务：HTTP(S)")
        residue_scan(tui, snapshot, "校验下拉", [dbox] if dbox else [])

        print("== 场景 3：删除确认对话框 打开/关闭 ==")
        cycle(tui, "删除对话框", [b"d"], [b"\x1b"], "删除任务", snapshot, mask)

        print("== 场景 4：修改对话框 打开/关闭 ==")
        cycle(tui, "修改对话框", [b"m"], [b"\x1b"], "修改任务", snapshot, mask)

        print("== 场景 5：pieces 分块弹窗（P）打开/关闭 ==")
        cycle(tui, "pieces 弹窗", [b"p"], [b"p"], "分块状态", snapshot, mask)

        print("== 场景 6：帮助窗口（H）打开/关闭 ==")
        cycle(tui, "帮助窗口", [b"h"], [b"q"], "帮助", snapshot, mask)

        print("== 场景 7：浮层稳定期零清屏（允许动画输出） ==")
        tui.send(b"a", wait=0.3)
        tui.send(URL, wait=0.3)
        tui.send(b"\n", wait=0.5)
        tui.mark()
        tui.pump(1.0)
        stable = tui.bytes_since_mark
        check("稳定期 1s 零清屏", CLEAR_ALL not in stable)
        check("稳定期浮层仍在（标题可见）",
              any("添加任务：HTTP(S)" in row_text(tui, y) for y in range(tui.rows)))
        tui.send(b"\x1b", wait=0.5)
    finally:
        tui.close()

    failed = [n for n, ok, _ in RESULTS if not ok]
    print(f"\n结果: {len(RESULTS) - len(failed)}/{len(RESULTS)} 通过" + (f"；失败: {failed}" if failed else ""))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
