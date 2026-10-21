#!/usr/bin/env python3
"""test_focus_style.py — lazygit 式聚焦显示 / 滚动条 / 弹窗拖拽 pty 验证。

复用 verify_ezr.py 的 run/check 框架，覆盖本次需求：

  F   聚焦边框：列表绿框/详情暗框 → Tab 高亮跟随；对话框打开后底层
      三面板全部暗框（bug 修复）；Esc 恢复；页签高亮跟随列表焦点
  G   通用滚动条（m3）：列表 / pieces 弹窗（边框绿色高亮 + ↑↓ 箭头 +
      ║ 双线滑块在 │ 单线轨道上滑动，与主面板高亮色统一）/ 窄终端并发面板
  H   对话框鼠标拖拽（位移生效、不误关）+ 点击判定保留（按下+松开）
  I   B 键语义迁移（HTTP 拒绝 toast）+ T 键页签切换（原 Tab 语义）
  J   弹窗高亮色统一（删除框/下拉框绿框）+ 列表滚动解锁（滚轮自由
      滚动不被拉回）+ 键盘选择视口跟随
  K   帮助窗口 / 点击聚焦 / 页签 [ ] 与点击切换 / pieces 图例 / 单行页脚 /
      模态窗外点击无操作（帮助 + pieces 守卫）
  L   需求修订：并发面板提示行删除；主界面退出键 Q/q（Esc 不退出）

用法: python3 scripts/test_focus_style.py [二进制路径] [场景集合 fghijkl]
"""

import fcntl  # noqa: E402
import os  # noqa: E402
import pty as pty_module  # noqa: E402
import select  # noqa: E402
import struct  # noqa: E402
import sys  # noqa: E402
import termios  # noqa: E402
import time  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import verify_ezr as V  # noqa: E402  复用 run/check/find_text_pos/cell_fg

PROG = sys.argv[1] if len(sys.argv) > 1 else V.PROG
V.PROG = PROG

GREEN_HEX = "00cd00"    # ratatui Color::Green（pyte ANSI 调色板十六进制）
BORDER_HEX = "384e56"   # 暗边框 Rgb(56,78,86)
CYAN_HEX = "00cdcd"     # ACCENT Color::Cyan


def is_green(fg):
    return fg is not None and str(fg).lower() in (GREEN_HEX, "green")


def is_cyan(fg):
    return fg is not None and str(fg).lower() in (CYAN_HEX, "cyan")


def row_cells(screen, y):
    """第 y 行的非空 cell 序列：返回 (拼接字符串, 真实列列表)。

    pyte 宽字符占 2 cell（首 cell 有字符、次 cell data 为空）——直接 join
    会让空 cell 消失、后续索引左移；这里只保留非空 cell 并记录各自真实列，
    字符串下标 → 真实列经 cols[] 映射（与 verify_ezr.find_text_pos 同口径）。"""
    chars = []
    cols = []
    for x in range(screen.columns):
        d = screen.buffer[y][x].data
        if d:
            chars.append(d)
            cols.append(x)
    return "".join(chars), cols


def find_border_col(screen, title: str):
    """由面板标题（顶边框行内）反推右边框真实列：标题右侧第一个 ╮。"""
    for y in range(screen.lines):
        row, cols = row_cells(screen, y)
        idx = row.find(title)
        if idx >= 0:
            for i in range(idx + len(title), len(row)):
                if row[i] in "╮┐":
                    return cols[i], y
    return None, None


def top_border_fg(screen, title: str):
    """标题行中标题后第一个非空字符的 (行, 列, 字符, 前景色)。
    面板顶边框标题后紧跟 ─，其前景色 = 边框色（右边框中段会被
    lazygit 滚动条覆盖，不可作边框色采样点）。"""
    for y in range(screen.lines):
        row, cols = row_cells(screen, y)
        idx = row.find(title)
        if idx >= 0:
            for i in range(idx + len(title), len(row)):
                if row[i] != " ":
                    return y, cols[i], row[i], screen.buffer[y][cols[i]].fg
    return None


