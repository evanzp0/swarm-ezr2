#!/usr/bin/env python3
"""verify_ezr.py — pty 自动化回归验证（Task 11 口径：5 槽位下载队列）。

分块策略：块大小按协议写死——HTTP 1 MB/块 · BT 256 KB/块，
总块数 = ceil(total/块大小)，与并发数解耦。
下载槽位：同时可下载任务数写死 5（下载中 + 待自动重试的已失败占用）；
「等待中」任务按列表顺序（从上往下）依次获得空闲槽位后开始下载。
重试计数连续性规则：连续失败（自上次失败后未下载到任何数据）→ 累加；
非连续失败（重试期间有下载进展）→ 重置为 1；达上限停止自动重试并释放槽位。
演示任务 win11：首次失败后的重试正常下载一段（有进展→重置），
之后的重试模拟连接卡死（速度 0、无进展→累加直至 5/5 达上限）。

场景 A（33s 无按键）：失败任务重试流转 已失败→等待中→下载中；全程无「连接中」；
                      重试计数连续性规则：非连续失败（有进展）toast「重试次数重置为 1/5」、
                      连续失败（无进展，卡死重试）toast「累加至 2/5」且出现「重试 2/5」；
                      分块行 = x/y · 1 MB/块；[等待中] 徽标黄色；
                      任务列表行不含线程数；头部「下载槽位 5/5」；rust 排队第 1 位。
场景 B（6s ↓↓↓）：    选中已暂停任务 blender → 分块行 x/y · 1 MB/块（无图示字符）。
场景 C（9s a,enter,d,2）：A 添加任务（ADD_POOL 轮换 Fedora）→ 等待中、排队第 4 位；
                      D 删除 → 任务消失。
场景 D（5s end）：      跳到最后一个任务（BT 下载中）→ [BT] 徽标黄色、
                        分块 x/y · 256 KB/块且 y=15984（4.19GB/256KB）。
场景 E（5s a）：        添加对话框打开：并发预填 4，且无「最大并发 · HTTP 4 / BT 20」提示。
场景 F（10s space）：   暂停 ubuntu 释放槽位 → 队首 rust 自动获得槽位并从断点
                        继续下载（[下载中]）；imagenet 递补为排队第 1 位；
                        toast「▶ 槽位空闲，开始下载: rust-…」。
场景 G（78s 无按键，长跑）：win11 连续失败（卡死无进展）累加至 5/5 →「重试 5/5 · 已达上限」
                        停止自动重试并释放槽位 → 队首 rust 自动开始下载；
                        toast「已达最大重试次数」出现。
场景 H（两次运行，共 ~13s）：等待中任务 rust 按 Space → 已暂停（退出等待队列，
                        toast「已暂停（退出等待队列）」），imagenet 递补第 1 位、
                        tensorflow 第 2 位；再次运行：已暂停 rust 再按 Space（槽位满
                        5/5）→ 回到等待中排队第 1 位，toast「无空闲下载槽位」。

用法：python3 scripts/verify_ezr.py [场景集合，默认 abcdefh；g 为 78s 长跑可单独运行]
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

PROG = "/home/z/my-project/ezr-tui-demo/target/release/ezr-tui-demo"
YEL_HEX = "e4b23e"      # 黄 Rgb(228,178,62)

PASS: list[str] = []
FAIL: list[str] = []

# 分块行：`分块     x/y · N 大小/块`（x=已完成块数，y=总块数）
CHUNK_RE = re.compile(r"分块\s+(\d+)/(\d+) · (\d+(?:\.\d+)?) ([KMG]B)/块")


def check(name: str, ok: bool, detail: str = "") -> None:
    (PASS if ok else FAIL).append(f"{name}{' :: ' + detail if detail else ''}")
    print(f"  [{'OK' if ok else 'FAIL'}] {name}" + (f"  ({detail})" if detail else ""))


def key_bytes(k: str) -> bytes:
    named = {
        "up": b"\x1b[A", "down": b"\x1b[B", "right": b"\x1b[C", "left": b"\x1b[D",
        "enter": b"\r", "esc": b"\x1b", "tab": b"\t", "space": b" ",
        "pgup": b"\x1b[5~", "pgdn": b"\x1b[6~", "home": b"\x1b[H", "end": b"\x1b[F",
    }
    return named.get(k, k.encode())


def run(cols, lines, duration, keys=None, key_delay_frac=0.45, watch=None):
    """运行 TUI。samples: [(t, rows_tuple)]；watch(t, rows, screen) 每帧回调（实时取色）。"""
    keys = keys or []
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp(PROG, [PROG])
        os._exit(1)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", lines, cols, 0, 0))
    screen = pyte.Screen(cols, lines)
    stream = pyte.ByteStream(screen)
    samples = []
    t0 = time.time()
    deadline = t0 + duration
    key_start = t0 + duration * key_delay_frac
    sent = 0
    while time.time() < deadline:
        if sent < len(keys) and time.time() >= key_start + sent * 0.22:
            os.write(fd, key_bytes(keys[sent]))
            sent += 1
        r, _, _ = select.select([fd], [], [], 0.05)
        if fd in r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            stream.feed(data)
        try:
            rows = tuple(screen.display)
        except Exception:
            # pyte 对行尾宽字符偶发渲染崩溃：沿用上一帧画面
            rows = samples[-1][1] if samples else ("",) * 38
        t = time.time() - t0
        samples.append((t, rows))
        if watch is not None:
            watch(t, rows, screen)
    try:
        os.kill(pid, 9)
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    return samples, screen


def badge_of(row: str):
    for b in ("[等待中]", "[下载中]", "[已失败]", "[已暂停]", "[校验中]",
              "[后期处理中]", "[已完成]", "[做种中]", "[连接中]"):
        if b in row:
            return b
    return None


def cell_fg(screen, y, x):
    return screen.buffer[y][x].fg


def badge_fg_ok(screen, row_y, needle, expect):
    """检查某行中 needle 徽标所有单元格的前景色"""
    row = screen.display[row_y]
    x = row.find(needle)
    if x < 0:
        return False, "not found"
    fgs = {cell_fg(screen, row_y, x + i) for i in range(len(needle))}
    return fgs == {expect}, str(fgs)


def chunk_match(rows):
    """在详情面板中找分块行，返回 CHUNK_RE 的 match（无则 None）"""
    for row in rows:
        m = CHUNK_RE.search(row)
        if m:
            return m
    return None


def list_rows_have_no_threads(rows):
    """任务列表行（含徽标的行）不应包含线程数"""
    for row in rows:
        if badge_of(row) and "线程" in row:
            return False, row.strip()[:60]
    return True, ""


def task_block(rows, name_part):
    """返回任务条目 3 行块（徽标行 + 进度条行 + 统计行）拼接文本；未找到返回 None"""
    for i, row in enumerate(rows):
        if name_part in row and badge_of(row):
            return "\n".join(rows[i:i + 3])
    return None


def scenario_a():
    print("\n== 场景 A：失败重试流转 / 无连接中 / 分块文字 / 列表无线程 / 槽位满 ==")
    wait_fg = None

    def watch(t, rows, scr):
        nonlocal wait_fg
        if wait_fg is None:
            for y, row in enumerate(rows):
                if "win11-24h2" in row and "[等待中]" in row:
                    x = row.find("[等待中]")
                    wait_fg = {cell_fg(scr, y, x + i) for i in range(5)}
                    break

    samples, screen = run(120, 44, 33.0, watch=watch)

    # 1) 全程不出现「连接中」
    all_text = "\n".join("".join(rows) for _, rows in samples)
    check("A1 全程无「连接中」", "连接中" not in all_text)

    # 2) win11 任务：已失败 → 等待中 → 下载中
    seen = {}
    for t, rows in samples:
        for row in rows:
            if "win11-24h2" in row:
                b = badge_of(row)
                if b and b not in seen:
                    seen[b] = t
                break
    print(f"  win11 状态时间线: { {k: round(v, 1) for k, v in sorted(seen.items(), key=lambda x: x[1])} }")
    check("A2 初始为已失败", "[已失败]" in seen and seen["[已失败]"] < 7.5)
    check("A3 重试后进入等待中(7.5~12s)",
          "[等待中]" in seen and 7.5 <= seen["[等待中]"] <= 12.0)
    check("A4 等待中之后直接下载中(无连接中)",
          "[下载中]" in seen and seen["[下载中]"] > seen.get("[等待中]", 99))
    check("A5 [等待中] 徽标为黄色", wait_fg == {YEL_HEX}, str(wait_fg))

    # 3) 详情面板：类型行 + 分块行文字格式
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)
    check("A6 详情含「类型」行", "\n 类型" in final or " 类型 " in final.replace("\n ", "\n", 1) or any(r.strip().startswith("类型") for r in final_rows))
    check("A7 类型值 = HTTPS", "HTTPS" in final)

    # 4) 协议徽标黄色（ubuntu 行 [HTTPS]）
    for y, row in enumerate(final_rows):
        if "[HTTPS]" in row:
            ok, det = badge_fg_ok(screen, y, "[HTTPS]", YEL_HEX)
            check("A8 [HTTPS] 徽标为黄色", ok, det)
            break

    # 5) 分块行：x/y · 1 MB/块 文字格式（HTTP 固定块大小）
    m = chunk_match(final_rows)
    check("A9 分块行为 x/y · N/块 文字格式", m is not None,
          m.group(0).strip() if m else "not found")
    if m:
        x, y = int(m.group(1)), int(m.group(2))
        check("A10 分块计数合理 0<=x<=y, y=5923(6.21GB/1MB ceil)",
              0 <= x <= y and y == 5923, f"x={x} y={y}")
        check("A11 块大小标签 = 1 MB",
              m.group(3) == "1" and m.group(4) == "MB", m.group(3) + " " + m.group(4))

    # 6) 列表行无线程数
    ok, det = list_rows_have_no_threads(final_rows)
    check("A12 任务列表行不含线程数", ok, det)

    # 7) 下载槽位（Task 11）
    check("A13 头部显示 下载槽位 5/5（失败重试占用槽位）", "下载槽位 5/5" in all_text)
    rust_blk = task_block(final_rows, "rust-toolchain-nightly")
    check("A14 rust 全程排队第 1 位（列表顺序优先）",
          rust_blk is not None and "排队第 1 位" in rust_blk,
          rust_blk.replace("\n", " | ").strip()[:80] if rust_blk else "not found")
    check("A15 imagenet 排队第 2 位", "排队第 2 位" in final)
    check("A16 tensorflow 排队第 3 位", "排队第 3 位" in final)
    check("A17 连续失败累加：出现「重试 2/5」（卡死重试无进展）", "重试 2/5" in all_text)
    check("A18 非连续失败重置：toast「重试次数重置为 1/5」",
          "非连续失败，重试次数重置为 1/5" in all_text)
    check("A19 连续失败累加：toast「重试次数累加至 2/5」",
          "连续失败，重试次数累加至 2/5" in all_text)


def scenario_b():
    print("\n== 场景 B：暂停任务的分块文字（无图示字符）==")
    samples, screen = run(120, 44, 6.0, keys=["down", "down", "down"], key_delay_frac=0.35)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)
    check("B1 选中 blender（已暂停）", "blender-4.5-linux-x64" in final and "[已暂停]" in final)
    check("B2 详情类型 = HTTP", "HTTP" in final)
    m = chunk_match(final_rows)
    check("B3 分块行为 x/y · N/块 文字格式", m is not None,
          m.group(0).strip() if m else "not found")
    if m:
        x, y = int(m.group(1)), int(m.group(2))
        check("B4 blender 计数合理 y=313(328MB/1MB ceil) 且 x<y",
              0 <= x <= y and y == 313, f"x={x} y={y}")
        check("B5 blender 块大小标签 = 1 MB",
              m.group(3) == "1" and m.group(4) == "MB", m.group(3) + " " + m.group(4))
    check("B6 列表行无线程数", list_rows_have_no_threads(final_rows)[0])


def scenario_c():
    print("\n== 场景 C：A 添加任务（等待中·排队第 4 位）+ D 删除回归 ==")
    # next_id=100 → ADD_POOL[100 % 6] = Fedora
    name = "Fedora-Workstation-Live-x86_64-42.iso"
    samples, _ = run(120, 44, 9.0, keys=["a", "enter", "d", "2"], key_delay_frac=0.3)
    added_seen = False
    added_wait = False
    added_queue_pos = False
    removed = True
    for t, rows in samples:
        joined = "\n".join(rows)
        if name in joined:
            added_seen = True
            blk = task_block(rows, name)
            if blk is not None:
                if "[等待中]" in blk:
                    added_wait = True
                if "排队第 4 位" in blk:
                    added_queue_pos = True
        elif added_seen and t > 4.5:
            removed = removed and name not in joined
    check("C1 A+Enter 添加任务成功", added_seen)
    check("C2 新任务初始为「等待中」(非连接中)", added_wait)
    check("C3 D+2 删除任务成功", removed and added_seen)
    check("C4 槽位已满时新任务排队第 4 位", added_queue_pos)


def scenario_d():
    print("\n== 场景 D：BT 下载中任务 [BT] 黄色徽标 + 分块 256 KB/块（y=15984）==")
    samples, screen = run(120, 44, 5.0, keys=["end"], key_delay_frac=0.3)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)
    check("D1 选中 sintel（BT）", "sintel-4k-2160p" in final)
    check("D2 类型 = BT", "BT" in final)
    for y, row in enumerate(final_rows):
        if "[BT]" in row:
            ok, det = badge_fg_ok(screen, y, "[BT]", YEL_HEX)
            check("D3 [BT] 徽标为黄色", ok, det)
            break
    m = chunk_match(final_rows)
    ok = m is not None and int(m.group(2)) == 15984
    check("D4 BT 分块行 x/y · 256 KB/块 且 y=15984(4.19GB/256KB ceil)", ok,
          m.group(0).strip() if m else "not found")
    if m:
        check("D5 BT 块大小标签 = 256 KB",
              m.group(3) == "256" and m.group(4) == "KB", m.group(3) + " " + m.group(4))
        x = int(m.group(1))
        # sintel 下载中：x 初值 4882(1.28GB/256KB)，以 ~11 块/s 增长（5s 场景 ≤ +80）
        check("D6 BT 已完成块数 x 从 4882 起持续增长", 4882 <= x <= 4980, f"x={x}")


def scenario_e():
    print("\n== 场景 E：添加对话框——并发预填 4，无『最大并发 · HTTP 4 / BT 20』提示 ==")
    samples, _ = run(120, 44, 5.0, keys=["a"], key_delay_frac=0.3)
    # 取按键后的最后几帧（对话框稳定可见）
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)
    check("E1 对话框已打开", "添加下载任务" in final)
    dialog_rows = [r for r in final_rows if any(
        s in r for s in ("添加下载任务", "URL", "保存到", "并发", "确认", "取消", "Enter 确认"))]
    dlg_text = "\n".join(dialog_rows)
    check("E2 无『最大并发』提示文字", "最大并发" not in dlg_text and "最大并发" not in final)
    check("E3 无『HTTP 4 / BT 20』提示文字", "BT 20" not in final)
    check("E4 并发字段预填默认值 4", re.search(r"并发\s*>\s*4", dlg_text) is not None,
          next((r.strip()[:40] for r in dialog_rows if "并发" in r and ">" in r), ""))
    check("E5 提示行保留快捷键说明", "Enter 确认" in final and "并发 = 最大下载线程数" in final)


def scenario_f():
    print("\n== 场景 F：暂停释放槽位 → 队首等待任务自动获得槽位并从断点继续 ==")
    samples, _ = run(120, 44, 10.0, keys=["space"], key_delay_frac=0.35)
    seen = {}
    for t, rows in samples:
        for row in rows:
            if "rust-toolchain-nightly" in row:
                b = badge_of(row)
                if b and b not in seen:
                    seen[b] = t
                break
    print(f"  rust 状态时间线: { {k: round(v, 1) for k, v in sorted(seen.items(), key=lambda x: x[1])} }")
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)
    all_text = "\n".join("".join(rows) for _, rows in samples)

    ub_blk = task_block(final_rows, "ubuntu-24.04")
    check("F1 ubuntu 已被暂停（释放槽位）", ub_blk is not None and "[已暂停]" in ub_blk,
          ub_blk.replace("\n", " | ").strip()[:80] if ub_blk else "not found")
    check("F2 rust 获得槽位后进入下载中", "[下载中]" in seen and seen["[下载中]"] > 3.0,
          str(round(seen.get("[下载中]", -1), 1)))
    img_blk = task_block(final_rows, "imagenet-mini-dataset")
    check("F3 imagenet 递补为排队第 1 位", img_blk is not None and "排队第 1 位" in img_blk,
          img_blk.replace("\n", " | ").strip()[:80] if img_blk else "not found")
    check("F4 toast 提示「槽位空闲，开始下载: rust-…」",
          "槽位空闲，开始下载: rust-toolchain-nightly" in all_text)
    check("F5 列表行无线程数", list_rows_have_no_threads(final_rows)[0])


def scenario_g():
    print("\n== 场景 G（78s 长跑）：重试计数达上限 → 停止自动重试并释放槽位 → 队首自动开始 ==")
    samples, _ = run(120, 44, 78.0)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)

    w_blk = task_block(final_rows, "win11-24h2")
    check("G1 win11 终止失败：重试 5/5 · 已达上限",
          w_blk is not None and "重试 5/5" in w_blk and "已达上限" in w_blk,
          w_blk.replace("\n", " | ").strip()[:80] if w_blk else "not found")
    check("G2 win11 徽标为已失败", w_blk is not None and "[已失败]" in w_blk)
    r_blk = task_block(final_rows, "rust-toolchain-nightly")
    check("G3 槽位释放后 rust 自动开始下载", r_blk is not None and "[下载中]" in r_blk,
          r_blk.replace("\n", " | ").strip()[:80] if r_blk else "not found")
    check("G4 toast 出现「已达最大重试次数」", "已达最大重试次数" in all_text)
    img_blk = task_block(final_rows, "imagenet-mini-dataset")
    check("G5 imagenet 递补为排队第 1 位", img_blk is not None and "排队第 1 位" in img_blk)
    check("G6 头部槽位保持 5/5（rust 递补后）", "下载槽位 5/5" in final)


def scenario_h():
    print("\n== 场景 H：等待中按 Space → 已暂停（退出等待队列）；槽位满再按 → 回队列 ==")
    # H 第一次运行：选中 rust（列表第 3 项，等待中，排队第 1）→ Space → 已暂停
    samples, _ = run(120, 44, 6.0,
                     keys=["home", "down", "down", "space"],
                     key_delay_frac=0.30)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)

    r_blk = task_block(final_rows, "rust-toolchain-nightly")
    check("H1 等待中 rust 按 Space 后变为已暂停",
          r_blk is not None and "[已暂停]" in r_blk and "排队第" not in r_blk,
          r_blk.replace("\n", " | ").strip()[:80] if r_blk else "not found")
    check("H2 toast「已暂停（退出等待队列）: rust-…」",
          "已暂停（退出等待队列）: rust-toolchain-nightly" in all_text)
    img_blk = task_block(final_rows, "imagenet-mini-dataset")
    tf_blk = task_block(final_rows, "tensorflow-cuda")
    check("H3 imagenet 递补为排队第 1 位", img_blk is not None and "排队第 1 位" in img_blk,
          img_blk.replace("\n", " | ").strip()[:80] if img_blk else "not found")
    check("H4 tensorflow 递补为排队第 2 位", tf_blk is not None and "排队第 2 位" in tf_blk,
          tf_blk.replace("\n", " | ").strip()[:80] if tf_blk else "not found")
    check("H5 头部槽位保持 5/5", "下载槽位 5/5" in final)

    # H 第二次运行（全新实例）：rust 暂停后再按 Space（槽位满 5/5）→ 回到等待中
    samples2, _ = run(120, 44, 7.0,
                      keys=["home", "down", "down", "space", "space"],
                      key_delay_frac=0.25)
    all_text2 = "\n".join("".join(rows) for _, rows in samples2)
    final_rows2 = samples2[-1][1]

    r_blk2 = task_block(final_rows2, "rust-toolchain-nightly")
    check("H6 已暂停 rust 再按 Space（槽位满）回到等待中排队第 1 位",
          r_blk2 is not None and "[等待中]" in r_blk2 and "排队第 1 位" in r_blk2,
          r_blk2.replace("\n", " | ").strip()[:80] if r_blk2 else "not found")
    check("H7 toast「无空闲下载槽位（5/5），已进入等待队列: rust-…」",
          "无空闲下载槽位（5/5），已进入等待队列: rust-toolchain-nightly" in all_text2)


if __name__ == "__main__":
    only = sys.argv[1] if len(sys.argv) > 1 else "abcdefh"
    if "a" in only:
        scenario_a()
    if "b" in only:
        scenario_b()
    if "c" in only:
        scenario_c()
    if "d" in only:
        scenario_d()
    if "e" in only:
        scenario_e()
    if "f" in only:
        scenario_f()
    if "g" in only:
        scenario_g()
    if "h" in only:
        scenario_h()
    print(f"\n通过 {len(PASS)} 项 / 失败 {len(FAIL)} 项")
    if FAIL:
        print("失败项:")
        for f in FAIL:
            print("  -", f)
        sys.exit(1)
