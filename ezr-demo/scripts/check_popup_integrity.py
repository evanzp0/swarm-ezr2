#!/usr/bin/env python3
"""check_popup_integrity.py — mini 终端模拟器验证浮层边框完整性。

把 TUI 的全部输出字节流喂进一个网格终端模型（ANSI MoveTo/Clear/SGR +
宽字符打印后游标推进 2 格），构建最终显示网格后断言：

  1. 添加对话框 (112x36 下居中于 x19,y12 74x12) 四边边框字符完整、无缺格
  2. 删除确认对话框 (x23,y14 66x7) 四边边框完整
  3. 校验算法下拉浮层 (x29,y18 24x10) 四边边框完整
  4. pieces 分块弹窗（P，x22,y8 68x20，网格为连续宽字符色块）四边边框完整
  5. 浮层区域内不得出现底层列表文本穿透（「等待空闲」「排队第」等字样）

两种模式：
  - 默认 = 理想终端：逐格应用写入（与主流真实终端一致）。对应现行契约：
    残影由渲染层 clip_wide_at_edges 在缓冲区层根治，浮层翻转不清屏
    （打开/关闭窗口零闪烁），diff 输出与终端宽字符语义自洽。
  - --ghost = 问题终端（历史模式）：向宽字符右半格（CONT 尾格）的写入
    一律忽略（无论主格是否已被改写）。这是为已退役的 terminal.clear()
    全屏重绘补丁定制的回归断言——按构造与无清屏的 diff 渲染互斥
    （顶/底边框行必然跨越中途 CJK，diff 必然逐格覆写），仅作历史对照用。

能检出两类问题：
  - 增量 diff 局部补丁：模型层跳过被覆盖尾格导致边框缺字（已由
    clip_wide_at_edges 根治）
  - 全量重绘错位：缓冲区存在「宽字符主格 + 浮层字符」重叠时，连续打印的
    宽字符使游标推进 2 格，边框字符被挤到错误的列——理想终端上边框也会断

用法: python3 scripts/check_popup_integrity.py [--ghost] [--cols N] [程序路径]
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
# pieces 弹窗四个色块字符：unicode-width 0.2（ratatui 同源）按 emoji presentation
# 计宽 2（真实终端同样推进 2 格），Python unicodedata EAW 为 N/否则 1——
# 理想终端模型需与 ratatui 口径一致，否则网格行内连续色块会错位
PIECE_WIDE = ("⬜", "🟩", "🟨", "🟦")


def rects_for(cols: int):
    """按终端宽度重算浮层矩形（居中结果）。

    关键差异：112 列下添加框 x=19 落在「待」的主格上（无矛盾）；
    110 列下 x=18 恰好是「等」字的右半格（正是用户遇到残影的坐标）。
    """
    ax = (cols - 74) // 2
    dx = (cols - 66) // 2
    # 本轮 padding 统一（§4.3 S2）：添加/删除框底部补齐 1 行（74×11→74×12、
    # 66×6→66×7，字段行/按钮行 y 不变）
    add = (ax, (ROWS - 12) // 2, 74, 12)
    dele = (dx, (ROWS - 7) // 2, 66, 7)
    # 下拉浮层挂在对话框内校验字段行下方：px = dlg.x + 1(边框) + 9
    # （字段行左 2 列 padding 后「> 」指示符同步 +1，本轮统一）
    # （需求修订：选项表新增 NONE 首行，8 项 → 高度 = 项数 + 2 = 10）
    drop = (ax + 10, 18, 24, 10)
    # pieces 分块弹窗（P）：68x20 居中（高度 19→21 为 padding 统一，
    # 21→23 为网格上下各空 1 行呼吸行，64→67 为加宽 3 列；
    # 23→20 为高度收缩 3 行/网格 11 行，67→68 为网格右边扩 1 列）
    pieces = ((cols - 68) // 2, (ROWS - 20) // 2, 68, 20)
    # 帮助窗口（H）：70x24 居中（内容超窗，右缘滚动条覆盖边框）
    help_dlg = ((cols - 70) // 2, (ROWS - 24) // 2, 70, 24)
    return add, dele, drop, pieces, help_dlg


class MiniTerm:
    """终端网格模型：MoveTo 绝对定位、SGR 忽略、宽字符推进 2 格。

    ghost=True 模拟「问题终端」（历史模式）：向宽字符右半格（CONT 尾格）
    的写入一律忽略——即使主格已被后续写入改写也不恢复。该模型是为已
    退役的 terminal.clear() 全屏重绘补丁定制的回归断言，与无清屏的
    diff 渲染按构造互斥，仅作历史对照（--ghost 显式启用）。
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
        if ch in PIECE_WIDE:
            return 2
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


