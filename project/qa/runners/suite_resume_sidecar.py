#!/usr/bin/env python3
"""suite_resume_sidecar.py — QA 套件 · 01-resume-sidecar 断点续传与 sidecar（期号 01）。

对应规程：project/qa/01-resume-sidecar-qa.md（13 用例）。
端到端 UI 层验证：TUI 状态 + 磁盘产物（目标文件/.ezr sidecar/注册表）+ kill/重启操作
+ fixture 不变量控制。只经用户可达入口（TUI 按键 / CLI 参数 / 磁盘文件），不进入项目内部 API。

时序纪律：门控 Given（fixture ?speed= 维持确定状态）→ 有界 wait_for 正探 → 单发 wait_gone 负探。
一致性头变更用受控 _Mutator 服务器（QA 工具入口）：ETag/LM/大小/最终 URL 均可在暂停后热变更。
"""

from __future__ import annotations

import os
import re
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from harness import (
    Env, EzrApp, Suite, TMP_ROOT, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, detail_text, expected_content, file_bytes,
    read_file, wait_file_size, task_pct, QA_FILE_SIZES,
)

EIGHT_M = QA_FILE_SIZES["eight-m.bin"]  # 8,000,000（十进制口径）


class _Mutator:
    """受控 HTTP 服务器：ETag / Last-Modified / 大小 / 重定向目标均可热变更。

    QA 工具入口（规程「环境前置」要求的一致性头动态变更能力，fixture 以查询
    参数注入而任务 URL 固定无法中途改参，故以此受控服务器补齐）。
    """

    def __init__(self, port: int = 0, root_dir: str = "/tmp"):
        # port=0 → OS 动态分配（避免跨用例/跨进程端口残留占用 EADDRINUSE）
        self.port = port
        self.root = root_dir
        self.etag = '"v1"'
        self.lm = "Mon, 09 Feb 2026 08:00:00 GMT"
        self.size = 2_000_000
        self.send_headers = True       # False = 不返回一致性头（noheaders 形态）
        self.redirect_path: str | None = None  # 仅该路径 302，其余路径直出
        self.redirect_to: str | None = None
        self._srv: ThreadingHTTPServer | None = None

    def url(self, path: str) -> str:
        return f"http://127.0.0.1:{self.port}/{path}"

    def start(self) -> None:
        mut = self

        class H(BaseHTTPRequestHandler):
            def do_GET(self):
                path = self.path.split("?")[0].lstrip("/")
                if (mut.redirect_path is not None and path == mut.redirect_path
                        and mut.redirect_to is not None):
                    self.send_response(302)
                    self.send_header("Location", mut.redirect_to)
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                rng = self.headers.get("Range")
                total = mut.size
                data_mode = True
                start, end = 0, total - 1
                status = 200
                if rng and rng.startswith("bytes="):
                    m = re.match(r"bytes=(\d+)-(\d*)", rng)
                    if m:
                        start = int(m.group(1))
                        end = int(m.group(2)) if m.group(2) else total - 1
                        end = min(end, total - 1)
                        status = 206
                if start >= total:
                    self.send_response(416)
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                self.send_response(status)
                self.send_header("Accept-Ranges", "bytes")
                self.send_header("Content-Length", str(end - start + 1))
                if status == 206:
                    self.send_header("Content-Range", f"bytes {start}-{end}/{total}")
                if mut.send_headers:
                    self.send_header("ETag", mut.etag)
                    self.send_header("Last-Modified", mut.lm)
                self.end_headers()
                # 分批写确定性内容（64KB/批，批间 10ms：可观察的传输窗口）
                pos = start
                try:
                    while pos <= end:
                        n = min(64 * 1024, end - pos + 1)
                        chunk = bytes(((pos + i) % 251) for i in range(n))
                        self.wfile.write(chunk)
                        pos += n
                        time.sleep(0.08)
                except OSError:
                    pass

            def log_message(self, *a):
                pass

        self._srv = ThreadingHTTPServer(("127.0.0.1", self.port), H)
        self.port = self._srv.server_address[1]  # 实际绑定端口（port=0 时）
        threading.Thread(target=self._srv.serve_forever, daemon=True).start()
        self.root = self.root  # 占位：内容按 i%251 现算，与 fixture 模式一致

    def stop(self) -> None:
        if self._srv:
            self._srv.shutdown()
            self._srv = None

    def gen_local(self, name: str, size: int) -> str:
        """按 fixture 同模式生成本地文件（供内容一致性比对）。"""
        p = os.path.join(self.root, name)
        with open(p, "wb") as f:
            f.write(expected_content(size))
        return p