def bottom_border_fg(screen, title: str):
    """面板底边框中段前景色。

    以 title 所在面板的左边框列为锚（标题行向左找 ╭），沿该列向下收集全部
    底左角 ╰，取**最后一个**——用于顶边框被浮层遮挡的面板：对话框盖住
    并发面板标题时，用同列的任务详情标题作锚，最后一个 ╰ 即并发面板底角。"""
    for y in range(screen.lines):
        row, cols = row_cells(screen, y)
        idx = row.find(title)
        if idx >= 0:
            left_col = None
            for i in range(idx, -1, -1):
                if row[i] in "╭┌":
                    left_col = cols[i]
                    break
            if left_col is None:
                continue
            bottom_rows = []
            for yy in range(y + 1, screen.lines):
                ch = screen.buffer[yy][left_col].data
                if ch in "╰└":
                    bottom_rows.append(yy)
                elif ch in "╯┘" and yy > y + 1:
                    # 同列的右下角（相邻面板右缘）——继续找
                    continue
            if not bottom_rows:
                return None

            def dash_run(yy):
                """该行从左边框列右侧起的连续 ─ 长度（真实面板底边框 ≥ 30，
                toast 等内容干扰的孤立 ╰ 行 ≈ 0，取最长者去噪）。"""
                n = 0
                for x in range(left_col + 1, screen.columns):
                    if screen.buffer[yy][x].data == "─":
                        n += 1
                    else:
                        break
                return n

            yy = max(bottom_rows, key=dash_run)
            for x in range(left_col + 1, screen.columns):
                c = screen.buffer[yy][x].data
                if c == "─":
                    return screen.buffer[yy][x].fg
            return None
    return None


def scrollbar_chars(screen, x_col):
    """收集某真实列全部字符（滚动条列应含 ↑↓║│）。"""
    return "".join(screen.buffer[y][x_col].data for y in range(screen.lines))


def thumb_top(screen, x_col):
    """滚动条列中首个 ║ 滑块行（0 基）；无则 None（滑块位置探针）。"""
    for y in range(screen.lines):
        if screen.buffer[y][x_col].data == "║":
            return y
    return None


def mouse(x, y, press=True):
    """SGR 鼠标序列（1 基列/行）。press=False 为释放。"""
    return f"\x1b[<0;{x};{y}{'M' if press else 'm'}".encode()


def proc_exits_after(keys, boot=2.5, settle=1.8):
    """启动 TUI → 发 keys → 判断进程是否退出（pty read EIO/EOF = 已退出）。

    用于退出键验证：主界面 Q/q 应退出；Esc 不应退出。"""
    pid, fd = pty_module.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp(V.PROG, [V.PROG])
        os._exit(1)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 34, 120, 0, 0))

    def drain(sec):
        """读取 sec 秒输出；读到 EOF/EIO 说明进程已退出。"""
        t0 = time.time()
        while time.time() - t0 < sec:
            r, _, _ = select.select([fd], [], [], 0.05)
            if fd in r:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    return True
                if not data:
                    return True
        return False

    dead = drain(boot)
    if not dead:
        for k in keys:
            try:
                os.write(fd, V.key_bytes(k))
            except OSError:
                dead = True
                break
            time.sleep(0.35)
        dead = dead or drain(settle)
    if not dead:
        try:
            wpid, _ = os.waitpid(pid, os.WNOHANG)
            dead = wpid == pid
        except ChildProcessError:
            dead = True
    try:
        os.kill(pid, 9)
        os.waitpid(pid, 0)
    except (ChildProcessError, ProcessLookupError):
        pass
    try:
        os.close(fd)
    except OSError:
        pass
    return dead


def tab_style(screen, label):
    """头部页签单元格 (fg, bg)；页签行在头部第 4 行（0 基 3）内。"""
    for y, row in enumerate(screen.display):
        if y > 5:
            break
        idx = row.find(label)
        if idx >= 0:
            c = screen.buffer[y][idx + 2]
            return c.fg, c.bg
    return None, None


# ---------------------------------------------------------------------------
# F：聚焦边框 / 页签高亮 / 浮层降级
# ---------------------------------------------------------------------------

