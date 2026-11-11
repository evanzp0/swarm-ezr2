#!/usr/bin/env python3
"""verify_size_move.py — PTY 实证：全类型任务行 <总字节> 上移 + 明细行尺寸对删除
+ 做种已上传对退役 + 页签切换保留/还原列表状态（FR-01-117）。

调整轮口径:
  1) 任务行（l1）: [协议] [状态] 后以间隔显示 <总字节>；尺寸未知则不显示（pct 列 ?%）
  2) 明细行（l3）: 不再出现 <已下载字节>/<总字节>（含 x/? B 形态）
  3) 做种明细行不再含 已上传 <x>/<总大小>（v1.50 退役；上传量仅经分享率体现）
  4) 页签切换保留/还原列表状态（v1.51/FR-01-117）: ] / [ 切走时快照
     「选中任务身份 + 选中索引 + 滚动位置」，切回时还原——切回后列表计数行
     （k/y 项）、光标 ▌ 行号、首个可见任务行均与切走前一致（场景 E 帧对断言）

键序设计（对齐 demo 模拟时序；~2.2s neovim 转已完成移入页签 1 后，两页签
集合成员在 17s 窗口内稳定：win11 ~8s 重试/~14s 再失败均留在页签 0，
校验中 gpt4all 失败留页签 0，槽位满队列任务不动）:
  - 0.4s 起以 0.12s 间隔连按 10 次 ↓（选中走到页签尾，捕获后期处理行）
  - neovim 后期处理 3400ms（启动时已进行 1.2s → ~2.2s 完成）
  - 6.6s Home + 2×↓ → 页签 0 状态 A：选中第 3 项（计数 3/10 项、滚动置顶）
  - 7.6s ] 切页签 1（快照 A 存档）→ 8.0s ↓（页签 1 选中下移制造差异）
  - 8.4s [ 切回页签 0 → 断言还原 A（计数/▌ 行号/首可见任务行 = 切走前）
  - 9.3s ] → 页签 1 浏览；13.0s End → 页签 1 选中尾项（计数 4/4 项，
    证明两页签状态独立）
  - 13.5s [ 切回页签 0 → 复验还原 A
  - 14.6s ↓（状态 B：4/10 项）→ 15.0s ]（快照 B 存档）→ 15.4s Home（页签 1 回顶）
  - 15.8s [ 切回页签 0 → 断言还原 B

说明:
  - pyte 宽字符占位格 data=''：重建时跳过 → 逻辑行（查子串）；补空格 → 显示行（快照）
  - 校验基于多帧逻辑行并集；任务详情侧栏保留自身尺寸展示（x / y 带空格形），
    明细行尺寸对的缺席校验用无空格 x/y 特征串区分
  - 场景 E 用逐帧显示行快照做帧对比较（计数行 + ▌ 行号 + 首可见任务行）

用法: python3 scripts/verify_size_move.py [程序路径]
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

import pyte

PROGRAM = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"
COLS, ROWS = 110, 40
DURATION = 17.0

FAILS: list[str] = []


def check(cond: bool, msg: str) -> None:
    tag = "PASS" if cond else "FAIL"
    print(f"  [{tag}] {msg}")
    if not cond:
        FAILS.append(msg)


def rebuild(screen: pyte.Screen) -> tuple[list[str], list[str]]:
    """返回 (逻辑行, 显示行)：占位格 data='' 跳过 → 逻辑行；补空格 → 显示行。"""
    logic: list[str] = []
    disp: list[str] = []
    for y in range(screen.lines):
        lo: list[str] = []
        di: list[str] = []
        for x in range(screen.columns):
            data = screen.buffer[y][x].data
            if data == "":
                di.append(" ")  # 宽字符右半占位
            else:
                lo.append(data)
                di.append(data)
        logic.append("".join(lo).rstrip())
        disp.append("".join(di).rstrip())
    return logic, disp


def latest_in(frames: list[tuple[float, list[str]]], lo: float, hi: float):
    """返回 (lo, hi) 时间窗内最后一帧的显示行，无帧返回 None。"""
    cand = [f for t, f in frames if lo <= t < hi]
    return cand[-1] if cand else None


def list_metrics(disp: list[str]):
    """从显示行提取列表状态：返回 (▌ 光标行号, 首可见任务行协议+状态+尺寸 token)
    或 None（未找到列表标题行）。列表标题行 = 首个含计数行「k/y 项」的行
    （pyte 显示行 CJK 带字间空格，标题文本按「下 载 任 务」形态无法稳定匹配）。"""
    title_y = next(
        (y for y, ln in enumerate(disp) if re.search(r"\d+/\d+ 项", ln)),
        None,
    )
    if title_y is None:
        return None
    first = disp[title_y + 1] if title_y + 1 < len(disp) else ""
    m = re.search(r"\[(?:HTTP|HTTPS|BT)\] \[[^\]]+\] \d[\d.]* (?:GB|MB|KiB|B)", first)
    token = m.group(0) if m else first.strip()[:24]
    cur_y = next((y for y, ln in enumerate(disp) if "▌" in ln), -1)
    return cur_y, token


def counter_of(disp: list[str]) -> str | None:
    """列表计数行文本（k/y 项），未找到返回 None。"""
    for ln in disp:
        m = re.search(r"\d+/\d+ 项", ln)
        if m:
            return m.group(0)
    return None


def main() -> None:
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp(PROGRAM, [PROGRAM])
        os._exit(1)

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    screen = pyte.Screen(COLS, ROWS)
    stream = pyte.ByteStream(screen)

    # (时刻s, 键): 10×down 快速走到页签尾（捕获后期处理）→ 停留等 neovim 完成 →
    # Home+2×down 构造状态 A → ]/[ 往返实证页签快照保留/还原（场景 E）
    schedule: list[tuple[float, bytes]] = []
    for i in range(10):
        schedule.append((0.4 + i * 0.12, b"\x1b[B"))
    schedule.append((2.0, b"\x1b[B"))  # → sintel；neovim 完成后触发夹紧
    # 场景 E：页签 0 状态 A（Home 回顶 + 2×down → 选中第 3 项、滚动置顶）
    schedule.append((6.6, b"\x1b[H"))  # Home → 选中回顶
    schedule.append((6.95, b"\x1b[B"))
    schedule.append((7.2, b"\x1b[B"))  # → 选中第 3 项（计数 3/10 项）
    schedule.append((7.6, b"]"))  # 切页签 1（快照 A 存档）
    schedule.append((8.0, b"\x1b[B"))  # 页签 1 选中下移（制造差异）
    schedule.append((8.4, b"["))  # 切回页签 0 → 还原 A
    schedule.append((9.3, b"]"))  # 再切页签 1（快照 A 再次存档）
    for t, key in [(9.7, b"\x1b[B"), (10.1, b"\x1b[B"), (10.5, b"\x1b[A"),
                   (11.0, b"\x1b[A"), (11.6, b"\x1b[B"), (12.2, b"\x1b[B")]:
        schedule.append((t, key))
    schedule.append((13.0, b"\x1b[F"))  # End → 页签 1 选中尾项（计数 4/4 项）
    schedule.append((13.5, b"["))  # 切回页签 0 → 复验还原 A
    schedule.append((14.6, b"\x1b[B"))  # → 选中第 4 项（状态 B：4/10 项）
    schedule.append((15.0, b"]"))  # 切页签 1（快照 B 存档）
    schedule.append((15.4, b"\x1b[H"))  # 页签 1 选中回顶（制造差异）
    schedule.append((15.8, b"["))  # 切回页签 0 → 还原 B

    start = time.time()
    sent = 0
    seen_logic: set[str] = set()
    last_disp: list[str] = []
    frames: list[tuple[float, list[str]]] = []
    while time.time() - start < DURATION:
        now = time.time() - start
        if sent < len(schedule) and now >= schedule[sent][0]:
            os.write(fd, schedule[sent][1])
            sent += 1
        ready, _, _ = select.select([fd], [], [], 0.04)
        if fd in ready:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            stream.feed(data)
            logic, disp = rebuild(screen)
            seen_logic.update(logic)
            last_disp = disp
            frames.append((now, disp))

    try:
        os.kill(pid, 9)
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass

    text = "\n".join(seen_logic)

    print("== 场景 A：任务行 [状态] 后携带 <总字节>（不变量：每个已知总尺寸都出现在某状态徽标后） ==")
    states = r"(?:等待中|下载中|已暂停|校验中|后期处理中|已完成|已失败|做种中)"
    known_sizes = ["6.21 GB", "4.32 GB", "1.24 GB", "328.0 MB", "4.92 GB", "2.26 GB",
                   "96.4 MB", "61.8 MB", "5.71 GB", "712.0 MB", "3.34 GB",
                   "23.4 MB", "4.19 GB"]
    for size in known_sizes:
        pat = rf"\[(?:HTTP|HTTPS|BT)\] \[{states}\] {re.escape(size)}"
        check(re.search(pat, text) is not None,
              f"总尺寸 {size} 出现在 [协议] [状态] 之后（有间隔）")
    check(re.search(rf"\[(?:HTTP|HTTPS|BT)\] \[{states}\] 812\.0 MB", text) is None,
          "尺寸未知的 HTTP 任务（812.0 MB）不伪造总字节")

    print("== 场景 B：关键状态行实证 ==")
    check("[HTTPS] [后期处理中] 23.4 MB" in text, "后期处理中任务行携带总字节（快速键序窗口内捕获）")
    check("[HTTPS] [已失败] 5.71 GB" in text, "已失败任务行携带总字节（win11 flaky 失败窗口）")
    check("[BT] [做种中] 4.32 GB" in text, "做种中任务行携带总字节（切页签后可见）")
    check("[HTTPS] [已完成] 96.4 MB" in text, "已完成任务行携带总字节")
    check("[HTTP] [已完成] 61.8 MB" in text, "已完成任务行携带总字节（HTTP）")
    check("[BT] [下载中] 4.19 GB" in text, "BT 下载中任务行携带总字节")
    check("[HTTP] [已暂停] 328.0 MB" in text, "已暂停任务行携带总字节")

    print("== 场景 C：明细行不再出现 <已下载字节>/<总字节>（无空格 x/y 特征） ==")
    for pair, label in [
        ("GB/6.21 GB", "ubuntu 旧尺寸对"),
        ("MB/? B", "media 旧 x/? B 对"),
        ("GB/5.71 GB", "win11 旧尺寸对"),
        ("MB/1.24 GB", "rust 旧尺寸对"),
        ("MB/328.0 MB", "blender 旧尺寸对"),
        ("MB/4.92 GB", "llama 旧尺寸对"),
        ("GB/4.19 GB", "sintel 旧尺寸对"),
    ]:
        check(pair not in text, f"明细行无 {label}（{pair}）")
    check(" · 96.4 MB" not in text and " · 61.8 MB" not in text,
          "已完成明细行不再缀总字节（· 96.4 MB / · 61.8 MB）")
    check("已上传" not in text, "做种明细行无 已上传 <x>/<总>（v1.50 退役）")
    check(re.search(r"\d+\.\d+/\d+\.\d+ (?:GB|MB)", text) is None,
          "无 fmt_size_pair 无空格 x/y 单位 尺寸对（已上传对特征串，全屏任意帧）")

    print("== 场景 D：保留项完好 ==")
    check("重试" in text, "已失败明细行保留重试信息")
    check("排队" in text or "槽位" in text, "等待中明细行保留排队/槽位提示")
    check("校验成功" in text or "无校验" in text, "已完成明细行保留校验结论")
    check("12/35/2.418" in text, "做种明细行保留健康度三段（FR-02-37）")
    check("分享" in text, "分享率保留（FR-02-38，做种上传量仅经分享率体现）")
    check("8/23/1.675" in text, "BT 下载明细行健康度三段保留")
    check("剩余" in text, "剩余时间/剩余做种保留")

    print("== 场景 E：页签切换保留/还原列表状态（FR-01-117，帧对断言） ==")
    # 差异制造证明：切走期间页签 1 状态确实被改动（与还原值不同）
    tab1_after_down = latest_in(frames, 8.05, 8.35)
    check(tab1_after_down is not None and counter_of(tab1_after_down) == "2/4 项",
          "切走期间页签 1 选中下移（计数 2/4 项，与还原值 3/10 项不同）")
    tab1_end = latest_in(frames, 13.05, 13.45)
    check(tab1_end is not None and counter_of(tab1_end) == "4/4 项",
          "页签 1 End 选中尾项（计数 4/4 项，两页签状态独立）")
    tab1_home = latest_in(frames, 15.45, 15.75)
    check(tab1_home is not None and counter_of(tab1_home) == "1/4 项",
          "页签 1 Home 选中回顶（计数 1/4 项，与还原值 4/10 项不同）")

    # 状态 A：切走前 vs 切回后（首次 + 复验两次）
    pre_a = latest_in(frames, 7.25, 7.55)
    for label, win in [("还原 A", (8.5, 9.1)), ("复验还原 A", (13.6, 14.3))]:
        post = latest_in(frames, *win)
        check(pre_a is not None and post is not None, f"{label}：捕获到切走前/切回后帧")
        if pre_a is None or post is None:
            continue
        ma, mb = list_metrics(pre_a), list_metrics(post)
        check(counter_of(post) == "3/10 项", f"{label}：列表计数还原（3/10 项）")
        check(ma is not None and mb is not None and ma[0] == mb[0],
              f"{label}：光标 ▌ 行号还原（{ma[0] if ma else '?'} → {mb[0] if mb else '?'}）")
        check(ma is not None and mb is not None and ma[1] == mb[1] and ma[1],
              f"{label}：首可见任务行还原（{ma[1] if ma else '?'}）")

    # 状态 B：切走前 vs 切回后
    pre_b = latest_in(frames, 14.65, 14.95)
    post_b = latest_in(frames, 15.9, 16.6)
    check(pre_b is not None and post_b is not None, "还原 B：捕获到切走前/切回后帧")
    if pre_b is not None and post_b is not None:
        mb0, mb1 = list_metrics(pre_b), list_metrics(post_b)
        check(counter_of(pre_b) == "4/10 项" and counter_of(post_b) == "4/10 项",
              "还原 B：列表计数还原（4/10 项，与状态 A 不同）")
        check(mb0 is not None and mb1 is not None and mb0[0] == mb1[0],
              "还原 B：光标 ▌ 行号还原")
        check(mb0 is not None and mb1 is not None and mb0[1] == mb1[1] and mb0[1],
              "还原 B：首可见任务行还原")

    if FAILS:
        print(f"\n== 实证失败 {len(FAILS)} 项 ==")
        for f in FAILS:
            print(f"  - {f}")
        print("\n== 末帧快照（显示行） ==")
        for ln in last_disp:
            print(ln)
        sys.exit(1)
    print("\n== 全部实证通过 ==")


if __name__ == "__main__":
    main()
