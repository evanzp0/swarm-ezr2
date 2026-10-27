#!/usr/bin/env python3
"""最小重现：A-UD 序列后跟 A6 点击序列，逐步 dump 文本框状态（复用 verify 脚本基建）。"""
import os, pty, re, select, sys, time, fcntl, struct, termios, types

program = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"
ROWS, COLS = 44, 120

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

    def raw_rows(self):
        return ["".join(self.term.grid[y]) for y in range(ROWS)]

    def close(self):
        try:
            os.kill(self.pid, 9)
            os.waitpid(self.pid, 0)
        except ChildProcessError:
            pass


s = Session(program)
s.rd(1.5)
s.send(b"a", 0.5)
print("after a        :", repr(s.raw_rows()[17][20:55]))
s.send(b"abcdef", 0.35)
s.send(b"\x1b[13;2u", 0.3)
s.send(b"xy", 0.3)
print("after xy       :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"\x1b[A", 0.3)
s.send(b"Z", 0.35)
print("after up+Z     :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"\x1b[B", 0.3)
s.send(b"W", 0.35)
print("after down+W   :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"\x7f" * 20, 0.5)
print("after clear20  :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"0123456789" * 8, 0.9)
print("after 80chars  :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"\x1b[A", 0.3)
s.send(b"Q", 0.4)
print("after up+Q     :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"\x7f" * 100, 0.8)
print("after clear100 :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
for _ in range(3):
    s.send(b"\x7f" * 10, 0.15)
print("after clear30  :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.send(b"hello world", 0.6)
print("after hello    :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
col = 25 + 6 + 1
s.send(f"\x1b[<0;{col};18M".encode(), 0.25)
s.send(f"\x1b[<0;{col};18m".encode(), 0.3)
print("after click    :", repr(s.raw_rows()[17][20:55]))
s.send(b"X", 0.5)
print("after X        :", repr(s.raw_rows()[17][20:55]), repr(s.raw_rows()[18][20:55]))
s.close()