def scenario_focus():
    print("\n== F：聚焦边框 / 页签高亮 / 浮层降级 ==")

    # 第一段：初始状态（无按键）→ 列表绿框、详情/并发暗框（顶边框取样）
    _, screen = V.run(120, 44, 5.0, key_delay_frac=0.5)
    list_fg = top_border_fg(screen, "下载任务")
    detail_fg = top_border_fg(screen, "任务详情")
    conns_fg = top_border_fg(screen, "并发连接")
    V.check("F1a 任务列表边框绿色（聚焦）", list_fg is not None and is_green(list_fg[3]), str(list_fg))
    V.check("F1b 任务详情边框暗色（未聚焦）",
            detail_fg is not None and detail_fg[3] == BORDER_HEX, str(detail_fg))
    V.check("F1c 并发连接边框暗色（未聚焦）",
            conns_fg is not None and conns_fg[3] == BORDER_HEX, str(conns_fg))

    # 第二段：Tab → 详情绿框、列表暗框；Tab×2 → 并发绿框
    _, screen2 = V.run(120, 44, 5.0, keys=["tab"], key_delay_frac=0.4)
    d2 = top_border_fg(screen2, "任务详情")
    l2 = top_border_fg(screen2, "下载任务")
    V.check("F2a Tab 后任务详情边框绿色（高亮跟随）", d2 is not None and is_green(d2[3]), str(d2))
    V.check("F2b Tab 后任务列表边框变暗", l2 is not None and l2[3] == BORDER_HEX, str(l2))
    _, screen3 = V.run(120, 44, 5.0, keys=["tab", "tab"], key_delay_frac=0.4)
    c3 = top_border_fg(screen3, "并发连接")
    V.check("F3 Tab×2 后并发连接边框绿色（高亮跟随）", c3 is not None and is_green(c3[3]), str(c3))

    # 第三段：对话框打开 → 底层全部暗框（bug 修复：弹窗后当前面板边框变暗）
    # （添加对话框居中时盖住并发面板顶边框 → 并发用底边框取样）
    _, screen4 = V.run(120, 44, 5.0, keys=["a"], key_delay_frac=0.4)
    l4 = top_border_fg(screen4, "下载任务")
    d4 = top_border_fg(screen4, "任务详情")
    c4 = bottom_border_fg(screen4, "任务详情")  # 同列锚：最后一个 ╰ = 并发面板底角
    V.check("F4a 对话框打开后列表边框变暗（bug 修复）",
            l4 is not None and l4[3] == BORDER_HEX, str(l4))
    V.check("F4b 对话框打开后详情边框变暗",
            d4 is not None and d4[3] == BORDER_HEX, str(d4))
    V.check("F4c 对话框打开后并发边框变暗（底边框取样）",
            c4 is not None and c4 == BORDER_HEX, str(c4))
    dlg_fg = top_border_fg(screen4, "添加任务：HTTP(S)")
    V.check("F4d 添加对话框边框绿色高亮（与主面板统一）",
            dlg_fg is not None and is_green(dlg_fg[3]), str(dlg_fg))

    # 第四段：Esc 关闭 → 列表恢复绿框
    _, screen5 = V.run(120, 44, 5.0, keys=["a", "esc"], key_delay_frac=0.3)
    l5 = top_border_fg(screen5, "下载任务")
    V.check("F5 Esc 关闭对话框后列表边框恢复绿色", l5 is not None and is_green(l5[3]), str(l5))

    # 第五段：页签高亮跟随列表焦点
    _, s_a = V.run(120, 44, 4.5, keys=[], key_delay_frac=0.5)
    fg_a, bg_a = tab_style(s_a, "正在下载")
    V.check("F6a 列表聚焦时当前页签青底（高亮）", bg_a == CYAN_HEX, f"fg={fg_a} bg={bg_a}")
    _, s_b = V.run(120, 44, 4.5, keys=["tab"], key_delay_frac=0.4)
    fg_b, bg_b = tab_style(s_b, "正在下载")
    V.check("F6b 焦点移走后页签无青底（不高亮）", bg_b != CYAN_HEX, f"fg={fg_b} bg={bg_b}")

    # 第六段：BackTab 反向切换（并发 → 列表）
    _, s_c = V.run(120, 44, 5.0, keys=["tab", "tab", "backtab", "backtab"], key_delay_frac=0.3)
    l6 = top_border_fg(s_c, "下载任务")
    V.check("F6c BackTab×2 回到列表（边框恢复绿色）", l6 is not None and is_green(l6[3]), str(l6))


