#!/usr/bin/env python3
"""verify_ezr.py — pty 自动化回归验证（Task 19 口径：M1 规则失败分类 + 指数退避）。

分块策略：块大小按协议写死——HTTP 1 MB/块 · BT 256 KB/块，
总块数 = ceil(total/块大小)，与并发数解耦。
下载槽位：同时可下载任务数写死 5（下载中 + 待自动重试的已失败占用）；
「等待中」任务按列表顺序（从上往下）依次获得空闲槽位后开始下载。
失败分类（FR-M1-40/43）：网络错误 / 408·429·5xx / 大小不符 → 指数退避 8→16→32→60s
封顶自动重试（Retry-After ≤60s 优先）；语义性 4xx / 磁盘空间不足 / SHA-256 校验失败
→ 不自动重试（停等，仅 R）；校验失败按 R = 重新校验（无块重传，生产 v1.17/FR-01-51
+ D34 同步）；校验码已清空后按 R = 不校验——文件完整性直判不经引擎零排队零重传
直接收尾「无校验」（生产 v1.18/FR-01-51 同步，toast「✓ 文件已完整，直接完成（无校验）」）；
续传一致性失效 → 断点作废、从头重新下载（FR-M1-22）。
重试计数连续性规则：连续失败（自上次失败后未下载到任何数据）→ 累加；
非连续失败（重试期间有下载进展）→ 重置为 1；达上限停止自动重试并释放槽位。
演示任务 win11：失败原因按 fail_case 轮换（超时→重置→大小不符→503 RA→一致性失效），
首次失败后的重试正常下载一段（有进展→重置），之后模拟连接卡死（无进展→累加至 5/5）。
演示任务 gpt4all：启动 10.6s 后 SHA-256 校验失败（不自动重试），R 后重新校验（无块重传）→ 校验通过完成。
演示任务 tensorflow：首次获得槽位开始前磁盘空间预检失败（FR-M1-44，场景 L 自动验证）。

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
场景 E（5s a）：        添加对话框打开：三按钮 [ 立即下载 ] [ 仅添加 ] [ 取消 ]（v1.15/FR-01-103 同步）；
                        并发预填 4；校验行默认 SHA-256；代理行默认直连；
                        校验码占位「64 位十六进制（可留空）」；提示行「Enter 立即下载」；无『最大并发』提示。
场景 F（10s space）：   暂停 ubuntu 释放槽位 → 队首 rust 自动获得槽位并从断点
                        继续下载（[下载中]）；imagenet 递补为排队第 1 位；
                        toast「▶ 槽位空闲，开始下载: rust-…」。
场景 G（150s 无按键，长跑）：退避序列 toast 8s/16s、Retry-After 优先 10s、
                        退避封顶 60s（一致性失效从头重下）+「断点已作废」；
                        连续失败累加至 5/5 →「重试 5/5 · 已达上限」停止自动重试
                        并释放槽位 → 队首 rust 自动开始下载；
                        toast「已达最大重试次数」出现。
场景 H（两次运行，共 ~13s）：等待中任务 rust 按 Space → 已暂停（退出等待队列，
                        toast「已暂停（退出等待队列）」），imagenet 递补第 1 位、
                        tensorflow 第 2 位；再次运行：已暂停 rust 再按 Space（槽位满
                        5/5）→ 回到等待中排队第 1 位，toast「无空闲下载槽位」。
场景 I（16s 键盘）：    校验算法下拉框键盘流：Tab×3 → Enter 展开 → ↓ 选 SHA-384 →
                        输入非法校验码 → toast「需为 96 位（当前 4 位）」；切回 MD5、
                        输入合法 32 位校验码 → 确认建任务，详情出现「校验 MD5 ·…（完成后校验）」。
场景 J（两轮鼠标）：    先跑一轮探测「SHA-256 ▾」与下拉项坐标；再以 SGR 鼠标点击
                        校验算法行展开下拉框 → 点击 SHA-512 选项 → 字段变为
                        「SHA-512 ▾」且下拉框收起。
场景 K（17s 键盘）：    gpt4all 启动 10.6s 后 SHA-256 校验失败（不自动重试，
                        详情「下载内容与校验值不符」）→ 选中后按 R →
                        toast「重新校验（无块重传）」（生产 v1.17/D10 同步）→
                        重校验通过 → 「✓ SHA-256 校验通过」收尾 → 后期处理中（归档类）
                        /已完成（非归档类，即离开页签）——不再回「等待中/已失败」。
场景 L（15s 键盘）：    暂停 ubuntu/media/llama3 释放 3 槽位 → rust/imagenet 递补开始、
                        tensorflow 获槽开始前磁盘预检失败（FR-M1-44：toast「不自动重试，
                        按 R 手动重试」、详情「磁盘空间不足」）→ R 重新排队 →
                        预检通过进入下载中。
场景 M（50s 键盘）：    46s 处选中 win11 → 详情「失败原因」展示含状态码文案
                        「HTTP 503 Service Unavailable…」（FR-M1-40）。
场景 N（三段运行）：   顶部布局（第八轮）：头部仪表面板通栏全宽 5 行（字段单行 + 分割线 +
                        页签行），右缘 = 屏幕右缘 = 任务详情右缘；流量图 = 单色面积
                        图（▁▂▃▄▅▆▇█ 八级部分块、1/8 格亚字符精度、自底向上填充，
                        全图仅一种颜色 = 下载淡蓝，REQ-8.5），内嵌头部右侧（宽
                        20%、占 3 行内容区全高、仅下行流量；│ 分隔符 + 图区左右各
                        1 列空白；双向 EMA + 窗口 min–max 缩放，每列双子样本取较
                        大值，无断列、无间隔、无盲文点阵）；头部字段顺序 = 并发、会
                        话已下
                        载、全局、峰值（单行、峰值写在面板内容里）；「任务队列」
                        面板移除，页签行（正在下载/已完成/下载槽位，槽位内联左对
                        齐、紧跟已完成并以 │ 分隔）并入头部第 3 内容行，任务列表
                        自头部正下方起；
                        HTTP 任务并发连接列 = 序号/下载速度/累计下载（无上传两列）；
                        BT 任务 = IP(掩码)/下载速度/累计下载/上传速度/累计上传 五列，
                        首列为掩码 IP（IPv4 首段.*.末端 / IPv6 首段:*:末端，
                        无完整地址泄露、互不重复、IPv4/IPv6 混合）；
                        已完成任务（godot）明细为空、活跃 0/0；
                        下载中任务并发明细速度不闪烁（采样 5s 无「-」闪现）。
场景 O（键盘）：        Ctrl+↓/↑ 选中并发明细行（高亮跟随）；BT 任务 Ctrl+B 断开
                        选中连接（行移除、掩码 IP 互不重复、活跃总数 -1、toast
                        含被断开的掩码 IP）；HTTP 任务 Ctrl+B 被拒绝（toast、
                        序号列行数不变）。
场景 P（明细门槛）：    仅「下载中」/「做种中」任务有并发明细：已暂停（blender）、
                        等待中（rust）、已失败（win11）、校验中（gpt4all）、
                        后期处理中（neovim）均明细空、活跃 0；无明细状态
                        Ctrl+↓/Ctrl+B 给出提示 toast；win11 按 R 重试开始下载后
                        明细恢复（rust 递补，生产 FR-01-34：R 让槽队首递补）；
                        做种中（arch）明细显示
                        （BT 五列、首列掩码 IP、活跃 1）。

用法：python3 scripts/verify_ezr.py [场景集合，默认 abcdefhjklmnop；g 为 150s 长跑可单独运行]
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
import unicodedata

import pyte

# 二进制路径：仓库内 release 构建（bin/ezr-tui-demo 为随包预编译副本）
PROG = "/home/z/swarm-ezr2/ezr-demo/target/release/ezr-tui-demo"
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
        "bs": b"\x7f", "backtab": b"\x1b[Z",
        # Ctrl 组合键：Ctrl+↑/↓（xterm 修饰键编码）与 Ctrl+B（0x02）
        "c-up": b"\x1b[1;5A", "c-down": b"\x1b[1;5B", "c-b": b"\x02",
    }
    if k in named:
        return named[k]
    # SGR 鼠标事件注入（与 capture.py 同口径，1 基列/行）：mclick:列:行
    if k.startswith("mclick:"):
        _, x, y = k.split(":")
        return f"\x1b[<0;{x};{y}M".encode()
    return k.encode()


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
    """返回任务条目 3 行块（徽标行 + 进度条行 + 统计行）拼接文本；未找到返回 None

    仅当状态徽标出现在任务名之后才视为列表行命中：详情面板首行的任务名
    与左侧列表行同处一行文本，其徽标位于名称之前（左侧列表列），
    据此排除详情行误匹配（否则选中任务的块永远定位到详情首行）。
    """
    for i, row in enumerate(rows):
        p = row.find(name_part)
        if p < 0:
            continue
        b = badge_of(row)
        if b and row.find(b) > p:
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
    print("\n== 场景 E：添加对话框——三按钮（v1.15/FR-01-103）+ 6 字段，默认 SHA-256 ==")
    samples, _ = run(120, 44, 5.0, keys=["a"], key_delay_frac=0.3)
    # 取按键后的最后几帧（对话框稳定可见）
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)
    check("E1 对话框已打开", "添加下载任务" in final)
    dialog_rows = [r for r in final_rows if any(
        s in r for s in ("添加下载任务", "URL", "保存到", "并发", "校验", "代理", "直连",
                         "立即下载", "仅添加", "取消", "Enter 立即下载"))]
    dlg_text = "\n".join(dialog_rows)
    check("E2 无『最大并发』提示文字", "最大并发" not in dlg_text and "最大并发" not in final)
    check("E3 校验行默认 SHA-256 下拉指示", re.search(r"校验\s*>\s*SHA-256 ▾", dlg_text) is not None,
          next((r.strip()[:44] for r in dialog_rows if "▾" in r), ""))
    check("E4 校验码占位为 64 位十六进制（可留空）", "64 位十六进制（可留空）" in dlg_text)
    check("E5 并发字段预填默认值 4", re.search(r"并发\s*>\s*4", dlg_text) is not None,
          next((r.strip()[:40] for r in dialog_rows if "并发" in r and ">" in r), ""))
    check("E6 提示行含校验码留空说明", "Enter 立即下载" in final and "校验码留空 = 不校验" in final)
    check("E7 代理行默认「直连」下拉指示", re.search(r"代理\s*>\s*直连 ▾", dlg_text) is not None,
          next((r.strip()[:44] for r in dialog_rows if "代理" in r and ">" in r), ""))
    check("E8 三按钮齐全（立即下载/仅添加/取消，v1.15/FR-01-103）",
          "[ 立即下载 ]" in final and "[ 仅添加 ]" in final and "[ 取消 ]" in final,
          next((r.strip()[:60] for r in final_rows if "仅添加" in r), "not found"))
    check("E9 旧文案「确认」按钮已废止", "[ 确认 ]" not in final)


def find_text_pos(screen, needle: str):
    """在 pyte 缓冲区中查找文本，返回 (行, 真实列)（0 基）；找不到返回 None。

    宽字符（CJK）在 buffer 中占 2 个 cell（首 cell 有字符、次 cell 为空占位），
    直接 join 会把宽字压缩成 1 个下标导致列号偏小——这里按「非空 cell 的真实
    列位」建立映射，返回 needle 首字符的真实列（此前返回压缩列号，背后行含
    CJK 时鼠标点击会向左偏移脱靶）。"""
    for y in range(screen.lines):
        cells = [(x, screen.buffer[y][x].data) for x in range(screen.columns)
                 if screen.buffer[y][x].data != ""]
        s = "".join(c for _, c in cells)
        i = s.find(needle)
        if i >= 0:
            return y, cells[i][0]
    return None


def scenario_i():
    print("\n== 场景 I：校验算法下拉框键盘流 + 校验码格式验证 + 合法 MD5 确认建任务 ==")
    keys = (["a", "tab", "tab", "tab", "enter", "down", "enter", "tab"]
            + ["a", "b", "c", "d", "enter"]          # SHA-384 + 仅 4 位校验码 → 长度报错
            + ["bs"] * 4                              # 清空校验码
            + ["backtab", "enter", "home", "enter", "tab"]  # 下拉切回 MD5
            + list("d41d8cd98f00b204e9800998ecf8427e")       # 合法 32 位
            + ["enter"])
    samples, _ = run(120, 44, 17.0, keys=keys, key_delay_frac=0.12)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)

    # 下拉框曾展开：全部 7 种算法与期望位数出现过
    for item in ("MD5", "SHA-1", "SHA-224", "SHA-384", "SHA-512", "Adler-32"):
        check(f"I1 下拉框展示 {item}", item in all_text)
    check("I2 非法校验码报错（SHA-384 需 96 位，当前 4 位）",
          "SHA-384 校验码需为 96 位十六进制（当前 4 位）" in all_text)
    check("I3 对话框已确认关闭", "添加下载任务" not in final)
    check("I4 toast「已添加任务」出现", "已添加任务 #" in all_text)
    # 新任务自动选中：详情出现校验行（MD5 · 校验码前缀 · 待校验）
    ck_rows = [r for r in final_rows if re.search(r"校验\s+MD5 · d41d8cd98f", r)]
    check("I5 详情出现校验行「MD5 · d41d8cd98f…（待校验）」",
          ck_rows and "（待校验）" in "\n".join(ck_rows),
          ck_rows[0].strip()[:60] if ck_rows else "not found")


def scenario_j():
    print("\n== 场景 J：鼠标点击展开校验算法下拉框并选择 SHA-512 ==")
    # 第一轮：仅打开对话框，探测「SHA-256 ▾」字段与下拉项坐标（布局确定性复用）
    _, screen1 = run(120, 44, 5.0, keys=["a"], key_delay_frac=0.3)
    pos_field = find_text_pos(screen1, "SHA-256 ▾")
    check("J1 第一轮定位校验算法字段", pos_field is not None, str(pos_field))
    if pos_field is None:
        return
    fy, fx = pos_field
    # 第二轮：a 打开 → 点击字段展开下拉框（保持展开以便定位选项）
    samples, screen2 = run(120, 44, 6.0,
                           keys=["a", f"mclick:{fx + 1}:{fy + 1}"],
                           key_delay_frac=0.25)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    # 下拉框曾展开（标题出现）；随后重新定位 SHA-512 选项坐标再点击
    check("J2 点击字段后下拉框展开（标题「校验算法」出现）", "校验算法" in all_text)
    pos_item = find_text_pos(screen2, "SHA-512")
    check("J3 下拉框中定位 SHA-512 选项", pos_item is not None, str(pos_item))
    if pos_item is None:
        return
    iy, ix = pos_item
    samples3, screen3 = run(120, 44, 7.0,
                            keys=["a", f"mclick:{fx + 1}:{fy + 1}", f"mclick:{ix + 1}:{iy + 1}"],
                            key_delay_frac=0.25)
    final3 = "\n".join(samples3[-1][1])
    check("J4 点击 SHA-512 后字段变为「SHA-512 ▾」", "SHA-512 ▾" in final3)
    check("J5 下拉框已收起（标题消失）", "校验算法" not in final3)
    check("J6 校验码占位同步为 128 位", "128 位十六进制（可留空）" in final3)


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
    print("\n== 场景 G（150s 长跑）：退避序列/Retry-After/一致性失效 → 达上限停等释放槽位 → 队首自动开始 ==")
    samples, _ = run(120, 44, 150.0)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)

    # 指数退避序列（FR-M1-42）：fail#2=8s、fail#3=16s；
    # fail#4=503 Retry-After 优先采用 10s（FR-M1-43，backoff(3)=32s ≠ 10s）；fail#5=一致性失效 60s 从头重下
    check("G1 退避序列：toast「8s 后自动重试」", "8s 后自动重试" in all_text)
    check("G2 退避序列：toast「16s 后自动重试」", "16s 后自动重试" in all_text)
    check("G3 Retry-After 优先：toast「10s 后自动重试」", "10s 后自动重试" in all_text)
    check("G4 Retry-After 与计数同条 toast（累加至 3/5 · 10s）",
          any("累加至 3/5" in row and "10s 后自动重试" in row for row in all_text.splitlines()))
    check("G5 退避封顶：toast「60s 后从头重新下载」", "60s 后从头重新下载" in all_text)
    check("G6 续传一致性失效：toast「断点已作废」", "断点已作废" in all_text)

    w_blk = task_block(final_rows, "win11-24h2")
    check("G7 win11 终止失败：重试 5/5 · 已达上限",
          w_blk is not None and "重试 5/5" in w_blk and "已达上限" in w_blk,
          w_blk.replace("\n", " | ").strip()[:80] if w_blk else "not found")
    check("G8 win11 徽标为已失败", w_blk is not None and "[已失败]" in w_blk)
    r_blk = task_block(final_rows, "rust-toolchain-nightly")
    check("G9 槽位释放后 rust 自动开始下载", r_blk is not None and "[下载中]" in r_blk,
          r_blk.replace("\n", " | ").strip()[:80] if r_blk else "not found")
    check("G10 toast 出现「已达最大重试次数」", "已达最大重试次数" in all_text)
    img_blk = task_block(final_rows, "imagenet-mini-dataset")
    check("G11 imagenet 递补为排队第 1 位", img_blk is not None and "排队第 1 位" in img_blk)
    check("G12 头部槽位保持 5/5（rust 递补后）", "下载槽位 5/5" in final)


def scenario_k():
    print("\n== 场景 K：SHA-256 校验失败停等（不自动重试）+ R 重新校验（无块重传）→ 校验通过完成 ==")
    # gpt4all 位于「正在下载」页签第 9 项（home + down×8）；校验失败在 10.6s 发生，
    # 按键从 65% 处开始（≈11.05s，r 落在 ≈12.5s）保证按 R 时任务已失败；
    # 重校验 2.6s 后通过 → 完成收尾 ≈15.2s（17s 运行内可见）
    samples, _ = run(120, 44, 17.0,
                     keys=["home"] + ["down"] * 8 + ["r"], key_delay_frac=0.65)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)

    check("K1 启动校验：toast「SHA-256 校验失败」出现", "SHA-256 校验失败" in all_text)
    check("K2 校验失败不自动重试（toast/行内「不自动重试」）", "不自动重试" in all_text)
    check("K3 详情失败原因「内容与校验值不符」", "内容与校验值不符" in all_text)
    check("K4 R 后 toast「重新校验（无块重传）」（v1.17/D10 同步）",
          "重新校验（无块重传）" in all_text)
    check("K5 重校验通过完成收尾（toast「✓ SHA-256 校验通过」）",
          "✓ SHA-256 校验通过" in all_text)
    # 校验通过 → 归档类任务进「后期处理中」（非归档类直接「已完成」离开页签）；
    # 绝不回「等待中/已失败」（生产 v1.17/D10+D34：R 重校验不重传、清码才不校验）
    g_blk = task_block(final_rows, "gpt4all-models-bundle")
    k6_left = g_blk is None  # 非归档类：已离开「正在下载」页签
    k6_post = g_blk is not None and "[后期处理中]" in g_blk
    check("K6 gpt4all 终态离开失败链路（后期处理中/已完成，非等待中/已失败）",
          k6_left or k6_post,
          (g_blk or "").replace("\n", " | ").strip()[:80])


def scenario_l():
    print("\n== 场景 L：开始前磁盘空间预检失败（不自动重试）→ R 手动重试预检通过 ==")
    # 暂停 ubuntu/media/llama3 释放 3 个槽位 → rust/imagenet 递补开始、
    # tensorflow（排队第 3 位）获得槽位后开始前预检失败（FR-M1-44：不自动重试、释放槽位）；
    # 随后选中 tensorflow 按 R → 重新排队 → 预检已通过进入下载中。
    # 导航（「进行中」页签；neovim 启动数秒后完成退页签，列表变 10 项）：
    # tensorflow = 第 8 项（home+down×7）；预检在 ~4.6s 发生，用未绑定键 x 垫待后按 R。
    samples, _ = run(120, 44, 15.0,
                     keys=["home", "space", "down", "space", "down", "down", "down", "space",
                           "down", "down", "down", "x", "x", "x", "x", "r"],
                     key_delay_frac=0.20)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    final = "\n".join(final_rows)

    check("L1 预检失败 toast「开始前预检失败」出现", "开始前预检失败" in all_text)
    check("L2 toast 注明不自动重试、按 R 手动重试",
          "不自动重试，按 R 手动重试: tensorflow" in all_text)
    check("L3 详情失败原因含「磁盘空间不足（保存目录」",
          "磁盘空间不足（保存目录" in all_text)
    tf_blk = task_block(final_rows, "tensorflow-cuda-12.8")
    check("L4 tensorflow R 重试后进入下载中（预检通过）",
          tf_blk is not None and "[下载中]" in tf_blk,
          tf_blk.replace("\n", " | ").strip()[:80] if tf_blk else "not found")
    r_blk = task_block(final_rows, "rust-toolchain-nightly")
    check("L5 rust 递补获得槽位进入下载中",
          r_blk is not None and "[下载中]" in r_blk,
          r_blk.replace("\n", " | ").strip()[:80] if r_blk else "not found")
    img_blk = task_block(final_rows, "imagenet-mini-dataset")
    check("L6 imagenet 递补获得槽位进入下载中",
          img_blk is not None and "[下载中]" in img_blk,
          img_blk.replace("\n", " | ").strip()[:80] if img_blk else "not found")


def scenario_m():
    print("\n== 场景 M：详情「失败原因」展示失败分类文案（FR-M1-40，附状态码）==")
    # win11 时间线（重试回队列含 3s 启动延迟）：f4（HTTP 503，Retry-After 10s）
    # 在 ~51-61s 之间停等；52s 处选中 win11（「进行中」页签第 7 项，home+down×6，
    # neovim 已完成退页签）→ 详情失败原因行应展示含状态码的失败文案。
    samples, _ = run(120, 44, 58.0,
                     keys=["home"] + ["down"] * 6, key_delay_frac=0.90)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]

    check("M1 详情失败原因含「HTTP 503 Service Unavailable」（附状态码）",
          "HTTP 503 Service Unavailable" in all_text)
    check("M2 失败原因行标签存在（失败原因）", "失败原因" in all_text)
    w_blk = task_block(final_rows, "win11-24h2-x64")
    check("M3 win11 处于已失败（停等重试中）",
          w_blk is not None and "[已失败]" in w_blk,
          w_blk.replace("\n", " | ").strip()[:80] if w_blk else "not found")


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


def is_wide_ch(ch: str) -> bool:
    """CJK 全宽字符（pyte 以 字符+空续格 两格存储，位置无漂移，仅 join 时坍缩）"""
    return len(ch) == 1 and unicodedata.east_asian_width(ch) in ("W", "F")


def true_row(screen, y: int) -> str:
    """按真实终端列位重建行文本：pyte 为宽字分配 字符+空(" ")续格 两格，
    续格 data 为空串，直接 join 会使后续字符在字符串中的下标左移；
    把空串补为空格后即为真实列位（长度恒等于终端列数）。"""
    return "".join(
        (screen.buffer[y][x].data or " ") for x in range(screen.columns)
    )[: screen.columns]


def conns_panel_left(screen):
    """并发连接面板左边框真实列位（由标题「并发连接」反推）；未找到返回 None。
    子串查找用 display 行（CJK 连续），列位经宽字计数换算回真实终端列。"""
    for y in range(screen.lines):
        disp = screen.display[y]
        p = disp.find("并发连接")
        if p >= 0:
            true_p = p + sum(1 for ch in disp[:p] if is_wide_ch(ch))
            return true_p - 2
    return None


# 并发连接首列标识：HTTP = 序号数字；BT = 掩码 IP
# IPv4 掩码「首段.*.末端」如 192.*.100；IPv6 掩码「首段:*:末端」如 2001:*:7334
IPV4_MASK_RE = re.compile(r"^(\d{1,3}\.\*\.\d{1,3})(?=\s|$)")
IPV6_MASK_RE = re.compile(r"^([0-9a-f]{1,4}:\*:[0-9a-f]{1,4})(?=\s|$)")
SERIAL_RE = re.compile(r"^\s{0,4}(\d{1,3})\s")
CONN_TOKEN_RE = re.compile(
    r"^(?:\d{1,3}\.\*\.\d{1,3}|[0-9a-f]{1,4}:\*:[0-9a-f]{1,4})$"
)


def conn_token(seg):
    """从并发明细行片段提取首列标识：掩码 IP（BT）或序号数字串（HTTP），无则 None"""
    m = IPV4_MASK_RE.match(seg) or IPV6_MASK_RE.match(seg) or SERIAL_RE.match(seg)
    return m.group(1) if m else None


def conns_data_rows(screen, px):
    """提取并发连接面板数据行（px = 面板左边框真实列，从内框起点切片）：
    返回 (首列标识, 行片段) 列表——HTTP 任务为序号数字串，BT 任务为掩码 IP"""
    out = []
    for y in range(screen.lines):
        seg = true_row(screen, y)[px + 1 :]
        tok = conn_token(seg)
        if tok is not None:
            out.append((tok, seg))
    return out


def selected_conn_ident(screen, px):
    """并发连接面板内高亮选中行（背景 SEL_BG=162a30）的首列标识
    （HTTP = 序号数字串 / BT = 掩码 IP）。
    任务列表选中行同色，但起始于列表内框（x 远小于 px），按列位区分。"""
    for y in range(screen.lines):
        row_buf = screen.buffer[y]
        for x in range(screen.columns):
            if row_buf[x].bg == "162a30" and x >= px - 2:
                seg = true_row(screen, y)[px + 1 :]
                return conn_token(seg)
    return None


def scenario_n():
    print("\n== 场景 N：顶部布局对齐 / 头部精简 / 并发连接列序（HTTP·BT）/ 已完成明细为空 / 速度防闪烁 ==")

    # 第一段：无按键 6s（默认选中 ubuntu，HTTP 下载中）——布局 + 头部内容 + HTTP 列 + 闪烁采样
    flicker = []

    def watch(t, rows, scr):
        if t < 1.5:
            return  # 预热期（首帧速度尚未建立）不参与闪烁判定
        px = conns_panel_left(scr)
        if px is None:
            return
        for _, seg in conns_data_rows(scr, px):
            if "B/s" not in seg:
                flicker.append(seg.strip()[:48])

    samples, screen = run(120, 44, 6.0, watch=watch)
    final_rows = samples[-1][1]

    # N1 头部字段（第七轮）：单行排列 = 并发、会话已下载、全局 ↓↑、峰值 ↓↑；
    # 峰值写在面板内容里而非标题行边框上
    d0, d1 = final_rows[0], final_rows[1]
    n1a = all(t in d1 for t in ("并发", "会话已下载", "全局", "峰值"))
    check("N1a 头部字段单行存在（row1 = 并发/会话已下载/全局/峰值）", n1a,
          d1.strip()[:96])
    if n1a:
        check("N1b 字段顺序 并发→会话已下载→全局→峰值（同一行）",
              d1.index("并发") < d1.index("会话已下载")
              < d1.index("全局") < d1.index("峰值"),
              d1.strip()[:96])
        check("N1c 字段行不含 并发线程/任务（页签行的 正在下载/已完成 不在此行）",
              all(t not in d1 for t in ("并发线程", "任务")),
              d1.strip()[:60])
    check("N1d 峰值不在标题行边框上、写在面板内容里（row1 含 峰值 ↓/↑）",
          "峰值" not in d0 and "峰值" in d1 and "↓" in d1 and "↑" in d1,
          d0.strip()[:50])

    # N2 顶部布局（宽字还原真实列位，第八轮：头部 5 行仪表面板 + 任务队列并入）：
    #   row0 = 头部顶边框（通栏 ╭…╮）；row1 = 字段单行 + │分隔 + 内嵌流量图（右端）；
    #   row2 = 分割线（字段区宽）+ │分隔 + 流量图；row3 = 页签行 + │分隔 + 流量图；
    #   row4 = 头部底边框；row5 = 任务列表/任务详情顶边框；
    #   row16 = 并发连接标题行（详情 11 行自 row5 起）
    r0, r1, r2, r3, r4, r5 = (true_row(screen, i) for i in (0, 1, 2, 3, 4, 5))
    tl0 = [x for x, ch in enumerate(r0) if ch == "╮"]
    tl5 = [x for x, ch in enumerate(r5) if ch == "╮"]
    borders = set("╭╮╰╯─│┌┐└┘├┤┬┴┼")

    # 图区块字符集：面积图填充用八级块元素（▁▂▃▄▅▆▇█）
    blocks = set("▁▂▃▄▅▆▇█")

    def stroke_cols(row, start):
        return [x for x in range(start, screen.columns) if row[x] in blocks]

    # 流量图区 = 头部内容区右端（120 列终端：内宽 118 = 字段区 92 + │ 1 + 空白 1
    # + 图区 23 + 空白 1）；左/右界 = rows1-3 中块字符（面积填充）最左/最右列
    all_strokes = [x for ty in (1, 2, 3) for x in stroke_cols(true_row(screen, ty), 1)]
    chart_l = min(all_strokes) if all_strokes else screen.columns - 24
    chart_r = max(all_strokes) if all_strokes else chart_l
    check("N2a 头部通栏全宽（右缘 = 屏幕右缘 = 任务详情面板右缘）",
          len(tl0) == 1 and tl0[0] == screen.columns - 1
          and len(tl5) == 2 and tl5[-1] == tl0[0],
          f"row0╮={tl0} row5╮={tl5}")
    check("N2b 头部面板 5 行高（row4 = 底边框）",
          r4[0] == "╰" and r4[screen.columns - 1] == "╯",
          f"row4 首尾={r4[0]!r}/{r4[screen.columns - 1]!r}")
    check("N2c 图区占内容区右端约 20%（23±2 列）且 rows1-3 均有波形",
          21 <= chart_r - chart_l + 1 <= 25 and bool(all_strokes),
          f"图宽={chart_r - chart_l + 1}")
    sep_x = chart_l - 2  # │ 分隔符列（图区左侧 1 列空白 + 分隔符）
    check("N2d 行2 = 分割线贯通字段区 + │ 分隔符 + 图区左右各 1 列留白",
          all(r2[x] == "─" for x in range(1, sep_x))
          and r2[sep_x] == "│" and r2[sep_x + 1] == " "
          and r2[screen.columns - 2] == " "
          and all(r2[x] not in borders for x in range(chart_l, screen.columns - 1)),
          repr(r2[1:6]) + repr(r2[sep_x - 1:sep_x + 2]))
    d3 = screen.display[3]  # display 行 CJK 连续（true_row 会在宽字后插占位列）
    check("N2e 页签行并入头部（row3 = 正在下载 ( → 已完成 ( → │ → 下载槽位，内联左对齐）",
          "正在下载 (" in d3 and "已完成 (" in d3 and "下载槽位" in d3
          and d3.index("正在下载 (") < d3.index("已完成 (") < d3.index("下载槽位")
          and d3.index("下载槽位") < 60
          and "│" in d3[d3.index("已完成 ("):d3.index("下载槽位")],
          d3.strip()[:70])
    check("N2f 任务队列面板已移除（全屏无「任务队列」标题）",
          all("任务队列" not in true_row(screen, y) for y in range(screen.lines)),
          next(("任务队列@row%d" % y for y in range(screen.lines)
                if "任务队列" in true_row(screen, y)), "none"))

    # 面积波形：图区每列均有块字符（自底向上填充、无断列）；纵向起伏 ≥2/3 行；
    # 全行无盲文点阵字符（不用点）；顶线含部分块字符（1/8 格亚字符精度）
    stroke_set = set(all_strokes)
    check("N2g 波形连续无断列（图区每个字符列均有 ≥1 面积块字符）",
          len(stroke_set) == chart_r - chart_l + 1,
          f"填充列={len(stroke_set)}/{chart_r - chart_l + 1}")
    stroke_rows = [ty for ty in (1, 2, 3) if stroke_cols(true_row(screen, ty), chart_l)]
    check("N2h 波形有纵向起伏（≥2/3 行有面积块字符）",
          len(stroke_rows) >= 2,
          f"有波形行={stroke_rows}")
    check("N2i 无盲文点阵字符（rows1-3 全行无 U+2800..28FF，面积块而非点）",
          all(not any(0x2800 <= ord(c) <= 0x28FF for c in true_row(screen, ty))
              for ty in (1, 2, 3)),
          next((hex(ord(c)) for ty in (1, 2, 3) for c in true_row(screen, ty)
                if 0x2800 <= ord(c) <= 0x28FF), "none"))
    chart_text = "".join(true_row(screen, ty) for ty in (1, 2, 3))
    part_n = sum(chart_text.count(ch) for ch in "▁▂▃▄▅▆▇")
    check("N2l 顶线亚格精度（图区含部分块字符 ▁▂▃▄▅▆▇，非整格阶梯）",
          part_n >= 1, f"部分块字符数={part_n}")
    check("N2j 任务列表自头部正下方起（row5 顶边框、左缘 col0）",
          r5[0] == "╭" and len(tl5) == 2,
          f"row5 首字符={r5[0]!r} ╮位={tl5}")
    # 详情 11 行（自 row5 起）→ 并发连接标题行 = row16
    check("N2k 并发连接面板（标题行 = 详情顶 + 11 行 = 第 16 行）",
          "并发连接" in screen.display[16], true_row(screen, 16).strip()[:40])

    # N3 HTTP 任务（ubuntu）并发连接列 = 序号/下载速度/累计下载，无上传两列
    head_line = next((r for r in final_rows if "序号" in r and "下载速度" in r), None)
    check("N3a 并发连接表头存在（序号/下载速度）", head_line is not None)
    if head_line:
        check("N3b HTTP 任务无 上传速度/累计上传 列",
              "上传速度" not in head_line and "累计上传" not in head_line, head_line.strip()[:60])
        check("N3c 表头含 累计下载 列", "累计下载" in head_line)

    # N4 速度防闪烁：1.5s 后所有并发明细行恒有速度值（无「-」闪现）
    check("N4 并发明细速度不闪烁（采样期无缺速行）", not flicker,
          flicker[0] if flicker else "采样 4.5s")

    # 第二段：end 选中 sintel（BT 下载中）→ 五列齐全 + 首列掩码 IP
    samples2, screen2 = run(120, 44, 5.0, keys=["end"], key_delay_frac=0.3)
    rows2 = samples2[-1][1]
    final2 = "\n".join(rows2)
    bt_head = next((r for r in rows2 if "下载速度" in r and "累计上传" in r), None)
    check("N5 BT 任务并发连接五列齐全（IP/下载速度/累计下载/上传速度/累计上传）",
          bt_head is not None and all(s in bt_head for s in ("IP", "下载速度", "累计下载", "上传速度", "累计上传")),
          bt_head.strip()[:70] if bt_head else "not found")
    check("N6 BT 下载中任务选中（sintel）", "sintel-4k-2160p" in final2)

    # N10/N11 BT 首列 = 掩码 IP：12 条互不重复、无完整地址泄露、IPv4/IPv6 混合
    px_n = conns_panel_left(screen2)
    bt_tokens = [s for s, _ in conns_data_rows(screen2, px_n)] if px_n is not None else []
    check("N10 BT 首列 12 条掩码 IP（格式合规且互不重复）",
          len(bt_tokens) == 12
          and all(CONN_TOKEN_RE.match(s) for s in bt_tokens)
          and len(set(bt_tokens)) == 12, str(bt_tokens[:4]))
    check("N11 BT 首列 IPv4/IPv6 掩码混合且无完整 IP 泄露",
          any(".*." in s for s in bt_tokens) and any(":*:" in s for s in bt_tokens)
          and not re.search(r"\d+\.\d+\.\d+\.\d+", " ".join(bt_tokens)),
          str(bt_tokens))

    # 第三段：end → tab 切到已完成页签（自动选中 godot）→ 明细为空、活跃 0/0
    samples3, _ = run(120, 44, 5.0, keys=["end", "tab"], key_delay_frac=0.35)
    rows3 = samples3[-1][1]
    final3 = "\n".join(rows3)
    act = next((r for r in rows3 if "活跃" in r and "并发连接" in r), None)
    check("N7 已完成任务并发连接明细为空（无并发连接）", "（无并发连接）" in final3)
    check("N8 已完成任务活跃显示 0（修订-3 同口径）", act is not None and "活跃 0 " in act,
          act.strip()[:50] if act else "not found")
    check("N9 已完成任务选中（godot·已完成）", "godot-4.4-stable" in final3 and "已完成" in final3)


def scenario_o():
    print("\n== 场景 O：Ctrl+↑/↓ 选中并发明细 + Ctrl+B 断开连接（BT 生效 / HTTP 拒绝）==")

    # 第一段：end 选中 sintel（BT，12 条连接）→ Ctrl+↓×3 → Ctrl+↑ → Ctrl+B 断开
    keys = ["end", "c-down", "c-down", "c-down", "c-up", "c-b"]
    samples, screen = run(120, 44, 9.0, keys=keys, key_delay_frac=0.35)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    final_rows = samples[-1][1]
    px = conns_panel_left(screen)
    check("O1 并发连接面板定位", px is not None, str(px))
    if px is None:
        return

    # Ctrl+↓×3 → Ctrl+↑ 后选中第 3 条（掩码 IP）；Ctrl+B 断开后选中第 3 条原位置
    sel = selected_conn_ident(screen, px)
    tokens = [s for s, _ in conns_data_rows(screen, px)]
    check("O2 Ctrl+↓×3 + Ctrl+↑ 选中第 3 条（高亮）",
          bool(tokens) and len(tokens) >= 3 and sel is not None and sel == tokens[2],
          f"选中={sel} 第3条={tokens[2] if len(tokens) >= 3 else None}")
    check("O3 Ctrl+B 断开后明细行移除（12→11 条掩码 IP，格式合规且互不重复）",
          len(tokens) == 11
          and all(CONN_TOKEN_RE.match(s) for s in tokens)
          and len(set(tokens)) == len(tokens), str(tokens))
    check("O4 Ctrl+B toast 出现「已断开连接」+ 被断开的掩码 IP",
          "已断开连接" in all_text
          and re.search(r"已断开连接 \S+（剩余 \d+ 条）", all_text) is not None,
          str([l for l in all_text.splitlines() if "已断开连接" in l][:1]))
    act = next((r for r in final_rows if "活跃" in r and "并发连接" in r), None)
    check("O5 断开后剩余 11 条连接全部活跃（活跃 11，修订-3 同口径）",
          act is not None and "活跃 11 " in act,
          act.strip()[:50] if act else "not found")
    check("O6 断开后任务仍正常下载（sintel [下载中]）",
          any("[下载中]" in r and "sintel-4k-2160p" in r for r in final_rows))
    check("O7 面板提示行显示快捷键（Ctrl+↑↓ 选择 · Ctrl+B 断开）",
          any("Ctrl+↑↓ 选择" in r and "Ctrl+B" in r for r in final_rows))

    # 第二段：ubuntu（HTTP）按 Ctrl+B → 拒绝（toast + 行数不变）
    samples2, screen2 = run(120, 44, 5.0, keys=["c-b"], key_delay_frac=0.4)
    all_text2 = "\n".join("".join(rows) for _, rows in samples2)
    final_rows2 = samples2[-1][1]
    px2 = conns_panel_left(screen2)
    serials2 = [int(s) for s, _ in conns_data_rows(screen2, px2)] if px2 is not None else []
    check("O8 HTTP 任务 Ctrl+B 被拒绝（toast 提示）",
          "仅 BT 任务支持断开并发连接" in all_text2)
    check("O9 HTTP 任务明细行数不变（序号 1..8）", serials2 == list(range(1, 9)), str(serials2))

    # 第三段：顶部再按 Ctrl+↑ → 选中保持第 1 条（边界钳制）
    samples3, screen3 = run(120, 44, 4.0, keys=["end", "c-up"], key_delay_frac=0.4)
    px3 = conns_panel_left(screen3)
    tokens3 = [s for s, _ in conns_data_rows(screen3, px3)] if px3 is not None else []
    sel3 = selected_conn_ident(screen3, px3) if px3 is not None else None
    check("O10 顶部 Ctrl+↑ 边界钳制（选中保持第 1 条）",
          bool(tokens3) and sel3 == tokens3[0], f"选中={sel3} 首行={tokens3[0] if tokens3 else None}")


def scenario_p():
    print("\n== 场景 P：仅「下载中」/「做种中」有并发明细（其余状态空、活跃 0/0）+ 交互守卫 ==")

    # 第一段：↓↓↓ 选中 blender（已暂停）→ 明细空；Ctrl+↓ 提示无明细可选
    samples, screen = run(120, 44, 8.0,
                          keys=["down", "down", "down", "c-down"], key_delay_frac=0.3)
    all_text = "\n".join("".join(rows) for _, rows in samples)
    rows = samples[-1][1]
    final = "\n".join(rows)
    act = next((r for r in rows if "活跃" in r and "并发连接" in r), None)
    check("P1 已暂停任务选中（blender）", "blender-4.5-linux-x64" in final)
    check("P2 已暂停任务明细为空（无并发连接）", "（无并发连接）" in final)
    check("P3 已暂停任务活跃 0（修订-3 同口径）", act is not None and "活跃 0 " in act,
          act.strip()[:50] if act else "not found")
    check("P4 已暂停任务 Ctrl+↓ 提示无明细", "当前状态无并发明细" in all_text)
    px = conns_panel_left(screen)
    serials = [s for s, _ in conns_data_rows(screen, px)] if px is not None else []
    check("P5 已暂停任务无明细数据行", serials == [], str(serials))
    # 第二段：↓↓ 选中 rust（等待中）→ 明细空；Ctrl+B 提示无可断开连接
    samples2, _ = run(120, 44, 5.0, keys=["down", "down", "c-b"], key_delay_frac=0.35)
    all_text2 = "\n".join("".join(rows) for _, rows in samples2)
    rows2 = samples2[-1][1]
    final2 = "\n".join(rows2)
    act2 = next((r for r in rows2 if "活跃" in r and "并发连接" in r), None)
    check("P6 等待中任务选中（rust）", "rust-toolchain-nightly" in final2)
    check("P7 等待中任务明细为空 + 活跃 0",
          "（无并发连接）" in final2 and act2 is not None and "活跃 0 " in act2,
          act2.strip()[:50] if act2 else "not found")
    check("P8 等待中任务 Ctrl+B 提示无可断开连接", "当前状态无可断开的并发连接" in all_text2)

    # 第三段：↓↓↓↓↓↓ 选中 win11（已失败）→ 明细空
    samples3, _ = run(120, 44, 4.5, keys=["down"] * 6, key_delay_frac=0.35)
    rows3 = samples3[-1][1]
    final3 = "\n".join(rows3)
    act3 = next((r for r in rows3 if "活跃" in r and "并发连接" in r), None)
    check("P9 已失败任务选中（win11·已失败）", "win11-24h2-x64" in final3 and "已失败" in final3)
    check("P10 已失败任务明细为空 + 活跃 0",
          "（无并发连接）" in final3 and act3 is not None and "活跃 0 " in act3,
          act3.strip()[:50] if act3 else "not found")

    # 第四段：win11 按 R 重试 → 生产 v1.17/FR-01-34 同口径：正常重新排队
    # 「等待中」并让出槽位（原持有槽位不保留），队首 rust 递补获得槽位开始下载
    samples4, screen4 = run(120, 44, 11.0, keys=["down"] * 6 + ["r"], key_delay_frac=0.22)
    rows4 = samples4[-1][1]
    final4 = "\n".join(rows4)
    check("P11 R 后 win11 重新排队（等待中，让出槽位）",
          any("[等待中]" in r and "win11-24h2" in r for r in rows4))
    check("P12 rust 递补获得槽位开始下载（队首优先，生产 FR-01-34 同口径）",
          any("[下载中]" in r and "rust-toolchain" in r for r in rows4))
    act4 = next((r for r in rows4 if "活跃" in r and "并发连接" in r), None)
    check("P13 等待中任务明细为空 + 活跃 0（选中项仍为 win11）",
          "（无并发连接）" in final4 and act4 is not None and "活跃 0 " in act4,
          act4.strip()[:50] if act4 else "not found")

    # 第五段：选中 gpt4all（校验中）→ 明细空
    samples5, _ = run(120, 44, 4.5, keys=["down"] * 8, key_delay_frac=0.3)
    rows5 = samples5[-1][1]
    final5 = "\n".join(rows5)
    act5 = next((r for r in rows5 if "活跃" in r and "并发连接" in r), None)
    check("P14 校验中任务选中（gpt4all·校验中）",
          "gpt4all-models-bundle" in final5 and "校验中" in final5)
    check("P15 校验中任务明细为空 + 活跃 0",
          "（无并发连接）" in final5 and act5 is not None and "活跃 0 " in act5,
          act5.strip()[:50] if act5 else "not found")

    # 第六段：neovim（后期处理中）——启动后约 2.2s 完成并移出本页签（列表缩短、
    # 选中项回跳），故用快速按键 + 采样帧断言：选中 neovim 且仍为 [后期处理中]
    # 的帧内明细为空、活跃 0/0（任务名出现在 详情+列表+保存路径 ≥2 处 = 已选中）
    samples6, _ = run(120, 44, 2.6, keys=["down"] * 9, key_delay_frac=0.02)
    hit6 = False
    for _, rows6 in samples6:
        text6 = "\n".join(rows6)
        if (text6.count("neovim-0.11.2") >= 2 and "[后期处理中]" in text6
                and "（无并发连接）" in text6 and "活跃 0 " in text6):
            hit6 = True
            break
    check("P16 后期处理中任务选中且明细为空 + 活跃 0（neovim 采样帧）", hit6)

    # 第七段：Tab → 已完成页签选中 arch（做种中）→ 明细显示（BT 五列、掩码 IP、活跃 1/1）
    samples7, screen7 = run(120, 44, 5.0, keys=["tab"], key_delay_frac=0.4)
    rows7 = samples7[-1][1]
    final7 = "\n".join(rows7)
    head7 = next((r for r in rows7 if "下载速度" in r and "累计上传" in r), None)
    px7 = conns_panel_left(screen7)
    tokens7 = [s for s, _ in conns_data_rows(screen7, px7)] if px7 is not None else []
    act7 = next((r for r in rows7 if "活跃" in r and "并发连接" in r), None)
    check("P18 做种中任务选中（arch）", "archlinux-x86_64" in final7)
    check("P19 做种中任务明细显示（1 条连接，首列为掩码 IP）",
          len(tokens7) == 1 and CONN_TOKEN_RE.match(tokens7[0]) is not None,
          str(tokens7))
    check("P20 做种中任务五列齐全（BT：IP/下载速度/累计下载/上传速度/累计上传）",
          head7 is not None and all(s in head7 for s in ("IP", "下载速度", "累计下载", "上传速度", "累计上传")),
          head7.strip()[:70] if head7 else "not found")
    check("P21 做种中任务活跃 1（上传连接活跃，修订-3 同口径）", act7 is not None and "活跃 1 " in act7,
          act7.strip()[:50] if act7 else "not found")


if __name__ == "__main__":
    only = sys.argv[1] if len(sys.argv) > 1 else "abcdefhjklmnop"
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
    if "i" in only:
        scenario_i()
    if "j" in only:
        scenario_j()
    if "k" in only:
        scenario_k()
    if "l" in only:
        scenario_l()
    if "m" in only:
        scenario_m()
    if "n" in only:
        scenario_n()
    if "o" in only:
        scenario_o()
    if "p" in only:
        scenario_p()
    print(f"\n通过 {len(PASS)} 项 / 失败 {len(FAIL)} 项")
    if FAIL:
        print("失败项:")
        for f in FAIL:
            print("  -", f)
        sys.exit(1)