class ResumeSidecarSuite(Suite):
    def __init__(self):
        super().__init__("01-resume-sidecar", port_base=41400)
        self._cur_app: "EzrApp | None" = None  # 失败现场快照用

    def _snapshot(self, env: Env, case_id: str) -> None:
        """失败现场：把当前画面写入 .work/tmp/qa-run/<case>.screen.txt。"""
        if self._cur_app is None:
            return
        try:
            path = os.path.join(TMP_ROOT, f"{case_id}.screen.txt")
            with open(path, "w", encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _wait_sidecar(env: Env, name: str, timeout: float = 8.0,
                      app: "EzrApp | None" = None) -> str:
        """等待 <name>.ezr sidecar 出现并返回路径（有界正探）。

        app 参数：轮询期间持续泵 PTY——阻塞式轮询会塞满 PTY 输出缓冲使应用
        写屏停摆（harness.wait_file_size 同款纪律，会话复盘教训）。
        """
        p = os.path.join(env.save_dir, f"{name}.ezr")
        deadline = time.time() + timeout
        while time.time() < deadline:
            if app is not None:
                app.pump(0.05)
            if file_bytes(p) is not None:
                return p
            time.sleep(0.05)
        raise AssertionError(f"sidecar 未出现: {p}")

    def _pause_at_progress(self, app: EzrApp, min_pct: float = 1.0,
                           timeout: float = 20.0) -> None:
        """等待进度达到 min_pct% 后 Space 暂停（播种稳定中间态纪律）。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.2)
            pct = task_pct(app)
            if pct is not None and pct >= min_pct:
                break
        app.send("space")
        assert app.wait_for("已暂停", 5), "暂停后应显示已暂停"

    def _pause_soon(self, app: EzrApp, delay: float = 0.5) -> None:
        """定时暂停：先等任务真正回到下载中，再固定延时后 Space。

        一致性失效链只需暂停时 sidecar 携带探测基准（stamp 来自探测而非进度）。
        若在等待中/探测中暂停，sidecar 未重建，恢复将无基准可比 —— 故必须
        先见「下载中」再暂停。
        """
        assert app.wait_for("下载中", 12), "暂停前任务应回到下载中"
        self._cur_app = app
        app.pump(delay)
        app.send("space")
        assert app.wait_for("已暂停", 5), "暂停后应显示已暂停"

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in range(1, 14):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-RS-01 下载 8MB 至中段：sidecar 存在且含 URL/大小/ETag/LM/块状态。"""
        @self.case("QA-RS-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 门控口径：?speed= 为每请求限速，聚合 = 速率 × min(并发, 块数)；
                # 8MB/4 worker 需 >4s 才能覆盖 sidecar 2s 周期落盘 → 400000/请求
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("eight-m.bin?speed=400000&etag=rs01-etag"))
                assert app.wait_for("下载中", 8)
                sc = self._wait_sidecar(env, "eight-m.bin", timeout=8, app=app)
                text = read_file(sc).decode("utf-8", "replace")
                assert "eight-m.bin" in text, "sidecar 应含 URL/文件标识"
                assert "8000000" in text, f"sidecar 应含大小 8000000: {text[:300]}"
                assert "rs01-etag" in text, "sidecar 应含 ETag"
                assert "last_modified" in text.lower() or "09 Feb 2026" in text, \
                    "sidecar 应含 Last-Modified"
                assert "blocks" in text, "sidecar 应含块状态表"
                # 优雅暂停收尾，保留 sidecar
                app.send("space")
                assert app.wait_for("已暂停", 5)
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-RS-02 下载期间 3 次 kill -9，每次重启续传；最终字节完整（NFR-2）。"""
        @self.case("QA-RS-02")
        def go(env: Env):
            name = "eight-m.bin"
            target = os.path.join(env.save_dir, name)
            # 门控：聚合 4×250KB/s=1MB/s（全程≈8s）；每轮「重启自动续传→
            # 暂停（即时落盘）→ kill9」，每轮仅消耗 ≈1MB，三轮后仍余 ≈5MB
            for rnd in range(3):
                app = EzrApp(env.home, save_dir=env.save_dir)
                app.pump(0.5)
                if rnd == 0:
                    add_task_via_dialog(
                        app, env, env.fixture.url(f"{name}?speed=250000"))
                assert app.wait_for("下载中", 12), f"第 {rnd+1} 轮应恢复下载"
                app.pump(0.6)
                app.send("space")
                assert app.wait_for("已暂停", 5)
                sc = os.path.join(env.save_dir, f"{name}.ezr")
                if file_bytes(sc) is None:
                    raise AssertionError("kill 前 sidecar 应存在")
                app.kill9()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("下载中", 12), "最终轮重启应自动续传"
                wait_file_size(target, EIGHT_M, 120, app=app)
                assert_file_content(target, EIGHT_M)
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-RS-03 8MB 下到 40% Space 暂停→继续：续传区间与已完成块零交集。"""
        @self.case("QA-RS-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("eight-m.bin?speed=900000"))
                assert app.wait_for("4/8", 60), "未在窗口内到达 ≥40%（4/8 块）"
                ts_pause = time.time()
                app.send("space")
                assert app.wait_for("已暂停", 5)
                app.send("space")
                assert app.wait_for("下载中", 5)
                target = os.path.join(env.save_dir, "eight-m.bin")
                wait_file_size(target, EIGHT_M, 90, app=app)
                resumed = [r for r in env.fixture.requests()
                           if r["range"] and "eight-m.bin" in r["path"]
                           and r["ts"] / 1000 >= ts_pause - 1
                           and r["range"] != "bytes=0-"]
                assert resumed, "恢复后应有 Range 续传数据请求"
                starts = []
                for r in resumed:
                    m = re.match(r"bytes=(\d+)-", r["range"])
                    assert m, f"Range 形态异常: {r['range']}"
                    starts.append(int(m.group(1)))
                assert all(s >= 4_000_000 for s in starts), \
                    f"续传起点与已完成块重叠: {sorted(starts)[:5]}"
                assert_file_content(target, EIGHT_M)
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-RS-04 8MB 传输中注入断连：自动重试续传、进度不回退、最终完整。"""
        @self.case("QA-RS-04")
        def go(env: Env):
            # 口径：disconnect=N 按「每请求」切新 N 字节。默认 1MB 块下每片请求
            # 都会在片尾被无害切断；为获得真实中途断连，配置块 4MB（2 块）＋
            # disconnect=2000000：两 worker 首连各传 2MB 被切（有进展瞬态失败），
            # 退避后重试请求片长 ≤2MB 不再被切 → 恰验证「断点续传补齐」
            env.write_config("block_size_http = 4194304\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("eight-m.bin?disconnect=2000000"))
                assert app.wait_for("s 后重试", 15), "断连后应进入自动重试倒计时"
                assert app.wait_for("重试 1/5", 5), "重试计数应 1/5"
                assert app.wait_for("下载中", 30), "退避后应自动重试恢复下载"
                target = os.path.join(env.save_dir, "eight-m.bin")
                wait_file_size(target, EIGHT_M, 90, app=app)
                resumed = [r for r in env.fixture.requests()
                           if r["range"] and "eight-m.bin" in r["path"]]
                starts = [int(re.match(r"bytes=(\d+)-", r["range"]).group(1))
                          for r in resumed if re.match(r"bytes=(\d+)-", r["range"])]
                assert any(s >= 2_000_000 for s in starts), \
                    f"应有 ≈2MB 起点的续传请求: {sorted(starts)}"
                assert_file_content(target, EIGHT_M)
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-RS-05 下到 30% kill -9→重启：回队列→获槽断点续传→完成（AC-2）。"""
        @self.case("QA-RS-05")
        def go(env: Env):
            name = "eight-m.bin"
            target = os.path.join(env.save_dir, name)
            # 门控：聚合 4×150KB/s=600KB/s → 全程 ≈13s；sidecar 2s 周期落盘
            # 在传输中段触发后于下载中态 kill；重启自动续传后剩余 ≈11s
            # 「下载中」窗口可观测
            app = EzrApp(env.home, save_dir=env.save_dir)
            add_task_via_dialog(app, env, env.fixture.url(f"{name}?speed=150000"))
            assert app.wait_for("下载中", 8)
            sc = self._wait_sidecar(env, name, timeout=8, app=app)
            assert file_bytes(sc) is not None, "kill 前 sidecar 应已周期落盘"
            app.kill9()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 崩溃恢复：下载中→等待中→获槽续传；等待中态一闪而过，
                # 「下载中」为主判据、「等待中」为辅助判据（二者任一即可）
                recovered = app.wait_for("下载中", 12) or app.wait_for("等待中", 4)
                assert recovered, "重启后任务应恢复（下载中或等待中）"
                wait_file_size(target, EIGHT_M, 120, app=app)
                resumed = [r for r in env.fixture.requests()
                           if r["range"] and name in r["path"]
                           and r["range"] != "bytes=0-"]
                starts = [int(re.match(r"bytes=(\d+)-", r["range"]).group(1))
                          for r in resumed if re.match(r"bytes=(\d+)-", r["range"])]
                assert any(s >= 2_000_000 for s in starts), \
                    f"应从断点（≈40%）续传: {sorted(starts)}"
                assert_file_content(target, EIGHT_M)
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-RS-06 预置 5 状态任务后 kill -9→重启：全部恢复；下载中→等待中。"""
        @self.case("QA-RS-06")
        def go(env: Env):
            add = add_task_via_dialog
            app = EzrApp(env.home, save_dir=env.save_dir)
            # ① 已完成：small.bin 全速
            add(app, env, env.fixture.url("small.bin"))
            assert app.wait_for("已完成", 10)
            # ② 已失败：ghost-404 探测必败
            add(app, env, env.fixture.url("ghost-404.bin"))
            assert app.wait_for("已失败", 10)
            # ③ 已暂停：five-m 慢速下载至暂停（聚合 1.2MB/s → 4.2s，窗口足够）
            add(app, env, env.fixture.url("five-m.bin?speed=300000"))
            app.pump(1.0)
            app.send("space")
            assert app.wait_for("已暂停", 5)
            # ④⑤ 下载中 ×5 占满 5 槽 + 第 6 个排队等待（聚合 400KB/s →
            #     每个 ≈20s，保证第 6 个加入时槽位仍全满）
            for i in range(5):
                add(app, env,
                    env.fixture.url(f"three-m.bin?swapsize={3000000 + i}&speed=100000"))
            add(app, env, env.fixture.url("two-m.bin?speed=100000"))
            assert app.wait_for("排队第", 15), "第 6 个下载任务应排队等待"
            app.kill9()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 恢复窗口：默认停在「正在下载」页签（已完成任务仅计入头部
                # 计数，名字在另一页签）；下载中 → 等待中一闪而过，以排队
                # 位次与头部计数为恢复判据
                assert app.wait_for("任务 9", 8), "全部 9 任务应恢复"
                assert app.wait_for("ghost-404", 8), "已失败任务应恢复"
                text = app.text()
                assert_in("已失败", text, "失败状态应保持")
                assert_in("已暂停", text, "暂停状态应保持")
                assert_in("已完成 1", text, "完成任务计数应保持")
                # 排队任务（two-m）位于列表末行可能被视口裁剪：End 滚动到底
                app.send("end")
                assert app.wait_for("排队第", 10), "等待队列应恢复排队位次"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-RS-07 暂停后分别变更 ETag/LM/大小/最终 URL/头缺失→继续：失效从头。

        Gherkin 为 Scenario Outline：每个变体行独立成例 → 每变体用独立任务，
        避免同任务连续失效触发 3 次停等（场景 08 语义，属正确产品行为）。
        """
        @self.case("QA-RS-07")
        def go(env: Env):
            mut = _Mutator(root_dir=env.fixture_root)  # OS 动态端口
            mut.start()
            try:
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    # 最终 URL 变更变体：go4.bin 经 302 → m4.bin；变更后 go4.bin 直出
                    mut.redirect_path = "go4.bin"
                    mut.redirect_to = mut.url("m4.bin")
                    variants = [
                        ("ETag 变更", "m1.bin",
                         lambda: setattr(mut, "etag", '"v2-changed"')),
                        ("LM 变更", "m2.bin",
                         lambda: setattr(mut, "lm", "Tue, 10 Feb 2027 09:00:00 GMT")),
                        ("大小变更", "m3.bin",
                         lambda: setattr(mut, "size", 2_500_000)),
                        ("最终 URL 变更", "go4.bin",
                         lambda: setattr(mut, "redirect_path", None)),
                        ("全部头缺失", "m5.bin",
                         lambda: setattr(mut, "send_headers", False)),
                    ]
                    for label, path, mutate in variants:
                        add_task_via_dialog(app, env, mut.url(path))
                        self._pause_soon(app)
                        mutate()
                        app.send("space")
                        assert app.wait_for("服务器内容已更新", 8), \
                            f"{label} 后应 toast 服务器内容已更新"
                        assert app.wait_gone("已暂停", 8), \
                            f"{label} 失效应转出已暂停（等待中从头重下）"
                        assert app.wait_for("下载中", 20), \
                            f"{label} 失效后应重新排队并从头下载"
                finally:
                    app.graceful_quit()
            finally:
                mut.stop()

    def _case_08(self):
        """QA-RS-08 连续 3 次一致性失效：转已失败「服务器内容持续变化」不自动重试。"""
        @self.case("QA-RS-08")
        def go(env: Env):
            mut = _Mutator(root_dir=env.fixture_root)  # OS 动态端口
            mut.start()
            try:
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(app, env, mut.url("m.bin"))
                    assert app.wait_for("下载中", 10)
                    for i in range(3):
                        self._pause_soon(app)
                        mut.etag = f'"flip-{i}"'
                        app.send("space")
                        if i < 2:
                            assert app.wait_for("服务器内容已更新", 8), \
                                f"第 {i+1} 次失效应 toast 已更新"
                            assert app.wait_for("下载中", 20), \
                                f"第 {i+1} 次失效后应从头重下"
                    assert app.wait_for("服务器内容持续变化", 10), \
                        "第 3 次失效应转「服务器内容持续变化」停等"
                    assert app.wait_for("不自动重试", 5), "停等应标注不自动重试"
                    text = app.text()
                    assert_in("下载槽位 0/5", text, "停等应释放槽位（唯一任务）")
                finally:
                    app.graceful_quit()
            finally:
                mut.stop()

    def _case_09(self):
        """QA-RS-09 停等后恢复一致按 R：计数清零重新探测续传；需再 3 次才停等。"""
        @self.case("QA-RS-09")
        def go(env: Env):
            mut = _Mutator(root_dir=env.fixture_root)  # OS 动态端口
            mut.start()
            try:
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(app, env, mut.url("m.bin"))
                    assert app.wait_for("下载中", 10)
                    # 制造停等：连续 3 次失效
                    for i in range(3):
                        self._pause_soon(app)
                        mut.etag = f'"s9-flip-{i}"'
                        app.send("space")
                        if i < 2:
                            assert app.wait_for("服务器内容已更新", 8), \
                                f"第 {i+1} 次失效应 toast 已更新"
                            assert app.wait_for("下载中", 20), \
                                f"第 {i+1} 次失效后应从头重下"
                    assert app.wait_for("服务器内容持续变化", 10)
                    # 恢复一致：把 ETag 改回 sidecar 现值不可能直读（文件级观察），
                    # 改用规程口径——「恢复一致」=服务器内容不再变化；按 R 后
                    # 重新探测：以当前头为基准重建基准并续传/重下，不立即停等
                    app.send("r")
                    assert app.wait_for("下载中", 25), "R 后应重新排队并探测下载"
                    assert app.wait_gone("服务器内容持续变化", 10), \
                        "R 应清停等状态"
                    # 再来 3 次失效才再次停等
                    for i in range(3):
                        self._pause_soon(app)
                        mut.etag = f'"s9-again-{i}"'
                        app.send("space")
                        if i < 2:
                            assert app.wait_for("服务器内容已更新", 8), \
                                f"重置后第 {i+1} 次失效应 toast 已更新"
                            assert app.wait_for("下载中", 20), \
                                f"重置后第 {i+1} 次失效后应从头重下"
                    assert app.wait_for("服务器内容持续变化", 10), \
                        "重置后仍需 3 次失效才再次停等"
                finally:
                    app.graceful_quit()
            finally:
                mut.stop()

    def _case_10(self):
        """QA-RS-10 下载中仅 .downloading；完成裸名存在、sidecar 删除（两种任务）。"""
        @self.case("QA-RS-10")
        def go(env: Env):
            import hashlib
            # 任务 1：无校验 three-m 慢速
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?speed=250000"))
                assert app.wait_for("下载中", 8)
                dl = os.path.join(env.save_dir, "three-m.bin.downloading")
                assert file_bytes(dl) is not None, "下载中应为 .downloading 形态"
                assert file_bytes(os.path.join(env.save_dir, "three-m.bin")) is None, \
                    "下载中不应存在裸名文件"
                wait_file_size(dl, QA_FILE_SIZES["three-m.bin"], 60, app=app)
                assert app.wait_gone("下载中", 15)
                assert file_bytes(os.path.join(env.save_dir, "three-m.bin")) is not None
                assert file_bytes(dl) is None, "完成后 .downloading 应移除"
                assert file_bytes(
                    os.path.join(env.save_dir, "three-m.bin.ezr")) is None, \
                    "完成后 sidecar 应删除"
                # 任务 2：有伴随校验 .sha256
                data = expected_content(1_000_000)
                with open(os.path.join(env.fixture_root, "ck10.bin"), "wb") as f:
                    f.write(data)
                with open(os.path.join(env.save_dir, "ck10.bin.sha256"), "w") as f:
                    f.write(hashlib.sha256(data).hexdigest())
                add_task_via_dialog(app, env,
                                    env.fixture.url("ck10.bin?speed=250000"))
                assert app.wait_for("SHA-256 校验通过", 60), "应完成并校验通过"
                assert file_bytes(os.path.join(env.save_dir, "ck10.bin")) is not None
                assert file_bytes(os.path.join(env.save_dir, "ck10.bin.ezr")) is None
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-RS-11 下载中 D→仅删除任务：记录消失；文件+sidecar 保留。"""
        @self.case("QA-RS-11")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=200000"))
                assert app.wait_for("下载中", 8)
                # 等 sidecar 周期落盘（2s tick）出现再删，才能验「保留 sidecar」
                self._wait_sidecar(env, "five-m.bin", timeout=8, app=app)
                app.send("D")
                assert app.wait_for("删除任务", 3), "删除对话框应打开"
                app.send("enter")  # 焦点默认首项=仅删除任务
                assert app.wait_for("已删除任务（保留文件）", 6), \
                    "应 toast 仅删除任务（保留文件）"
                assert app.wait_gone("five-m.bin", 8), "列表记录应消失"
                assert file_bytes(os.path.join(env.save_dir, "five-m.bin.downloading")) \
                    is not None, "仅删任务应保留 .downloading 文件"
                assert file_bytes(os.path.join(env.save_dir, "five-m.bin.ezr")) \
                    is not None, "仅删任务应保留 sidecar"
            finally:
                app.graceful_quit()

    def _case_12(self):
        """QA-RS-12 下载中/已完成任务 D→删除任务和文件：目标、sidecar、记录全删。"""
        @self.case("QA-RS-12")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 已完成任务：small.bin
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("已完成", 10)
                # 下载中任务：five-m 慢速
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=200000"))
                assert app.wait_for("下载中", 8)
                app.pump(0.8)
                app.send("D")
                assert app.wait_for("删除任务", 3)
                app.send("tab")  # 焦点移至「删除任务和文件」
                app.send("enter")
                assert app.wait_for("正在停止引擎并清理本地文件", 6)
                assert app.wait_gone("five-m.bin", 8)
                # 引擎停止确认后延迟删除（防竞态），给清理窗口
                deadline = time.time() + 10
                while time.time() < deadline:
                    app.pump(0.2)
                    if (file_bytes(os.path.join(env.save_dir, "five-m.bin.downloading"))
                            is None
                            and file_bytes(os.path.join(env.save_dir, "five-m.bin.ezr"))
                            is None):
                        break
                assert file_bytes(
                    os.path.join(env.save_dir, "five-m.bin.downloading")) is None, \
                    "删除任务和文件应清理 .downloading"
                assert file_bytes(os.path.join(env.save_dir, "five-m.bin.ezr")) is None, \
                    "删除任务和文件应清理 sidecar"
                # 已完成任务同语义
                app.send("home")
                app.pump(0.3)
                # 切到已完成页签选中 small.bin 删除
                app.send("tab")
                app.pump(0.4)
                app.send("D")
                assert app.wait_for("删除任务", 3)
                app.send("tab")
                app.send("enter")
                assert app.wait_for("正在停止引擎并清理本地文件", 6)
                assert app.wait_gone("small.bin", 8), "已完成任务记录应消失"
                assert file_bytes(os.path.join(env.save_dir, "small.bin")) is None, \
                    "已完成任务文件应删除"
            finally:
                app.graceful_quit()

    def _case_13(self):
        """QA-RS-13 仅删任务后重加同 URL+目录：接续提示、不追加序号、断点合并续传。"""
        @self.case("QA-RS-13")
        def go(env: Env):
            name = "eight-m.bin"
            target = os.path.join(env.save_dir, name)
            app = EzrApp(env.home, save_dir=env.save_dir)
            add_task_via_dialog(app, env, env.fixture.url(f"{name}?speed=800000"))
            self._pause_at_progress(app, min_pct=30.0)
            app.send("D")
            assert app.wait_for("删除任务", 3)
            app.send("enter")  # 仅删除任务
            assert app.wait_for("已删除任务（保留文件）", 6)
            # 重加同 URL 同目录
            app.send("A")
            assert app.wait_for("添加下载任务", 3)
            app.type_text(env.fixture.url(f"{name}?speed=800000"))
            app.send("tab")
            app.type_text(env.save_dir)
            app.send("enter")
            app.pump(0.5)
            try:
                assert app.wait_for("发现有效断点，将自动接续", 8), \
                    "重加应 toast 发现有效断点接续"
                assert file_bytes(os.path.join(env.save_dir, f"{name}.1")) is None, \
                    "不应追加 .1 序号"
                wait_file_size(target, EIGHT_M, 120, app=app)
                resumed = [r for r in env.fixture.requests()
                           if r["range"] and name in r["path"]
                           and r["range"] != "bytes=0-"]
                assert resumed, "应从断点续传（存在非 0 起点 Range）"
                assert_file_content(target, EIGHT_M)
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = ResumeSidecarSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