# ---------------------------------------------------------------------------
# G：lazygit 式滚动条
# ---------------------------------------------------------------------------

def scenario_scrollbars():
    print("\n== G：通用滚动条（↑↓ 箭头 + │ 单线轨道 + ║ 双线滑块）==")

    # 列表滚动条：120x44 下 10 任务 > 9 可视块 → 右边框列出现 ↑/↓/║
    _, screen = V.run(120, 44, 5.0, key_delay_frac=0.5)
    lx, _ = find_border_col(screen, "下载任务")
    V.check("G1a 任务列表右边框列定位", lx is not None, str(lx))
    if lx is not None:
        col = scrollbar_chars(screen, lx)
        V.check("G1b 列表滚动条含 ↑ 上箭头", "↑" in col, repr(col[:50]))
        V.check("G1c 列表滚动条含 ↓ 下箭头", "↓" in col, repr(col[:50]))
        V.check("G1d 列表滚动条含 ║ 滑块", "║" in col, repr(col[:50]))

    # pieces 弹窗：边框绿色高亮（与主面板高亮色统一）+ 滚动条 ↑↓║
    _, screen2 = V.run(120, 44, 5.0, keys=["p"], key_delay_frac=0.4)
    text2 = "\n".join(screen2.display)
    V.check("G2a pieces 弹窗打开（分块状态）", "分块状态" in text2)
    px_fg = top_border_fg(screen2, "分块状态")
    V.check("G2b pieces 弹窗边框绿色高亮（与主面板统一）",
            px_fg is not None and is_green(px_fg[3]), str(px_fg))
    pxc, _ = find_border_col(screen2, "分块状态")
    if pxc is not None:
        col2 = scrollbar_chars(screen2, pxc)
        V.check("G2c pieces 弹窗滚动条含 ↑↓ 箭头（bug 修复）",
                "↑" in col2 and "↓" in col2, repr(col2[:50]))
        V.check("G2d pieces 弹窗滚动条含 ║ 滑块", "║" in col2, repr(col2[:50]))

    # 并发面板滚动条：窄终端 100x34 → 可视行 10 < 12（sintel BT 12 条）触发
    # （组件契约：面板轴向 ≥ 10 才绘制；100x30 时面板高 9 低于下限，属正常退化）
    _, screen3 = V.run(100, 34, 6.0, keys=["end", "tab", "tab"], key_delay_frac=0.3)
    cx3, _ = find_border_col(screen3, "并发连接")
    if cx3 is not None:
        col3 = scrollbar_chars(screen3, cx3)
        V.check("G3 窄终端并发面板滚动条出现（12 条 > 可视行，↑↓）",
                "↑" in col3 and "↓" in col3, repr(col3[:40]))
    else:
        V.check("G3 窄终端并发面板滚动条出现（12 条 > 可视行，↑↓）", False, "panel not found")


# ---------------------------------------------------------------------------
# H：对话框鼠标拖拽 + 点击判定
# ---------------------------------------------------------------------------

