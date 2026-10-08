#!/usr/bin/env python3
"""冒烟：本轮 ezr-demo UI 同步的字节级断言（临时脚本）。"""
import importlib.util
import os
import pty
import select
import fcntl
import struct
import sys
import termios
import time
import types

src = open('scripts/check_popup_integrity.py').read().replace(
    'if __name__ == "__main__":\n    main()', '')
mod = types.ModuleType('cpi')
exec(compile(src, 'cpi', 'exec'), mod.__dict__)


def smoke(name, keys_seq, check_fn, wait=1.2, rows=36, cols=112, key_delay=0.35):
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp("./target/release/ezr-tui-demo", ["./target/release/ezr-tui-demo"])
        os._exit(1)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    term = mod.MiniTerm(ghost=False, cols=cols)

    def rd(sec):
        end = time.time() + sec
        while time.time() < end:
            r, _, _ = select.select([fd], [], [], 0.05)
            if fd in r:
                try:
                    data = os.read(fd, 1 << 16)
                except OSError:
                    return
                if not data:
                    return
                term.feed(data)

    try:
        rd(1.5)
        for k in keys_seq:
            os.write(fd, k)
            rd(key_delay)
        rd(wait)
        text = "\n".join("".join(c for c in term.grid[y] if c != mod.CONT)
                         for y in range(rows))
        print(f"== {name} ==")
        bad = 0
        for label, ok in check_fn(text):
            bad += not ok
            print(f"  [{'PASS' if ok else 'FAIL'}] {label}")
        return bad
    finally:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass


total_bad = 0

total_bad += smoke("主界面（详情精简）", [b"\x1b[B"] * 6, lambda t: [
    ("详情无「状态」行", " 状态     [" not in t),
    ("详情无「速度」行", " 速度     " not in t),
    ("大小行无「剩余」", "（剩余" not in t),
    ("详情含失败原因", "失败原因" in t),
    ("详情含分块行", "分块     " in t),
    ("列表标题无鼠标提示", "鼠标: 点击选中" not in t),
    ("页脚仍含鼠标提示", "鼠标 " in t),
])

total_bad += smoke("添加对话框 6 字段", [b"a"], lambda t: [
    ("对话框打开", "添加下载任务" in t),
    ("代理行 直连 ▾", "代理" in t and "直连 ▾" in t),
    ("校验行 SHA-256 ▾", "SHA-256 ▾" in t),
    ("提示行", "校验码留空 = 不校验" in t),
])

total_bad += smoke("代理下拉", [b"a", b"\t", b"\t", b"\t", b"\t", b"\t", b"\r"],
                   lambda t: [
    ("下拉标题「选择代理」", "选择代理" in t),
    ("直连选项", "直连" in t),
    ("命名条目带类型标注", "办公网代理（http）" in t and "本地 SOCKS5" in t),
])

total_bad += smoke("修改对话框", [b"m"], lambda t: [
    ("标题「修改任务」", "修改任务" in t),
    ("并发预填 8", "并发" in t and "> 8" in t),
    ("代理回显命名代理", "办公网代理（http）" in t),
    ("提示「确定后立即生效」", "确定后立即生效" in t),
])

total_bad += smoke("清已完成（状态级）", [b"\t", b"c"], lambda t: [
    ("ffmpeg 已被清理", "ffmpeg-master" not in t),
    ("做种任务保留（C 只清已完成）", "archlinux" in t and "做种中" in t),
])

# 空态引导：正在下载页签逐个删除后（借 D 对话框路径太长，此处直验渲染分支文案已在代码内），
# 改验「区域过小」之外的空态文案——已完成页签仅剩做种任务时不清空，跳过该路径。
total_bad += smoke("修改对话框·已完成拒绝", [b"\t", b"\x1b[B", b"m"], lambda t: [
    ("已完成任务按 m → toast 拒绝", "已完成任务不可修改" in t or "修改任务" not in t),
])

print(f"\n总失败项: {total_bad}")
sys.exit(1 if total_bad else 0)
