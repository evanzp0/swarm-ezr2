#!/usr/bin/env python3
"""capture.py — 在伪终端中运行 EZR TUI Demo，并把画面解码为纯文本截图（开发调试用）。

用法:
    python3 scripts/capture.py [程序路径] [列数] [行数] [捕获秒数]

示例:
    python3 scripts/capture.py ./target/release/ezr-tui-demo 110 34 4
    python3 scripts/capture.py ./target/release/ezr-tui-demo 110 34 6 "jjj,space,tab"
"""

import fcntl
import os
import pty
import select
import struct
import sys
import termios
import time

import pyte


def main() -> None:
    args = sys.argv[1:]
    program = args[0] if args else "./target/release/ezr-tui-demo"
    cols = int(args[1]) if len(args) > 1 else 110
    lines = int(args[2]) if len(args) > 2 else 34
    duration = float(args[3]) if len(args) > 3 else 4.0

    pid, fd = pty.fork()
    if pid == 0:  # 子进程：在 pty 中运行 TUI
        os.environ["TERM"] = "xterm-256color"
        os.execvp(program, [program])
        os._exit(1)

    # 设置伪终端窗口大小
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", lines, cols, 0, 0))

    screen = pyte.Screen(cols, lines)
    stream = pyte.ByteStream(screen)
    deadline = time.time() + duration

    # 可选按键注入：在捕获中段逐个发送（如 "jjj a space tab"）
    keys_arg = args[4] if len(args) > 4 else ""
    keys = [k for k in keys_arg.split(",") if k]
    key_start = time.time() + duration * 0.45

    def key_bytes(k: str) -> bytes:
        named = {
            "up": b"\x1b[A", "down": b"\x1b[B", "right": b"\x1b[C", "left": b"\x1b[D",
            "enter": b"\r", "esc": b"\x1b", "tab": b"\t", "space": b" ",
            "pgup": b"\x1b[5~", "pgdn": b"\x1b[6~", "home": b"\x1b[H", "end": b"\x1b[F",
            "bs": b"\x7f",
        }
        if k in named:
            return named[k]
        # SGR 鼠标事件注入: mclick:列:行 / mup:列:行 / mwu:列:行 / mwd:列:行
        if k.startswith("mclick:"):
            _, x, y = k.split(":")
            return f"\x1b[<0;{x};{y}M".encode()
        if k.startswith("mup:"):
            _, x, y = k.split(":")
            return f"\x1b[<0;{x};{y}m".encode()
        if k.startswith("mwu:"):
            _, x, y = k.split(":")
            return f"\x1b[<64;{x};{y}M".encode()
        if k.startswith("mwd:"):
            _, x, y = k.split(":")
            return f"\x1b[<65;{x};{y}M".encode()
        return k.encode()

    sent = 0
    while time.time() < deadline:
        # 到达发送时刻则逐键发送
        if sent < len(keys) and time.time() >= key_start + sent * 0.18:
            os.write(fd, key_bytes(keys[sent]))
            sent += 1
        ready, _, _ = select.select([fd], [], [], 0.05)
        if fd in ready:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            stream.feed(data)

    try:
        os.kill(pid, 9)
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass

    for row in screen.display:
        print(row.rstrip())


if __name__ == "__main__":
    main()