def scenario_drag():
    print("\n== H：对话框鼠标拖拽 + 点击判定 ==")

    # 第一轮：打开添加对话框，探测标题位置
    _, screen1 = V.run(120, 44, 5.0, keys=["a"], key_delay_frac=0.4)
    pos = V.find_text_pos(screen1, "添加任务：HTTP(S)")
    V.check("H1 添加对话框打开（标题定位）", pos is not None, str(pos))
    if pos is None:
        return
    ty, tx = pos  # (行, 真实列) 0 基

    # 第二轮：按住标题行 (tx+1, ty+1) 拖动三步到 (+14,+6) 后松开 → 窗口位移
    seq = (
        mouse(tx + 1, ty + 1, True)
        + f"\x1b[<32;{tx + 6};{ty + 3}M".encode()
        + f"\x1b[<32;{tx + 10};{ty + 5}M".encode()
        + f"\x1b[<32;{tx + 14};{ty + 6}M".encode()
        + mouse(tx + 14, ty + 6, False)
    )
    orig_kb = V.key_bytes
    V.key_bytes = lambda k: seq if k == "mdrag:seq" else orig_kb(k)
    samples, screen2 = V.run(120, 44, 6.0, keys=["a", "mdrag:seq"], key_delay_frac=0.35)
    V.key_bytes = orig_kb
    pos2 = V.find_text_pos(screen2, "添加任务：HTTP(S)")
    moved = pos2 is not None and (pos2[0] > ty and pos2[1] > tx)
    V.check("H2 拖拽后对话框标题位移（可拖拽）", moved, f"{(ty, tx)} → {pos2}")
    text2 = "\n".join(samples[-1][1])
    V.check("H3 拖拽后对话框保持打开", "添加任务：HTTP(S)" in text2)

    # 第三轮：点击（按下+松开未移动）URL 字段聚焦后输入 → 值更新
    _, screen3 = V.run(120, 44, 6.0, keys=["a"], key_delay_frac=0.4)
    pos3 = V.find_text_pos(screen3, "URL")
    if pos3 is None:
        V.check("H4 点击聚焦 URL 字段（行定位）", False, "URL not found")
        return
    uy, ux = pos3
    click = mouse(ux + 20, uy, True) + mouse(ux + 20, uy, False)
    V.key_bytes = lambda k: click if k == "mclick:seq" else orig_kb(k)
    samples3, _ = V.run(120, 44, 7.0,
                        keys=["a", "mclick:seq", "x", "y", "z"],
                        key_delay_frac=0.3)
    V.key_bytes = orig_kb
    text4 = "\n".join(samples3[-1][1])
    V.check("H4 点击聚焦 URL 字段后输入生效（xyz 出现）", "xyz" in text4,
            str([r.strip() for r in samples3[-1][1] if "xyz" in r][:1]))


# ---------------------------------------------------------------------------
# I：B 键 / T 键语义迁移
# ---------------------------------------------------------------------------

def scenario_keys():
    print("\n== I：B/T 键语义迁移 ==")
    samples, _ = V.run(120, 44, 5.0, keys=["b"], key_delay_frac=0.4)
    all_text = "\n".join("\n".join(rows) for _, rows in samples)
    V.check("I1 HTTP 任务按 B 提示仅 BT 可断开（原 Ctrl+B 语义）",
            "仅 BT 任务支持断开并发连接（B）" in all_text)
    samples2, _ = V.run(120, 44, 5.0, keys=["t"], key_delay_frac=0.4)
    text2 = "\n".join(samples2[-1][1])
    V.check("I2 T 切换到已完成页签（原 Tab 语义迁移）", "已完成 (" in text2)
    V.check("I3 已完成任务页签显示（arch 做种中可见）", "archlinux-x86_64" in text2)


# ---------------------------------------------------------------------------
# J：弹窗高亮色统一 + 列表滚动解锁
# ---------------------------------------------------------------------------

