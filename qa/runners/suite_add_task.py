#!/usr/bin/env python3
"""suite_add_task.py — QA 套件 · 01-add-task 任务添加（期号 01）。

对应规程：project/qa/01-add-task-qa.md（17 用例）。
端到端 UI 层验证：只用用户可达入口（TUI 按键/鼠标、CLI 参数、磁盘产物观察），
不进入项目内部 API。fixture 校验伴随文件按用例预置/清空。

时序纪律：添加即建任务（等待中/下载中），详情断言经选中项详情面板；CLI 非法参数
用普通 subprocess 捕获退出码与 stderr（无需 PTY）。
"""

from __future__ import annotations

import os
import subprocess
import time

from harness import (
    EZR_BIN, Env, EzrApp, Suite, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, detail_text, expected_content, file_bytes,
    wait_file_size, QA_FILE_SIZES,
)

MD5_UPPER = "D869DB7FE62DE01E7F7E48A90C9E9D2E"  # 规程指定前缀 d869db7f


class AddTaskSuite(Suite):
    def __init__(self):
        super().__init__("01-add-task", port_base=41100)

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _task_count(app: EzrApp) -> int:
        import re
        # v1.13/FR-01-94 页签行口径：正在下载 (n) 计全部非已完成任务
        # （含失败/暂停/等待，04-ui-conns-04 数据面注记），已完成 (m) 计终态
        mm = re.search(r"正在下载 \((\d+)\) │ 已完成 \((\d+)\)", app.text())
        return int(mm.group(1)) + int(mm.group(2)) if mm else -1

    @staticmethod
    def _cli(env: Env, *args: str, timeout: float = 15.0):
        return subprocess.run(
            [EZR_BIN, *args], input="", capture_output=True, text=True,
            env={"HOME": env.home, "TERM": "xterm-256color",
                 "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                 "LANG": "C.UTF-8"},
            timeout=timeout)

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in range(1, 18):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-01-01 A 对话框五字段输入确认：任务出现；详情 HTTP/4 并发/SHA-256；落盘。"""
        @self.case("QA-01-01")
        def go(env: Env):
            import hashlib
            data = expected_content(QA_FILE_SIZES["five-m.bin"])
            good_ck = hashlib.sha256(data).hexdigest()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=700000"),
                    ck_value=good_ck)
                assert app.wait_for("已添加任务", 6), "应 toast 已添加"
                text = app.text()
                assert_in("five-m.bin", text, "任务应出现在列表")
                d = detail_text(app)
                assert_in("HTTP", d, "类型徽标 HTTP")
                # v1.14/FR-01-100：详情类型行无并发数，并发信息由头部数据面
                # 字段承载（04-ui-conns-16）——门控下 4 worker 活跃即可观测
                assert app.wait_for("并发 4", 15), "头部并发 4（配置默认生效）"
                assert_in("SHA-256", d, "校验算法 SHA-256")
                target = os.path.join(env.save_dir, "five-m.bin")
                wait_file_size(target, QA_FILE_SIZES["five-m.bin"], 90, app=app)
                assert_file_content(target, QA_FILE_SIZES["five-m.bin"])
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-01-02 算法下拉展开与选择：7 算法齐备；选 SHA-512 后占位 128 位。"""
        @self.case("QA-01-02")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                app.type_text(env.fixture.url("small.bin"))
                app.send("tab", "tab", "tab")  # 焦点至校验字段
                app.send("enter")  # 展开下拉
                assert app.wait_for("MD5", 3), "下拉应展开"
                text = app.text()
                for algo in ("MD5", "SHA-1", "SHA-224", "SHA-256",
                             "SHA-384", "SHA-512", "Adler-32"):
                    assert_in(algo, text, f"下拉应含 {algo}")
                # End 到末项 Adler-32 → 上移到 SHA-512（或逐 ↓）；用 End+up 序列
                app.send("end")
                app.pump(0.2)
                app.send("up")
                app.pump(0.2)
                app.send("enter")  # 选中 SHA-512
                app.pump(0.3)
                # 对话框居中渲染，不在右栏详情区：用整屏断言
                assert_in("SHA-512", app.text(), "选中 SHA-512 应生效")
                assert app.wait_for("128 位十六进制", 3), "校验码占位应显示 128 位"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-01-03 校验码含 zz/-：产品逐键过滤非十六进制字符（输入即拒绝）。

        口径注记（已裁决）：操作者裁决改规程文案随实现——非法输入不污染校验值的
        规格意图以更强形式（输入即拒绝）成立；Gherkin 场景 03 与规程均已改为
        逐键过滤语义（纯非法输入字段留空，确认无格式报错，空值按无校验创建）。
        """
        @self.case("QA-01-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                app.type_text(env.fixture.url("five-m.bin?speed=400000"))
                app.send("tab")
                app.type_text(env.save_dir)
                app.send("tab", "tab", "tab")
                app.type_text("zz" + "-" * 30)
                app.pump(0.3)
                # 非法字符应被逐键拒绝：字段保持空（占位符仍显示）
                assert_in("位十六进制（可留空）", app.text(), "非法字符应被拒绝、字段留空")
                app.send("enter")
                assert app.wait_for("（无校验）", 30), "空校验码应正常创建并无校验完成"
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-01-04 位数不符：SHA-256 配 32 位 / MD5 配 64 位均报错。"""
        @self.case("QA-01-04")
        def go(env: Env):
            for algo_sel, want, wrong in (("SHA-256（默认）", "64", "b" * 32),
                                          ("MD5", "32", "c" * 64)):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    app.send("A")
                    assert app.wait_for("添加下载任务", 3)
                    app.type_text(env.fixture.url("small.bin"))
                    app.send("tab", "tab", "tab")
                    if algo_sel != "SHA-256（默认）":
                        app.send("enter")
                        app.send("home")
                        app.pump(0.2)
                        app.send("enter")  # 选 MD5（表首）
                        app.pump(0.3)
                    app.send("tab")  # 焦点至校验码
                    app.type_text(wrong)
                    app.send("enter")
                    assert app.wait_for("位十六进制", 5), \
                        f"{algo_sel} 位数不符应报错（期望 {want} 位）"
                    assert_in("添加下载任务", app.text(), "对话框应保持打开")
                finally:
                    app.graceful_quit()

    def _case_05(self):
        """QA-01-05 ①校验码留空=无校验；②大写 MD5 统一小写显示前缀 d869db7f。"""
        @self.case("QA-01-05")
        def go(env: Env):
            # ① 留空（门控保持下载中，「无校验」列表行可见）
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=400000"))
                assert app.wait_for("（无校验）", 30), "留空应无校验完成"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ② 大写 MD5 → 小写前缀
            # 用限速 ten-m（25s 窗口）：1KB 文件秒完成会使「（待校验）」
            # 状态断言与完成竞速（实测偶发 F）；断言须落在稳定下载中段。
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                app.type_text(env.fixture.url("ten-m.bin?speed=400000"))
                app.send("tab", "tab", "tab", "enter")
                app.send("home")
                app.pump(0.2)
                app.send("enter")  # MD5
                app.pump(0.3)
                app.send("tab")
                app.type_text(MD5_UPPER)
                app.send("enter")
                # 创建后详情校验行立即渲染小写前缀（toast 会被完成 toast 竞速覆盖）
                assert app.wait_for(MD5_UPPER.lower()[:10], 10), \
                    "校验行应显示小写前缀 d869db7f"
                assert_in("（待校验）", app.text(), "应处于待校验状态")
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-01-06 并发字段 留空/0/65/abc：均建任务且下载中并发 4。"""
        @self.case("QA-01-06")
        def go(env: Env):
            import re
            for idx, conns in enumerate(("", "0", "65", "abc")):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(
                            f"five-m.bin?swapsize={5000000 + idx}&speed=400000"),
                        conns=conns)
                    assert app.wait_for("下载中", 8), f"conns={conns!r} 应建任务"
                    # 头部「并发 4」= 活跃连接数（v1.13 数据面口径）；门控下
                    # 4 worker 持续活跃，断言窗口内出现即验证配置并发生效
                    assert app.wait_for("并发 4", 15), \
                        f"conns={conns!r} 头部应显示并发 4（配置默认生效）"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_07(self):
        """QA-01-07 URL 无协议/ftp:///本地路径：toast 报错且列表不新增。"""
        @self.case("QA-01-07")
        def go(env: Env):
            for bad in ("www.fixture.local/file.bin", "ftp://fixture.local/file",
                        "/etc/hosts"):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(app, env, bad)
                    assert app.wait_for("仅支持 http", 5), \
                        f"{bad!r} 应报仅支持 http/https"
                    assert_not_in("已添加任务", app.text())
                    assert self._task_count(app) == 0, "列表不应新增任务"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_08(self):
        """QA-01-08 文件名推导三级：CD filename= / URL 末段 / CD+URL 末段空。"""
        @self.case("QA-01-08")
        def go(env: Env):
            with open(os.path.join(env.fixture_root, "cd1.bin"), "wb") as f:
                f.write(expected_content(400_000))
            # ① CD filename=report.pdf
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("cd1.bin?cd=report.pdf&speed=800000"))
                assert app.wait_for("report.pdf", 8), "CD filename 应生效"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ② 无 CD，URL 末段 dataset.tar.gz
            with open(os.path.join(env.fixture_root, "dataset.tar.gz"), "wb") as f:
                f.write(b"")

            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("dataset.tar.gz?speed=800000"))
                assert app.wait_for("dataset.tar.gz", 8), "URL 末段应生效"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ③ URL 末段为空 + CD archive.iso：fixture 无法对 "/" 路径出内容，
            # 用动态端口 QA 小服务器提供 CD 头（QA 工具入口）
            import threading
            from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

            class CdHandler(BaseHTTPRequestHandler):
                def do_GET(self):
                    self.send_response(200)
                    self.send_header("Content-Disposition",
                                     'attachment; filename="archive.iso"')
                    self.send_header("Content-Length", "200000")
                    self.end_headers()
                    try:
                        self.wfile.write(expected_content(200_000))
                    except OSError:
                        pass

                def log_message(self, *a):
                    pass

            srv = ThreadingHTTPServer(("127.0.0.1", 0), CdHandler)
            port = srv.server_address[1]
            threading.Thread(target=srv.serve_forever, daemon=True).start()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, f"http://127.0.0.1:{port}/?speed=800000")
                assert app.wait_for("archive.iso", 10), \
                    "URL 末段为空时应取 CD 名"
            finally:
                app.graceful_quit()
                srv.shutdown()

    def _case_09(self):
        """QA-01-09 CD 缺失且两级 URL 末段均空：download-<数字时间戳>。"""
        @self.case("QA-01-09")
        def go(env: Env):
            import re
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("?speed=800000"))
                assert app.wait_for("download-", 8), "应回退 download- 前缀"
                m = re.search(r"download-(\d+)", app.text())
                assert m and len(m.group(1)) >= 10, \
                    f"应为 download-<数字时间戳>: {m.group(0) if m else '?'}"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-01-10 预置同名文件后添加同名任务：新目标 report.pdf.1，原文件未变。"""
        @self.case("QA-01-10")
        def go(env: Env):
            original = b"ORIGINAL-CONTENT-MUST-NOT-CHANGE"
            with open(os.path.join(env.fixture_root, "cd1.bin"), "wb") as f:
                f.write(expected_content(400_000))
            with open(os.path.join(env.save_dir, "report.pdf"), "wb") as f:
                f.write(original)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("cd1.bin?cd=report.pdf&speed=800000"))
                assert app.wait_for("report.pdf.1", 10), "新目标应为 report.pdf.1"
                with open(os.path.join(env.save_dir, "report.pdf"), "rb") as f:
                    assert f.read() == original, "原文件字节不应改变"
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-01-11 目录解析顺序：输入 > 配置 download_dir > ~/Downloads；自动创建。"""
        @self.case("QA-01-11")
        def go(env: Env):
            cfg_dir = os.path.join(env.home, "cfgdl")
            input_dir = os.path.join(env.home, "inputdl")

            def manual_add(url: str, dir_text: str) -> None:
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                app.type_text(url)
                app.send("tab")
                if dir_text:
                    app.type_text(dir_text)
                app.send("enter")
                app.pump(0.5)

            # ③ 无输入无配置 → ~/Downloads（目录留空确认）
            app = EzrApp(env.home, save_dir=None)
            try:
                manual_add(env.fixture.url("small.bin"), "")
                assert app.wait_for("（无校验）", 30), "应无校验完成"
                assert file_bytes(os.path.join(env.home, "Downloads", "small.bin")) \
                    is not None, "无输入无配置应落 ~/Downloads"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ② 无输入 + 配置 download_dir → cfg_dir（自动创建）
            env.write_config(f'download_dir = "{cfg_dir}"\n')
            app = EzrApp(env.home, save_dir=None)
            try:
                manual_add(env.fixture.url("small.bin"), "")
                assert app.wait_for("（无校验）", 30)
                assert file_bytes(os.path.join(cfg_dir, "small.bin")) is not None, \
                    "配置 download_dir 应生效（自动创建）"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ① 输入目录优先 + ④ 目录不存在自动创建
            app = EzrApp(env.home, save_dir=None)
            try:
                manual_add(env.fixture.url("small.bin"),
                           os.path.join(input_dir, "sub", "created"))
                assert app.wait_for("（无校验）", 30)
                assert file_bytes(
                    os.path.join(input_dir, "sub", "created", "small.bin")) \
                    is not None, "输入目录应优先且自动创建"
            finally:
                app.graceful_quit()

    def _case_12(self):
        """QA-01-12 CLI 创建任务：-x 算法=码 依次 SHA-256/MD5/Adler-32（大小写不敏感）。"""
        @self.case("QA-01-12")
        def go(env: Env):
            for xarg, algo in (("sha256=" + "a" * 64, "SHA-256"),
                               ("MD5=" + "b" * 32, "MD5"),
                               ("ADLER32=" + "00112233", "Adler-32")):
                app = EzrApp(env.home, save_dir=env.save_dir,
                             args=[env.fixture.url("small.bin"),
                                   "-d", env.save_dir, "-x", xarg])
                try:
                    # 小文件瞬时完成，「已添加」toast 会被校验结果覆盖：
                    # 以详情/列表常驻的算法名行为锚
                    assert app.wait_for(algo, 10), f"详情算法应 {algo}"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_13(self):
        """QA-01-13 CLI -x 非法（裸码/sha3=/md5=64位）：非零退出+报错，无任务。"""
        @self.case("QA-01-13")
        def go(env: Env):
            for xarg, why in (("deadbeef", "形式"),
                              ("sha3=" + "a" * 64, "无法识别"),
                              ("md5=" + "a" * 64, "位数")):
                r = self._cli(env, env.fixture.url("small.bin"),
                              "-d", env.save_dir, "-x", xarg)
                assert r.returncode != 0, f"-x {xarg!r} 应非零退出"
                assert "-x 参数非法" in r.stderr, \
                    f"-x {xarg!r} 应报参数非法（{why}）: {r.stderr!r}"
            # 无任务创建：注册表不存在或无任务
            reg = os.path.join(env.home, ".ezr", "state", "registry.json")
            if file_bytes(reg) is not None:
                text = open(reg, encoding="utf-8").read()
                assert '"tasks": []' in text.replace(" ", "") or \
                    "small.bin" not in text, "不应创建任务"

    def _case_14(self):
        """QA-01-14 CLI -c 0/65/abc：非零退出+并发数非法报错；无任务创建。"""
        @self.case("QA-01-14")
        def go(env: Env):
            for cval in ("0", "65", "abc"):
                r = self._cli(env, env.fixture.url("small.bin"),
                              "-d", env.save_dir, "-c", cval)
                assert r.returncode != 0, f"-c {cval} 应非零退出"
                assert "并发数非法" in r.stderr, \
                    f"-c {cval} 应报并发数非法: {r.stderr!r}"

    def _case_15(self):
        """QA-01-15 CLI 好 URL+非法 URL 同时给出：非零退出；合法 URL 亦未创建。"""
        @self.case("QA-01-15")
        def go(env: Env):
            r = self._cli(env, env.fixture.url("small.bin"),
                          "not-a-url", "-d", env.save_dir)
            assert r.returncode != 0, "含非法 URL 应非零退出"
            assert "仅支持" in r.stderr or "非法" in r.stderr or "http" in r.stderr, \
                f"应报 URL 非法: {r.stderr!r}"
            reg = os.path.join(env.home, ".ezr", "state", "registry.json")
            if file_bytes(reg) is not None:
                text = open(reg, encoding="utf-8").read()
                assert "small.bin" not in text, "合法 URL 亦不应创建（全部拒绝）"

    def _case_16(self):
        """QA-01-16 重复添加同 URL+同目录：toast「任务已存在」；列表数量不变。"""
        @self.case("QA-01-16")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=500000"))
                assert app.wait_for("已添加任务", 6)
                n1 = self._task_count(app)
                assert n1 == 1, f"首次添加后任务数 1: {n1}"
                # 再次添加相同 URL 与目录
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                app.type_text(env.fixture.url("five-m.bin?speed=500000"))
                app.send("tab")
                app.type_text(env.save_dir)
                app.send("enter")
                app.pump(0.5)
                assert app.wait_for("任务已存在", 5), "应 toast 任务已存在"
                n2 = self._task_count(app)
                assert n2 == 1, f"列表数量应不变: {n1}→{n2}"
            finally:
                app.graceful_quit()

    def _case_17(self):
        """QA-01-17 bracketed paste 粘贴 200 字符 URL：完整显示；确认创建成功。"""
        @self.case("QA-01-17")
        def go(env: Env):
            # 平铺长文件名（子路径会因 fixture 根目录无该子目录而 404）
            long_name = "p" * 170 + ".bin"
            url = env.fixture.url(f"{long_name}?speed=800000")
            assert len(url) >= 200, f"URL 应 ≥200 字符: {len(url)}"
            with open(os.path.join(env.fixture_root, long_name), "wb") as f:
                f.write(b"0" * 1024)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                os.write(app.fd, b"\x1b[200~" + url.encode() + b"\x1b[201~")
                app.pump(0.6)
                assert_in(url[-40:], app.text(), "字段应完整显示粘贴内容（尾部可见）")
                app.send("tab")
                app.type_text(env.save_dir)
                app.send("enter")
                # 小文件瞬时完成：「已添加」toast 会被完成 toast 覆盖，
                # 以「已完成 (1)」页签计数断言创建成功（v1.13 页签行口径）
                assert app.wait_for("已完成 (1)", 15), "粘贴 URL 确认后应创建成功"
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = AddTaskSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