def check_box(term: MiniTerm, rect, name: str, results: list, title_w: int,
              right_extra: tuple = ()) -> None:
    x, y, w, h = rect
    x1, y1 = x + w - 1, y + h - 1
    bad = []
    if term.cell(x, y) != "╭" or term.cell(x1, y) != "╮" \
            or term.cell(x, y1) != "╰" or term.cell(x1, y1) != "╯":
        bad.append("角")

    def gap_ok(yy):
        """留白格（历史 m3 外侧留白；现垂直已改紧贴不留白，容忍仅为兼容保留）：
        仅当紧邻 ↑/↓ 箭头时合法。"""
        return (term.cell(x1, yy - 1) in ("↑", "↓")
                or term.cell(x1, yy + 1) in ("↑", "↓"))

    for yy in range(y + 1, y1):
        if term.cell(x, yy) != "│":
            bad.append(f"左边框@({x},{yy})={term.cell(x, yy)!r}")
        # right_extra：合法覆盖右边框的字符（pieces 弹窗通用滚动条：
        # ↑↓ 上下箭头 + ║ 双线滑块 + │ 单线轨道，紧贴绘制不留白；
        # 留白容忍仅为历史兼容，防误吞真缺格）
        r = term.cell(x1, yy)
        if r != "│" and r not in right_extra \
                and not (r == " " and gap_ok(yy)):
            bad.append(f"右边框@({x1},{yy})={r!r}")
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
    ghost = "--ghost" in a
    prog = [x for x in a if not x.startswith("--")]
    program = prog[0] if prog else "./target/release/ezr-tui-demo"
    add_dlg, del_dlg, dropdown, pieces_dlg, help_dlg = rects_for(cols)
    print(f"模式: {'问题终端（半格写入产生残影）' if ghost else '理想终端'} · 终端 {cols}x{ROWS}")
    print(f"浮层矩形: 添加={add_dlg} 删除={del_dlg} 下拉={dropdown} pieces={pieces_dlg}")
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
        os.write(fd, b"\x1b")  # 收起下拉
        read(term, fd, 0.4)
        os.write(fd, b"\x1b")  # 关闭添加对话框（恢复列表，p 才能作用于选中任务）
        read(term, fd, 0.5)

        print("场景 4：pieces 分块弹窗（P，网格全宽字符色块 + 图例行）")
        os.write(fd, b"p")
        read(term, fd, 1.5)
        check_box(term, pieces_dlg, "pieces 弹窗四边边框完整 + 无穿透",
                  results, title_w=19, right_extra=("↑", "║", "↓", "│"))
        os.write(fd, b"\x1b")
        read(term, fd, 0.5)

        print("场景 5：帮助窗口（H，分类说明 + 右缘滚动条）")
        os.write(fd, b"h")
        read(term, fd, 1.5)
        check_box(term, help_dlg, "帮助窗口四边边框完整 + 无穿透",
                  results, title_w=9, right_extra=("↑", "║", "↓", "│"))
        os.write(fd, b"\x1b")
        read(term, fd, 0.5)
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