def scenario_unify_scroll():
    print("\n== J：弹窗高亮色统一 + 列表滚动解锁 ==")

    # J1 删除对话框：边框绿色（原红色 → 与主面板聚焦色统一）
    _, s1 = V.run(120, 44, 5.0, keys=["d"], key_delay_frac=0.4)
    d1 = top_border_fg(s1, "删除任务")
    V.check("J1 删除对话框边框绿色（与主面板高亮统一）",
            d1 is not None and is_green(d1[3]), str(d1))

    # J2 校验下拉浮层：边框绿色（原青色 → 与主面板聚焦色统一）
    _, s2 = V.run(120, 44, 6.0, keys=["a", "tab", "tab", "tab", "enter"],
                  key_delay_frac=0.3)
    d2 = top_border_fg(s2, "校验算法")
    V.check("J2 校验下拉浮层边框绿色（与主面板高亮统一）",
            d2 is not None and is_green(d2[3]), str(d2))

    # 探测：列表面板标题位置 + 基准滑块行（scroll=0 → 滑块应贴顶）
    _, probe = V.run(100, 26, 4.0, key_delay_frac=0.5)
    pos = V.find_text_pos(probe, "下载任务")
    V.check("J3a 列表面板定位", pos is not None, str(pos))
    colp, _ = find_border_col(probe, "下载任务")
    thumb0 = thumb_top(probe, colp) if colp is not None else None

    # J3 滚动解锁：选中第一行时滚轮下滚 → 视口下移且到运行结束仍保持
    # （旧逻辑 tick 每帧把视口拉回选中行 → 滚动条锁死；修复后自由滚动）。
    # 100x26 列表可视块 ~3 → 10 任务 max_scroll≈7：滚轮 ×4（每次 +2）
    if pos is not None:
        ty, tx = pos
        orig_kb = V.key_bytes
        wheel = (f"\x1b[<65;{tx + 21};{ty + 3}M".encode()) * 4
        V.key_bytes = lambda k: wheel if k == "mwheel:seq" else orig_kb(k)
        _, s3 = V.run(100, 26, 7.0, keys=["mwheel:seq"], key_delay_frac=0.3)
        V.key_bytes = orig_kb
        col3, _ = find_border_col(s3, "下载任务")
        thumb1 = thumb_top(s3, col3) if col3 is not None else None
        V.check("J3b 滚轮下滚后滑块下移且保持（滚动解锁，末帧仍在下位）",
                thumb0 is not None and thumb1 is not None and thumb1 > thumb0,
                f"thumb {thumb0} → {thumb1}")

        # J4 键盘选择视口跟随仍生效：↓×6 选中第 7 项 → 视口跟随滚动
        _, s4 = V.run(100, 26, 6.0, keys=["down"] * 6, key_delay_frac=0.35)
        col4, _ = find_border_col(s4, "下载任务")
        thumb2 = thumb_top(s4, col4) if col4 is not None else None
        V.check("J4 键盘 ↓×6 后滑块下移（选择跟随视口）",
                thumb0 is not None and thumb2 is not None and thumb2 > thumb0,
                f"thumb {thumb0} → {thumb2}")


# ---------------------------------------------------------------------------
# K：需求修订——帮助窗口 / 点击聚焦 / 页签点击与中括号 / 图例 / 单行页脚
# ---------------------------------------------------------------------------

