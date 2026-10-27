#!/usr/bin/env python3
"""verify_phase2_ui.py — phase2 UI 调整的 PTY 级自验证（临时脚本）。

覆盖面（与需求一一对应）：
  A. 命令窗口
     A1 标题 = 「请输入命令或链接」；旧标题「命令窗口」不再出现
     A2 内嵌文本框无「输入」小标题；空缓冲无任何输入提示（占位提示已去除）
     A2b 按钮行右侧无键位提示（phase2.5 移除「Enter 换行 · Ctrl+Enter 确认」）
     A3 word wrap + 无字数上限：400 字符无空格长 token → 70×5+50 六段全渲染
     A4 Shift+Enter 换行（kitty 协议 CSI 13;2u 模拟；两行都渲染在段首列）
     A4b Alt+Enter（ESC 前缀降级）换行
     A7 文本框滚动条在文本框右边框上（panel 层级，与 ScrollPanel 同构）
     A6 鼠标点击定位光标：点字符格 = 光标停其前；点行尾空白 = 段尾
     A5 种子路径 + Enter（= 触发 [确定] 按钮 click）→ 分流打开「添加任务：BT」
     A-UD ↑↓ 可视行导航（列保持 + 短行落行尾 + 跨 wrap 段行；回归旧版逻辑行+无条件行尾）
  B. 添加任务：BT（78×26，顶 padding 1 行；列表框 = 对话框内 x+2、y+4、宽 72、高 9 = 7 可视行
     （phase2.10 由 5 加高 2 行）；列表内容行首 2 列 + 行尾 2 列 S2 padding）
     B1 列表框左上标题固定 = 「选择下载文件」（旧动态标题/旧 (x/y) 格式消失）；
        动态统计「已选 x 个文件，共 z」移至列表框右下角（底边框右侧）
     B2 右上 [全选] [全不选] 按钮在位
     B3 目录默认收拢（▸ 有；展开标记 0）
     B4 逐行 → 展开后 ▸ 消失（树状展开可用）
     B5 目录三态：全不选后逐行 Space → [-] 部分选择态出现
     B6 计数/字节联动：n → 已选 0 个文件，共 0 B；a → 全选（数 > 初始默认；
     演示口径默认取消 1–2 个小文件，故初始非全选）
     B7 内容执行 S2 padding：首行 = 2 空格 + 「[」起（行首 2 列 padding，无额外位移）
     B8 展开后行数溢出 → 列表框右边框列（panel 层级）出现垂直滚动条（↑ ║ ↓）
     B-TP 顶 padding：对话框顶边框下一行为空（首行内容 = 内框 y+1，种子标签在 y+2）
     B-TR trackers 多行输入 panel：title = trackers；空缓冲占位提示
          「可输入多行 traker，使用 ALT+ENTER 换行」；Alt+Enter 换行可视；
          框高 7 = 边框 2 + 5 可视行；无 word wrap（长行不折行）+ 水平滚动条
          （下边框 ← ═ →，箭头点击右移/不回弹）+ 垂直滚动条；
          ↑↓ 列保持导航（↑ 到上行 / ← 中列 / ↓ 回下行插字中间）
  C. 修改任务：BT（BT 任务 M 分流；78×24，列表框 y+3，6 可视行）
     C1 标题 + 右下动态统计 + [全选]/[全不选] 按钮；trackers 预填首行可见
     C2 首行无位移 + 默认收拢

用法: python3 scripts/verify_phase2_ui.py [程序路径]
"""
import os
import pty
import re
import select
import fcntl
import struct
import sys
import termios
import time
import types

ROWS, COLS = 44, 120
TORRENT = "/srv/torrents/ubuntu-24.04.2-desktop-amd64.iso.torrent"

BAD = 0


def check(name, cond, detail=""):
    global BAD
    BAD += not cond
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" —— {detail}" if detail and not cond else ""))


# 复用 check_popup_integrity.py 的 MiniTerm 网格模型（久经考验的 ANSI 子集解析）
src = open('scripts/check_popup_integrity.py').read().replace(
    'if __name__ == "__main__":\n    main()', '')
mod = types.ModuleType('cpi')
exec(compile(src, 'cpi', 'exec'), mod.__dict__)
mod.ROWS = ROWS  # 终端行数对齐（默认 36）
CONT = mod.CONT


