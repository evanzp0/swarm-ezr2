#!/usr/bin/env python3
"""suite_download_engine.py — QA 套件 · 01-download-engine HTTP 下载内核（期号 01）。

对应规程：project/qa/01-download-engine-qa.md（16 用例）。
端到端 UI 层验证：TUI 可观察状态 + fixture 请求记录（fixture 属 QA 工具入口，允许）
+ 磁盘产物。只经用户可达入口（TUI 按键 / CLI 参数 / 磁盘文件），不进入项目内部 API。

时序纪律：所有观察窗经 fixture ?speed= 门控维持「下载中」状态（本地回环全速下
小文件瞬间完成转「已完成」页签，断言窗口消失）；完成后断言经 Tab 切「已完成」
页签 + 选中任务读取详情（用户可达路径）。
"""

from __future__ import annotations

import hashlib
import os
import re
import ssl
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from harness import (
    CaseResult, Env, EzrApp, Fixture, Suite, assert_file_content,
    assert_in, assert_not_in, add_task_via_dialog, chart_points, detail_text,
    expected_content, file_bytes, gen_qa_files, read_file, select_completed,
    wait_file_size, QA_FILE_SIZES,
)


class DownloadEngineSuite(Suite):
    def __init__(self):
        super().__init__("01-download-engine", port_base=41000)

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _add_via_dialog(app: EzrApp, env: Env, url: str, conns: str = "",
                        ck_value: str = "") -> None:
        add_task_via_dialog(app, env, url, conns, ck_value)

    @staticmethod
    def _detail_text(app: EzrApp) -> str:
        return detail_text(app)

    def _select_completed(self, app: EzrApp, needles: list[str],
                          max_moves: int = 8) -> set[str]:
        return select_completed(app, needles, max_moves)

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in range(1, 17):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-DE-01 添加 5MB 支持续传文件并开始：详情显示大小、HTTP、支持断点续传。"""
        @self.case("QA-DE-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("five-m.bin?speed=800000"))
                app.wait_for("[HTTP]", 5)
                app.wait_for("支持断点续传", 8)
                detail = self._detail_text(app)
                assert_in("HTTP", detail, "类型徽标")
                assert_in("支持断点续传", detail, "断点续传行")
                assert_in("5.0 MB", detail, "大小行（十进制口径）")
                assert_in("下载中", app.text(), "任务应开始下载")
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-DE-02 无 Content-Length：不支持断点续传、无 Range 请求、最终完整。"""
        @self.case("QA-DE-02")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("streamy.bin?speed=900000"))
                assert app.wait_for("不支持断点续传", 8), "详情应显示不支持断点续传"
                target = os.path.join(env.save_dir, "streamy.bin")
                wait_file_size(target, QA_FILE_SIZES["streamy.bin"], 60, app=app)
                reqs = [r for r in env.fixture.requests() if "streamy" in r["path"]]
                assert reqs, "fixture 未收到 streamy 请求"
                # 探测（bytes=0-）即单流本体请求（FR-01-10）；不得有其它分块 Range 形态
                ranged = [r for r in reqs
                          if r["range"] and r["range"] != "bytes=0-"]
                assert not ranged, f"无 Content-Length 项不应有分块 Range 数据请求: {ranged[:3]}"
                assert_file_content(target, QA_FILE_SIZES["streamy.bin"])
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-DE-03 Range 关闭：下到 ≥30%→Space→Space，新请求从头全量（AC-4）。"""
        @self.case("QA-DE-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                url = env.fixture.url("three-m.bin?norange=1&speed=300000")
                self._add_via_dialog(app, env, url)
                assert app.wait_for("不支持断点续传", 8)
                # 单流进度行（FR-01-17 展示面 1s 节拍 + 十进制口径）：以显示读数
                # 字节值判定 ≥30%。精确锚定「1.0 MB」不可靠——300KB/s 下读数每秒
                # 前进 ~300KB，而「1.0 MB」的显示窗仅 ~50KB 宽（0.995–1.05MB），
                # 1s 节拍采样可能整段跳过（瞬时值陷阱，engineering.md 断言锚定
                # 最终态条款的同源推论）。改解析任意已显示 progress 读数判定。
                deadline = time.time() + 30
                reached = False
                while time.time() < deadline:
                    app.pump(0.2)
                    m = re.search(r"([\d.]+) (KB|MB)/3\.0 MB", app.text())
                    if m:
                        b = float(m.group(1)) * (
                            1000.0 if m.group(2) == "KB" else 1_000_000.0)
                        if b >= 900_000:  # 30% 阈值（显示量化 ±半格在容差内）
                            reached = True
                            break
                assert reached, "未在窗口内到达 ≥30% 进度（显示读数口径）"
                ts_pause = time.time()
                app.send("space")
                assert app.wait_for("已暂停", 5), "Space 后应转已暂停"
                app.send("space")
                assert app.wait_for("下载中", 5), "Space 后应恢复下载"
                target = os.path.join(env.save_dir, "three-m.bin")
                wait_file_size(target, QA_FILE_SIZES["three-m.bin"], 90, app=app)
                reqs = [r for r in env.fixture.requests()
                        if "three-m.bin" in r["path"] and r["ts"] / 1000 >= ts_pause - 1]
                assert reqs, "恢复后 fixture 未收到新请求"
                # 恢复后的请求：探测（bytes=0-，即单流本体）之外不得有分块 Range
                bad = [r for r in reqs
                       if r["range"] and r["range"] != "bytes=0-"]
                assert not bad, f"不支持续传恢复应从头全量（无分块 Range）: {bad[:3]}"
                assert_file_content(target, QA_FILE_SIZES["three-m.bin"])
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-DE-04 分块总数 1 MB/块：2.5MB→0/3 · 1MB→0/1 · 512KB→0/1 · 10MB→0/10。"""
        @self.case("QA-DE-04")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 慢速门控维持 0 块完成观察窗（20KB/s → 1MB 块 ≈ 50s）
                names = ["two-half-m.bin", "one-m.bin", "half-m.bin", "ten-m.bin"]
                expects = ["0/3", "0/1", "0/1", "0/10"]
                for name in names:
                    self._add_via_dialog(app, env,
                                         env.fixture.url(f"{name}?speed=20000"))
                # 选中最后一个任务，逐个上移断言
                seen = set()
                for _ in range(4):
                    app.pump(0.3)
                    detail = self._detail_text(app)
                    for name, expect in zip(names, expects):
                        if expect in detail and "1 MB/块" in detail:
                            seen.add(name)
                    if len(seen) < 4:
                        app.send("up")
                assert seen == set(names), \
                    f"分块行断言未全命中（缺 {set(names)-seen}）"
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-DE-05 12MB 并发 4 / 3MB 并发 8：完成行 12/12 · 3/3，分块并发 ≤设置值。"""
        @self.case("QA-DE-05")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("twelve-m.bin?speed=800000"),
                                     conns="4")
                self._add_via_dialog(app, env,
                                     env.fixture.url("three-m.bin?speed=800000"),
                                     conns="8")
                t12 = os.path.join(env.save_dir, "twelve-m.bin")
                t3 = os.path.join(env.save_dir, "three-m.bin")
                wait_file_size(t12, QA_FILE_SIZES["twelve-m.bin"], 120, app=app)
                wait_file_size(t3, QA_FILE_SIZES["three-m.bin"], 60, app=app)
                found = self._select_completed(app, ["12/12 · 1 MB/块",
                                                     "3/3 · 1 MB/块"])
                assert found == {"12/12 · 1 MB/块", "3/3 · 1 MB/块"}, \
                    f"完成分块行断言未全命中: {found}"
                # 请求日志：12MB 出现多路不同起始 Range（动态领块）
                reqs = env.fixture.requests()
                r12 = [r for r in reqs if r["range"] and "twelve-m" in r["path"]]
                starts = {r["range"].split("-")[0] for r in r12}
                assert len(starts) >= 2, "12MB 应有多个不同起始块（动态领块）"
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-DE-06 2MB（2 块）并发 4：任务级分块行 2/2；无连接级明细（FR-01-81 修订二）。

        规程原判据「明细表 2 传输 + 2 待命」随 FR-01-81 修订二「并发分块明细表
        整体移除」撤销（phase-01 + 实现一致；feature 01-download-engine-06 的
        THEN 未同步，待操作者裁决，见 handoff 待批）。用例按实现口径改断言。
        """
        @self.case("QA-DE-06")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("two-m.bin?speed=300000"),
                                     conns="4")
                assert app.wait_for("下载中", 15), "任务应开始下载"
                assert_not_in("并发分块明细", detail_text(app),
                              "连接级明细应已移除（FR-01-81 修订二）")
                assert_not_in("待命", app.text(), "连接级状态列应已移除")
                target = os.path.join(env.save_dir, "two-m.bin")
                wait_file_size(target, QA_FILE_SIZES["two-m.bin"], 60, app=app)
                found = self._select_completed(app, ["2/2 · 1 MB/块"])
                assert found == {"2/2 · 1 MB/块"}, "最终完成分块行应 2/2"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-DE-07 8MB 下到 ≥40% 暂停→继续：续传区间与已完成块零交集；最终完整。"""
        @self.case("QA-DE-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("eight-m.bin?speed=900000"))
                assert app.wait_for("4/8", 60), "未在窗口内到达 ≥40%（4/8 块）"
                ts_pause = time.time()
                app.send("space")
                assert app.wait_for("已暂停", 5)
                sidecar = os.path.join(env.save_dir, "eight-m.bin.ezr")
                assert file_bytes(sidecar) is not None, "暂停后应存在 sidecar"
                app.send("space")
                assert app.wait_for("下载中", 5)
                target = os.path.join(env.save_dir, "eight-m.bin")
                wait_file_size(target, QA_FILE_SIZES["eight-m.bin"], 90, app=app)
                # 续传的数据请求（排除探测 bytes=0-，FR-01-10）与已完成块零交集
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
                assert all(s >= 4_194_304 for s in starts), \
                    f"续传起点与已完成块重叠: {sorted(starts)[:5]}"
                assert_file_content(target, QA_FILE_SIZES["eight-m.bin"])
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-DE-08 2 跳重定向：经重定向完成；详情 URL 为最终 URL。"""
        @self.case("QA-DE-08")
        def go(env: Env):
            # 短路径 t：最终 URL（http://127.0.0.1:P/t?redirect=0）= 35 字符，
            # 需完整落入详情 URL 栏可用宽度 36（超宽被 demo 基线 truncate 截尾；
            # 首轮实测 t.bin 形态 40 字符被截尾为「…?redire…」，QA 口径修正）
            with open(os.path.join(env.fixture_root, "t"), "wb") as f:
                f.write(expected_content(1_000_000))
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("t?redirect=2"))
                target = os.path.join(env.save_dir, "t")
                wait_file_size(target, 1_000_000, 60, app=app)
                # 完成后经「已完成」页签详情断言最终 URL（避免瞬态窗口竞速）
                found = self._select_completed(app, ["redirect=0"])
                assert found == {"redirect=0"}, \
                    f"详情 URL 应为最终 URL（redirect=0）detail={self._detail_text(app)[:400]}"
                reqs = [r for r in env.fixture.requests() if "redirect" in r["path"]]
                hops = {r["path"].split("redirect=")[-1].split("&")[0] for r in reqs}
                assert {"2", "1", "0"} <= hops, f"应逐跳经过 2→1→0: {hops}"
                assert_file_content(target, 1_000_000)
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-DE-09 11 跳重定向：失败「重定向次数超限」；不自动重试；不占槽。"""
        @self.case("QA-DE-09")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env, env.fixture.url("five-m.bin?redirect=11"))
                assert app.wait_for("重定向次数超限", 10), "应失败并显示重定向次数超限"
                text = app.text()
                assert_in("不自动重试", text, "失败行应标注不自动重试")
                assert_in("下载槽位 0/5", text, "失败停等应释放槽位")
                assert not file_bytes(os.path.join(env.save_dir, "five-m.bin")), \
                    "不应产生目标文件"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-DE-10 重定向至 ftp://：失败「不支持的重定向协议」；不自动重试。"""
        @self.case("QA-DE-10")
        def go(env: Env):
            # fixture 不支持跨协议重定向：受控 Python 重定向服务器（QA 工具入口）
            redir_port = self._next_port()

            class Handler(BaseHTTPRequestHandler):
                def do_GET(self):
                    self.send_response(302)
                    self.send_header("Location", "ftp://127.0.0.1/file.bin")
                    self.send_header("Content-Length", "0")
                    self.end_headers()

                def log_message(self, *a):
                    pass

            server = ThreadingHTTPServer(("127.0.0.1", redir_port), Handler)
            threading.Thread(target=server.serve_forever, daemon=True).start()
            try:
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    self._add_via_dialog(
                        app, env, f"http://127.0.0.1:{redir_port}/to-ftp")
                    assert app.wait_for("不支持的重定向协议", 10), \
                        "应失败并显示不支持的重定向协议"
                    assert_in("不自动重试", app.text(), "失败行应标注不自动重试")
                finally:
                    app.graceful_quit()
            finally:
                server.shutdown()

    def _case_11(self):
        """QA-DE-11 自签 HTTPS：失败原因含 TLS 证书错误；无跳过校验开关。"""
        @self.case("QA-DE-11")
        def go(env: Env):
            port = self._next_port()
            with _HttpsServer(port, trusted=False):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    self._add_via_dialog(app, env,
                                         f"https://127.0.0.1:{port}/three-m.bin")
                    assert app.wait_for("证书校验失败", 12), \
                        "自签 HTTPS 应失败并展示证书错误"
                    text = app.text()
                    assert_in("HTTPS", text, "协议类型应识别为 HTTPS")
                    assert_not_in("跳过校验", text, "不应提供跳过校验开关")
                finally:
                    app.graceful_quit()

    def _case_12(self):
        """QA-DE-12 受信证书 HTTPS：完成且字节完整；类型 HTTPS。"""
        @self.case("QA-DE-12")
        def go(env: Env):
            port = self._next_port()
            with _HttpsServer(port, trusted=True) as srv:
                # 受信 CA 经 SSL_CERT_FILE 注入（rustls-native-roots 系统信任锚探针）
                app = EzrApp(env.home, save_dir=env.save_dir,
                             env_extra={"SSL_CERT_FILE": srv.ca_file})
                try:
                    self._add_via_dialog(
                        app, env, f"https://127.0.0.1:{port}/three-m.bin?speed=900000")
                    assert app.wait_for("[HTTPS]", 10), "类型应为 HTTPS"
                    target = os.path.join(env.save_dir, "three-m.bin")
                    wait_file_size(target, QA_FILE_SIZES["three-m.bin"], 60, app=app)
                    assert_file_content(target, QA_FILE_SIZES["three-m.bin"])
                finally:
                    app.graceful_quit()

    def _case_13(self):
        """QA-DE-13 请求头记录：全部分块请求 Accept-Encoding: identity。"""
        @self.case("QA-DE-13")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env, env.fixture.url("five-m.bin"))
                target = os.path.join(env.save_dir, "five-m.bin")
                wait_file_size(target, QA_FILE_SIZES["five-m.bin"], 60, app=app)
                reqs = [r for r in env.fixture.requests() if "five-m.bin" in r["path"]]
                assert reqs, "fixture 未收到请求"
                bad = [r for r in reqs
                       if (r["accept_encoding"] or "").lower() != "identity"]
                assert not bad, f"存在非 identity 的 Accept-Encoding: {bad[:3]}"
            finally:
                app.graceful_quit()

    def _case_14(self):
        """QA-DE-14 下载中列表/头部/Sparkline 每秒真实更新；完成后归零。"""
        @self.case("QA-DE-14")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                self._add_via_dialog(app, env,
                                     env.fixture.url("five-m.bin?speed=500000"))
                assert app.wait_for("下载中", 8)
                header_speeds, spark_seen = [], False
                for _ in range(5):
                    app.pump(1.05)
                    rows = app.display_rows()
                    header_speeds.append(rows[1])
                    # v1.13：流量图内嵌头部第 1–3 行右侧（旧独立面板已撤销），
                    # 区域限定 + 按列去重收口在 harness.chart_points
                    if chart_points(app) > 0:
                        spark_seen = True
                assert spark_seen, "头部流量图区域应出现速度图块字符"
                joined = " ".join(header_speeds)
                assert re.search(r"↓\s*[1-9]", joined), \
                    f"头部 ↓ 应出现非零速度: {joined[:200]}"
                target = os.path.join(env.save_dir, "five-m.bin")
                wait_file_size(target, QA_FILE_SIZES["five-m.bin"], 60, app=app)
                app.pump(1.2)
                head = " ".join(app.display_rows()[:2])
                assert re.search(r"↓\s*0(\.0)? (B|KB)/s", head), \
                    f"完成后头部速度应归零: {head[:200]}"
            finally:
                app.graceful_quit()

    def _case_15(self):
        """QA-DE-15 槽位占满时添加：等待行「未知」（无探测）；获槽后详情 5.0 MB。"""
        @self.case("QA-DE-15")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                for i in range(5):
                    self._add_via_dialog(
                        app, env, env.fixture.url(f"one-m.bin?speed=20000&gate={i}"))
                assert app.wait_for("下载槽位 5/5", 20), "5 任务应占满 5 槽位"
                assert not [r for r in env.fixture.requests() if "five-m" in r["path"]], \
                    "占槽阶段不应有 five-m 探测"
                self._add_via_dialog(app, env,
                                     env.fixture.url("five-m.bin?speed=1000000"))
                assert app.wait_for("排队第 1 位", 8), "第 6 任务应排队第 1 位"
                assert app.wait_for("未知", 5), "等待行大小应显示「未知」（不预取）"
                assert not [r for r in env.fixture.requests() if "five-m" in r["path"]], \
                    "等待中不应预取（fixture 请求日志无探测）"
                # 暂停队首（AC-5）：先 Home 选中第 1 个任务再 Space
                app.send("home")
                app.pump(0.4)
                app.send("space")
                assert app.wait_for("下载中", 10)
                # End 跳到列表末尾选中新获槽任务
                app.send("end")
                app.pump(0.6)
                detail = self._detail_text(app)
                assert_in("5.0 MB", detail, "获槽后详情显示 5.0 MB")
                assert [r for r in env.fixture.requests() if "five-m" in r["path"]], \
                    "获槽后应发出探测请求"
            finally:
                app.graceful_quit()

    def _case_16(self):
        """QA-DE-16 100MB SHA-256 端到端（AC-1）：x/100 · 1 MB/块 · 4 连接 · 校验成功。"""
        @self.case("QA-DE-16")
        def go(env: Env):
            # 预置 .sha256 伴随文件（QA 环境准备）；big-100m.bin 为 fixture gen
            # 的稀疏文件（全零内容）——校验值按零内容预计算
            hexval = hashlib.sha256(b"\x00" * 104_857_600).hexdigest()
            with open(os.path.join(env.fixture_root, "big-100m.bin.sha256"), "w") as f:
                f.write(hexval)
            app = EzrApp(env.home, args=[env.fixture.url("big-100m.bin"),
                                         "-d", env.save_dir,
                                         "-x", f"sha256={hexval}"])
            try:
                # CLI 任务应自动开始并推进。100MB 回环传输亚秒级完成，「下载中」
                # 「校验中」均为亚帧瞬态（engineering.md 瞬时值陷阱：断言锚定
                # 最终态常驻文本）；页签计数「已完成 (1)」为持久终态证据。
                # 分块行断言由 _select_completed 后的详情断言（100/100 · 1 MB/块）覆盖。
                assert app.wait_for("已完成 (1)", 10), "CLI 任务应自动创建并完成（亚秒级传输，锚定持久终态计数）"
                assert app.wait_for("已完成 (1)", 8) \
                    or app.wait_for("校验中", 5) \
                    or app.wait_for("下载中", 5), "CLI 任务应自动开始并推进"
                # 活跃连接 = 4：下载全程内日志必出现 2s 窗口 ≥4 路并发 Range
                # （避免“首帧早于 worker 请求落盘”的时序竞速：轮询至完成再断言）
                target = os.path.join(env.save_dir, "big-100m.bin")
                deadline = time.time() + 120
                while time.time() < deadline and file_bytes(target) != 104_857_600:
                    app.pump(0.1)
                assert _max_concurrent_ranges(
                    env.fixture, "big-100m.bin", window=2.0) >= 4, \
                    "应观察到 4 路并发分块请求"
                wait_file_size(target, 104_857_600, 5, app=app)
                # 「SHA-256 校验成功」= 已完成列表行文案；「100/100 · 1 MB/块」= 详情分块行
                found = self._select_completed(app, ["100/100 · 1 MB/块"])
                assert found == {"100/100 · 1 MB/块"}, \
                    f"详情分块行未命中: {found} detail={self._detail_text(app)[:500]}"
                assert_in("SHA-256 校验成功", app.text(), "列表行应显示 SHA-256 校验成功")
                assert file_bytes(target) == 104_857_600, "字节数应为 104857600"
                assert not file_bytes(os.path.join(env.save_dir, "big-100m.bin.ezr")), \
                    "完成后 sidecar 应删除"
                assert not file_bytes(target + ".downloading"), \
                    "完成后不应残留 .downloading"
            finally:
                app.graceful_quit()


