#!/usr/bin/env python3
"""check_overlay_redraw.py — 验证浮层（对话框/下拉）翻转时的全量重绘修复。

背景 bug：浮层边框恰好落在底层 CJK 宽字符的右半格上时，增量 diff 只输出
单个单元格的补丁，部分终端无法正确处理"覆盖宽字符半格"，宽字残留在浮层
边框之上（"等待空闲下…"盖住对话框左侧框线）且被后续帧固化。

修复：浮层翻转（打开/关闭对话框、展开/收起下拉）那一帧先 terminal.clear()
清屏，随后全量重绘。本脚本通过检查输出字节流断言该行为：

  1. 打开添加对话框（A）      -> 窗口内出现 \\x1b[2J 且 MoveTo 数量显著
  2. 关闭对话框（Esc）        -> 同上
  3. 再次打开（A）+ 展开下拉（Tab*3 + Enter）-> 同上
  4. 收起下拉（Esc）          -> 同上
  5. 浮层稳定期间（无翻转）   -> 不应出现多余的 \\x1b[2J（避免无谓闪屏）

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

MOVE_TO = re.compile(rb"\x1b\[\d+;\d+H")
CLEAR_ALL = b"\x1b[2J"


def read_window(fd: int, seconds: float) -> bytes:
    """读取 seconds 秒内的全部输出字节。"""
    buf = b""
    deadline = time.time() + seconds
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if fd in ready:
            try:
                data = os.read(fd, 1 << 16)
            except OSError:
                break
            if not data:
                break
            buf += data
    return buf


def check(name: str, window: bytes, expect_clear: bool, results: list) -> None:
    has_clear = CLEAR_ALL in window
    moves = len(MOVE_TO.findall(window))
    if expect_clear:
        ok = has_clear and moves >= 50
        detail = f"clear={'Y' if has_clear else 'N'} move_to={moves}"
    else:
        ok = not has_clear
        detail = f"clear={'Y' if has_clear else 'N'} move_to={moves}"
    results.append((name, ok, detail))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name:<28} ({detail})")


def main() -> None:
    program = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"
    cols, lines = 112, 36

    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp(program, [program])
        os._exit(1)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", lines, cols, 0, 0))

    results: list = []
    try:
        # 启动稳定期（首帧绘制，不视为翻转）
        warm = read_window(fd, 1.5)
        print(f"启动窗口: clear={'Y' if CLEAR_ALL in warm else 'N'} "
              f"move_to={len(MOVE_TO.findall(warm))}")

        def step(name: str, key: bytes, expect_clear: bool, wait: float = 0.7) -> None:
            os.write(fd, key)
            window = read_window(fd, wait)
            check(name, window, expect_clear, results)

        print("场景 1：添加对话框 打开/关闭")
        step("A  打开添加对话框", b"a", True)
        step("Esc 关闭添加对话框", b"\x1b", True)

        print("场景 2：校验算法下拉 展开/收起")
        step("A  打开添加对话框", b"a", True)
        for _ in range(3):  # URL -> 目录 -> 并发 -> 校验
            os.write(fd, b"\t")
            time.sleep(0.15)
        read_window(fd, 0.2)  # 丢弃 Tab 期间输出（无翻转）
        step("Enter 展开下拉浮层", b"\r", True)
        step("Esc 收起下拉浮层", b"\x1b", True)
        step("Esc 关闭添加对话框", b"\x1b", True)

        print("场景 3：删除确认对话框 打开/关闭")
        step("D  打开删除对话框", b"d", True)
        step("Esc 关闭删除对话框", b"\x1b", True)

        print("场景 4：浮层稳定期不应反复清屏")
        step("A  打开添加对话框", b"a", True)
        stable = read_window(fd, 1.0)
        check("稳定期 1s 无清屏", stable, False, results)
        os.write(fd, b"\x1b")
        read_window(fd, 0.5)
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
