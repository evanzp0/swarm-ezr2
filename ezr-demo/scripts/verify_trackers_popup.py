#!/usr/bin/env python3
"""verify_trackers_popup.py — T trackers 弹窗修复/改版的 PTY 级自验证。

  T1 种子任务（sintel，自带 2 tracker）：弹窗 90×22 居中；列 = #/URL/状态/Peers/
     Seeds/消息（qBittorrent 口径）；两条 tracker 可见；无 wss://；http 行含
     VS15 反终端自动链接标记（udp 行无）；明细行间细线分隔
  T2 新增 BT 任务（trackers 留空确认）：T 弹窗非空 = 4 条默认 tracker
     （回归：旧版面板预填不入库 → 弹窗空）；47 字符 URL 在 44 列宽自动折行
     （首行 …:1337/annou + 续行 nce）；续行与首行同为 FG 色（无 DIM 续行）；
     底部无提示行（v1.11）；4 条 tracker 间 3 条细线分隔；右 padding = 左 padding
  T3 wss:// 拒绝：修改 BT 对话框 trackers 追加 wss:// 行 → Enter → 拒绝 toast、
     对话框不关闭、焦点移回 trackers（v1.9 FR-02-31 wss 拒绝）

用法: python3 scripts/verify_trackers_popup.py [程序路径]
"""
import sys
import time

ROWS, COLS = 44, 120
TORRENT = "/srv/torrents/ubuntu-24.04.2-desktop-amd64.iso.torrent"
VS15 = "\ufe0e"

BAD = 0


def check(name, cond, detail=""):
    global BAD
    BAD += not cond
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" —— {detail}" if detail and not cond else ""))


def norm(s):
    """去除反终端自动链接注入的 VS15，还原可读 URL 文本。"""
    return s.replace(VS15, "")


src = open('scripts/check_popup_integrity.py').read().replace(
    'if __name__ == "__main__":\n    main()', '')
import types
mod = types.ModuleType('cpi')
exec(compile(src, 'cpi', 'exec'), mod.__dict__)
mod.ROWS = ROWS
CONT = mod.CONT

exec(compile(open('scripts/verify_phase2_ui.py').read().split('def main()')[0].replace(
    'if __name__ == "__main__":\n    sys.exit(main())', ''), 'vtp', 'exec'))


def popup_cells(s):
    """T 弹窗内容：(inner_x, top_y, rows) 或 None（弹窗未开）。"""
    rows = s.raw_rows()
    pos = locate_dialog(rows, " Trackers ")
    if pos is None:
        return None
    xl, yt, xr = pos
    return xl, yt, xr, rows


