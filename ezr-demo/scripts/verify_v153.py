#!/usr/bin/env python3
"""v1.53 轮 PTY 验证（specifier 会话直接落地轮）：

A. 任务行 1 文件名 32 显示列尾部折叠（FR-01-118/D75）
B. BT 下载臂统计行：x1(x2)/y1(y2) + SR + LT + HTTP 下载行同改 LT
   （FR-02-37 v1.2 / FR-02-38 v1.21；v1.54 裁决 LT 扩面 FR-02-38 v1.22/D77②）
C. 做种臂：x1(x2)/y1(y2) + SR +「剩余做种」项退役（FR-02-38 v1.21）；
   v1.56 真实做种形态 0(45)/35(120)（D80④ 修订，操作者钦定）
D. BT 对话框「按顺序下载」复选项：添加 78×29 / 修改 78×27 / 默认 ☐ /
   Space·Enter 切换 / 确认落库 / 重开预填（FR-02-39/D76）
E. 帮助窗口【任务列表】组 SR/LT 条目（FR-02-38 v1.21/D77）

用法：python3 scripts/verify_v153.py [program]（默认 ./target/debug/ezr-tui-demo）
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
LONG_SEG = "c" * 40 + "-release.bin"
FOLDED = "c" * 29 + "..."

BAD = 0


def check(name, cond, detail=""):
    global BAD
    BAD += not cond
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" —— {detail}" if detail and not cond else ""))


src = open('scripts/check_popup_integrity.py').read().replace(
    'if __name__ == "__main__":\n    main()', '')
mod = types.ModuleType('cpi')
exec(compile(src, 'cpi', 'exec'), mod.__dict__)
mod.ROWS = ROWS
CONT = mod.CONT


class Session:
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


def scroll_until(s, pred, key, max_steps=30, wait=0.18):
    """按键滚动直到谓词成立（谓词作用于整屏文本）；用尽步数返回最后状态。"""
    for _ in range(max_steps):
        if pred(s.screen()):
            return True
        s.send(key, wait)
    return pred(s.screen())


def dialog_height(raw_rows, title):
    pos = locate_dialog(raw_rows, title)
    if pos is None:
        return None
    xl, yt, _ = pos
    for y in range(yt + 1, ROWS):
        if raw_rows[y][xl] == "╰":
            return y - yt + 1
    return None


def main():
    program = sys.argv[1] if len(sys.argv) > 1 else "./target/debug/ezr-tui-demo"

    s = Session(program)
    try:
        s.rd(1.5)

        print("== A. 任务行 1 文件名折叠（FR-01-118/D75） ==")
        scr = s.screen()
        check("A1 恰 32 列文件名（ubuntu…iso）原样不折叠", "ubuntu-24.04.2-desktop-amd64.iso" in scr)
        # 长名任务：A → 长段 URL → Enter → HTTP 添加框 → Enter 确认
        s.send(b"a", 0.6)
        url = ("https://dl.example.com/pool/" + LONG_SEG).encode()
        s.send(url, 0.5)
        s.send(b"\r", 0.6)
        scr = s.screen()
        check("A2 长段 URL 打开添加对话框", "添加任务：HTTP(S)" in scr)
        s.send(b"\r", 0.6)
        s.rd(2.8)
        scr = s.screen()
        check("A3 行 1 尾部折叠 = 前 29 列 + ...（三个 ASCII 句点）", FOLDED in scr, "未见 " + FOLDED)
        row = next((r for r in s.rows() if FOLDED in r), "")
        check("A4 该行 1 不含完整长名（尾部已折叠）", "-release.bin" not in row, row[:80])
        scroll_until(s, lambda t: "ubuntu-24.04.2-desktop-amd64.iso" in t, b"\x1b[A", 32)
        check("A5 任务列表其余行不受影响（ubuntu 名完整）",
              "ubuntu-24.04.2-desktop-amd64.iso" in s.screen())

        print("== C. 做种臂（FR-02-38 v1.21；v1.56 真实做种形态 D80④ 修订） ==")
        s.send(b"]", 0.5)
        scroll_until(s, lambda t: "0(45)/35(120)" in t, b"\x1b[B", 32)
        scr = s.screen()
        check("C1 做种行 x1(x2)/y1(y2)/健康度 = 0(45)/35(120)/2.418（做种态 SEEDS=0 而 PEERS 有数）",
              "0(45)/35(120)/2.418" in scr, "未见做种行真实做种形态")
        seed_row = next((r for r in s.rows() if "0(45)/35(120)" in r), "")
        check("C2 做种行 SR 标签在位", " SR " in seed_row, seed_row[:80])
        check("C3 做种行「剩余做种」项整项退役（全屏无「剩余做种」）",
              "剩余做种" not in scr, "仍见「剩余做种」字样")

        print("== B. BT 下载臂统计行（FR-02-37 v1.3/FR-02-38 v1.21） ==")
        s.send(b"[", 0.5)
        scroll_until(s, lambda t: "8(14)/15(31)" in t, b"\x1b[B", 32)
        scr = s.screen()
        check("B1 seeders/peers = x1(x2)/y1(y2)（8(14)/15(31)，y1 = 15 非种子 peers 直读 FR-02-37 v1.3/D80④）", "8(14)/15(31)" in scr, "未见 8(14)/15(31)")
        check("B2 健康度三段完整 = 8(14)/15(31)/1.675", "8(14)/15(31)/1.675" in scr)
        bt_row = next((r for r in s.rows() if "8(14)/15(31)" in r), "")
        check("B3 BT 下载臂剩余时间标签 = LT", " LT " in bt_row, bt_row[:80])
        check("B4 BT 行不含旧「分享」标签（改 SR）", " 分享 " not in bt_row, bt_row[:80])
        check("B5 SR 标签在位（BT 下载臂）", " SR " in bt_row, bt_row[:80])
        http_row = next((r for r in s.rows()
                         if "↓" in r and " LT " in r and "↑" not in r and " SR " not in r), "")
        check("B6 HTTP 下载行「剩余」同改 LT（FR-02-38 v1.22/D77②，v1.54 裁决扩面）",
              bool(http_row) and " LT " in http_row and "剩余" not in http_row, http_row[:80])
        check("B7 全屏无「剩余」标签（HTTP/BT 两臂统一 LT）",
              all("剩余" not in r for r in s.rows()), "仍有行含「剩余」")

        print("== D. BT 对话框「按顺序下载」复选项（FR-02-39/D76） ==")
        s.send(b"a", 0.6)
        s.send(b"/tmp/v153-fixture.torrent", 0.5)
        s.send(b"\r", 0.7)
        rows = s.raw_rows()
        scr = s.screen()
        check("D1 添加任务：BT 对话框打开", "添加任务：BT" in scr)
        h = dialog_height(rows, "添加任务：BT")
        check("D2 对话框尺寸 78×29（v1.51：28→29）", h == 29, f"实测高 = {h}")
        check("D3 复选项标签「按顺序下载」在位", "按顺序下载" in scr)
        check("D4 默认未勾选（☐）", "☐" in scr and "☑" not in scr)
        for _ in range(7):
            s.send(b"\t", 0.15)
        scr = s.screen()
        s.send(b" ", 0.4)
        scr = s.screen()
        check("D5 Space 切换 → ☑（不确认不关窗）", "☑" in scr and "添加任务：BT" in scr)
        s.send(b"\t", 0.15)
        s.send(b"\t", 0.15)
        s.send(b"\r", 0.7)
        s.rd(2.6)
        scr = s.screen()
        check("D6 确认创建任务（对话框关闭）", "添加任务：BT" not in scr)
        s.send(b"m", 0.7)
        rows = s.raw_rows()
        scr = s.screen()
        check("D7 修改对话框打开且预填 ☑（随任务持久化）",
              "修改任务：BT" in scr and "☑" in scr, scr[:0] + "无 ☑")
        h = dialog_height(rows, "修改任务：BT")
        check("D8 修改对话框尺寸 78×27（v1.51：26→27）", h == 27, f"实测高 = {h}")
        for _ in range(5):
            s.send(b"\t", 0.15)
        s.send(b" ", 0.4)
        scr = s.screen()
        check("D9 修改框内 Space 切换 → ☐", "☐" in scr and "☑" not in scr)
        s.send(b"\t", 0.15)
        s.send(b"\t", 0.15)
        s.send(b"\r", 0.7)
        s.rd(1.5)
        s.send(b"m", 0.7)
        scr = s.screen()
        check("D10 确认后重开预填 ☐（翻转落库）",
              "修改任务：BT" in scr and "☐" in scr and "☑" not in scr)
        s.send(b"\x1b", 0.4)

        print("== E. 帮助窗口 SR/LT 条目（FR-02-38 v1.21/D77） ==")
        s.send(b"h", 0.6)
        scr = s.screen()
        check("E1 帮助打开", "帮助" in scr)
        scroll_until(s, lambda t: "Share Ratio" in t, b"\x1b[B", 30, 0.12)
        s.send(b"\x1b[B", 0.15)
        s.send(b"\x1b[B", 0.15)
        s.send(b"\x1b[B", 0.15)
        scr = s.screen()
        check("E2 SR 条目 = 分享率（Share Ratio）说明", "分享率（Share Ratio）" in scr)
        check("E3 LT 条目 = 剩余时间（Left Time）说明", "剩余时间（Left Time）" in scr)
        s.send(b"\x1b", 0.4)

        print()
        if BAD == 0:
            print("== v1.53 PTY 验证：全部通过 ==")
        else:
            print(f"== v1.53 PTY 验证：{BAD} 项失败 ==")
        sys.exit(1 if BAD else 0)
    finally:
        s.close()


if __name__ == "__main__":
    main()