def scenario_k():
    print("\n== K：帮助窗口 / 点击聚焦 / 页签切换 / 图例 / 页脚 ==")

    # K1 H 打开帮助窗口（标题 + 绿色高亮边框，与主面板统一）
    _, s1 = V.run(120, 44, 5.0, keys=["h"], key_delay_frac=0.4)
    text1 = "\n".join(s1.display)
    # K1a 内容多一行 Q 键说明后【并发连接】组在首屏视口外 → 断言可见组 +
    # 新增 Q 键说明行；【并发连接】等后段分组由 K11（End 滚动）覆盖
    V.check("K1a H 打开帮助窗口（含全局组与 Q 退出说明）",
            "【全局】" in text1 and "【任务列表】" in text1 and "退出程序" in text1, "")
    h_fg = top_border_fg(s1, "帮助")
    V.check("K1b 帮助窗口边框绿色高亮", h_fg is not None and is_green(h_fg[3]), str(h_fg))

    # K2 Esc 关闭帮助；K3 单行页脚（仅 H/Q 提示 + toast）
    _, s2 = V.run(120, 44, 5.0, keys=["h", "esc"], key_delay_frac=0.3)
    text2 = "\n".join(s2.display)
    V.check("K2 Esc 关闭帮助窗口", "【全局】" not in text2)
    V.check("K3a 页脚单行含 H 帮助 / Q 退出", "H 帮助" in text2 and "Q 退出" in text2)
    V.check("K3b 旧版多键页脚提示已删除", "暂停/继续" not in text2 and "清已完成" not in text2
            and "Esc 退出" not in text2)

    # K4 pieces 底部图例替代快捷键提示
    _, s4 = V.run(120, 44, 5.0, keys=["p"], key_delay_frac=0.4)
    text4 = "\n".join(s4.display)
    V.check("K4a pieces 图例四色说明", "已下载" in text4 and "正在下载" in text4
            and "部分下载" in text4 and "未下载" in text4)
    V.check("K4b pieces 快捷键提示已删除", "Esc/P 关闭" not in text4)

    # K5 添加对话框底部提示已删除
    _, s5 = V.run(120, 44, 5.0, keys=["a"], key_delay_frac=0.4)
    text5 = "\n".join(s5.display)
    V.check("K5 添加对话框底部提示已删除",
            "校验码留空 = 不校验" not in text5 and "添加任务：HTTP(S)" in text5)

    # K6 鼠标点击并发连接面板 = 聚焦（边框变绿）
    _, sp = V.run(120, 44, 4.5, key_delay_frac=0.5)
    cpos = V.find_text_pos(sp, "并发连接")
    V.check("K6a 并发面板定位", cpos is not None, str(cpos))
    if cpos is not None:
        cy, cx = cpos
        seq = mouse(cx + 5, cy + 3, True) + mouse(cx + 5, cy + 3, False)
        orig_kb = V.key_bytes
        V.key_bytes = lambda k: seq if k == "mclick:seq" else orig_kb(k)
        _, s6 = V.run(120, 44, 5.0, keys=["mclick:seq"], key_delay_frac=0.4)
        V.key_bytes = orig_kb
        c6 = top_border_fg(s6, "并发连接")
        V.check("K6b 点击后并发面板边框绿色（点击聚焦）",
                c6 is not None and is_green(c6[3]), str(c6))

    # K7 ] 切换到已完成页签；K8 [ 切回正在下载
    _, s7 = V.run(120, 44, 5.0, keys=["]"], key_delay_frac=0.4)
    t7 = "\n".join(s7.display)
    V.check("K7 ] 切到已完成页签（arch 做种可见）", "archlinux-x86_64" in t7)
    _, s8 = V.run(120, 44, 5.0, keys=["]", "["], key_delay_frac=0.3)
    t8 = "\n".join(s8.display)
    V.check("K8 [ 切回正在下载页签（ubuntu 可见）", "ubuntu-24.04.2" in t8)

    # K9 鼠标点击页签切换（点击头部「已完成」页签）
    _, sp2 = V.run(120, 44, 4.5, key_delay_frac=0.5)
    tpos = V.find_text_pos(sp2, "已完成 (")
    V.check("K9a 已完成页签定位", tpos is not None, str(tpos))
    if tpos is not None:
        ty, tx = tpos
        # 页签文本在 0 基行 ty → SGR 1 基行 ty+1；列取文本内 tx+2（1 基）
        seq9 = mouse(tx + 2, ty + 1, True) + mouse(tx + 2, ty + 1, False)
        orig_kb = V.key_bytes
        V.key_bytes = lambda k: seq9 if k == "mtab:seq" else orig_kb(k)
        _, s9 = V.run(120, 44, 5.0, keys=["mtab:seq"], key_delay_frac=0.4)
        V.key_bytes = orig_kb
        t9 = "\n".join(s9.display)
        V.check("K9b 点击页签切到已完成（arch 做种可见）", "archlinux-x86_64" in t9)

    # K10 帮助窗口拖拽（标题位移）
    _, s10p = V.run(120, 44, 4.5, keys=["h"], key_delay_frac=0.4)
    hpos = V.find_text_pos(s10p, "帮助")
    if hpos is None:
        V.check("K10 帮助窗口拖拽", False, "帮助 not found")
    else:
        hy, hx = hpos
        seq10 = (
            mouse(hx + 1, hy + 1, True)
            + f"\x1b[<32;{hx + 10};{hy + 5}M".encode()
            + mouse(hx + 10, hy + 5, False)
        )
        orig_kb = V.key_bytes
        V.key_bytes = lambda k: seq10 if k == "mdrag2:seq" else orig_kb(k)
        _, s10 = V.run(120, 44, 6.0, keys=["h", "mdrag2:seq"], key_delay_frac=0.35)
        V.key_bytes = orig_kb
        hpos2 = V.find_text_pos(s10, "帮助")
        moved = hpos2 is not None and hpos2[0] > hy and hpos2[1] > hx
        V.check("K10 帮助窗口拖拽位移", moved, f"{(hy, hx)} → {hpos2}")

    # K11 帮助内容滚动（End → 末行提示可见）
    _, s11 = V.run(120, 44, 5.0, keys=["h", "end"], key_delay_frac=0.3)
    t11 = "\n".join(s11.display)
    V.check("K11 帮助内容可滚动（End 后末行提示可见）",
            "滚动本页" in t11, t11[-400:] if len(t11) > 400 else "")

    # K12/K13 模态统一口径：浮层打开时点击窗口外部 = 无任何操作
    #（帮助窗口 70x24 / pieces 64x21 在 120x44 下均远离左上角 (3,3)）
    def click_outside_seq():
        return mouse(3, 3, True) + mouse(3, 3, False)

    orig_kb = V.key_bytes
    V.key_bytes = lambda k: click_outside_seq() if k == "mout:seq" else orig_kb(k)
    _, s12 = V.run(120, 44, 5.0, keys=["h", "mout:seq"], key_delay_frac=0.4)
    t12 = "\n".join(s12.display)
    V.check("K12 帮助窗口打开时点击外部 = 无任何操作（窗口仍开）",
            "【全局】" in t12 and "退出程序" in t12, t12[:120])
    _, s13 = V.run(120, 44, 5.0, keys=["p", "mout:seq"], key_delay_frac=0.4)
    t13 = "\n".join(s13.display)
    V.check("K13 pieces 弹窗打开时点击外部 = 无任何操作（弹窗仍开）",
            "已下载" in t13 and "正在下载" in t13, t13[:120])
    V.key_bytes = orig_kb


