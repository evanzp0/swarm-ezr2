#!/usr/bin/env python3
"""check_popup_integrity.py — mini 终端模拟器验证浮层边框完整性。

把 TUI 的全部输出字节流喂进一个理想网格终端（ANSI MoveTo/Clear/SGR +
宽字符打印后游标推进 2 格），构建最终显示网格后断言：

  1. 添加对话框 (112x36 下居中于 x19,y12 74x12) 四边边框字符完整、无缺格
  2. 删除确认对话框 (x23,y14 66x7) 四边边框完整
  3. 校验算法下拉浮层 (x28,y18 24x9) 四边边框完整
  4. 浮层区域内不得出现底层列表文本穿透（「等待空闲」「排队第」等字样）

能检出两类问题：
  - 增量 diff 局部补丁：覆盖 CJK 宽字符半格时部分终端残留（真实终端问题，
    理想模型不残留但可检出「边框列在网格上缺字符」的错位输出）
  - 全量重绘错位：缓冲区存在「宽字符主格 + 浮层字符」重叠时，连续打印的
    宽字符使游标推进 2 格，边框字符被挤到错误的列——理想终端上边框也会断

用法: python3 scripts/check_popup_integrity.py [程序路径]
"""

import codecs
import fcntl
import os
import pty
import select
import struct
import sys
import termios
import time
import unicodedata

COLS, ROWS = 112, 36
CONT = "\u0000"  # 宽字符的右半格占位
PENETRATE_WORDS = ("等待空闲", "排队第")


def rects_for(cols: int):
    """按终端宽度重算浮层矩形（居中结果）。

    关键差异：112 列下添加框 x=19 落在「待」的主格上（无矛盾）；
    110 列下 x=18 恰好是「等」字的右半格（正是用户遇到残影的坐标）。
    """
    ax = (cols - 74) // 2
    dx = (cols - 66) // 2
    add = (ax, 12, 74, 12)
    dele = (dx, 14, 66, 7)
    # 下拉浮层挂在对话框内校验字段行下方：px = dlg.x + 1(边框) + 8
    drop = (ax + 9, 18, 24, 9)
    return add, dele, drop


class MiniTerm:
    """终端网格模型：MoveTo 绝对定位、SGR 忽略、宽字符推进 2 格。

    ghost=True 模拟「问题终端」行为：向宽字符右半格独立写入窄字符时
    保留原宽字符显示（写入被忽略）——对应「下层的字遮住浮层框线」的
    残影机制。修复后的输出流不会出现这种写入，两种模型下都应通过。
    """

    def __init__(self, ghost: bool = True, cols: int = COLS) -> None:
        self.ghost = ghost
        self.cols = cols
        self.grid: list[list[str]] = [[" "] * cols for _ in range(ROWS)]
        self.x = 0
        self.y = 0
        self.state = "ground"
        self.csi = ""
        self.dec = codecs.getincrementaldecoder("utf-8")()

    @staticmethod
    def cw(ch: str) -> int:
        return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1

    def put(self, ch: str) -> None:
        if self.y < ROWS and self.x < self.cols:
            # 问题终端：向宽字符右半格独立写入窄字符 → 保留原宽字符（残影）
            if self.ghost and self.grid[self.y][self.x] == CONT and self.cw(ch) == 1:
                self.x = min(self.x + 1, self.cols - 1)
                return
            self.grid[self.y][self.x] = ch
            if self.cw(ch) == 2 and self.x + 1 < self.cols:
                self.grid[self.y][self.x + 1] = CONT
        self.x = min(self.x + self.cw(ch), self.cols - 1)

    def feed(self, data: bytes) -> None:
        for ch in self.dec.decode(data):
            if self.state == "ground":
                if ch == "\x1b":
                    self.state = "esc"
                elif ch == "\r":
                    self.x = 0
                elif ch == "\n":
                    self.y = min(self.y + 1, ROWS - 1)
                elif ch == "\x07":
                    pass
                else:
                    self.put(ch)
            elif self.state == "esc":
                self.state = "csi" if ch == "[" else ("osc" if ch == "]" else "ground")
                self.csi = ""
            elif self.state == "csi":
                if ("@" <= ch <= "~"):
                    self.handle_csi(self.csi + ch)
                    self.state = "ground"
                else:
                    self.csi += ch
            elif self.state == "osc":
                if ch in ("\x07", "\x1b"):
                    self.state = "ground"

    def handle_csi(self, s: str) -> None:
        final, params = s[-1], s[:-1]
        if final in "Hf":
            p = params.split(";")
            r = int(p[0]) if p and p[0] else 1
            c = int(p[1]) if len(p) > 1 and p[1] else 1
            self.y = min(max(r - 1, 0), ROWS - 1)
            self.x = min(max(c - 1, 0), self.cols - 1)
        elif final == "J" and params in ("2", "3"):
            self.grid = [[" "] * self.cols for _ in range(ROWS)]

    # ---- 检查辅助 ----
    def cell(self, x: int, y: int) -> str:
        return self.grid[y][x] if 0 <= y < ROWS and 0 <= x < self.cols else "?"

    def row_text(self, x0: int, x1: int, y: int) -> str:
        return "".join(c for c in self.grid[y][x0:x1] if c != CONT)


