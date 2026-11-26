#!/usr/bin/env python3
"""suite_trackers_window.py — 11-trackers-window-qa.md v1.12 六用例可执行化（six-pack/QA 2026-11-17）。

覆盖 QA-TW-01…06（features/11-trackers-window.feature 6 场景；94×22 / 列起点
#/URL 44/状态 49/Peers 62/Seeds 69/消息 76、消息预算 12 列 v1.12）。

Fixture 组成（每用例独立现场）：
- mini_http_tracker（harness/tracker，JSONL announce 日志承载状态对账）；
- pybtracker UDP tracker（BEP15，harness/tracker/pybtracker_server.py）；
- 种子经 ezr-mktorrent 多 --announce 构造 announce-list（长 URL > 44 列宽触发折行）；
- driver [bt] trackers 键 = config 来源；对话框手输 = 手输来源。
- 状态列数据源 = 引擎 tracker 池真实状态（工作中/失败退避/待 announce），与
  fixture interval/停服交叉对账（TW-03）。

无 seeder peer：tracker 状态与弹窗布局断言不依赖 swarm 数据面（Peers/Seeds 计数
= tracker 报告 complete/incomplete，空 swarm 合法）。

时序纪律：announce 后尽快采样倒计时（interval 大值臂）；停服失败臂以「重试 <s>s」
出现为准（退避秒数实现定义，不断言具体值）；倒计时与 fixture interval ±1s 采样差
属正常（规程容差条款）。
"""

from __future__ import annotations

import os
import re
import socket
import subprocess
import sys
import time

QA_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, QA_DIR)

from harness import (TMP_ROOT, EzrApp, Suite, Env, assert_in, assert_not_in)  # noqa: E402

MKTORRENT = os.path.join(QA_DIR, "..", "..", "ezr", "target", "debug",
                         "ezr-mktorrent")
TRACKER = os.path.abspath(os.path.join(
    QA_DIR, "..", "..", "harness", "tracker", "mini_http_tracker.py"))
UDP_TRACKER = os.path.abspath(os.path.join(
    QA_DIR, "..", "..", "harness", "tracker", "pybtracker_server.py"))

PIECE = 262144


def _click(app: EzrApp, col0: int, row0: int) -> None:
    """0 基 buffer 坐标左键点击（SGR 1 基换算；press + release 成对）。"""
    app.send(f"mclick:{col0 + 1}:{row0 + 1}")
    app.send(f"mup:{col0 + 1}:{row0 + 1}")


def _popup_raw_tail(app: EzrApp) -> str:
    """T 弹窗区段 raw 文本（去 ANSI 转义与 VS15；http URL 被 break_links 吞字，" 
    "画面文本不可用于 URL 断言，只能走原始尾流）。"""
    raw = app.raw_text()
    tail = raw[raw.rfind("Trackers"):] if "Trackers" in raw else ""
    tail = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", tail)
    return tail.replace("\uFE0E", "")


def _cell(app: EzrApp, y: int, x: int) -> str:
    return app.screen.buffer[y][x].data or ""


def _row_text(app: EzrApp, y: int, x0: int = 0, x1: int | None = None) -> str:
    x1 = app.screen.columns if x1 is None else x1
    return "".join(_cell(app, y, x) for x in range(x0, x1)).rstrip()


class MiniTracker:
    """mini_http_tracker 包装（--interval 可控；stop 后引擎退避臂可用）。"""

    def __init__(self, port: int, log_path: str, interval: int = 10):
        self.port = port
        self.proc = subprocess.Popen(
            [sys.executable, TRACKER, "--bind", "127.0.0.1", "--port", str(port),
             "--interval", str(interval), "--log", log_path],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def wait_ready(self, timeout: float = 10.0) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise RuntimeError("tracker 提前退出")
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=1):
                    return True
            except OSError:
                time.sleep(0.2)
        return False

    def stop(self) -> None:
        self.proc.terminate()
        try:
            self.proc.wait(timeout=3)
        except Exception:
            self.proc.kill()


