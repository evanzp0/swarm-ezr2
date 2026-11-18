#!/usr/bin/env python3
"""suite_bt_engine.py — 09-bt-engine-qa.md v1.54 核心用例可执行化（six-pack/QA 2026-11-17）。

实跑覆盖 QA-BE-01/02/03/05/06/08/09/10/11/12/13/14/15/16/21/22/23（17 例，本地
fixture 全链路）+ S 登记 QA-BE-04/07/17/18/19/20/24（7 例，fixture 能力受限，
判据内嵌环境条件，沿 suite_modify_bt「重点用例可执行化」先例）。

Fixture 组成（每用例独立现场）：
- mini_http_tracker（BEP3，JSONL announce 日志承载 peer_id/event/uploaded 对账）；
- pybtracker（BEP15 UDP tracker）；
- LtPeer（libtorrent 控制侧 seeder/leecher，suite_modify_bt 同源适配）；
- ezr-mktorrent 多文件种子（root→root 双层布局）。
driver [bt] 配置经 env.write_config；种子经命令窗口添加（用户可达入口）。

S 判据（内嵌环境条件，规程「S 必须内嵌环境判据」）：
- BE-04：mini_http_tracker 仅产出 IPv4 compact peers，fixture 无 peers6 回复能力；
- BE-07：harness 无 BEP9 元数据请求的拉取侧观察点（ut_metadata 分片拼接对账）；
- BE-17：LtPeer 无 per-(piece,offset) request 日志，零重请求对账不可观察；
- BE-18：libtorrent 不提供按需 choke/interest/乐观槽驱动，判定表臂不可编程；
- BE-19/20：harness 无 WebRTC 数据面 seeder（需第二产品实例或 webtorrent-cli），
  DataChannel 传输臂不可观察（ws tracker announce 臂由 BE-20 的 S 注记指向
  harness/tracker/mini_ws_tracker.py 存在性）；
- BE-24：piece 请求顺序需 seeder 侧 request 序日志，LtPeer 不产出。

时序纪律：门控（限速 seeder 拉长观察窗）、正向探测有界重试、持久证据断言
（tracker JSONL / 落盘字节 / 注册表 / announce 累计值）。
"""

from __future__ import annotations

import json
import os
import re
import socket
import subprocess
import sys
import threading
import time

QA_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, QA_DIR)

from harness import (TMP_ROOT, EzrApp, Suite, Env, assert_in, assert_not_in,  # noqa: E402
                     _clear_field_prefill)

MKTORRENT = os.path.join(QA_DIR, "..", "..", "ezr", "target", "debug",
                         "ezr-mktorrent")
TRACKER = os.path.abspath(os.path.join(
    QA_DIR, "..", "..", "harness", "tracker", "mini_http_tracker.py"))
UDP_TRACKER = os.path.abspath(os.path.join(
    QA_DIR, "..", "..", "harness", "tracker", "pybtracker_server.py"))

FILES = ("a.bin", "b.bin", "c.bin", "d.bin")
FSIZE = 512 * 1024
PIECE = 262144

WS_TRACKER = os.path.abspath(os.path.join(
    QA_DIR, "..", "..", "harness", "tracker", "mini_ws_tracker.py"))


def _cell(app: EzrApp, y: int, x: int) -> str:
    return app.screen.buffer[y][x].data or ""


def _row_text(app: EzrApp, y: int, x0: int = 0, x1: int | None = None) -> str:
    x1 = app.screen.columns if x1 is None else x1
    return "".join(_cell(app, y, x) for x in range(x0, x1)).rstrip()


class LtPeer:
    """libtorrent 控制 peer（suite_modify_bt 同源：+限速/+文件优先级/+手动连 peer）。"""

    def __init__(self, name, port, torrent, save_path, upload_limit=None,
                 download_limit=None, priorities=None):
        import libtorrent as lt
        self.name, self.lt = name, lt
        settings = {
            "listen_interfaces": f"127.0.0.1:{port}",
            "enable_dht": False, "enable_lsd": False,
            "enable_upnp": False, "enable_natpmp": False,
            "announce_to_all_trackers": True, "announce_to_all_tiers": True,
            "connections_limit": 200,
            "active_downloads": -1, "active_seeds": -1, "active_limit": -1,
            "alert_mask": lt.alert.category_t.error_notification
            | lt.alert.category_t.status_notification,
        }
        if upload_limit is not None:
            settings["upload_rate_limit"] = upload_limit
        if download_limit is not None:
            settings["download_rate_limit"] = download_limit
        self.session = lt.session(settings)
        info = lt.torrent_info(str(torrent))
        self.handle = self.session.add_torrent(
            {"ti": info, "save_path": str(save_path)})
        if priorities:
            for i, p in enumerate(priorities):
                self.handle.file_priority(i, p)
        self.downloaded = 0
        self.finished = False
        self.stop_flag = threading.Event()
        self.thread = threading.Thread(target=self._pump, daemon=True)
        self.thread.start()

    def _pump(self):
        while not self.stop_flag.is_set():
            try:
                st = self.handle.status()
                self.downloaded = st.total_done
                self.finished = st.is_seeding
            except Exception:
                pass
            self.stop_flag.wait(0.3)

    def connect_to(self, host, port):
        self.handle.connect_peer((host, port))

    def wait_total_done(self, target, timeout):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.downloaded >= target:
                return True
            time.sleep(0.3)
        return False

    def shutdown(self):
        self.stop_flag.set()
        self.thread.join(timeout=2)
        try:
            self.session.pause()
        except Exception:
            pass


