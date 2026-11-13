#!/usr/bin/env python3
"""suite_modify_bt.py — 08-modify-bt-qa.md v1.8 重点用例可执行化（six-pack/QA 2026-11-10）。

覆盖 QA-MB-03/10/11/12（取消勾选删除+勾回从头补下 / 做种回退臂 / 做种中文件删除 /
暂停态修改，D73①②③④）。
QA-BE-08 的 UI 层严格检查面 = QA-MB-03/10/11/12（09-bt-engine-qa.md v1.43）；
引擎面（piece 集合/落盘字节）由单测+集成锁承载，本套件只经用户可达入口断言。

Fixture 组成（每用例独立现场）：
- mini_http_tracker（BEP3 HTTP，--interval 5 快速 announce，JSONL 日志承载
  「tracker announce uploaded 不增长」观察面）；
- libtorrent seeder（完整 payload；用例 1 限上传 64KB/s 拉长暂停观察窗至
  ≥3 个 announce 周期）；
- 用例 1 专用 libtorrent leecher：不配 tracker、手动 connect_peer 指向被测 ezr
  （driver 是其唯一 peer，仅要 a/b 文件、60KB/s 限速）——driver 上传暂停时
  leecher 停滞、恢复时续拉完成，上传暂停→恢复两端面必真可观察。
- 种子：ezr-mktorrent 四文件（a/b/c/d.bin 各 512KB，piece 256KiB=2 piece/文件）；
  工具固定 name=root 且相对路径首段为 root/ → 双层目录树 root/root/<file>，
  落盘 = save_dir/root/root/<file>（QA-BE-08 钦定布局，lt seeder 同构）。

UI 结构事实（driver 侧实测锚定）：
- 任务页签两枚（正在下载/已完成），键 ] 正向循环——做种行在「已完成」页签；
- BT 文件树双层 root→root→文件，Right 展开方向键逐层展开；
- BT 添加对话框焦点链 = URL0/目录1/树2/并发3/下载限速4/上传限速5/代理6/trackers7/
  按钮 8-10（树到「立即下载」= 6×Tab）；修改对话框 = 树0/并发1/下载限速2/上传限速3/
  代理4/trackers5/按钮 6-7（树到「确定」= 6×Tab）。

时序纪律（engineering.md）：门控 Given（fixture 限速钉住稳定窗）、正向探测有界
重试（轮询至短超时）、持久证据断言（tracker 日志/落盘文件/详情行累计值）。
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

from harness import TMP_ROOT, EzrApp, Suite, Env, assert_in, assert_not_in  # noqa: E402

MKTORRENT = os.path.join(QA_DIR, "..", "..", "ezr", "target", "debug",
                         "ezr-mktorrent")
TRACKER = os.path.abspath(os.path.join(
    QA_DIR, "..", "..", "harness", "tracker", "mini_http_tracker.py"))

FILES = ("a.bin", "b.bin", "c.bin", "d.bin")
FSIZE = 512 * 1024
PIECE = 262144


def _cycle_to_visible(app: EzrApp, needle: str, timeout: float) -> bool:
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


class LtPeer:
    """libtorrent 控制 peer（swarm_e2e.LtPeer 适配：+限速/+文件优先级/+手动连 peer）。"""

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
        if upload_limit is not None:
            self.handle.set_upload_limit(upload_limit)
        if download_limit is not None:
            self.handle.set_download_limit(download_limit)
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


class _Tracker:
    """mini_http_tracker 包装：子进程 + JSONL 日志路径 + TCP 就绪探测。"""

    def __init__(self, port, log_path):
        self.port = port
        self.proc = subprocess.Popen(
            [sys.executable, TRACKER, "--bind", "127.0.0.1",
             "--port", str(port), "--interval", "5", "--log", log_path],
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


def _make_payload(root: str) -> str:
    """lt seeder 布局 = <root>/root/root/<file>（lt 存储路径 = save_path + name + 相对路径）。"""
    pay = os.path.join(root, "payload")
    lay = os.path.join(pay, "root", "root")
    os.makedirs(lay, exist_ok=True)
    for name in FILES:
        with open(os.path.join(lay, name), "wb") as f:
            f.write(os.urandom(FSIZE))
    return pay


def _make_torrent(pay: str, announce: str) -> str:
    lay = os.path.join(pay, "root", "root")
    out = os.path.join(pay, "m.torrent")
    r = subprocess.run(
        [MKTORRENT, "--out", out, "--piece-len", str(PIECE),
         "--announce", announce] + [os.path.join(lay, n) for n in FILES],
        capture_output=True, text=True)
    assert r.returncode == 0, f"mktorrent 失败: {r.stderr.strip()}"
    return out


def _driver_config(env: Env, tr_port: int, listen_port: int) -> None:
    env.write_config(
        "[common]\n"
        f'download_dir = "{env.save_dir}"\n'
        "\n"
        "[bt]\n"
        f'trackers = ["http://127.0.0.1:{tr_port}/announce"]\n'
        f"listen_port = {listen_port}\n"
        "seed_duration_secs = 90.0\n"
    )


def _disk_file(save_dir: str, name: str) -> str | None:
    """落盘文件发现（遍历 save_dir；兼容钦定布局细节）。"""
    for base, _dirs, fnames in os.walk(save_dir):
        if name in fnames:
            return os.path.join(base, name)
    return None


def _read_driver_announces(log_path: str, driver_port: int) -> list[dict]:
    if not os.path.exists(log_path):
        return []
    rows = []
    with open(log_path, encoding="utf-8") as f:
        for line in f:
            try:
                row = json.loads(line)
            except Exception:
                continue
            if row.get("port") == driver_port:
                rows.append(row)
    return rows


def _tree_rows(app: EzrApp) -> list[tuple[str, str]]:
    """当前画面的树行（marker, name）：「选择下载文件」标题后的复选框行。"""
    rows: list[tuple[str, str]] = []
    in_tree = False
    for line in app.text().split("\n"):
        if "选择下载文件" in line:
            in_tree = True
            continue
        if not in_tree:
            continue
        m = re.match(r".*?(\[x\]|\[-\]|\[ \])\s+(.*?)(?:\s{2,}|$)", line)
        if m:
            marker = m.group(1).replace("[", "").replace("]", "")
            name = (m.group(2).strip().rstrip("/")
                    .lstrip("\u25b8\u25be").strip())
            rows.append((marker, name))
    return rows


def _tree_goto(app: EzrApp, name: str, cursor_hint: str | None = None) -> tuple[int, str]:
    """移动树光标至目标行并返回（行序号, marker）。cursor_hint = 光标当前所在行名。"""
    rows = _tree_rows(app)
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


def _focused_btn(app: EzrApp) -> tuple[int, str] | tuple[None, None]:
    """当前聚焦按钮（行, 文本）：聚焦按钮 = ACCENT 青底（pyte bg 00cdcd，实测）。"""
    buf = app.screen._screen.buffer
    for y in range(25, 45):
        line = "".join(buf[y][x].data for x in range(20, 102))
        if "确定" in line or "立即下载" in line or "取消" in line:
            segs: dict[str, list[int]] = {}
            for x in range(20, 102):
                segs.setdefault(str(buf[y][x].bg), []).append(x)
            hl = segs.get("00cdcd")
            if hl:
                txt = "".join(buf[y][x].data for x in range(hl[0], hl[-1] + 1))
                return y, txt.strip()
    return None, None


def _tab_to_button(app: EzrApp, want: str, max_tabs: int = 10) -> None:
    """自适应 tab 直至目标按钮青底聚焦（有界；tab 计数对渲染时序脆弱，实测口径）。"""
    for _ in range(max_tabs):
        _y, label = _focused_btn(app)
        if label and want in label:
            app.pump(0.1)
            return
        app.send("tab")
        app.pump(0.15)
    raise AssertionError(f"未能聚焦按钮 {want!r}")


def _add_bt_task(app: EzrApp, torrent: str, keep: tuple[str, ...]) -> None:
    """命令窗口添加 BT 任务：文件树取消 keep 之外的勾选（双层 root→root 结构）。"""
    app.send("a")
    assert app.wait_for("请输入命令或链接", 5), "命令窗口应打开"
    app.type_text(torrent)
    app.send("enter")
    assert app.wait_for("添加任务：BT", 6), "应分流打开 BT 对话框"
    app.send("tab", "tab")
    app.send("right", "down", "right", "up")
    app.pump(0.3)
    deselect = tuple(n for n in FILES if n not in keep)
    prev = None
    for name in deselect:
        idx, marker = _tree_goto(app, name, prev)
        assert marker in ("x", "-"), f"{name} 初始应已勾选，实得 {marker!r}"
        app.send("space")
        app.pump(0.2)
        prev = name
    _tab_to_button(app, "立即下载")
    app.send("enter")
    assert app.wait_for("已添加任务", 6), "应创建任务"


def _confirm_modify_files(app: EzrApp, actions: list[tuple[str, bool]],
                          from_state: str | None = None) -> None:
    """M 开修改对话框 → 双层展开 → 按 actions（文件名, 勾上?）逐行空格 → Tab×6 确定。"""
    app.send("m")
    assert app.wait_for("修改任务：BT", 5), "M 应打开 BT 修改对话框"
    app.send("right", "down", "right", "up")
    app.pump(0.3)
    prev = None
    for name, turn_on in actions:
        idx, marker = _tree_goto(app, name, prev)
        if turn_on:
            assert marker == " ", f"{name} 应为未勾选态，实得 {marker!r}"
        else:
            assert marker in ("x", "-"), f"{name} 应为已勾选态，实得 {marker!r}"
        app.send("space")
        app.pump(0.2)
        prev = name
    _tab_to_button(app, "确定")
    app.send("enter")
    assert app.wait_for("BT 任务已更新", 5), "确认应出更新 toast"


class ModifyBtSuite(Suite):
    def __init__(self):
        super().__init__("08-modify-bt", port_base=45900)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in (3, 10, 11, 12):
            getattr(self, f"_case_{i}")()
        return self.results

    def _snapshot(self, env: Env, case_id: str) -> None:
        """失败现场：当前画面 + tracker 日志尾落盘 .work/tmp/qa-run/。"""
        if self._cur_app is None:
            return
        try:
            path = os.path.join(TMP_ROOT, f"{case_id}.screen.txt")
            with open(path, "w", encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    def _case_3(self):
        """QA-MB-03 下载中取消未完成文件勾选 → 落盘删除+total 重算；重开勾回 → 从头补下。

        门控 = seeder1 限速 16KB/s（带宽限制下相邻 piece 完成间隔 ≈ 16s，
        远大于轮询间隔）：完成序由调度决定非索引序（实测 b→c→a→d），
        门控取「恰 2 文件字节级完成」态，取消一个未完成文件。
        偏差注记（QA 终验轮 20261112）：列表进度断言按 v1.49 口径 —— 明细行尺寸对
        已退役，重算证据 = 任务行[状态]徽标后总字节（1.6 MB）+ 进度百分数（66.7%）。"""
        @self.case("QA-MB-03")
        def go(env: Env):
            tr_port = self._next_port()
            seed1_port = self._next_port()
            drv_port = self._next_port() + 10
            tracker_log = os.path.join(env.fixture_root, "tracker.jsonl")
            tracker = _Tracker(tr_port, tracker_log)
            seeder1 = None
            app = None
            try:
                assert tracker.wait_ready(), "tracker 就绪超时"
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr_port}/announce")
                seeder1 = LtPeer("seeder1", seed1_port, torrent, pay,
                                 upload_limit=16384)
                assert seeder1.wait_total_done(4 * FSIZE, 25), "seeder1 校验超时"
                _driver_config(env, tr_port, drv_port)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=50)
                self._cur_app = app
                _add_bt_task(app, torrent, keep=FILES)
                src = lambda n: os.path.join(env.fixture_root, "payload",
                                             "root", "root", n)

                def bytes_match(name: str) -> bool:
                    f = _disk_file(env.save_dir, name)
                    if f is None:
                        return False
                    with open(f, "rb") as fh:
                        return fh.read() == open(src(name), "rb").read()

                deadline = time.time() + 150
                two = False
                cancel_target = None
                preserved = None
                while time.time() < deadline:
                    st = {n: bytes_match(n) for n in FILES}
                    done_n = [n for n in FILES if st[n]]
                    todo_n = [n for n in FILES if not st[n]]
                    if len(done_n) == 2 and todo_n:
                        two = True
                        cancel_target = todo_n[0]
                        preserved = done_n[0]
                        break
                    app.pump(0.4)
                assert two and cancel_target is not None and preserved is not None, \
                    "门控：恰 2 文件字节级完成的观察态应可达（16KB/s 下相邻完成间隔 ≈ 16s）"
                assert not bytes_match(cancel_target), \
                    "门控：取消目标应确为未完成文件"
                assert _cycle_to_visible(app, "下载中", 6), \
                    "存在未完文件应保持下载中（门控窗）"
                _confirm_modify_files(app, [(cancel_target, False)])
                assert_in(
                    "✓ BT 任务已更新：3/4 文件 · 共 1.6 MB（立即生效）",
                    app.text(),
                    "toast 逐字 = 更新后选中集合合计（3×512KiB = 1.5 MiB → 1.6 MB）")
                deadline = time.time() + 10
                gone = False
                while time.time() < deadline:
                    if _disk_file(env.save_dir, cancel_target) is None:
                        gone = True
                        break
                    app.pump(0.3)
                assert gone, "被取消未完成文件应从保存目录删除"
                assert _disk_file(env.save_dir, preserved) is not None, \
                    "未取消文件应保留"
                deadline = time.time() + 8
                saw = False
                while time.time() < deadline:
                    rows = app.display_rows()
                    bt_row = next((r for r in rows if "[下载中]" in r), "")
                    if "1.6 MB" in bt_row and "66.7%" in "\n".join(rows):
                        saw = True
                        break
                    app.pump(0.4)
                assert saw, (
                    "total 应按选中集合重算且任务行即时更新（[下载中] 徽标后 1.6 MB + 进度 66.7%；"
                    "v1.49 口径：明细行 1.0/1.6 MB 尺寸对已退役，任务行[状态]徽标后"
                    "单空格显示总字节；与 08-modify-bt-qa.md QA-MB-03 v1.49 锚点对齐口径一致）")
                assert _cycle_to_visible(app, "下载中", 6), \
                    "剩余未完文件应保持下载中"
                _confirm_modify_files(app, [(cancel_target, True)])
                assert_in(
                    "✓ BT 任务已更新：4/4 文件 · 共 2.1 MB（立即生效）",
                    app.text(),
                    "勾回 toast 逐字 = 全集合计")
                deadline = time.time() + 150
                fresh = False
                while time.time() < deadline:
                    if bytes_match(cancel_target):
                        fresh = True
                        break
                    app.pump(0.5)
                assert fresh, (
                    "勾回文件应从头补下至与原件字节一致（数据已删无断点，"
                    "占位文件全尺寸稀疏预分配不构成完成证据）")
            finally:
                if app is not None:
                    app.graceful_quit()
                if seeder1 is not None:
                    seeder1.shutdown()
                tracker.stop()

    def _case_10(self):
        """QA-MB-10 做种中勾 2 个新文件 → 即时回「下载中」+ 上传暂停（D73②）→ 回做种 + seed_left 重置（D73③）。"""
        @self.case("QA-MB-10")
        def go(env: Env):
            tr_port = self._next_port()
            seed_port = self._next_port()
            leech_port = self._next_port()
            drv_port = self._next_port() + 10
            tracker_log = os.path.join(env.fixture_root, "tracker.jsonl")
            tracker = _Tracker(tr_port, tracker_log)
            seeder = None
            leecher = None
            app = None
            try:
                assert tracker.wait_ready(), "tracker 就绪超时"
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr_port}/announce")
                seeder = LtPeer("seeder", seed_port, torrent, pay,
                                upload_limit=65536)
                assert seeder.wait_total_done(4 * FSIZE, 25), "seeder 校验超时"

                _driver_config(env, tr_port, drv_port)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=50)
                self._cur_app = app
                _add_bt_task(app, torrent, keep=("a.bin", "b.bin"))
                assert _cycle_to_visible(app, "做种中", 150), \
                    "a/b 完成后应转做种中（已完成页签）"
                assert_in("剩余做种", app.text(), "做种中应显示倒计时")
                leecher = LtPeer("leecher", leech_port, torrent,
                                 os.path.join(env.fixture_root, "leech"),
                                 download_limit=61440, priorities=(4, 4, 0, 0))
                leecher.connect_to("127.0.0.1", drv_port)
                up0 = 0
                deadline = time.time() + 60
                while time.time() < deadline:
                    rows = _read_driver_announces(tracker_log, drv_port)
                    ups = [r["uploaded"] for r in rows]
                    if ups and max(ups) > 0:
                        up0 = max(ups)
                        break
                    app.pump(0.3)
                assert up0 > 0, "做种中应向 leecher 上传（announce uploaded>0）"

                _confirm_modify_files(
                    app, [("c.bin", True), ("d.bin", True)])
                assert _cycle_to_visible(app, "下载中", 8), \
                    "做种回退应即时翻下载中（正在下载页签）"
                text = app.text()
                assert_not_in("剩余做种", text, "下载中不应显示做种倒计时")

                t_pause = time.time()
                pause_str = time.strftime("%H:%M:%S", time.localtime(t_pause))
                toast_done = False
                resume_str: str | None = None
                deadline = time.time() + 150
                while time.time() < deadline:
                    txt = app.text()
                    if "✓ 下载完成" in txt:
                        toast_done = True
                    if "做种中" in txt:
                        resume_str = time.strftime(
                            "%H:%M:%S", time.localtime(time.time()))
                        break
                    app.send("]")
                    app.pump(0.3)
                    txt = app.text()
                    if "✓ 下载完成" in txt:
                        toast_done = True
                    if "做种中" in txt:
                        resume_str = time.strftime(
                            "%H:%M:%S", time.localtime(time.time()))
                        break
                    app.pump(0.1)
                assert resume_str is not None, (
                    "暂停窗内应完成 c/d 补下并回做种（150s 超时）")
                for n in FILES:
                    got = _disk_file(env.save_dir, n)
                    assert got is not None, f"{n} 应已落盘"
                    src = os.path.join(env.fixture_root, "payload",
                                       "root", "root", n)
                    with open(got, "rb") as fa, open(src, "rb") as fb:
                        assert fa.read() == fb.read(), (
                            f"{n} 内容应与种子原件一致（reconfigure 对新勾选文件"
                            "全尺寸稀疏预分配，存在/大小均非完成证据）")
                rows = [r for r in _read_driver_announces(tracker_log, drv_port)
                        if pause_str <= r["ts"] <= resume_str]
                ups = [r["uploaded"] for r in rows]
                assert len(ups) >= 2 and ups[0] == ups[-1] and ups[0] >= up0, (
                    f"暂停窗内 announce uploaded 应冻结（up0={up0}，"
                    f"窗内 rows={rows[-4:] if rows else '空'}）")
                assert toast_done, "回做种应伴「✓ 下载完成」toast"
                assert_in("做种中", app.text(), "全部完成后应回做种中")
                assert_in("剩余做种", app.text(), "回做种应恢复倒计时显示")
                rem = None
                m = re.search(r"剩余做种\s+(\d{2}):(\d{2})", app.text())
                if m:
                    rem = int(m.group(1)) * 60 + int(m.group(2))
                assert rem is not None, "回做种后倒计时不可解析"
                assert rem > 75, f"回做种剩余应 ≈90s（D73③ 重置），实得 {rem}s"
                assert leecher.wait_total_done(2 * FSIZE, 90), \
                    "上传恢复后 leecher 应拉完 a/b（driver 唯一源）"
            finally:
                if app is not None:
                    app.graceful_quit()
                if leecher is not None:
                    leecher.shutdown()
                if seeder is not None:
                    seeder.shutdown()
                tracker.stop()

    def _case_11(self):
        """QA-MB-11 做种中取消已完成文件勾选 → 落盘删除 + 保持做种中。"""
        @self.case("QA-MB-11")
        def go(env: Env):
            tr_port = self._next_port()
            seed_port = self._next_port()
            drv_port = self._next_port() + 10
            tracker_log = os.path.join(env.fixture_root, "tracker.jsonl")
            tracker = _Tracker(tr_port, tracker_log)
            seeder = None
            app = None
            try:
                assert tracker.wait_ready(), "tracker 就绪超时"
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr_port}/announce")
                seeder = LtPeer("seeder", seed_port, torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25), "seeder 校验超时"
                _driver_config(env, tr_port, drv_port)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=50)
                self._cur_app = app
                _add_bt_task(app, torrent, keep=("a.bin", "b.bin"))
                assert _cycle_to_visible(app, "做种中", 150), \
                    "a/b 完成后应转做种中"
                a_path = _disk_file(env.save_dir, "a.bin")
                assert a_path is not None, "a.bin 应已落盘"
                _confirm_modify_files(app, [("a.bin", False)])
                deadline = time.time() + 10
                gone = False
                while time.time() < deadline:
                    if _disk_file(env.save_dir, "a.bin") is None:
                        gone = True
                        break
                    app.pump(0.3)
                assert gone, "被取消文件应从保存目录删除"
                assert _disk_file(env.save_dir, "b.bin") is not None, \
                    "未取消文件应保留"
                app.pump(1.5)
                assert_in("做种中", app.text(), "任务应保持做种中继续做种")
            finally:
                if app is not None:
                    app.graceful_quit()
                if seeder is not None:
                    seeder.shutdown()
                tracker.stop()

    def _case_12(self):
        """QA-MB-12 已暂停任务取消勾选+勾新文件 → 确认时立即删除/不唤醒 → 恢复后缺口补下。

        「已暂停」口径 = 下载中途暂停（做种态空格 = 停止做种转已完成，tasks.rs
        toggle_pause Seeding 臂）。门控：seeder1 只供 a/b（c 无源停稳在下载中），
        a/b 落盘即暂停；恢复后拉起 seeder2（只供 c/d）经 tracker 发现供缺口补下。
        """
        @self.case("QA-MB-12")
        def go(env: Env):
            tr_port = self._next_port()
            seed1_port = self._next_port()
            seed2_port = self._next_port()
            drv_port = self._next_port() + 10
            tracker_log = os.path.join(env.fixture_root, "tracker.jsonl")
            tracker = _Tracker(tr_port, tracker_log)
            seeder1 = None
            seeder2 = None
            app = None
            try:
                assert tracker.wait_ready(), "tracker 就绪超时"
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr_port}/announce")
                seeder1 = LtPeer("seeder1", seed1_port, torrent, pay,
                                 priorities=(4, 4, 0, 0))
                assert seeder1.wait_total_done(2 * FSIZE, 25), "seeder1 校验超时"
                _driver_config(env, tr_port, drv_port)
                app = EzrApp(env.home, save_dir=env.save_dir, rows=50)
                self._cur_app = app
                _add_bt_task(app, torrent, keep=("a.bin", "b.bin", "c.bin"))
                deadline = time.time() + 60
                while time.time() < deadline:
                    if (_disk_file(env.save_dir, "a.bin")
                            and _disk_file(env.save_dir, "b.bin")):
                        break
                    app.pump(0.4)
                assert _cycle_to_visible(app, "下载中", 6), \
                    "c 无源应保持下载中（门控窗）"
                app.send("space")
                assert _cycle_to_visible(app, "已暂停", 6), "空格应暂停任务"
                _confirm_modify_files(app, [("a.bin", False), ("d.bin", True)])
                deadline = time.time() + 10
                gone = False
                while time.time() < deadline:
                    if _disk_file(env.save_dir, "a.bin") is None:
                        gone = True
                        break
                    app.pump(0.3)
                assert gone, "被取消文件确认时应立即删除"
                assert _disk_file(env.save_dir, "b.bin") is not None, \
                    "未取消文件应保留"
                app.pump(2.5)
                assert _cycle_to_visible(app, "已暂停", 4), \
                    "任务应保持已暂停不被唤醒"
                assert _disk_file(env.save_dir, "d.bin") is None, \
                    "暂停态新勾选文件不应落盘"
                seeder2 = LtPeer("seeder2", seed2_port, torrent, pay,
                                 priorities=(0, 0, 4, 4))
                assert seeder2.wait_total_done(2 * FSIZE, 25), "seeder2 校验超时"
                app.send("space")
                assert _cycle_to_visible(app, "下载中", 8), "恢复后应进入下载"
                deadline = time.time() + 120
                ok = False
                while time.time() < deadline:
                    if _disk_file(env.save_dir, "d.bin") is not None:
                        ok = True
                        break
                    app.pump(0.5)
                assert ok, "恢复后新勾选文件应缺口补下落盘（seeder2 发现+补下）"
            finally:
                if app is not None:
                    app.graceful_quit()
                if seeder1 is not None:
                    seeder1.shutdown()
                if seeder2 is not None:
                    seeder2.shutdown()
                tracker.stop()


if __name__ == "__main__":
    suite = ModifyBtSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