V_BORDERS = {"│", "╭", "╮", "╰", "╯"}
H_BORDERS = {"─", "╭", "╮", "╰", "╯"}


def check_box(term: MiniTerm, rect, name: str, results: list, title_w: int) -> None:
    x, y, w, h = rect
    x1, y1 = x + w - 1, y + h - 1
    bad = []
    if term.cell(x, y) != "╭" or term.cell(x1, y) != "╮" \
            or term.cell(x, y1) != "╰" or term.cell(x1, y1) != "╯":
        bad.append("角")
    for yy in range(y + 1, y1):
        if term.cell(x, yy) != "│":
            bad.append(f"左边框@({x},{yy})={term.cell(x, yy)!r}")
        if term.cell(x1, yy) != "│":
            bad.append(f"右边框@({x1},{yy})={term.cell(x1, yy)!r}")
    # 水平边：title 文字（含其合法半格）之外的区域不得是宽字符半格（错位特征）
    for xx in range(x + 1 + title_w, x1):
        if term.cell(xx, y) == CONT:
            bad.append(f"顶边半格@({xx},{y})")
        if term.cell(xx, y1) == CONT:
            bad.append(f"底边半格@({xx},{y1})")
    text = "\n".join(term.row_text(x + 1, x1, yy) for yy in range(y, y + h))
    for word in PENETRATE_WORDS:
        if word in text:
            bad.append(f"穿透文字「{word}」")
    ok = not bad
    results.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}")
    for b in bad[:6]:
        print(f"         -> {b}")


def read(term: MiniTerm, fd: int, seconds: float) -> None:
    deadline = time.time() + seconds
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if fd in ready:
            try:
                data = os.read(fd, 1 << 16)
            except OSError:
                return
            if not data:
                return
            term.feed(data)


def main() -> None:
    a = sys.argv[1:]
    cols = COLS
    if "--cols" in a:
        i = a.index("--cols")
        cols = int(a[i + 1])
        del a[i:i + 2]
    ghost = "--ideal" not in a
    prog = [x for x in a if not x.startswith("--")]
    program = prog[0] if prog else "./target/release/ezr-tui-demo"
    add_dlg, del_dlg, dropdown = rects_for(cols)
    print(f"模式: {'问题终端（半格写入产生残影）' if ghost else '理想终端'} · 终端 {cols}x{ROWS}")
    print(f"浮层矩形: 添加={add_dlg} 删除={del_dlg} 下拉={dropdown}")
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp(program, [program])
        os._exit(1)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, cols, 0, 0))

    results: list = []
    term = MiniTerm(ghost=ghost, cols=cols)
    try:
        read(term, fd, 1.5)  # 启动稳定
        print(f"场景 1：添加对话框")
        os.write(fd, b"a")
        read(term, fd, 1.0)
        check_box(term, add_dlg, "添加对话框四边边框完整 + 无穿透", results, title_w=14)
        os.write(fd, b"\x1b")
        read(term, fd, 0.5)

        print("场景 2：删除确认对话框")
        os.write(fd, b"d")
        read(term, fd, 1.0)
        check_box(term, del_dlg, "删除对话框四边边框完整 + 无穿透", results, title_w=10)
        os.write(fd, b"\x1b")
        read(term, fd, 0.5)

        print("场景 3：校验算法下拉浮层")
        os.write(fd, b"a")
        read(term, fd, 0.5)
        for _ in range(3):
            os.write(fd, b"\t")
            time.sleep(0.15)
        os.write(fd, b"\r")
        read(term, fd, 1.0)
        check_box(term, dropdown, "下拉浮层四边边框完整 + 无穿透", results, title_w=10)
    finally:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass

    failed = [r for r in results if not r[1]]
    print(f"\n结果: {len(results) - len(failed)}/{len(results)} 通过")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