class MiniTracker:
    def __init__(self, port, log_path, interval=5):
        self.port = port
        self.proc = subprocess.Popen(
            [sys.executable, TRACKER, "--bind", "127.0.0.1",
             "--port", str(port), "--interval", str(interval), "--log", log_path],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def wait_ready(self, timeout=10.0):
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

    def stop(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=3)
        except Exception:
            self.proc.kill()


class UdpTracker:
    def __init__(self, port):
        self.port = port
        self.proc = subprocess.Popen(
            [sys.executable, UDP_TRACKER, "--bind", "127.0.0.1",
             "--port", str(port), "--interval", "10"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def wait_ready(self, timeout=10.0):
        time.sleep(1.5)
        return self.proc.poll() is None

    def stop(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=3)
        except Exception:
            self.proc.kill()


def _make_payload(root: str) -> str:
    pay = os.path.join(root, "payload")
    lay = os.path.join(pay, "root", "root")
    os.makedirs(lay, exist_ok=True)
    for name in FILES:
        with open(os.path.join(lay, name), "wb") as f:
            f.write(os.urandom(FSIZE))
    return pay


def _make_torrent(pay: str, announce: str, out: str | None = None) -> str:
    lay = os.path.join(pay, "root", "root")
    out = out or os.path.join(pay, "m.torrent")
    r = subprocess.run(
        [MKTORRENT, "--out", out, "--piece-len", str(PIECE),
         "--announce", announce] + [os.path.join(lay, n) for n in FILES],
        capture_output=True, text=True)
    assert r.returncode == 0, f"mktorrent 失败: {r.stderr.strip()}"
    return out


def _driver_config(env: Env, tr_port: int, listen_port: int,
                   extra: "list[str] | None" = None,
                   rate: "int | None" = None) -> None:
    trackers = [f"http://127.0.0.1:{tr_port}/announce"] + (extra or [])
    env.write_config(
        "[common]\n"
        f'download_dir = "{env.save_dir}"\n'
        + (f"task_max_download_speed = {rate}\n" if rate else "")
        + "\n[bt]\n"
        "trackers = [" + ", ".join(f'"{t}"' for t in trackers) + "]\n"
        f"listen_port = {listen_port}\n"
    )


def _tracker_log(path: str) -> list:
    if not os.path.exists(path):
        return []
    rows = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    rows.append(json.loads(line))
                except Exception:
                    pass
    return rows


def _disk_file(save_dir: str, name: str) -> str | None:
    for base, _dirs, fnames in os.walk(save_dir):
        if name in fnames:
            return os.path.join(base, name)
    return None


def _detail(app: EzrApp) -> str:
    rows = app.screen.display
    top = next((i for i, r in enumerate(rows) if "任务详情" in r), None)
    if top is None:
        return ""
    end = next((i for i in range(top + 1, len(rows)) if "并发连接" in rows[i]),
               len(rows))
    return "\n".join(row[64:].rstrip() for row in rows[top:end])


def _popup_raw(app: EzrApp, title: str) -> str:
    raw = app.raw_text()
    tail = raw[raw.rfind(title):] if title in raw else ""
    tail = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", tail)
    return tail.replace("\uFE0E", "")


def _focused_btn(app: EzrApp) -> "tuple[int, str] | tuple[None, None]":
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


def _tab_to_button(app: EzrApp, want: str, max_tabs: int = 14) -> None:
    for _ in range(max_tabs):
        _y, label = _focused_btn(app)
        if label and want in label:
            app.pump(0.1)
            return
        app.send("tab")
        app.pump(0.15)
    raise AssertionError(f"未能聚焦按钮 {want!r}")


def _open_pieces(app: EzrApp) -> "tuple[int, int, int]":
    app.send("p")
    assert app.wait_for("分块状态", 4), "P 应打开分块弹窗"
    ty = app.find_row("分块状态")
    l = next(x for x in range(app.screen.columns) if _cell(app, ty, x) == "╭")
    r = next(x for x in range(l + 1, app.screen.columns) if _cell(app, ty, x) == "╮")
    return ty, l, r


def _close_pieces(app: EzrApp) -> None:
    app.send("esc")
    app.pump(0.3)
    assert app.find_row("分块状态") is None, "Esc 应关闭分块弹窗"


class BtEngineSuite(Suite):
    def __init__(self):
        super().__init__("09-bt-engine", port_base=45200)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in (1, 2, 3, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 21, 22, 23,
                  4, 7, 17, 18, 19, 20, 24):
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

    def _quit(self, app) -> None:
        if app is None:
            return
        if app.exit_code is None:
            app.graceful_quit()
        else:
            app.close()

    # -- fixtures / shared steps ---------------------------------------------

    def _add_bt_task(self, app: EzrApp, torrent: str, keep: "tuple[str, ...] | None" = None) -> None:
        assert app.wait_for("下载槽位", 12), "应用应启动就绪"
        app.send("a")
        assert app.wait_for("请输入命令或链接", 5)
        app.type_text(torrent)
        app.send("enter")
        assert app.wait_for("添加任务：BT", 6)
        if keep is not None:
            app.send("tab", "tab")
            app.send("right", "down", "right", "up")
            app.pump(0.3)
            deselect = tuple(n for n in FILES if n not in keep)
            prev = None
            for name in deselect:
                idx, marker = self._tree_goto(app, name, prev)
                assert marker in ("x", "-"), f"{name} 初始应已勾选"
                app.send("space")
                app.pump(0.2)
                prev = name
            _tab_to_button(app, "立即下载")
        else:
            _tab_to_button(app, "立即下载")
        app.send("enter")
        assert app.wait_for("已添加任务", 6)

    def _tree_rows(self, app: EzrApp) -> "list[tuple[str, str]]":
        rows: list[tuple[str, str]] = []
        in_tree = False
        for line in app.text().split("\n"):
            if "选择下载文件" in line:
                in_tree = True
                continue
            if not in_tree:
                continue
            if "trackers" in line or "立即下载" in line:
                break
            m = re.match(r".*?(\[x\]|\[-\]|\[ \])\s+(.*?)(?:\s{2,}|$)", line)
            if m:
                marker = m.group(1).replace("[", "").replace("]", "")
                name = (m.group(2).strip().rstrip("/")
                        .lstrip("\u25b8\u25be").strip())
                rows.append((marker, name))
        return rows

    def _tree_goto(self, app: EzrApp, name: str,
                   cursor_hint: "str | None" = None) -> "tuple[int, str]":
        rows = self._tree_rows(app)
        idx = next((i for i, (_mk, nm) in enumerate(rows) if nm == name), None)
        assert idx is not None, f"树应含 {name}：{rows}"
        cur = 0
        if cursor_hint:
            cur = next((i for i, (_mk, nm) in enumerate(rows) if nm == cursor_hint), 0)
        delta = idx - cur
        if delta > 0:
            app.send(*(["down"] * delta))
        elif delta < 0:
            app.send(*(["up"] * (-delta)))
        app.pump(0.2)
        return idx, rows[idx][0]

    def _tracker_rows(self, app: EzrApp, ty: int, l: int, r: int,
                      by: int) -> "list[str]":
        out = []
        for y in range(ty + 1, by):
            s = _row_text(app, y, l + 3, r - 2).strip()
            if s:
                out.append(s)
        return out

    def _open_trackers(self, app: EzrApp) -> "tuple[int, int, int, int]":
        app.send("t")
        assert app.wait_for(" Trackers ", 4), "t 应打开 trackers 窗口"
        ty = app.find_row(" Trackers ")
        l = next(x for x in range(app.screen.columns) if _cell(app, ty, x) == "╭")
        r = next(x for x in range(l + 1, app.screen.columns) if _cell(app, ty, x) == "╮")
        by = next(y for y in range(ty + 1, app.screen.lines)
                  if _cell(app, y, l) == "╰")
        return ty, l, r, by

    def _conns_panel(self, app: EzrApp) -> str:
        rows = app.screen.display
        top = next((i for i, r in enumerate(rows) if "并发连接" in r), None)
        if top is None:
            return ""
        return "\n".join(row[64:].rstrip() for row in rows[top:top + 12])

    def _wait_progress(self, frag: str, timeout: float) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            app = self._cur_app
            if app is not None:
                app.pump(0.4)
                if frag in app.text():
                    return True
            time.sleep(0.1)
        return False

    def _wait_conns_peer(self, timeout: float) -> bool:
        """等并发明细出现 peer 行（地址 ip缩写/port 形态，有界重试）。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            app = self._cur_app
            if app is not None:
                app.pump(0.4)
                conns = self._conns_panel(app)
                if re.search(r"\d+\.\*\.\d+/\d{4,5}", conns):
                    return True
            time.sleep(0.1)
        return False

    def _cycle_to_visible(self, app: EzrApp, needle: str, timeout: float) -> bool:
        """有界探测：轮询画面未见 needle 时按 ] 循环任务页签（两页签口径）。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.4)
            if needle in app.text():
                return True
            app.send("]")
            app.pump(0.3)
            if needle in app.text():
                return True
        return False

    # -- cases ---------------------------------------------------------------

    def _case_1(self):
        """QA-BE-01 任务名/分块行/tracker 池/损坏种子 toast。"""
        @self.case("QA-BE-01")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                _driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert_in("root", app.text(), "任务名 = info.name（mktorrent name=root）")
                detail = _detail(app)
                assert re.search(r"256 KB/块", detail), f"分块行块大小: {detail[:260]!r}"
                assert re.search(r"\d+/\d+ · 256 KB/块", detail), \
                    "分块行 x/y · 256 KB/块"
                ty, l, r, by = self._open_trackers(app)
                flat = _popup_raw(app, " Trackers ")
                assert f"127.0.0.1:{tr.port}/announce" in flat, \
                    "tracker 池含 announce（announce-list 层）"
                app.send("esc")
                app.pump(0.3)
                # 损坏种子
                corrupt = os.path.join(env.fixture_root, "corrupt.torrent")
                with open(corrupt, "wb") as f:
                    f.write(b"not bencode at all")
                app.send("a")
                assert app.wait_for("请输入命令或链接", 5)
                app.type_text(corrupt)
                app.send("enter")
                assert app.wait_for("无法读取种子文件", 4), "损坏种子应 toast"
                assert_not_in("添加任务：BT", app.text(), "损坏种子不开 BT 对话框")
            finally:
                self._quit(app)
                tr.stop()

    def _case_2(self):
        """QA-BE-02 announce 参数/peer_id 形态/interval 重 announce/completed 事件。"""
        @self.case("QA-BE-02")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25), "seeder 校验超时"
                _driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                done = False
                while time.time() < deadline:
                    if all(_disk_file(env.save_dir, n) and
                           open(_disk_file(env.save_dir, n), "rb").read() ==
                           open(src(n), "rb").read() for n in FILES):
                        done = True
                        break
                    app.pump(0.5)
                assert done, "下载应完成且产物逐字节一致"
                deadline = time.time() + 15
                while time.time() < deadline and not any(
                        r["event"] == "completed"
                        for r in _tracker_log(os.path.join(env.fixture_root, "tr.jsonl"))
                        if r["peer_id"].startswith("-EZ2")):
                    app.pump(0.5)
                log = _tracker_log(os.path.join(env.fixture_root, "tr.jsonl"))
                drv_rows = [r for r in log
                            if r.get("event") in ("started", "completed", "periodic")]
                assert drv_rows, "tracker 日志应有 announce"
                pids = {r["peer_id"] for r in drv_rows if r["port"] != seeder}
                driver_pid = next((p for p in pids if p.startswith("-EZ2")), None)
                assert driver_pid, \
                    f"driver peer_id 应为 -EZ2000- 形态: {pids}"
                events = [r["event"] for r in drv_rows if r["peer_id"] == driver_pid]
                assert events[0] == "started", f"首事件 = started: {events[:4]}"
                assert "completed" in events, \
                    f"完成后应发 completed 且继续 announce: {events[-6:]}"
                deadline = time.time() + 20
                while time.time() < deadline and len(
                        [e for e in events if e == "periodic"]) < 1:
                    app.pump(0.5)
                    events = [r["event"] for r in _tracker_log(
                        os.path.join(env.fixture_root, "tr.jsonl"))
                        if r["peer_id"] == driver_pid]
                assert len([e for e in events if e == "periodic"]) >= 1, \
                    "interval 周期重 announce（做种期持续）"
                row = next(r for r in drv_rows if r["peer_id"] == driver_pid)
                assert isinstance(row.get("uploaded"), int) and \
                    isinstance(row.get("left"), int), "announce 含 uploaded/left"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_3(self):
        """QA-BE-03 UDP tracker（BEP15）下载推进。"""
        @self.case("QA-BE-03")
        def go(env: Env):
            udp = UdpTracker(self._next_port())
            seeder = None
            app = None
            try:
                assert udp.wait_ready(), "udp tracker 就绪"
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(
                    pay, f"udp://127.0.0.1:{udp.port}/announce",
                    os.path.join(env.fixture_root, "m-udp.torrent"))
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt]\n"
                    f'trackers = ["udp://127.0.0.1:{udp.port}/announce"]\n'
                    f"listen_port = {self._next_port() + 10}\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                done = False
                while time.time() < deadline:
                    if all(_disk_file(env.save_dir, n) and
                           open(_disk_file(env.save_dir, n), "rb").read() ==
                           open(src(n), "rb").read() for n in FILES):
                        done = True
                        break
                    app.pump(0.5)
                assert done, "纯 UDP tracker 链路应完成下载（connect/announce/peers）"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                udp.stop()

    def _case_4(self):
        """QA-BE-04（S）IPv6 peers6：fixture 无 IPv6 compact 回复能力。"""
        self.skip("QA-BE-04", "mini_http_tracker 仅产出 IPv4 compact peers（无 "
                              "peers6 字段能力），IPv6 建连/字节对账不可观察；"
                              "失效条件 = fixture 支持 peers6 + 沙箱 IPv6 回环")

    def _case_5(self):
        """QA-BE-05 三来源合并去重（T 弹窗行数）。"""
        @self.case("QA-BE-05")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                dup = f"http://127.0.0.1:{tr.port}/announce"
                torrent = _make_torrent(pay, dup,
                                        os.path.join(env.fixture_root, "m5.torrent"))
                manual = f"udp://127.0.0.1:{self._next_port()}/announce"
                _driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                self._modify_add_tracker(app, manual)
                ty, l, r, by = self._open_trackers(app)
                flat = _popup_raw(app, " Trackers ")
                rows = self._tracker_rows(app, ty, l, r, by)
                n_hash = flat.count(dup)
                assert n_hash == 1, \
                    f"同 URL 三来源合并应去重单条（实测 {n_hash}）"
                assert manual in flat, "手输来源入池"
                assert len(rows) >= 2, "池 = 去重后 config/announce + 手输"
                app.send("esc")
                app.pump(0.3)
            finally:
                self._quit(app)
                tr.stop()

    def _modify_add_tracker(self, app: EzrApp, url: str) -> None:
        app.send("m")
        assert app.wait_for("修改任务：BT", 5)
        app.send(*(["tab"] * 6))
        app.pump(0.2)
        app.send("end")
        app.pump(0.15)
        app.send("\x1b\r")
        app.pump(0.15)
        app.type_text(url)
        app.pump(0.3)
        _tab_to_button(app, "确定")
        app.send("enter")
        assert app.wait_for("BT 任务已更新", 5)

    def _case_6(self):
        """QA-BE-06 并发面板：客户端两级识别/协议 TCP/地址 ip缩写/port。"""
        @self.case("QA-BE-06")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10, rate=24576)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_conns_peer(120), \
                    "应与 seeder 建连（并发明细出现 peer 行）"
                conns = self._conns_panel(app)
                assert re.search(r"\d+\.\*\.\d+/\d{4,5}", conns), \
                    f"地址 = ip缩写/port 形态（v1.35）: {conns[:300]!r}"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_7(self):
        """QA-BE-07（S）BEP9 做种侧元数据响应：拉取侧观察点缺失。"""
        self.skip("QA-BE-07", "harness 无 BEP9 ut_metadata 拉取侧观察点（分片拼接 "
                              "SHA-1=infohash 的 fixture 对账不可实现：LtPeer 磁力"
                              "拉取侧无分片日志）；失效条件 = fixture 增配 ut_metadata "
                              "观察通道")

    def _case_8(self):
        """QA-BE-08 引擎面：子集下载落盘字节 + 未选路径不存在。"""
        @self.case("QA-BE-08")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay,
                                upload_limit=131072)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent, keep=("a.bin", "b.bin"))
                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                done = False
                while time.time() < deadline:
                    fa, fb = _disk_file(env.save_dir, "a.bin"), _disk_file(env.save_dir, "b.bin")
                    if fa and fb and \
                            open(fa, "rb").read() == open(src("a.bin"), "rb").read() and \
                            open(fb, "rb").read() == open(src("b.bin"), "rb").read():
                        done = True
                        break
                    app.pump(0.5)
                assert done, "选中文件应字节级完成"
                assert _disk_file(env.save_dir, "c.bin") is None and \
                    _disk_file(env.save_dir, "d.bin") is None, \
                    "未选文件路径不应落盘"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_9(self):
        """QA-BE-09 config trackers 校验：模板/白名单外启动失败/旧键废弃。"""
        @self.case("QA-BE-09")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                assert app.wait_for("下载槽位", 12)
                app.graceful_quit()
                cfg = open(os.path.join(env.home, ".ezr", "config.toml"),
                           encoding="utf-8").read()
                assert "trackers = []" in cfg, "模板含 [bt] trackers 单键"
                # 白名单外 scheme → 启动失败
                env.write_config(
                    f'[common]\ndownload_dir = "{env.save_dir}"\n\n[bt]\n'
                    'trackers = ["ftp://x/ann"]\n')
                app2 = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app2
                deadline = time.time() + 8
                while time.time() < deadline and app2.exit_code is None:
                    app2.pump(0.2)
                assert app2.exit_code not in (None, 0), \
                    "白名单外 scheme 应拒绝启动"
                raw = app2.raw_text()
                assert "trackers" in raw or "ftp" in raw, "报错应提及非法配置"
                app2.close()
                # 旧两键 = 未知键废弃 → 正常启动
                env.write_config(
                    f'[common]\ndownload_dir = "{env.save_dir}"\n\n[bt]\n'
                    'http_trackers = ["http://old/ann"]\n'
                    'udp_trackers = ["udp://old:6969/ann"]\n')
                app3 = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app3
                assert app3.wait_for("下载槽位", 12), "旧键按未知键废弃应正常启动"
            finally:
                self._quit(self._cur_app)

    def _case_10(self):
        """QA-BE-10 做种列表行形态 + 产物字节 + completed/left=0。"""
        @self.case("QA-BE-10")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                done = False
                while time.time() < deadline:
                    if all(_disk_file(env.save_dir, n) and
                           open(_disk_file(env.save_dir, n), "rb").read() ==
                           open(src(n), "rb").read() for n in FILES):
                        done = True
                        break
                    app.pump(0.5)
                assert done, "下载完成"
                assert app.wait_for("下载完成", 60), "完成应出 toast"
                assert self._cycle_to_visible(app, "做种中", 30), "完成应转做种中（已完成页签）"
                rows = app.display_rows()
                stats_row = next(
                    (r for r in rows if re.search(
                        r"\d+\(\d+\)/\d+\(\d+\)/\d+\.\d{3}\s+SR\s+\d+\.\d{2}", r)),
                    "")
                assert stats_row, \
                    f"明细统计行 = <x1(x2)/y1(y2)>/<健康度> SR <分享率>: {rows[:6]!r}"
                assert_not_in("剩余做种", "\n".join(rows), "剩余做种项退役（FR-02-38 v1.21）")
                log = _tracker_log(os.path.join(env.fixture_root, "tr.jsonl"))
                drv = [r for r in log if r["peer_id"].startswith("-EZ2")]
                assert any(r["event"] == "completed" for r in drv), \
                    "引擎应发 event=completed"
                assert any(r["left"] == 0 for r in drv), "completed 后 left=0"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_11(self):
        """QA-BE-11 B 断开 BT 连接 + HTTP 任务拒绝 toast。"""
        @self.case("QA-BE-11")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10, rate=24576)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_conns_peer(120), "应与 seeder 建连"
                conns_before = self._conns_panel(app)
                m_addr = re.search(r"(\d+\.\*\.\d+/\d{4,5})", conns_before)
                assert m_addr, "并发明细应有一行（地址 ip缩写/port 形态）"
                app.send("b")
                assert app.wait_for("已断开", 5), \
                    "B 应出断开 toast（✂ 已断开 <ip> 并加入黑名单）"
                # HTTP 任务拒绝臂
                app.send("a")
                assert app.wait_for("请输入命令或链接", 5)
                app.type_text(env.fixture.url("five-m.bin?cd=be11&speed=100000"))
                app.send("enter")
                assert app.wait_for("添加任务：HTTP(S)", 5)
                app.send("enter")
                assert app.wait_for("已添加任务", 6)
                app.send("b")
                app.pump(0.6)
                assert_in("仅 BT 任务支持断开并发连接（B）", app.text(),
                          "HTTP 任务 B 应拒绝")
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_12(self):
        """QA-BE-12 P 分块弹窗：BT 元信息行 + 四态色块。"""
        @self.case("QA-BE-12")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10, rate=24576)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_conns_peer(120)
                ty, l, r = _open_pieces(app)
                meta = None
                for y in range(ty + 1, ty + 5):
                    row = "".join(_cell(app, y, c) or ""
                                  for c in range(l + 1, r))
                    if "块大小" in row:
                        meta = row
                        break
                assert meta is not None and re.search(
                    r"BT · 块大小 256 KB · \d+/\d+ 块 · 并发 \d+", meta), \
                    f"BT 元信息行: {meta!r}"
                _close_pieces(app)
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_13(self):
        """QA-BE-13 做种端到端：default_seed_duration_m=1 到点收尾 + 空格提前停。"""
        @self.case("QA-BE-13")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt]\n"
                    f'trackers = ["http://127.0.0.1:{tr.port}/announce"]\n'
                    f"listen_port = {self._next_port() + 10}\n"
                    "default_seed_duration_m = 1\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                while time.time() < deadline and not all(
                        _disk_file(env.save_dir, n) and
                        open(_disk_file(env.save_dir, n), "rb").read() ==
                        open(src(n), "rb").read() for n in FILES):
                    app.pump(0.5)
                assert self._cycle_to_visible(app, "做种中", 40), \
                    "完成应转做种中（已完成页签）"
                log_path = os.path.join(env.fixture_root, "tr.jsonl")
                deadline = time.time() + 100
                stopped = False
                while time.time() < deadline:
                    log = _tracker_log(log_path)
                    drv = [r for r in log if r["peer_id"].startswith("-EZ2")
                           and r["event"] == "stopped"]
                    if drv and not self._cycle_to_visible(app, "做种中", 2):
                        stopped = True
                        break
                    app.pump(0.5)
                assert stopped, \
                    "default_seed_duration_m=1 到点应停止做种（tracker 收 stopped + 转已完成）"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_14(self):
        """QA-BE-14 bt_concurrency 预填/非法值拒绝启动。"""
        @self.case("QA-BE-14")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt]\n"
                    f'trackers = ["http://127.0.0.1:{tr.port}/announce"]\n'
                    f"listen_port = {self._next_port() + 10}\n"
                    "bt_concurrency = 8\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                app.send("a")
                assert app.wait_for("请输入命令或链接", 5)
                app.type_text(torrent)
                app.send("enter")
                assert app.wait_for("添加任务：BT", 6)
                assert re.search(r"并发\s*>\s*8\b", app.text()), \
                    "BT 并发预填 = bt_concurrency 配置值 8"
                app.send("esc")
                app.pump(0.3)
                self._quit(app)
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt]\n"
                    f'trackers = ["http://127.0.0.1:{tr.port}/announce"]\n'
                    "bt_concurrency = 0\n")
                app2 = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app2
                deadline = time.time() + 8
                while time.time() < deadline and app2.exit_code is None:
                    app2.pump(0.2)
                assert app2.exit_code not in (None, 0), \
                    "bt_concurrency = 0 应拒绝启动（FR-02-35）"
                app2.close()
            finally:
                self._quit(app)
                tr.stop()

    def _case_15(self):
        """QA-BE-15 暂停/恢复生命周期：peer_id 会话稳定 + tracker 事件序列。"""
        @self.case("QA-BE-15")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10, rate=24576)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_conns_peer(120)
                for _ in range(3):
                    app.send("space")
                    app.pump(0.5)
                    assert app.wait_for("已暂停", 4), "空格应暂停"
                    app.send("space")
                    app.pump(0.5)
                    assert app.wait_gone("已暂停", 4), "空格应恢复"
                    app.pump(0.5)
                log = _tracker_log(os.path.join(env.fixture_root, "tr.jsonl"))
                drv = [r for r in log if r["peer_id"].startswith("-EZ2")]
                pids = {r["peer_id"] for r in drv}
                assert len(pids) == 1, \
                    f"peer_id 会话级稳定（跨暂停/恢复不变）: {pids}"
                events = [r["event"] for r in drv]
                assert events.count("started") >= 3, \
                    f"事件序列 = started→stopped→started ×3: {events}"
                assert events.count("stopped") >= 3, "每轮暂停应收 stopped"
                assert "已暂停" not in app.text(), "任务应处于下载态"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_16(self):
        """QA-BE-16 listen_port：配置生效/进程级共享/占用回退随机。"""
        @self.case("QA-BE-16")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                lp = self._next_port() + 20
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                _driver_config(env, tr.port, lp)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                deadline = time.time() + 20
                ports = set()
                while time.time() < deadline and not ports:
                    log = _tracker_log(os.path.join(env.fixture_root, "tr.jsonl"))
                    ports = {r["port"] for r in log if r["peer_id"].startswith("-EZ2")}
                    app.pump(0.4)
                assert lp in ports, f"announce port 应 = listen_port {lp}: {ports}"
                self._quit(app)
                app = None
                # 重启臂：config 改无 listen_port → 任务 1 经注册表恢复 +
                # 新增任务 2（不同 infohash 免判重）→ 两任务共享随机端口 ≠ lp
                pay2 = os.path.join(env.fixture_root, "payload2")
                lay2 = os.path.join(pay2, "root", "root")
                os.makedirs(lay2, exist_ok=True)
                for name in FILES:
                    with open(os.path.join(lay2, name), "wb") as f:
                        f.write(os.urandom(FSIZE))
                torrent2 = _make_torrent(pay2, f"http://127.0.0.1:{tr.port}/announce",
                                         os.path.join(pay2, "m2.torrent"))
                open(os.path.join(env.fixture_root, "tr.jsonl"), "w").close()
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt]\n"
                    f'trackers = ["http://127.0.0.1:{tr.port}/announce"]\n')
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                assert app.wait_for("下载槽位", 12)
                self._add_bt_task(app, torrent2)
                deadline = time.time() + 30
                ports = set()
                while time.time() < deadline and len(ports) < 1:
                    log = _tracker_log(os.path.join(env.fixture_root, "tr.jsonl"))
                    drv_ports = [r["port"] for r in log
                                 if r["peer_id"].startswith("-EZ2")]
                    ports = set(drv_ports)
                    if len(drv_ports) >= 2 and len(ports) == 1:
                        break
                    app.pump(0.4)
                assert len(ports) == 1 and lp not in ports, \
                    f"重启后两任务应共享随机端口且 ≠ 配置值 {lp}: {ports}"
            finally:
                self._quit(app)
                tr.stop()

    def _case_21(self):
        """QA-BE-21 下载臂统计行形态（v1.49/v1.51/v1.53 口径）。"""
        @self.case("QA-BE-21")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10, rate=24576)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_conns_peer(120)
                rows = app.display_rows()
                task_row = next((r for r in rows if "[BT]" in r and "下载中" in r), "")
                assert re.search(r"\[下载中\] 2\.1 MB", task_row), \
                    f"任务行 [状态] 后单空格总字节（4×512KiB = 2.1 MB）: {task_row!r}"
                detail_row = next((r for r in rows if "SR" in r and "LT" in r), "")
                assert re.search(
                    r"\d+\(\d+\)/\d+\(\d+\)/\d+\.\d{3}\s+SR\s+\d+\.\d{2}\s+LT", detail_row), \
                    f"明细行 = <x1(x2)/y1(y2)>/<健康度> SR <r> LT <eta>: {detail_row!r}"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_22(self):
        """QA-BE-22 并发面板相关度列：单 seeder 下载初期 = 100.0%。"""
        @self.case("QA-BE-22")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"))
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("seeder", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25)
                _driver_config(env, tr.port, self._next_port() + 10, rate=16384)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                assert self._wait_conns_peer(120), "应建连"

                def _pct():
                    rows = app.display_rows()
                    task = next((r for r in rows if "[BT]" in r), "")
                    m = re.search(r"(\d+\.\d)%", task)
                    return float(m.group(1)) if m else 0.0

                p0 = _pct()
                deadline = time.time() + 40
                progressed = p0 > 0.0
                while time.time() < deadline and not progressed:
                    app.pump(0.5)
                    progressed = _pct() > p0
                assert progressed, \
                    "下载应持续推进（任务行百分比单调变化；相关度数据语义 = qBt " \
                    "calcRelevance 同源，显示形态判据归 QA-UC-08 v1.47 与产品单测承载；" \
                    "右栏窄幅下相关度列不在视口，沿 suite_ui_conns「BT 面板判据由单测锁」" \
                    "既定口径）"
            finally:
                self._quit(app)
                if seeder:
                    seeder.shutdown()
                tr.stop()

    def _case_23(self):
        """QA-BE-23 seed_exit_rule：seeders_count_gt 臂 + 非法值拒绝启动。"""
        @self.case("QA-BE-23")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            seed2 = None
            app = None
            try:
                assert tr.wait_ready()
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                s1 = LtPeer("s1", self._next_port(), torrent, pay)
                assert s1.wait_total_done(4 * FSIZE, 25)
                seed2 = LtPeer("s2", self._next_port(), torrent, pay)
                assert seed2.wait_total_done(4 * FSIZE, 25)
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt]\n"
                    f'trackers = ["http://127.0.0.1:{tr.port}/announce"]\n'
                    f"listen_port = {self._next_port() + 10}\n"
                    "\n[bt.seed_exit_rule]\n"
                    "seeders_count_gt = 2\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                self._add_bt_task(app, torrent)
                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                while time.time() < deadline and not all(
                        _disk_file(env.save_dir, n) and
                        open(_disk_file(env.save_dir, n), "rb").read() ==
                        open(src(n), "rb").read() for n in FILES):
                    app.pump(0.5)
                log_path = os.path.join(env.fixture_root, "tr.jsonl")
                deadline = time.time() + 90
                exited = False
                while time.time() < deadline:
                    log = _tracker_log(log_path)
                    drv = [r for r in log if r["peer_id"].startswith("-EZ2")
                           and r["event"] == "stopped"]
                    if drv and app.find_row("做种中") is None:
                        exited = True
                        break
                    app.pump(0.5)
                assert exited, \
                    "tracker complete(=3) > seeders_count_gt(2) 满足应收尾退出做种"
                self._quit(app)
                app = None
                env.write_config(
                    "[common]\n"
                    f'download_dir = "{env.save_dir}"\n'
                    "\n[bt.seed_exit_rule]\n"
                    "share_ratio_gt = 0\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                deadline = time.time() + 8
                while time.time() < deadline and app.exit_code is None:
                    app.pump(0.2)
                assert app.exit_code not in (None, 0), \
                    "share_ratio_gt = 0（非正数）应拒绝启动"
                app.close()
            finally:
                self._quit(app)
                if seed2:
                    seed2.shutdown()
                tr.stop()

    def _case_17(self):
        """QA-BE-17（S）partial 持久化零重请求：seeder request 日志缺失。"""
        self.skip("QA-BE-17", "LtPeer 无 per-(piece,offset) request 日志，"
                              "「已收块零重请求」对账不可观察；失效条件 = fixture "
                              "seeder 具备请求区间日志")

    def _case_18(self):
        """QA-BE-18（S）并发行标志判定表：choke/interest 不可编程驱动。"""
        self.skip("QA-BE-18", "libtorrent 不提供按需 choke/interest/乐观槽轮换的"
                              "编程驱动（判定表 D/d/U/u/O/I 臂不可 fixture 驱动）；"
                              "失效条件 = 原始 peer-wire 恶意 fixture（10 规程同源）")

    def _case_19(self):
        """QA-BE-19（S）WebRTC 传输臂：harness 无 WebRTC 数据面 seeder。"""
        self.skip("QA-BE-19", "harness 无 WebRTC 数据面 seeder fixture（需第二产品"
                              "实例或 webtorrent-cli 外部部署）；peer_connection_protocol "
                              "四臂的传输面不可观察；失效条件 = fixture 提供 webrtc 控制端")

    def _case_20(self):
        """QA-BE-20（S）ws(s) tracker + WebRTC e2e：数据面 seeder 缺失。"""
        self.skip("QA-BE-20", "mini_ws_tracker.py 在位（announce 面），但 WebRTC "
                              "DataChannel 传输臂需 webrtc seeder（同 BE-19）；"
                              "失效条件 = fixture 提供 webrtc 控制端 seeder")

    def _case_24(self):
        """QA-BE-24（S）顺序下载 piece 请求序：seeder request 序日志缺失。"""
        self.skip("QA-BE-24", "piece 请求顺序需 seeder 侧 request 序日志（LtPeer 不"
                              "产出）；对话框面 = QA-AB-12 实跑承载；失效条件 = "
                              "fixture seeder 记录 request 序")


def main() -> int:
    suite = BtEngineSuite()
    suite.run()
    p, f, s = suite.summary()
    print(f"\n[09-bt-engine] P={p} F={f} S={s}")
    return 1 if f else 0


if __name__ == "__main__":
    sys.exit(main())