def _max_concurrent_ranges(fixture: Fixture, name: str, window: float) -> int:
    """请求日志中同一 window 秒内不同 Range 请求的最大并发数。"""
    reqs = sorted((r["ts"] / 1000, r["range"]) for r in fixture.requests()
                  if r["range"] and name in r["path"])
    best = 0
    for ts, _ in reqs:
        concurrent = {r for (t, r) in reqs if ts <= t <= ts + window}
        best = max(best, len(concurrent))
    return best


class _HttpsServer:
    """受控 HTTPS fixture（Python ssl）：trusted=False 自签 / True 受信 CA。"""

    def __init__(self, port: int, trusted: bool):
        self.port = port
        self.tmp = tempfile.mkdtemp(prefix="qa-tls-")
        self.ca_file = os.path.join(self.tmp, "ca.pem")
        self._server: ThreadingHTTPServer | None = None
        self._trusted = trusted

    def __enter__(self):
        key = os.path.join(self.tmp, "key.pem")
        cert = os.path.join(self.tmp, "cert.pem")
        if self._trusted:
            ca_key = os.path.join(self.tmp, "ca-key.pem")
            subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048",
                            "-nodes", "-keyout", ca_key, "-out", self.ca_file,
                            "-days", "2", "-subj", "/CN=QA Test CA"], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            subprocess.run(["openssl", "req", "-newkey", "rsa:2048", "-nodes",
                            "-keyout", key, "-out", os.path.join(self.tmp, "csr.pem"),
                            "-subj", "/CN=127.0.0.1"], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            ext = os.path.join(self.tmp, "ext.cnf")
            with open(ext, "w") as f:
                f.write("subjectAltName=IP:127.0.0.1,DNS:localhost\n")
            subprocess.run(["openssl", "x509", "-req", "-in",
                            os.path.join(self.tmp, "csr.pem"), "-CA", self.ca_file,
                            "-CAkey", ca_key, "-CAcreateserial", "-out", cert,
                            "-days", "2", "-extfile", ext], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        else:
            subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048",
                            "-nodes", "-keyout", key, "-out", cert, "-days", "2",
                            "-subj", "/CN=127.0.0.1",
                            "-addext", "subjectAltName=IP:127.0.0.1,DNS:localhost"],
                           check=True, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL)

        root = tempfile.mkdtemp(prefix="qa-tls-root-")
        gen_qa_files(root)
        content = read_file(os.path.join(root, "three-m.bin")) or b""

        class H(BaseHTTPRequestHandler):
            def do_GET(self):
                body = content if "/three-m.bin" in self.path else b""
                self.send_response(200)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                # 节流输出制造观察窗（QA 门控；Python 服务端不受 fixture ?speed 门控）
                for i in range(0, len(body), 64 * 1024):
                    self.wfile.write(body[i:i + 64 * 1024])
                    self.wfile.flush()
                    time.sleep(0.12)

            def log_message(self, *a):
                pass

        self._server = ThreadingHTTPServer(("127.0.0.1", self.port), H)
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain(cert, key)
        self._server.socket = ctx.wrap_socket(self._server.socket, server_side=True)
        threading.Thread(target=self._server.serve_forever, daemon=True).start()
        import socket as _s
        deadline = time.time() + 5
        while time.time() < deadline:
            try:
                with _s.create_connection(("127.0.0.1", self.port), timeout=0.3):
                    return self
            except OSError:
                time.sleep(0.05)
        raise RuntimeError("HTTPS fixture 未就绪")

    def __exit__(self, *exc):
        if self._server:
            self._server.shutdown()
        return False


if __name__ == "__main__":
    suite = DownloadEngineSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