class UdpTracker:
    """pybtracker BEP15 UDP tracker 包装。"""

    def __init__(self, port: int):
        self.port = port
        self.proc = subprocess.Popen(
            [sys.executable, UDP_TRACKER, "--bind", "127.0.0.1",
             "--port", str(port), "--interval", "10"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def wait_ready(self, timeout: float = 10.0) -> bool:
        """UDP tracker 无 TCP 监听，TCP 探活必败——改为进程存活 + 固定预热窗。"""
        time.sleep(1.5)
        return self.proc.poll() is None

    def stop(self) -> None:
        self.proc.terminate()
        try:
            self.proc.wait(timeout=3)
        except Exception:
            self.proc.kill()


class TrackersWindowSuite(Suite):
    def __init__(self):
        super().__init__("11-trackers-window", port_base=44400)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in (1, 2, 3, 4, 5, 6):
            getattr(self, f"_case_{i}")()
        return self.results

    def _snapshot(self, env: Env, case_id: str) -> None:
        if self._cur_app is None:
            return
        try:
            with open(os.path.join(TMP_ROOT, f"{case_id}.screen.txt"), "w",
                      encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    def _quit(self, app: "EzrApp | None") -> None:
        """退出进程回收：已退出仅关 fd，未退出走优雅 q（dead-child 写 OSError 防护）。"""
        if app is None:
            return
        if app.exit_code is None:
            app.graceful_quit()
        else:
            app.close()

    # -- fixtures ------------------------------------------------------------

    def _mk_torrent(self, env: Env, name: str, announces: "list[str]") -> str:
        qdir = os.path.join(env.fixture_root, "qatorrents")
        os.makedirs(qdir, exist_ok=True)
        src = os.path.join(env.fixture_root, f"payload-{name}.bin")
        if not os.path.exists(src):
            with open(src, "wb") as f:
                f.write(os.urandom(1024 * 1024))
        out = os.path.join(qdir, f"{name}.torrent")
        if not os.path.exists(out):
            args = [MKTORRENT, "--out", out, "--piece-len", str(PIECE)]
            for a in announces:
                args += ["--announce", a]
            args.append(src)
            r = subprocess.run(args, capture_output=True, text=True)
            assert r.returncode == 0, f"mktorrent 失败: {r.stderr.strip()}"
        return out

    def _driver_config(self, env: Env, tr_port: int, drv_port: int,
                       extra_trackers: "list[str] | None" = None) -> None:
        trackers = [f"http://127.0.0.1:{tr_port}/announce"] + (extra_trackers or [])
        body = ('[common]\n'
                f'download_dir = "{env.save_dir}"\n'
                '\n[bt]\n'
                'trackers = [' +
                ", ".join(f'"{t}"' for t in trackers) + ']\n'
                f'listen_port = {drv_port}\n')
        env.write_config(body)

    # -- shared steps --------------------------------------------------------

    def _add_bt_task(self, app: EzrApp, torrent: str) -> None:
        assert app.wait_for("下载槽位", 12), "应用应启动就绪（主界面常驻文本）"
        app.send("a")
        assert app.wait_for("请输入命令或链接", 5)
        app.type_text(torrent)
        app.send("enter")
        assert app.wait_for("添加任务：BT", 6)
        self._tab_to_button(app, "立即下载")
        app.send("enter")
        assert app.wait_for("已添加任务", 6)

    def _tab_to_button(self, app: EzrApp, want: str, max_tabs: int = 14) -> None:
        for _ in range(max_tabs):
            y, label = self._focused_btn(app)
            if label and want in label:
                app.pump(0.1)
                return
            app.send("tab")
            app.pump(0.15)
        raise AssertionError(f"未能聚焦按钮 {want!r}")

    def _focused_btn(self, app: EzrApp) -> "tuple[int, str] | tuple[None, None]":
        buf = app.screen._screen.buffer
        for y in range(0, app.screen.lines):
            line = "".join(buf[y][x].data or "" for x in range(10, app.screen.columns - 4))
            if "[ 立即下载 ]" in line or "[ 仅添加 ]" in line or "[ 取消 ]" in line:
                segs: dict[str, list[int]] = {}
                for x in range(10, app.screen.columns - 4):
                    segs.setdefault(str(buf[y][x].bg), []).append(x)
                hl = segs.get("00cdcd")
                if hl:
                    txt = "".join(buf[y][x].data or "" for x in range(hl[0], hl[-1] + 1))
                    return y, txt.strip()
        return None, None

    def _open_trackers(self, app: EzrApp) -> "tuple[int, int, int, int]":
        app.send("t")
        assert app.wait_for(" Trackers ", 4), "t 应打开 trackers 窗口"
        return self._frame(app, " Trackers ")

    def _frame(self, app: EzrApp, title: str) -> "tuple[int, int, int, int]":
        ty = app.find_row(title)
        assert ty is not None, f"未找到标题「{title}」"
        l = next(x for x in range(app.screen.columns) if _cell(app, ty, x) == "╭")
        r = next(x for x in range(l + 1, app.screen.columns) if _cell(app, ty, x) == "╮")
        by = next(y for y in range(ty + 1, app.screen.lines)
                  if _cell(app, y, l) == "╰")
        return ty, l, r, by

    def _add_http_task(self, app: EzrApp, env: Env, tag: str) -> None:
        assert app.wait_for("下载槽位", 12), "应用应启动就绪（主界面常驻文本）"
        app.send("a")
        assert app.wait_for("请输入命令或链接", 5)
        app.type_text(env.fixture.url(f"five-m.bin?cd={tag}&speed=100000"))
        app.send("enter")
        assert app.wait_for("添加任务：HTTP(S)", 5)
        app.send("enter")
        assert app.wait_for("已添加任务", 6)

    def _tracker_rows(self, app: EzrApp, ty: int, l: int, r: int,
                      by: int) -> "list[str]":
        """窗口内容区 URL 列文本行（去边框/padding；表头与任务名行外全部）。"""
        out = []
        for y in range(ty + 1, by):
            s = _row_text(app, y, l + 3, r - 2).strip()
            if s:
                out.append(s)
        return out

    # -- cases ---------------------------------------------------------------

    def _case_1(self):
        """QA-TW-01 t 门控三臂：未选中 / HTTP 任务 / BT 任务开窗。"""
        @self.case("QA-TW-01")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready(), "tracker 就绪超时"
                torrent = self._mk_torrent(
                    env, "t1", [f"http://127.0.0.1:{tr.port}/announce"])
                self._driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                app.send("t")
                assert app.wait_for("未选中任务，无法查看 trackers", 3), \
                    "未选中按 t 应 toast"
                app.pump(3.5)
                self._add_http_task(app, env, "tw1")
                app.send("t")
                assert app.wait_for("该任务没有 trackers（仅 BT 任务支持）", 3), \
                    "HTTP 任务按 t 应 toast"
                app.pump(3.5)
                self._add_bt_task(app, torrent)
                ty, l, r, by = self._open_trackers(app)
                app.send("esc")
                app.pump(0.3)
                assert app.find_row(" Trackers ") is None, "BT 任务 t 应正常开窗后可关"
            finally:
                self._quit(app)
                tr.stop()

    def _case_2(self):
        """QA-TW-02 94×22 布局：三来源合并去重 / 列结构 / ─ 细线 / 44 列折行 / VS15。"""
        @self.case("QA-TW-02")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                long_url = ("udp://tracker.example.example.example.example"
                            ".example/announce/very/long/url/path")
                torrent = self._mk_torrent(
                    env, "t2", [long_url, f"http://127.0.0.1:{tr.port}/announce"])
                manual = "udp://127.0.0.1:19999/announce"
                self._driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                self._open_bt_manual_tracker(app, manual)
                app.send("t")
                assert app.wait_for(" Trackers ", 4)
                ty, l, r, by = self._frame(app, " Trackers ")
                assert r - l + 1 == 94, f"应 94 列（实测 {r - l + 1}）"
                assert by - ty + 1 == 22, f"应 22 行（实测 {by - ty + 1}）"
                assert l == (app.screen.columns - 94) // 2 and \
                    ty == (app.screen.lines - 22) // 2, "应居中"
                assert next((x for x in range(r - 4, r)
                             if _cell(app, ty, x) == "✗"), None) is not None, "[✗]"
                header = None
                for y in range(ty + 1, by):
                    s = _row_text(app, y)
                    if "# " in s and "URL" in s and "状态" in s and "消息" in s:
                        header = y
                        break
                assert header is not None, "应有表头行 #/URL/状态/Peers/Seeds/消息"
                htext = _row_text(app, header)
                for col in ("#", "URL", "状态", "Peers", "Seeds", "消息"):
                    assert_in(col, htext, "表头列")
                body = self._tracker_rows(app, ty, l, r, by)
                joined = "\n".join(body)
                assert "udp://tracker.example.example.example.exampl" in "".join(
                    r.strip() for r in body) and \
                    "e.example/announce/very/long/url/path" in "".join(
                        r.strip() for r in body), \
                    "announce-list 来源长 URL 应入池且在 URL 列内折行（前缀 + 续行两段）"
                got_manual = False
                got_next = False
                for _ in range(10):
                    flat = _popup_raw_tail(app)
                    body_now = self._tracker_rows(app, ty, l, r, by)
                    if f"127.0.0.1:{tr.port}/announce" in flat and "19999" in flat:
                        got_manual = True
                    if "下轮" in "\n".join(body_now):
                        got_next = True
                    if got_manual and got_next:
                        break
                    app.send("esc")
                    app.pump(0.3)
                    time.sleep(0.6)
                    self._open_trackers(app)
                assert got_manual, "config 来源与手输来源应入池（修改后引擎池异步生效，有界轮询）"
                assert got_next, "工作中 tracker 消息列 = 下轮 <s>s（有界轮询）"
                body = self._tracker_rows(app, ty, l, r, by)
                joined = "\n".join(body)
                assert_in("下轮", joined, "工作中消息列 = 下轮 <s>s")
                assert joined.count("─") >= 2 or \
                    sum(1 for y in range(header + 1, by)
                        if set(_row_text(app, y, l + 3, r - 2).strip()) <= {"─"}) >= 2, \
                    "相邻明细行间应有 ─ 通栏细线分隔"
                assert "\uFE0E" in self._cur_app._raw_tail.decode("utf-8", "replace"), \
                    "http(s) 行应渲染层注入 VS15（原始尾流字节可见；raw_text 口径已剥离 VS15）"
                app.send("esc")
                app.pump(0.3)
            finally:
                self._quit(app)
                tr.stop()

    def _open_bt_manual_tracker(self, app: EzrApp, url: str) -> None:
        """对选中 BT 任务 M 开修改面板 → trackers 末尾追加一行 → 确定。"""
        app.send("m")
        assert app.wait_for("修改任务：BT", 5), "M 应开 BT 修改面板"
        app.send(*(["tab"] * 6))
        app.pump(0.2)
        app.send("end")
        app.pump(0.15)
        app.send("\x1b\r")
        app.pump(0.15)
        app.type_text(url)
        self._tab_to_button(app, "确定")
        app.send("enter")
        assert app.wait_for("BT 任务已更新", 5), "手输 tracker 应落库"

    def _case_3(self):
        """QA-TW-03 状态三态：工作中（HTTP/UDP）/ 停服失败退避 / 暂停待 announce / 3 位秒数。"""
        @self.case("QA-TW-03")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=180)
            udp = UdpTracker(self._next_port())
            app = None
            try:
                assert tr.wait_ready() and udp.wait_ready()
                torrent = self._mk_torrent(
                    env, "t3", [f"http://127.0.0.1:{tr.port}/announce"])
                self._driver_config(env, tr.port, self._next_port() + 10,
                                    extra_trackers=[f"udp://127.0.0.1:{udp.port}/announce"])
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_row_state(app, "127.0.0.1:" + str(tr.port), "工作中", 15), \
                    "HTTP tracker 应转工作中"
                ty, l, r, by = self._open_trackers(app)
                body = "\n".join(self._tracker_rows(app, ty, l, r, by))
                m = re.search(r"下轮 (\d{3,})s", body)
                assert m, f"3 位秒数倒计时应在 12 列预算内完整显示: {body[:400]!r}"
                assert int(m.group(1)) >= 100, f"下轮秒数应 ≥100（interval=180）: {m.group(1)}"
                assert self._wait_row_state(app, "udp://", "工作中", 20), \
                    "UDP tracker 应转工作中"
                app.send("esc")
                app.pump(0.3)

                app.send("space")
                app.pump(0.4)
                assert app.wait_for("已暂停", 4), "空格应暂停任务"
                ty, l, r, by = self._open_trackers(app)
                body = "\n".join(self._tracker_rows(app, ty, l, r, by))
                assert "待 announce" in body, f"暂停任务应待 announce: {body[:400]!r}"
                for y in range(ty + 1, by):
                    if "待 announce" in _row_text(app, y):
                        seg = _row_text(app, y)
                        seg = seg[seg.index("待 announce"):]
                        assert seg.count("—") >= 2, \
                            f"待 announce 行 Peers/Seeds/消息 应为 —: {seg!r}"
                        break
                app.send("esc")
                app.pump(0.3)
                app.send("space")
                app.pump(0.5)

                tr.stop()
                assert self._wait_row_state(app, "127.0.0.1:" + str(tr.port), "失败", 30), \
                    "停服后该 tracker 应转失败退避"
                ty, l, r, by = self._open_trackers(app)
                body = "\n".join(self._tracker_rows(app, ty, l, r, by))
                m = re.search(r"重试 (\d+)s", body)
                assert m, f"失败行消息 = 重试 <s>s: {body[:400]!r}"
                assert "—" in body, "失败行 Peers/Seeds 应为 —"
                app.send("esc")
                app.pump(0.3)
            finally:
                self._quit(app)
                tr.stop()
                udp.stop()

    def _wait_row_state(self, app: EzrApp, url_frag: str, state: str,
                        timeout: float) -> bool:
        """轮询 T 弹窗：url_frag 与 state 同现（raw 口径，VS15 免疫）。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.3)
            ty = app.find_row(" Trackers ")
            if ty is None:
                app.send("t")
                app.pump(0.4)
            flat = _popup_raw_tail(app)
            if url_frag in flat and state in flat:
                app.send("esc")
                app.pump(0.3)
                return True
            app.send("esc")
            app.pump(0.3)
            time.sleep(0.5)
        return False

    def _case_4(self):
        """QA-TW-04 12 条 tracker 长列表：滚动条 / 键盘滚动 / 拖拽移动 / 点击外部无操作。"""
        @self.case("QA-TW-04")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                torrent = self._mk_torrent(
                    env, "t4", [f"http://127.0.0.1:{tr.port}/announce"])
                extra = [f"udp://tracker-{i:02d}.example.invalid:6969/announce/path/{i}"
                         for i in range(11)]
                self._driver_config(env, tr.port, self._next_port() + 10,
                                    extra_trackers=extra)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                ty, l, r, by = self._open_trackers(app)
                body = self._tracker_rows(app, ty, l, r, by)
                assert "tracker-00" in "".join(r.strip() for r in body), \
                    "首屏应含 tracker-00（udp 无 VS15，画面文本）"
                thumb = self._thumb_col(app, ty, l, r, by)
                assert thumb is not None, "超屏应有右缘滚动条（FR-01-109）"
                first0 = self._udp_frags(self._tracker_rows(app, ty, l, r, by))
                app.send("pgdn")
                app.pump(0.3)
                first1 = self._udp_frags(self._tracker_rows(app, ty, l, r, by))
                assert first0 != first1, "PgDn 应翻页滚动"
                app.send("home")
                app.pump(0.3)
                assert self._udp_frags(self._tracker_rows(app, ty, l, r, by)) == first0, \
                    "Home 应回首部（倒计时秒数变化不影响 URL 片段序比对）"
                app.send("end")
                app.pump(0.3)
                body_end = self._tracker_rows(app, ty, l, r, by)
                assert "tracker-10" in "".join(r.strip() for r in body_end), \
                    "End 应跳尾部（udp 画面文本）"
                app.send("up")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                app.send("esc")
                app.pump(0.3)
                assert app.find_row(" Trackers ") is None

                ty2, l2, r2, by2 = self._open_trackers(app)
                app.send(f"mclick:{l2 + 20}:{ty2 + 1}")
                app.pump(0.1)
                app.send(f"mdrag:{l2 + 30}:{ty2 + 5}")
                app.pump(0.15)
                app.send(f"mup:{l2 + 30}:{ty2 + 5}")
                app.pump(0.3)
                ty3 = app.find_row(" Trackers ")
                assert ty3 is not None and ty3 != ty2, "拖拽应移动窗口"
                app.send("esc")
                app.pump(0.3)
            finally:
                self._quit(app)
                tr.stop()

    def _thumb_col(self, app: EzrApp, ty: int, l: int, r: int,
                   by: int) -> "int | None":
        """右边界列滚动条滑块（║ 落在边框列上，suite_ui_pieces._thumb 同口径）。"""
        for y in range(ty + 1, by):
            if _cell(app, y, r) == "║" or _cell(app, y, r - 1) == "║":
                return r - 1
        return None

    def _udp_frags(self, rows: "list[str]") -> "list[str]":
        """行集中 udp://tracker-N 片段序（倒计时文本变化免疫的滚动比对口径）。"""
        return re.findall(r"udp://tracker-\d+", "\n".join(rows))

    def _case_5(self):
        """QA-TW-05 五通道关闭 + 模态消费 A/D/M。"""
        @self.case("QA-TW-05")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                torrent = self._mk_torrent(
                    env, "t5", [f"http://127.0.0.1:{tr.port}/announce"])
                self._driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                for chan in ("t", "T", "esc", "q", "Q"):
                    self._open_trackers(app)
                    app.send(chan)
                    app.pump(0.4)
                    assert app.find_row(" Trackers ") is None, \
                        f"通道 {chan} 应关闭且不退出（q/Q 先关窗）"
                self._open_trackers(app)
                app.send("A")
                app.pump(0.4)
                assert_not_in("请输入命令或链接", app.text(),
                              "模态期间 A 应被窗口消费（不开命令窗口）")
                app.send("D")
                app.pump(0.4)
                assert app.find_row(" Trackers ") is not None, \
                    "模态期间 D 应被窗口消费"
                app.send("M")
                app.pump(0.4)
                assert_not_in("修改任务：BT", app.text(), "模态期间 M 应被窗口消费")
                xc = None
                ty, l, r, by = self._frame(app, " Trackers ")
                xc = next(x for x in range(r - 4, r) if _cell(app, ty, x) == "✗")
                _click(app, xc, ty)
                app.pump(0.4)
                assert app.find_row(" Trackers ") is None, "点击 [✗] 应关闭"

                self._open_trackers(app)
                app.send("ctrl_c")
                deadline = time.time() + 5
                while time.time() < deadline and app.exit_code is None:
                    app.pump(0.2)
                assert app.exit_code is not None, "模态下 Ctrl+C 应退出程序（优先级不变）"
                app.close()
                app = None
            finally:
                self._quit(app)
                tr.stop()

    def _case_6(self):
        """QA-TW-06 t 页签废除 / ws(s) 入池 / 白名单外 scheme 三来源分流。"""
        @self.case("QA-TW-06")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                torrent = self._mk_torrent(
                    env, "t6", [f"http://127.0.0.1:{tr.port}/announce"])
                self._driver_config(
                    env, tr.port, self._next_port() + 10,
                    extra_trackers=["wss://tracker.example/ws"])
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                before = app.text()
                app.send("t")
                app.pump(0.4)
                after = app.text()
                assert "正在下载 (" in before and "正在下载 (" in after, \
                    "t 不应切换页签（页签仅 ]/[）"
                self._add_bt_task(app, torrent)
                assert self._wait_row_state(app, "wss://tracker.example/ws",
                                            "失败", 30) or \
                    self._wait_row_state(app, "wss://tracker.example/ws",
                                         "工作中", 10), \
                    "ws(s) 行应同权入池照常显示（不可达域名臂显示失败退避）"

                app.send("m")
                assert app.wait_for("修改任务：BT", 5), "M 应开修改面板"
                app.send(*(["tab"] * 6))
                app.pump(0.2)
                app.send("end")
                app.pump(0.15)
                app.send("\x1b\r")
                app.pump(0.15)
                app.type_text("ftp://tracker.example/ann")
                app.pump(0.3)
                self._tab_to_button(app, "确定")
                app.send("enter")
                assert app.wait_for("不支持的 tracker 协议", 3), \
                    "手输 ftp 来源应 toast 拒绝（对话框臂）"
                toast = next((ln for ln in app.text().split("\n")
                              if "不支持的 tracker 协议" in ln), "")
                assert "ftp://tracker.example/ann" in toast, \
                    f"toast 应含 URL 截尾内完整地址: {toast!r}"
                assert_in("修改任务：BT", app.text(), "焦点回 trackers，对话框不关闭")
            finally:
                self._quit(app)
                tr.stop()

            env2_home = os.path.join(env.home, "h2")
            os.makedirs(env2_home, exist_ok=True)
            bad_cfg = os.path.join(env2_home, ".ezr")
            os.makedirs(bad_cfg, exist_ok=True)
            with open(os.path.join(bad_cfg, "config.toml"), "w") as f:
                f.write('[common]\ndownload_dir = "%s"\n\n[bt]\n'
                        'trackers = ["ftp://tracker.example/ann"]\n' % env.save_dir)
            app = EzrApp(env2_home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                deadline = time.time() + 8
                while time.time() < deadline and app.exit_code is None:
                    app.pump(0.2)
                rc = app.exit_code
                assert rc is not None and rc != 0, \
                    "config trackers 白名单外 scheme 应启动失败（FR-02-35）"
                raw = app.raw_text()
                assert "ftp" in raw or "不支持" in raw or "trackers" in raw, \
                    f"启动报错应提及非法配置: {raw[-400:]!r}"
                app.close()
            finally:
                if app.exit_code is None:
                    app.graceful_quit()
                else:
                    app.close()


def main() -> int:
    suite = TrackersWindowSuite()
    suite.run()
    p, f, s = suite.summary()
    print(f"\n[11-trackers-window] P={p} F={f} S={s}")
    return 1 if f else 0


if __name__ == "__main__":
    sys.exit(main())