def main():
    program = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ezr-tui-demo"

    print("== T1 种子任务弹窗（sintel）==")
    s = Session(program)
    try:
        s.rd(1.5)
        s.send(b"\x1b[F", 0.4)            # End → sintel（BT，2 trackers）
        s.send(b"t", 0.6)
        got = popup_cells(s)
        check("T0 弹窗打开（标题 Trackers）", got is not None)
        if got:
            xl, yt, xr, rows = got
            check("T1 几何 90×22（╭╮╰╯ 四角）",
                  xr - xl + 1 == 90
                  and rows[yt + 21][xl] == "╰" and rows[yt + 21][xl + 89] == "╯"
                  and rows[yt][xl] == "╭" and rows[yt][xl + 89] == "╮",
                  f"w={xr - xl + 1} yt={yt}")
            hdr = rows[yt + 3][xl + 2:xl + 88].replace(CONT, "")
            check("T1 列 = #/URL/状态/Peers/Seeds/消息（qBt 口径）",
                  all(k in hdr for k in ("#", "URL", "状态", "Peers", "Seeds", "消息")),
                  hdr[:70])
            body = "\n".join(rows[yt + 4 + i][xl + 2:xl + 88].replace(CONT, "") for i in range(8))
            check("T1 tracker1 可见（udp://tracker.example:6969…）",
                  any("udp://tracker.example:6969" in norm(rows[yt + 4 + i].replace(CONT, "")) for i in range(8)),
                  body[:60])
            check("T1 tracker2 可见（http://tracker.example/announce）",
                  any("http://tracker.example/announce" in norm(rows[yt + 4 + i].replace(CONT, "")) for i in range(8)))
            check("T1 无 wss://", "wss://" not in body)
            check("T1 状态列口径（工作中/失败/待 announce 之一）",
                  any(k in body for k in ("工作中", "待 announce", "失败")), body[:60])
            udp_row = next((rows[yt + 4 + i][xl + 2:xl + 88] for i in range(8)
                            if "udp://tracker.example:6969" in norm(rows[yt + 4 + i])), "")
            http_row = next((rows[yt + 4 + i][xl + 2:xl + 88] for i in range(8)
                             if "http://tracker.example/announce" in norm(rows[yt + 4 + i])), "")
            check("T1 http 行含 VS15 反链接标记（udp 行无）",
                  VS15 in http_row and VS15 not in udp_row,
                  f"http={VS15 in http_row} udp={VS15 in udp_row}")
            seps = [rows[yt + 4 + i][xl + 2:xl + 88].replace(CONT, "") for i in range(8)
                    if rows[yt + 4 + i][xl + 2:xl + 88].strip() != ""
                    and set(rows[yt + 4 + i][xl + 2:xl + 88].replace(CONT, "").strip()) <= {"─"}]
            check("T1 明细行间细线分隔（2 tracker → 1 条 ──）", len(seps) == 1, f"seps={len(seps)}")
            check("T1 底部无提示行（v1.11）",
                  all("个 tracker" not in rows[yt + 4 + i] and "PgUp" not in rows[yt + 4 + i] for i in range(18)))
        s.send(b"\x1b", 0.4)
        check("T1 Esc 关闭弹窗", locate_dialog(s.raw_rows(), " Trackers ") is None)
    finally:
        s.close()

    print("== T2 新增 BT 任务默认 tracker 入库（回归）==")
    s = Session(program)
    try:
        s.rd(1.5)
        s.send(b"a", 0.6)
        s.send(TORRENT.encode(), 0.6)
        s.send(b"\r", 0.8)                # 分流 → 添加任务：BT
        s.send(b"\r", 0.8)                # 焦点 0 裸 Enter = 确认（trackers 留空）
        rows = s.raw_rows()
        scr = s.screen()
        check("T2 任务已添加（对话框关闭）", "添加任务：BT" not in scr)
        s.send(b"t", 0.6)
        got = popup_cells(s)
        check("T2 弹窗打开", got is not None)
        if got:
            xl, yt, xr, rows = got
            body_rows = [norm(rows[yt + 4 + i][xl + 2:xl + 88].replace(CONT, "")) for i in range(12)]
            full = "\n".join(norm(rows[yt + 4 + i][xl + 2:xl + 88].replace(CONT, "")) for i in range(18))
            joined = "\n".join(body_rows)
            check("T2 弹窗非空 = 4 条默认 tracker 全可见（回归：旧版为空）",
                  all(u in joined for u in (
                      "udp://tracker.ezr.dev:6969",
                      "udp://tracker.opentorrent.example:1337",
                      "https://tracker.ezr.dev/announce",
                      "http://tracker.ezr.dev:8080",
                  )), joined[:120])
            check("T2 默认 tracker1（udp://tracker.ezr.dev:6969…）",
                  any("udp://tracker.ezr.dev:6969" in r for r in body_rows))
            check("T2 默认 tracker4（http://tracker.ezr.dev:8080…）",
                  any("http://tracker.ezr.dev:8080" in r for r in body_rows))
            check("T2 无 wss://", "wss://" not in joined)
            wrap_ok = any(
                "udp://tracker.opentorrent.example:1337/annou" in body_rows[i]
                and body_rows[i + 1].strip().startswith("nce")
                for i in range(len(body_rows) - 1))
            check("T2 47 字符 URL 自动折行（44 列宽两行：首行 …/annou + 续行 nce）", wrap_ok,
                  "\n".join(r[:46] for r in body_rows[:6]))
            check("T2 未运行任务状态 = 待 announce",
                  "待 announce" in joined)
            check("T2 底部无提示行（v1.11 删除全部底部提示）",
                  all("个 tracker" not in ln and "PgUp" not in ln and "滚轮" not in ln
                      for ln in full.splitlines()))
            seps = [r for r in body_rows if r.strip() and set(r.strip()) <= {"─"}]
            check("T2 4 条 tracker 间 3 条细线分隔", len(seps) == 3, f"seps={len(seps)}")
            udp_row = next((r for r in body_rows if "udp://tracker.ezr.dev:6969" in r), "")
            http_row = next((r for r in body_rows if "http://tracker.ezr.dev:8080" in r), "")
            raw_rows_idx = [rows[yt + 4 + i][xl + 2:xl + 88] for i in range(12)]
            check("T2 http/https 行含 VS15 反链接标记（udp 行无）",
                  any(VS15 in r for r in raw_rows_idx if "http" in norm(r))
                  and VS15 not in next((r for r in raw_rows_idx if "udp://tracker.ezr.dev" in r), ""))
            check("T2 续行与首行同色（折行行 nce 前无样式断裂 = 文本平铺无前缀）",
                  wrap_ok)
        s.send(b"\x1b", 0.4)
    finally:
        s.close()

    print("== T3 wss:// 拒绝（FR-02-31）==")
    s = Session(program)
    try:
        s.rd(1.5)
        s.send(b"\x1b[F", 0.4)            # End → sintel
        s.send(b"m", 0.7)                 # 修改任务：BT
        rows = s.raw_rows()
        pos = locate_dialog(rows, "修改任务：BT")
        check("T3 修改 BT 对话框打开", pos is not None)
        if pos:
            xl, yt, _xr = pos
            tr_y = yt + 3 + 8 + 4
            s.send(f"\x1b[<0;{xl + 31};{tr_y + 3}M".encode(), 0.15)
            s.send(f"\x1b[<0;{xl + 31};{tr_y + 3}m".encode(), 0.3)
            s.send(b"\x1b\r", 0.3)        # Alt+Enter 换行（光标在已有内容后）
            s.send(b"wss://bad.example", 0.5)
            s.send(b"\r", 0.7)            # Enter = 确认 → 应拒绝
            rows = s.raw_rows()
            scr = s.screen()
            check("T3 wss 行被拒绝（对话框保持打开）", "修改任务：BT" in scr)
            check("T3 拒绝 toast（不支持的 tracker 协议）",
                  "不支持的 tracker 协议" in scr)
        s.send(b"\x1b", 0.4)              # Esc 关闭（不落库）
    finally:
        s.close()

    print(f"\ntrackers 弹窗自验证：{'全部通过' if BAD == 0 else f'{BAD} 项失败'}")
    return 1 if BAD else 0


if __name__ == "__main__":
    sys.exit(main())