class Session:
    """一个 PTY 会话：输出喂进 MiniTerm 网格，按需取整屏/单行文本。"""

    def __init__(self, program):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ["TERM"] = "xterm-256color"
            os.execvp(program, [program])
            os._exit(1)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.term = mod.MiniTerm(ghost=False, cols=COLS)

    def rd(self, sec):
        end = time.time() + sec
        while time.time() < end:
            r, _, _ = select.select([self.fd], [], [], 0.02)
            if self.fd in r:
                try:
                    data = os.read(self.fd, 1 << 16)
                except OSError:
                    return
                if not data:
                    return
                self.term.feed(data)

    def send(self, payload, wait=0.4):
        os.write(self.fd, payload)
        self.rd(wait)

    def rows(self):
        return ["".join(c for c in self.term.grid[y] if c != CONT) for y in range(ROWS)]

    def raw_rows(self):
        """保留宽字符尾格占位（列号 = 网格列号），供几何定位用。"""
        return ["".join(self.term.grid[y]) for y in range(ROWS)]

    def screen(self):
        return "\n".join(self.rows())

    def close(self):
        try:
            os.kill(self.pid, 9)
            os.waitpid(self.pid, 0)
        except ChildProcessError:
            pass


def locate_dialog(rows, title):
    """按标题定位浮层（rows = 原始网格行，含宽字符尾格占位）：返回 (xl, yt, xr) 或 None。

    标题查找走「非占位字符序列」（宽字符在网格中主格+占位相间，直接 find 会失败），
    再映射回真实列号找 ╭/╮ 角。
    """
    for y, r in enumerate(rows):
        seq = [(x, c) for x, c in enumerate(r) if c != CONT]
        txt = "".join(c for _, c in seq)
        i = txt.find(title)
        if i < 0:
            continue
        col = seq[i][0]
        xl = next((x for x in range(col, -1, -1) if r[x] == "╭"), None)
        if xl is None:
            continue
        xr = next((x for x in range(len(r) - 1, col, -1) if r[x] == "╮"), None)
        if xr is None:
            continue
        return xl, y, xr
    return None


def bt_listbox(rows, title, add):
    """BT 对话框文件列表框几何：dict(box_x, box_y, box_h, inner_x, inner_y0, sb_col)。"""
    pos = locate_dialog(rows, title)
    if pos is None:
        return None
    xl, yt, _xr = pos
    box_x = xl + 3          # 对话框内框 x+2 = 列表框左边框列
    box_y = yt + (4 if add else 3)   # 顶 padding 1 行后列表框顶边框行
    box_h = (7 if add else 6) + 2    # 标准 7/6 可视行（phase2.10）+ 上下边框
    return dict(box_x=box_x, box_y=box_y, box_h=box_h, inner_x=box_x + 1,
                inner_y0=box_y + 1, sb_col=box_x + 71)  # 滚动条 = 列表框右边框列


def caption_stats(rows, g):
    """列表框底边框行右下角动态统计「已选 x 个文件，共 z」→ (Match) 或 None。"""
    r = rows[g["box_y"] + g["box_h"] - 1][g["box_x"]:g["box_x"] + 72].replace(CONT, "")
    return re.search(r"已选 (\d+) 个文件，共 ([\d.]+ [KMG]?B)", r)