# ---------------------------------------------------------------------------
# L：需求修订——并发面板提示行删除 / 退出键 Q
# ---------------------------------------------------------------------------

def scenario_quit():
    print("\n== L：并发提示行删除 / Q 退出 / Esc 不退出 ==")

    # L1 并发面板提示行已删除（BT 聚焦态）
    _, s1 = V.run(120, 44, 5.0, keys=["end", "tab", "tab"], key_delay_frac=0.3)
    t1 = "\n".join(s1.display)
    V.check("L1a BT 并发面板提示行已删除（无选择连接/B 断开提示）",
            "选择连接" not in t1 and "B 断开连接" not in t1)

    # L1b 非聚焦口径提示行也已删除
    _, s2 = V.run(120, 44, 5.0, key_delay_frac=0.5)
    t2 = "\n".join(s2.display)
    V.check("L1b 非聚焦提示行已删除（无 Tab 聚焦后提示）", "Tab 聚焦后" not in t2)

    # L2 主界面 Esc 不退出（进程存活）
    V.check("L2 主界面 Esc 不退出（进程存活）", not proc_exits_after(["esc"]))

    # L3/L4 主界面 Q / q 退出程序（pty 读到 EOF/EIO = 进程已退出）
    V.check("L3 主界面 Q 退出程序", proc_exits_after(["Q"]))
    V.check("L4 主界面 q 退出程序", proc_exits_after(["q"]))


def main():
    only = sys.argv[2] if len(sys.argv) > 2 else "fgihjkl"
    if "f" in only:
        scenario_focus()
    if "g" in only:
        scenario_scrollbars()
    if "h" in only:
        scenario_drag()
    if "i" in only:
        scenario_keys()
    if "j" in only:
        scenario_unify_scroll()
    if "k" in only:
        scenario_k()
    if "l" in only:
        scenario_quit()

    print(f"\n========== 结果：{len(V.PASS)} 通过 / {len(V.FAIL)} 失败 ==========")
    if V.FAIL:
        print("失败项：")
        for f in V.FAIL:
            print(f"  ✗ {f}")
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
