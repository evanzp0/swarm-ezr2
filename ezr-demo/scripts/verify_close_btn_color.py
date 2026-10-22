#!/usr/bin/env python3
"""临时字节级验证：弹窗右上角 [✗] 配色拆分（本轮需求修订-13）。

规格：
  1. 常态：中括号 `[`/`]` 与弹窗边框线同色（green + bold），✗ = DIM2 (58666c) 无 bold；
  2. 悬停（motion）：仅 ✗ = RED (e25c54) + bold，中括号不变；
  3. 按压（down）：仅 ✗ = RED + bold，中括号不变；
  4. 原地松开 = 关闭弹窗（回归）。
"""
import fcntl, os, pty, select, struct, sys, termios, time
import pyte

COLS, LINES = 110, 34
PROG = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"

pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    os.execvp(PROG, [PROG])
    os._exit(1)
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", LINES, COLS, 0, 0))
screen = pyte.Screen(COLS, LINES)
stream = pyte.ByteStream(screen)


def pump(seconds: float) -> None:
    deadline = time.time() + seconds
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.05)
        if fd in r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            stream.feed(data)


def find_about_row():
    for y in range(LINES):
        if " About " in screen.display[y]:
            return y
    return None


def cell(x, y):
    return screen.buffer[y][x]


GREEN = "00cd00"  # pyte 调色盘中 ANSI green 的十六进制（Color::Green → SGR 38;5;2）
DIM2 = "58666c"  # Rgb(88, 102, 108)
RED = "e25c54"  # Rgb(226, 92, 84)

ok = True
pump(1.5)
os.write(fd, b"o")  # 打开 About 弹窗
pump(1.0)

ty = find_about_row()
assert ty is not None, "About 弹窗未找到"
row = screen.display[ty]
bxs = [x for x, ch in enumerate(row) if ch in "╭╮╰╯"]
left, right = min(bxs), max(bxs)
bx = right - 4  # 0-based：popup.x + width - 5
seg = row[bx:bx + 3]
assert seg == "[✗]", f"关闭按钮字符异常: {seg!r}"
# 左右 padding 列保持边框线（─）原样
assert row[bx - 1] == "─" and row[bx + 3] == "─", \
    f"padding 列应保持边框线: {row[bx-1]!r} {row[bx+3]!r}"


def check_brackets(tag, border_cell, b0, x0, b1):
    """中括号与边框线同色同粗（不随悬停/按压变化）。"""
    good = (
        b0.fg == border_cell.fg and b0.bold == border_cell.bold
        and b1.fg == border_cell.fg and b1.bold == border_cell.bold
    )
    if not good:
        print(
            f"FAIL: {tag} 中括号与边框线不同色: "
            f"边框={border_cell.fg}/bold={border_cell.bold} "
            f"[={b0.fg}/bold={b0.bold} ]={b1.fg}/bold={b1.bold}"
        )
    return good


# ---- 1) 常态 ----
border0 = cell(bx - 1, ty)  # 左 padding 列的边框 ─
b0, x0, b1 = cell(bx, ty), cell(bx + 1, ty), cell(bx + 2, ty)
print(
    f"常态  : 边框={border0.fg}/bold={border0.bold} | "
    f"[ = {b0.fg}/bold={b0.bold} | ✗ = {x0.fg}/bold={x0.bold} | ] = {b1.fg}/bold={b1.bold}"
)
ok = check_brackets("常态", border0, b0, x0, b1) and ok
if b0.fg != GREEN or b1.fg != GREEN:
    ok = False
    print(f"FAIL: 常态中括号非绿: {b0.fg!r} {b1.fg!r}")
if x0.fg != DIM2 or x0.bold:
    ok = False
    print(f"FAIL: 常态 ✗ 非纯 DIM2/无 bold: {x0.fg!r}/bold={x0.bold}")

# ---- 2) 悬停（SGR motion 35） ----
cx, cy = bx + 2, ty + 1  # 1-based SGR 坐标（✗ 列）
os.write(fd, f"\x1b[<35;{cx};{cy}M".encode())
pump(0.4)
border0 = cell(bx - 1, ty)
b0, x0, b1 = cell(bx, ty), cell(bx + 1, ty), cell(bx + 2, ty)
print(f"悬停  : [ = {b0.fg}/bold={b0.bold} | ✗ = {x0.fg}/bold={x0.bold} | ] = {b1.fg}/bold={b1.bold}")
ok = check_brackets("悬停", border0, b0, x0, b1) and ok
if x0.fg != RED or not x0.bold:
    ok = False
    print(f"FAIL: 悬停 ✗ 非红+粗: {x0.fg!r}/bold={x0.bold}")

# ---- 3) 按压（down） ----
os.write(fd, f"\x1b[<0;{cx};{cy}M".encode())
pump(0.4)
border0 = cell(bx - 1, ty)
b0, x0, b1 = cell(bx, ty), cell(bx + 1, ty), cell(bx + 2, ty)
print(f"按压  : [ = {b0.fg}/bold={b0.bold} | ✗ = {x0.fg}/bold={x0.bold} | ] = {b1.fg}/bold={b1.bold}")
ok = check_brackets("按压", border0, b0, x0, b1) and ok
if x0.fg != RED or not x0.bold:
    ok = False
    print(f"FAIL: 按压 ✗ 非红+粗（无背景/无反色检查：bg 应为 default）: {x0.fg!r}/bold={x0.bold}")
if x0.bg not in (None, "", "default"):
    ok = False
    print(f"FAIL: 按压 ✗ 出现背景色: {x0.bg!r}")

# ---- 4) 原地松开 = 关闭 ----
os.write(fd, f"\x1b[<0;{cx};{cy}m".encode())
pump(0.6)
if find_about_row() is not None:
    ok = False
    print("FAIL: 原地松开后弹窗未关闭")

os.kill(pid, 9)
try:
    os.waitpid(pid, 0)
except ChildProcessError:
    pass
print("RESULT:", "ALL PASS" if ok else "HAS FAILURES")
sys.exit(0 if ok else 1)