def main():
    program = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"

    print("== A. 命令窗口 ==")
    s = Session(program)
    try:
        s.rd(1.5)
        s.send(b"a", 0.6)
        rows = s.raw_rows()
        scr = s.screen()
        # 命令窗口 78×16 居中于 120×44 → x21,y14；文本区 x25,y17，宽 70
        check("A0 命令窗口定位（x21,y14）", (locate_dialog(rows, "请输入命令或链接") is not None
                                        or rows[14][21] == "╭"))
        check("A1 标题 = 请输入命令或链接", "请输入命令或链接" in scr)
        check("A1 旧标题「命令窗口」不再出现", "命令窗口" not in scr)
        check("A2 空缓冲无输入提示（首行空白）",
              s.rows()[17][25:95].strip() == "",
              repr(s.rows()[17][25:95]))
        check("A2b 按钮行右侧无键位提示（phase2.5 移除）",
              "Enter 换行" not in scr and "Ctrl+Enter 确认" not in scr)

        # 无字数上限：400 个 x（旧版上限 300 会被吞）→ 70×5+50 六段全渲染
        s.send(b"x" * 400, 1.2)
        rows = s.raw_rows()
        segs = [rows[17 + i][25:25 + 70].replace(CONT, "").rstrip() for i in range(6)]
        check("A3 word wrap + 无字数上限（70×5+50 全可见）",
              segs[:5] == ["x" * 70] * 5 and segs[5] == "x" * 50,
              f"segs={[len(s2) for s2 in segs]}")

        # 追加至 1000 字符 → 15 段 > 9 可视行 → 滚动条出现在文本框右边框列 95
        s.send(b"x" * 600, 1.2)
        col = "".join(s.raw_rows()[y][95] for y in range(17, 26))
        check("A7 文本框滚动条在文本框右边框（panel 层级）",
              ("↑" in col and "↓" in col) or "║" in col, f"滚动条列 = {col!r}")

        # A8 滚动条活性：清空 → 10 行（> 9 可视行）→ 点 ↑ 箭头视口上滚且不回弹
        # （回归：旧版每帧强制回贴光标，滚动条点了也白点——"死"滚动条）
        for _ in range(3):
            s.send(b"\x7f" * 400, 0.3)
        se = b"\x1b[13;2u"   # Shift+Enter（kitty CSI 13;2u）= 换行（\r 现为确认）
        s.send(se.join(b"L%d" % i for i in range(10)), 0.8)
        rows = s.raw_rows()
        check("A8 前置：10 行溢出且视口跟随光标（首行 = L1）",
              rows[17][25:27] == "L1",
              f"首行 = {rows[17][25:31]!r}")
        s.send(b"\x1b[<0;96;18M", 0.15)   # 按下滚动条 ↑ 箭头（0 基 col 95, row 17）
        s.send(b"\x1b[<0;96;18m", 0.15)   # 释放
        rows = s.raw_rows()
        check("A8 点击 ↑ 箭头 → 视口上滚一行（首行 = L0）",
              rows[17][25:27] == "L0",
              f"首行 = {rows[17][25:31]!r}")
        s.rd(0.7)                         # 多帧重绘后仍不回弹（按需跟随）
        rows = s.raw_rows()
        check("A8 视口停留原处（不回贴光标）",
              rows[17][25:27] == "L0",
              f"首行 = {rows[17][25:31]!r}")

        # 清空（1000 个 x → 10 轮退格）→ Shift+Enter 换行两行（kitty CSI 13;2u）
        for _ in range(10):
            s.send(b"\x7f" * 100, 0.3)
        s.send(b"ab", 0.25)
        s.send(b"\x1b[13;2u", 0.3)
        s.send(b"cd", 0.4)
        rows = s.raw_rows()
        check("A4 Shift+Enter 换行：两行都渲染在首列",
              rows[17][25:27] == "ab" and rows[18][25:27] == "cd",
              f"r17={rows[17][25:31]!r} r18={rows[18][25:31]!r}")

        # 清空后 Alt+Enter（ESC 前缀）换行两行
        s.send(b"\x7f" * 10, 0.3)
        s.send(b"ab", 0.25)
        s.send(b"\x1b\r", 0.3)
        s.send(b"cd", 0.4)
        rows = s.raw_rows()
        check("A4b Alt+Enter 换行：两行都渲染在首列",
              rows[17][25:27] == "ab" and rows[18][25:27] == "cd",
              f"r17={rows[17][25:31]!r} r18={rows[18][25:31]!r}")

        # A-UD ↑↓ 可视行导航（word wrap 口径：列保持 + 短行落行尾 + wrap 段行；
        # 回归：旧版 ↑↓ = 逻辑行 + 无条件行尾，列位置全部丢失）
        # 清空（ab⏎cd = 5 次退格，10 次富余）
        s.send(b"\x7f" * 10, 0.3)
        # 两行 abcdef ⏎ xy（光标在 xy 行尾）→ ↑ 列保持到上行列 2 → 输入 Z 插中间
        s.send(b"abcdef", 0.35)
        s.send(b"\x1b[13;2u", 0.3)       # Shift+Enter 换行
        s.send(b"xy", 0.3)
        s.send(b"\x1b[A", 0.3)           # ↑ → 上行列 2（"abcdef" 中间有字符）
        s.send(b"Z", 0.35)
        rows = s.raw_rows()
        check("A-UD ↑ 列保持（Z 插在上行列 2 = abZcdef，非行尾 abcdefZ）",
              rows[17][25:25 + 7].replace(CONT, "") == "abZcdef",
              f"row17 = {rows[17][25:45]!r}")
        s.send(b"\x1b[B", 0.3)           # ↓ → 下行列 3 > "xy" 宽 2 → 行尾
        s.send(b"W", 0.35)
        rows = s.raw_rows()
        check("A-UD ↓ 短行落行尾（W 追加 = xyW）",
              rows[18][25:25 + 3].replace(CONT, "") == "xyW",
              f"row18 = {rows[18][25:35]!r}")
        # wrap 段行：清空 → 80 字符无空格长行（折行 70+10 两段）→ ↑ 跨段回
        # 第一段（同一逻辑行）列 10 → 输入 Q（旧版逻辑行 no-op，Q 落在第二段）
        s.send(b"\x7f" * 20, 0.4)
        s.send(b"0123456789" * 8, 0.8)
        s.send(b"\x1b[A", 0.3)           # ↑ → 可视段 0 列 10（行尾插入点钳制列 10）
        s.send(b"Q", 0.4)
        rows = s.raw_rows()
        check("A-UD ↑ 跨 wrap 段行（Q 插在第一段列 10）",
              rows[17][25:25 + 11].replace(CONT, "") == "0123456789Q",
              f"row17 = {rows[17][25:45]!r}")
        # 清空（81 字符）继续原 A6 流程：End 先把光标送回行尾——↑+Q 后光标在
        # 行中（列保持导航的结果），行中退格清不掉光标之后的 70 个字符
        s.send(b"\x1b[F", 0.3)           # End → 末行行尾
        s.send(b"\x7f" * 100, 0.5)

        # 清空后输入种子路径 + Enter → 添加任务：BT（光标随换行在末行尾）
        for _ in range(3):
            s.send(b"\x7f" * 10, 0.12)

        # A6 鼠标点击定位光标：hello world → 点 'w'（第 7 字符）→ 输入 X
        #    ⇒ hello Xworld（点字符格 = 光标停其前；旧版偏后一字）
        s.send(b"hello world", 0.4)
        col = 25 + 6 + 1   # 终端 0 基列 31 → SGR 1 基 32；行 y17 → SGR 18
        s.send(f"\x1b[<0;{col};18M".encode(), 0.25)
        s.send(f"\x1b[<0;{col};18m".encode(), 0.3)
        s.send(b"X", 0.4)
        rows = s.raw_rows()
        check("A6 鼠标点击 'w' 定位光标（X 插在其前）",
              rows[17][25:25 + 12].replace(CONT, "") == "hello Xworld",
              f"row17 = {rows[17][25:45]!r}")
        # 点行尾空白 → 光标到段尾；再输入 Y ⇒ hello XworldY
        col2 = 25 + 40 + 1
        s.send(f"\x1b[<0;{col2};18M".encode(), 0.25)
        s.send(f"\x1b[<0;{col2};18m".encode(), 0.3)
        s.send(b"Y", 0.4)
        rows = s.raw_rows()
        check("A6b 点击行尾空白 = 光标到段尾",
              rows[17][25:25 + 13].replace(CONT, "") == "hello XworldY",
              f"row17 = {rows[17][25:45]!r}")

        # 清空 A6 残留后输入种子路径 + Enter（= 触发 [确定] 按钮 click）→ 添加任务：BT
        s.send(b"\x7f" * 20, 0.35)
        s.send(TORRENT.encode(), 0.5)
        s.send(b"\r", 0.9)
        rows = s.raw_rows()
        check("A5 种子路径 Enter(触发[确定] click) → 添加任务：BT",
              locate_dialog(rows, "添加任务：BT") is not None
              or any("添加任务：BT" in r.replace(CONT, "") for r in rows))
    finally:
        s.close()

    print("== B. 添加任务：BT ==")
    s = Session(program)
    try:
        s.rd(1.5)
        s.send(b"a", 0.4)
        s.send(TORRENT.encode(), 0.4)
        s.send(b"\r", 0.9)     # Enter = 触发 [确定] click → 添加任务：BT
        s.send(b"\t", 0.25)     # 焦点 → 保存到
        s.send(b"\t", 0.25)     # 焦点 → 文件树
        rows = s.raw_rows()
        g = bt_listbox(rows, "添加任务：BT", add=True)
        check("B0 列表框几何定位", g is not None)
        if g is None:
            return 1

        # 右上标题固定「选择下载文件」；动态统计在右下角（底边框右侧）
        top = rows[g["box_y"]][g["box_x"]:g["box_x"] + 74].replace(CONT, "")
        check("B1 列表框标题固定 = 选择下载文件", "选择下载文件" in top, top[:44])
        check("B1 旧 (x/y) 格式消失",
              not re.search(r"\(\d+/\d+\)", top))
        m = caption_stats(rows, g)
        x0, z0 = (m.group(1), m.group(2)) if m else (None, None)
        check("B1 右下统计 = 已选 x 个文件，共 z", m is not None,
              rows[g["box_y"] + g["box_h"] - 1][g["box_x"]:g["box_x"] + 72].replace(CONT, ""))
        check("B1 初始已选字节非 0（演示口径默认取消 1–2 个小文件）",
              z0 is not None and z0 != "0 B" and x0 not in (None, "0"), str((x0, z0)))
        check("B1 右下角框角完整（╰ ╯ 未被统计覆盖）",
              rows[g["box_y"] + g["box_h"] - 1][g["box_x"]] == "╰"
              and rows[g["box_y"] + g["box_h"] - 1][g["box_x"] + 71] == "╯",
              rows[g["box_y"] + g["box_h"] - 1][g["box_x"]:g["box_x"] + 72].replace(CONT, ""))
        first_row = rows[g["box_y"]][g["box_x"]:g["box_x"] + 74].replace(CONT, "")
        check("B2 [全选] 按钮在位", "[全选]" in first_row)
        check("B2 [全不选] 按钮在位", "[全不选]" in first_row)
        check("B3 目录默认收拢（▸ 有）", "▸" in s.screen())
        first = rows[g["inner_y0"]][g["inner_x"]:g["inner_x"] + 71].replace(CONT, "")
        check("B7 内容执行 S2 padding（首行 = 2 空格 + [ 起）",
              first.startswith("  ["),
              f"首行 = {first[:14]!r}")

        # 逐行 → 展开所有目录（▸ 消失为止）
        all_open = False
        for _ in range(80):
            if "▸" not in s.screen():
                all_open = True
                break
            s.send(b"\x1b[C", 0.1)   # → 展开
            s.send(b"\x1b[B", 0.1)   # ↓
        check("B4 逐行 → 后目录全部展开（▸ 消失）", all_open)

        # 计数/字节联动 + 三态（右下角动态统计）
        s.send(b"n", 0.5)
        rows = s.raw_rows()
        m0 = caption_stats(rows, g)
        check("B6 全不选后 已选 0 个文件，共 0 B",
              m0 is not None and m0.group(1) == "0" and m0.group(2) == "0 B",
              rows[g["box_y"] + g["box_h"] - 1][g["box_x"]:g["box_x"] + 72].replace(CONT, ""))
        s.send(b"a", 0.5)
        rows = s.raw_rows()
        m1 = caption_stats(rows, g)
        check("B6 全选后数量大于初始默认（≥ 1–2 个被默认取消的小文件被勾回）",
              m1 is not None and m1.group(2) != "0 B"
              and int(m1.group(1)) > int(x0 or 0),
              f"x0={x0} z0={z0} got={m1.groups() if m1 else None} raw="
              + rows[g["box_y"] + g["box_h"] - 1][g["box_x"]:g["box_x"] + 72].replace(CONT, ""))

        # 逐行 Space 直到出现 [-]（部分选择态；目录/文件行均可触发）
        partial = False
        for _ in range(8):
            if "[-]" in s.screen():
                partial = True
                break
            s.send(b" ", 0.3)
            s.send(b"\x1b[B", 0.15)
        check("B5 部分选择态 [-] 出现", partial)

        # 垂直滚动条：列表框右边框列（panel 层级）↑ ║ ↓（箭头在首/末内容行）
        col = "".join(s.raw_rows()[y][g["sb_col"]]
                      for y in range(g["box_y"] + 1, g["box_y"] + g["box_h"] - 1))
        check("B8 文件树垂直滚动条在列表框右边框（↑║↓）",
              ("↑" in col and "↓" in col) or "║" in col, f"滚动条列 = {col!r}")

        # B9 滚动条活性：光标 ↓ 到末行（视口钉在底部）→ 点 ↑ 箭头仍可上滚且不回弹
        # （回归：旧版每帧强制回贴光标，光标停在末行时滚动条完全无法上滚）
        for _ in range(60):
            s.send(b"\x1b[B", 0.04)          # ↓ 移动光标至最后一行
        s.rd(0.3)
        top_before = s.raw_rows()[g["box_y"] + 1][g["inner_x"]:g["inner_x"] + 71].replace(CONT, "")
        sbc, sbr = g["sb_col"] + 1, g["box_y"] + 2   # SGR 1 基坐标（↑ 箭头）
        s.send(f"\x1b[<0;{sbc};{sbr}M".encode(), 0.15)
        s.send(f"\x1b[<0;{sbc};{sbr}m".encode(), 0.15)
        top_after = s.raw_rows()[g["box_y"] + 1][g["inner_x"]:g["inner_x"] + 71].replace(CONT, "")
        check("B9 光标在末行时点 ↑ 箭头 → 视口仍可上滚",
              top_after != top_before,
              f"前={top_before.strip()[:20]!r} 后={top_after.strip()[:20]!r}")
        s.rd(0.7)
        top_stay = s.raw_rows()[g["box_y"] + 1][g["inner_x"]:g["inner_x"] + 71].replace(CONT, "")
        check("B9 视口停留原处（不回贴末行光标）",
              top_stay == top_after,
              f"停留={top_stay.strip()[:20]!r} 期望={top_after.strip()[:20]!r}")

        # B-TP 顶 padding（§4.3 S2）：对话框顶边框下一行为空，首行内容（种子）在 y+2
        rows = s.raw_rows()
        pos = locate_dialog(rows, "添加任务：BT")
        if pos is not None:
            xl, yt, _xr = pos
            inner_x = xl + 1
            check("B-TP 顶 padding：内框首行为空（首行内容 = 内框 y+1）",
                  rows[yt + 1][inner_x:inner_x + 8].strip() == "",
                  f"内框首行 = {rows[yt + 1][inner_x:inner_x + 12]!r}")
            check("B-TP 种子字段行 = 内框 y+1（行首 2 列 S2 padding）",
                  rows[yt + 2][inner_x + 2:inner_x + 6].replace(CONT, "") == "种子",
                  f"y+1 行 = {rows[yt + 2][inner_x:inner_x + 8]!r}")
        else:
            check("B-TP 顶 padding 几何定位", False)

        # B-TR trackers 多行输入 panel：Tab×3（列表 → 并发 → 代理 → trackers）
        # 焦点循环后到达 trackers；空缓冲显示占位提示；Alt+Enter 换行可视；
        # 5 可视行 + 无 word wrap + 水平/垂直滚动条
        s.send(b"\t", 0.15)
        s.send(b"\t", 0.15)
        s.send(b"\t", 0.15)
        rows = s.raw_rows()
        pos = locate_dialog(rows, "添加任务：BT")
        if pos is not None:
            xl, yt, _xr = pos
            # trackers 框 = 并发/代理行下：Add 布局 y = 列表框底 + 2
            # （列表框 7 可视行 + 2 边框 = 框高 9；yt+4 = 列表框顶）
            tr_y = yt + 4 + 9 + 2            # 列表框顶 +2 行偏移 +框高 9 + 并发/代理 2
            top_tr = rows[tr_y][xl + 3:xl + 3 + 72].replace(CONT, "")
            check("B-TR trackers panel 标题 = trackers", " trackers " in top_tr, top_tr[:44])
            check("B-TR trackers 框高 7 = 边框 2 + 5 可视行（底边框 ╰ ╯ 在 tr_y+6）",
                  rows[tr_y + 6][xl + 3] == "╰" and rows[tr_y + 6][xl + 3 + 71] == "╯",
                  f"tr_y+6 = {rows[tr_y + 6][xl + 3:xl + 3 + 72].replace(CONT, '')!r}")
            inner_tr = rows[tr_y + 1][xl + 4:xl + 4 + 70].replace(CONT, "")
            check("B-TR 空缓冲占位提示（可输入多行 traker…）",
                  "可输入多行 traker，使用 ALT+ENTER 换行" in inner_tr,
                  f"内容 = {inner_tr.strip()[:44]!r}")
            # Alt+Enter 换行：tr1 → ESC+\r → tr2（视口跟随光标，第二行可见）
            s.send(b"tr1", 0.3)
            s.send(b"\x1b\r", 0.3)           # Alt+Enter = 换行
            s.send(b"tr2", 0.4)
            rows = s.raw_rows()
            check("B-TR Alt+Enter 换行：tr1/tr2 均渲染在首列",
                  rows[tr_y + 1][xl + 4:xl + 4 + 3].replace(CONT, "") == "tr1"
                  and rows[tr_y + 2][xl + 4:xl + 4 + 3].replace(CONT, "") == "tr2",
                  f"r0={rows[tr_y + 1][xl + 4:xl + 12]!r} r1={rows[tr_y + 2][xl + 4:xl + 12]!r}")
            # B-TR ↑↓ 可视行导航（列保持 + 短行落行尾；与命令窗口同一组件口径）：
            # ↑ 到上行（列 3 = "tr1" 行尾）→ ← 列 2 → ↓ 下行列 2（"tr2" 中间）
            # → 输入 Y 插在其前 = "trY2"（回归：旧版 ↑↓ 无条件行尾 = "tr2Y"）
            s.send(b"\x1b[A", 0.25)          # ↑
            s.send(b"\x1b[D", 0.25)          # ←
            s.send(b"\x1b[B", 0.25)          # ↓
            s.send(b"Y", 0.35)
            rows = s.raw_rows()
            check("B-TR ↑↓ 列保持导航（Y 插在下行列 2 = trY2）",
                  rows[tr_y + 2][xl + 4:xl + 4 + 4].replace(CONT, "") == "trY2",
                  f"row = {rows[tr_y + 2][xl + 4:xl + 12]!r}")
            # 无 word wrap：清空后输入 91 字符无空格长行 ⇒ 只渲染 70 列水平窗口
            # （跟随行尾光标：view_x 钳在 max_scroll_x=21，首行 = 末 70 列前缀
            # 123456789…），第二行必须为空（不折行）；水平滚动条出现在下边框
            # （← 轨道 ═ →）
            # （tr1⏎trY2 = 7 字符 + 1 次行首合并，退格 ≥ 8 次才清空；发 10 次富余）
            s.send(b"\x7f" * 10, 0.4)
            s.send(b"0123456789" * 9 + b"0", 0.9)
            rows = s.raw_rows()
            row0 = rows[tr_y + 1][xl + 4:xl + 4 + 70].replace(CONT, "")
            row1 = rows[tr_y + 2][xl + 4:xl + 4 + 70].replace(CONT, "")
            check("B-TR 无 word wrap：91 字符长行不折行（第二行为空）",
                  row0.startswith("123456789") and row1.strip() == "",
                  f"row0={row0[:16]!r} row1={row1[:16]!r}")
            bot_tr = rows[tr_y + 6][xl + 3:xl + 3 + 72].replace(CONT, "")
            check("B-TR 水平滚动条在下边框（← ═ →）",
                  "←" in bot_tr and "→" in bot_tr, f"底边框 = {bot_tr!r}")
            # Home → 光标回行首，水平视口回贴 view_x=0（首行渲染完整前缀）
            s.send(b"\x1b[H", 0.4)
            rows = s.raw_rows()
            row0 = rows[tr_y + 1][xl + 4:xl + 4 + 70].replace(CONT, "")
            check("B-TR Home 后首行渲染前缀（view_x=0）",
                  row0.startswith("0123456789"), f"row0={row0[:16]!r}")
            # 点水平滚动条 → 箭头（0 基 col xl+72, row tr_y+6）：视口右移 2 列且不回弹
            s.send(f"\x1b[<0;{xl + 73};{tr_y + 7}M".encode(), 0.15)
            s.send(f"\x1b[<0;{xl + 73};{tr_y + 7}m".encode(), 0.25)
            rows = s.raw_rows()
            row0 = rows[tr_y + 1][xl + 4:xl + 4 + 70].replace(CONT, "")
            check("B-TR 点 → 箭头 → 水平视口右移 2 列（前缀 01 消失）",
                  row0.startswith("23456789"), f"row0={row0[:16]!r}")
            s.rd(0.7)                         # 多帧重绘后仍不回弹（按需跟随）
            rows = s.raw_rows()
            row0 = rows[tr_y + 1][xl + 4:xl + 4 + 70].replace(CONT, "")
            check("B-TR 水平视口停留原处（不回贴行首光标）",
                  row0.startswith("23456789"), f"row0={row0[:16]!r}")
            # 清空长行后：trackers 焦点下裸 Enter = 确认（与 [立即下载] click 同义）
            # → 校验分流触发
            s.send(b"\x7f" * 100, 0.4)
            s.send(b"\r", 0.6)
            scr = s.screen()
            check("B-TR trackers 焦点 Enter = 确认（窗口关闭并分流）",
                  "添加任务：BT" not in scr,
                  "对话框仍未关闭")
        else:
            check("B-TR trackers panel 几何定位", False)
    finally:
        s.close()

    print("== C. 修改任务：BT ==")
    s = Session(program)
    try:
        s.rd(1.5)
        s.send(b"\x1b[F", 0.4)   # End → sintel（BT 任务）
        s.send(b"m", 0.7)
        rows = s.raw_rows()
        scr = s.screen()
        check("C1 标题 = 修改任务：BT", "修改任务：BT" in scr)
        g = bt_listbox(rows, "修改任务：BT", add=False)
        if g is not None:
            first = rows[g["inner_y0"]][g["inner_x"]:g["inner_x"] + 71].replace(CONT, "")
            check("C2 内容执行 S2 padding（首行 = 2 空格 + [ 起）", first.startswith("  ["),
                  f"首行 = {first[:14]!r}")
            top = rows[g["box_y"]][g["box_x"]:g["box_x"] + 74].replace(CONT, "")
            check("C1 列表框标题固定 = 选择下载文件", "选择下载文件" in top, top[:44])
            m = caption_stats(rows, g)
            check("C1 右下统计「已选 x 个文件，共 z」在位", m is not None,
                  rows[g["box_y"] + g["box_h"] - 1][g["box_x"]:g["box_x"] + 72].replace(CONT, ""))
            top = rows[g["box_y"]][g["box_x"]:g["box_x"] + 74].replace(CONT, "")
            check("C1 [全选] / [全不选] 按钮在位", "[全选]" in top and "[全不选]" in top)
            check("C2 目录默认收拢（▸ 有）", "▸" in scr)
            # C-TR trackers 预填（修改对话框演示数据）：首行 tracker 可见；
            # 4 行演示数据在 5 可视行内全可见（无 wrap，框高 7）
            pos = locate_dialog(rows, "修改任务：BT")
            if pos is not None:
                xl, yt, _xr = pos
                tr_y = yt + 3 + 8 + 2             # 列表框顶 +2 +框高 8 + 并发/代理 2
                top_tr = rows[tr_y][xl + 3:xl + 3 + 72].replace(CONT, "")
                check("C-TR trackers panel 标题 = trackers", " trackers " in top_tr,
                      top_tr[:44])
                check("C-TR trackers 框高 7 = 边框 2 + 5 可视行",
                      rows[tr_y + 6][xl + 3] == "╰" and rows[tr_y + 6][xl + 3 + 71] == "╯",
                      rows[tr_y + 6][xl + 3:xl + 3 + 72].replace(CONT, ""))
                inner_tr = rows[tr_y + 1][xl + 4:xl + 4 + 70].replace(CONT, "")
                check("C-TR trackers 预填首行可见（udp://tracker.ezr.dev…）",
                      inner_tr.startswith("udp://tracker.ezr.dev"),
                      f"首行 = {inner_tr.strip()[:44]!r}")
                inner_tr4 = rows[tr_y + 4][xl + 4:xl + 4 + 70].replace(CONT, "")
                check("C-TR 预填 4 行在 5 可视行内全可见（末行 wss://…）",
                      inner_tr4.startswith("wss://webtorrent.ezr.dev"),
                      f"末行 = {inner_tr4.strip()[:44]!r}")
            else:
                check("C-TR trackers 几何定位", False)
        else:
            check("C1 修改 BT 对话框几何定位", False)
    finally:
        s.close()

    print(f"\nphase2 UI 自验证：{'全部通过' if BAD == 0 else f'{BAD} 项失败'}")
    return 1 if BAD else 0


if __name__ == "__main__":
    sys.exit(main())
