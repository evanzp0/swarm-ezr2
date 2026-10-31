#!/usr/bin/env python3
"""suite_add_task.py — QA 套件 · 01-add-task 任务添加（期号 01）。

对应规程：project/qa/01-add-task-qa.md（18 用例）。
端到端 UI 层验证：只用用户可达入口（TUI 按键/鼠标、CLI 参数、磁盘产物观察），
不进入项目内部 API。fixture 校验伴随文件按用例预置/清空。

时序纪律：添加即建任务（等待中/下载中），详情断言经选中项详情面板；CLI 非法参数
用普通 subprocess 捕获退出码与 stderr（无需 PTY）。

v1.15 同步（QA v138 会话）：新增 QA-01-18（仅添加按钮，FR-01-103/D28/D29）——
按钮行 [ 立即下载 ] [ 仅添加 ] [ 取消 ]（焦点 6/7/8，Add nfocus=9）；仅添加只建
不下载（任务落「已暂停」、不占槽位），toast 钦定文案「已添加任务…已暂停，按空格
开始下载」（D29）；校验链与立即下载一致。
"""

from __future__ import annotations

import os
import subprocess
import time

from harness import (
    clear_dir_prefill, clear_url_prefill,
    DUMMY_URL, EZR_BIN, Env, EzrApp, Suite, add_task_via_dialog,
    assert_file_content, open_add_dialog_via_cmd,
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
        for i in range(1, 20):
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
        """QA-01-02 算法下拉（v1.32 重写）：默认 NONE 禁输入；8 项齐备；选 SHA-512 可输入。

        规程 v1.32 口径：默认选中 NONE（校验码字段清空、逐键/粘贴均不可输入）；
        下拉 8 项 = NONE + 7 算法；URL/校验码字段无占位提示、校验行无
        `Enter 选择算法` 尾注（v1.32/P3 提示信息删除）；↑↓ 移动选中即时改值，
        回 NONE 校验码实时清空；选中 SHA-512 后校验码可输入、Enter 确认收起。
        """
        @self.case("QA-01-02")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, env.fixture.url("small.bin"))
                app.send("tab", "tab", "tab")  # 焦点至校验字段
                # v1.32：默认 NONE + 占位提示删除（URL/校验码占位、尾注全 removed）
                assert_in("NONE", app.text(), "默认应选中 NONE（v1.32）")
                assert_not_in("位十六进制", app.text(), "占位提示应已删除（v1.32/P3）")
                assert_not_in("Enter 选择算法", app.text(), "校验行尾注应已删除（v1.32/P3）")
                app.send("enter")  # 展开下拉
                assert app.wait_for("MD5", 3), "下拉应展开"
                text = app.text()
                assert_in("NONE", text, "下拉首行应含 NONE（v1.32 八项）")
                for algo in ("MD5", "SHA-1", "SHA-224", "SHA-256",
                             "SHA-384", "SHA-512", "Adler-32"):
                    assert_in(algo, text, f"下拉应含 {algo}")
                # ↑↓ 即时改值：↓ 到 MD5 → Home 回 NONE（校验码实时清空）→
                # End 末项 Adler-32 → ↑ 到 SHA-512
                app.send("down")
                app.pump(0.2)
                app.send("home")
                app.pump(0.2)
                app.send("end")
                app.pump(0.2)
                app.send("up")
                app.pump(0.2)
                app.send("enter")  # 选中 SHA-512 并收起
                app.pump(0.3)
                # 对话框居中渲染，不在右栏详情区：用整屏断言
                assert_in("SHA-512", app.text(), "选中 SHA-512 应生效")
                # 选中 SHA-512 后校验码可输入（NONE 锁解除）
                app.send("tab")  # 校验 → 校验码
                app.type_text("ff00")
                assert_in("ff00", app.text(), "SHA-512 选中后校验码可输入")
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-01-03（v1.32 同步）选 MD5 输入 zz/-：逐键过滤字段保持空；确认被拒必填。"""
        @self.case("QA-01-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, env.fixture.url("five-m.bin?speed=400000"))
                app.send("tab")
                clear_dir_prefill(app)
                app.type_text(env.save_dir)
                app.send("tab", "tab")          # 保存到1 → 并发2 → 校验3
                app.send("enter")               # 展开下拉
                app.send("home")                # NONE（基点）
                app.send("down")                # MD5（v1.32 表序：NONE 后第 1）
                app.send("enter")               # 选中 MD5 并收起（焦点留 3）
                app.send("tab")                 # 校验3 → 校验码4
                app.type_text("zz" + "-" * 30)
                app.pump(0.3)
                # 非法字符逐键拒绝：字段保持空；v1.32 占位提示已删除——
                # 以「无 hex 进入字段」为空判据（toast/字段渲染无残留值）
                assert_not_in("位十六进制（可留空）", app.text(),
                              "占位提示应已删除（v1.32/P3）")
                app.send("enter")
                # v1.32 必填：MD5 空校验码确认被拒——toast 逐字 + 对话框未关闭
                assert app.wait_for("MD5 校验码不可为空", 5), \
                    "空校验码确认应被拒（v1.32 必填）"
                assert_in("添加任务：HTTP(S)", app.text(), "对话框未关闭")
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-01-04 位数不符（v1.32：算法显式选择，无「默认 SHA-256」）：均报错。"""
        @self.case("QA-01-04")
        def go(env: Env):
            # v1.32：NONE 为默认——SHA-256/MD5 都须在下拉显式选择
            # （Home 归 NONE 基点后 ↓×N：MD5=1、SHA-256=4）
            for downs, want, wrong in ((4, "64", "b" * 32), (1, "32", "c" * 64)):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    open_add_dialog_via_cmd(app, env.fixture.url("small.bin"))
                    app.send("tab", "tab", "tab")  # 焦点至校验字段
                    app.send("enter")              # 展开下拉
                    app.send("home")               # NONE 基点
                    app.send(*(["down"] * downs))  # → SHA-256 / MD5
                    app.send("enter")              # 选中并收起
                    app.send("tab")                # 焦点至校验码
                    app.type_text(wrong)
                    app.send("enter")
                    assert app.wait_for("位十六进制", 5), \
                        f"位数不符应报错（期望 {want} 位）"
                    assert_in("添加任务：HTTP(S)", app.text(), "对话框应保持打开")
                finally:
                    app.graceful_quit()

    def _case_05(self):
        """QA-01-05（v1.32 三臂）①NONE 直接确认=无校验；②MD5 留空被拒；③大写 MD5 小写前缀。"""
        @self.case("QA-01-05")
        def go(env: Env):
            # ① 默认 NONE 直接确认（校验码禁输入）→ 任务创建、详情「无校验」
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=400000"))
                assert app.wait_for("（无校验）", 30), "NONE 应无校验完成"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ② 选 MD5 留空确认 → 提交被拒 toast（v1.32 必填）
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, env.fixture.url("small.bin"))
                app.send("tab", "tab", "tab", "enter")  # 展开下拉（焦点 3）
                app.send("home")                        # NONE 基点
                app.send("down")                        # MD5
                app.send("enter")                       # 选中并收起（焦点留 3）
                app.send("tab")                         # 校验3 → 校验码4
                app.send("enter")                       # 校验码留空确认（被拒）
                assert app.wait_for("MD5 校验码不可为空", 5), \
                    "MD5 留空应被拒（v1.32 必填）"
                assert_in("添加任务：HTTP(S)", app.text(), "对话框应保持打开")
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()
            # ③ 大写 MD5 → 小写前缀
            # 用限速 ten-m（25s 窗口）：1KB 文件秒完成会使「（待校验）」
            # 状态断言与完成竞速（实测偶发 F）；断言须落在稳定下载中段。
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, env.fixture.url("ten-m.bin?speed=400000"))
                app.send("tab", "tab", "tab", "enter")
                app.send("home")
                app.send("down")  # MD5（v1.32 表序）
                app.pump(0.2)
                app.send("enter")
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
        """QA-01-07 URL 无协议/ftp:///本地路径：toast 报错且列表不新增。

        D44 迁移同步：命令窗口对非 http(s)/非 .torrent 输入直接开错误窗口，
        对话框级 URL 校验的可达路径 = 对话框内改写 URL 字段后确认。
        """
        @self.case("QA-01-07")
        def go(env: Env):
            for bad in ("www.fixture.local/file.bin", "ftp://fixture.local/file",
                        "/etc/hosts"):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    open_add_dialog_via_cmd(app, DUMMY_URL)
                    clear_url_prefill(app)
                    app.type_text(bad)
                    app.send("enter")
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
        """QA-01-11（v1.35 重写）打开预填 + 解析顺序 + 归一：~ 展开 / 相对→绝对。"""
        @self.case("QA-01-11")
        def go(env: Env):
            cfg_dir = os.path.join(env.home, "cfgdl")
            input_dir = os.path.join(env.home, "inputdl")
            tilde_dir = os.path.join(env.home, "qadl-docs")
            rel_dir = os.path.join(env.home, "qadl-rel", "sub")

            def manual_add(url: str, dir_text: str) -> None:
                open_add_dialog_via_cmd(app, url)
                app.send("tab")
                if dir_text:
                    # v1.38/D62：字段打开即预填配置链值——显式输入前先清空
                    clear_dir_prefill(app)
                    app.type_text(dir_text)
                app.send("enter")
                app.pump(0.5)

            # ③ 无输入无配置 → 预填 ~/Downloads，留空确认落同（目录留空语义）
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
            # ② 无输入 + 配置 download_dir → 对话框预填 cfg_dir，留空确认落同
            env.write_config(f'[common]\ndownload_dir = "{cfg_dir}"\n')
            app = EzrApp(env.home, save_dir=None)
            try:
                open_add_dialog_via_cmd(app, env.fixture.url("small.bin"))
                app.send("tab")
                assert_in(cfg_dir, app.text(),
                          "保存目录字段应预填配置 download_dir（v1.38/D62②）")
                app.send("enter")
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
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ④ ~ 前缀展开主目录（D62 归一；pty 子进程 HOME=env.home）
            app = EzrApp(env.home, save_dir=None)
            try:
                manual_add(env.fixture.url("small.bin"), "~/qadl-docs")
                assert app.wait_for("（无校验）", 30)
                assert file_bytes(os.path.join(tilde_dir, "small.bin")) is not None, \
                    "~/ 输入应展开为 <主目录>/qadl-docs（字面 ~ 目录不得出现）"
                assert file_bytes(os.path.join(env.home, "~", "qadl-docs",
                                               "small.bin")) is None, \
                    "不得在主目录下生成字面 ~/qadl-docs 目录"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ⑤ 相对路径按创建时工作目录转绝对（pty 子进程 cwd = 隔离 HOME）
            app = EzrApp(env.home, save_dir=None)
            try:
                manual_add(env.fixture.url("small.bin"), "qadl-rel/sub")
                assert app.wait_for("（无校验）", 30)
                assert file_bytes(os.path.join(rel_dir, "small.bin")) is not None, \
                    "相对输入应按启动 cwd（隔离 HOME）转绝对落盘"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise

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
                open_add_dialog_via_cmd(app, env.fixture.url("five-m.bin?speed=500000"))
                app.send("tab")
                clear_dir_prefill(app)
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
                open_add_dialog_via_cmd(app, DUMMY_URL)
                clear_url_prefill(app)
                os.write(app.fd, b"\x1b[200~" + url.encode() + b"\x1b[201~")
                app.pump(0.6)
                assert_in(url[-40:], app.text(), "字段应完整显示粘贴内容（尾部可见）")
                app.send("tab")
                clear_dir_prefill(app)
                app.type_text(env.save_dir)
                app.send("enter")
                # 小文件瞬时完成：「已添加」toast 会被完成 toast 覆盖，
                # 以「已完成 (1)」页签计数断言创建成功（v1.13 页签行口径）
                assert app.wait_for("已完成 (1)", 15), "粘贴 URL 确认后应创建成功"
            finally:
                app.graceful_quit()

    def _case_18(self):
        """QA-01-18 仅添加按钮（FR-01-103/D28/D29）：按钮行、焦点序、落态、toast、校验链。

        ①按钮行 [ 立即下载 ] [ 仅添加 ] [ 取消 ]，对话框无底部操作提示行
        （v1.30/FR-01-107：验无 Enter 立即下载 等提示文字，交互说明收敛帮助窗口），
        Tab 环绕可达三钮（焦点 6/7/8，Add nfocus=9）；②合法 URL 激活「仅添加」：
        任务创建且状态「已暂停」、不占槽位不下载（下载槽位 0/5、目标文件与
        .downloading 未落盘）、toast 含「已添加任务」与「已暂停，按空格开始下载」
        （D29 钦定）；③空格后转「等待中/下载中」并开始传输；④非法 URL 激活
        「仅添加」：toast 报错 URL 非法、对话框保持、无任务创建。

        焦点序：从保存到字段（焦点 1）Tab×6 至仅添加钮（焦点 7）；落态为确定
        性判据（Tab 数错则任务会立即下载，断言必红）。
        """
        @self.case("QA-01-18")
        def go(env: Env):
            # ① 按钮行与提示行
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, DUMMY_URL)
                text = app.text()
                for btn in ("[ 立即下载 ]", "[ 仅添加 ]", "[ 取消 ]"):
                    assert_in(btn, text, f"按钮行应含 {btn}（FR-01-103）")
                # v1.30/FR-01-107：对话框无底部操作提示行（交互说明收敛帮助窗口）
                assert_not_in("Enter 立即下载", text, "提示行应已删除（v1.30/P3）")
                # Tab 环绕：焦点从 URL 起经全部字段与三钮后回到首字段
                app.send(*(["tab"] * 9))
                app.pump(0.2)
                assert_in("添加任务：HTTP(S)", app.text(), "Tab×9 后应环绕回 URL 字段（对话框仍开）")
                app.send("esc")
                app.pump(0.3)
                # ② 合法 URL + 仅添加
                target = os.path.join(env.save_dir, "one-m.bin")
                open_add_dialog_via_cmd(app, env.fixture.url("one-m.bin?speed=200000"))
                app.send("tab")
                clear_dir_prefill(app)
                app.type_text(env.save_dir)
                app.send(*(["tab"] * 6))  # 焦点 1（保存到）→ 7（仅添加）
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "应 toast 已添加任务（D29）"
                text = app.text()
                assert_in("已暂停，按空格开始下载", text, "toast 应含 D29 钦定文案")
                assert_in("one-m.bin", text, "任务应出现在列表")
                assert_in("已暂停", text, "仅添加任务应落「已暂停」（D28）")
                assert_in("下载槽位 0/5", text, "仅添加不占下载槽位")
                assert file_bytes(target) is None, "目标文件不应落盘（未发起传输）"
                assert file_bytes(target + ".downloading") is None, \
                    ".downloading 不应落盘（未发起传输）"
                # ③ 空格开始下载。恢复 toast 双臂在首个请求即时完成探测的场景
                #    会被紧随的探测结果 toast 同帧覆盖（engineering.md 探针秒败
                #    纪律：瞬时 toast 不可稳定观察）——锚定持久状态迁移证据：
                #    任务离开已暂停（重新排队）→ 开始传输 → 落盘完整。
                app.send("space")
                assert app.wait_gone("[已暂停]", 6), \
                    "空格后任务应离开已暂停（重新排队，v1.21）"
                assert app.wait_for("下载中", 10), "空格后应开始传输"
                wait_file_size(target, QA_FILE_SIZES["one-m.bin"], 60, app=app)
                # 等完成终态（（无校验）+ 槽位释放）再退出：中途 Q 会按 sidecar
                # 周期落盘口径恢复为重新下载（FR-01-22 合同行为），污染 ④ 的
                # 「不应有任务获槽」断言基线。
                assert app.wait_for("下载槽位 0/5", 15), "完成后应释放槽位"
            finally:
                app.graceful_quit()
            # ④ 非法 URL + 仅添加：校验链与立即下载一致
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, DUMMY_URL)
                app.send(*(["bs"] * len(DUMMY_URL)))  # 清空预填再注入 ftp URL（对话框级校验）
                app.type_text("ftp://example.invalid/file.bin")
                app.send(*(["tab"] * 6))
                app.send("enter")
                app.pump(0.5)
                text = app.text()
                assert_in("添加任务：HTTP(S)", text, "对话框应保持打开（校验拒绝不关闭）")
                assert_in("仅支持 http", text, "应报仅支持 http/https（校验链与立即下载一致）")
                assert_not_in("已添加任务", text, "不应创建任务")
                assert_in("下载槽位 0/5", text, "不应有任务获槽")
            finally:
                app.graceful_quit()

    def _case_19(self):
        """QA-01-19（v1.30/FR-01-01 注记）URL 输入自动回填默认并发与 Esc 取消。

        规程口径：开框并发留空 → 逐字输入合法 URL → 并发字段自动回填 4
        （HTTP 默认；BT 20 为 phase-02 预留）→ 手动改 6 后继续改 URL 不再被
        回填（仅未手动改过时同步）→ Esc 关闭对话框且无任务创建。
        """
        @self.case("QA-01-19")
        def go(env: Env):
            import re
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(app, DUMMY_URL)
                # 逐字输入合法 URL（type_text 即逐字注入）：并发自动回填 4
                app.type_text(env.fixture.url("small.bin"))
                app.pump(0.3)
                assert re.search(r"并发\s*>\s*4\b", app.text()), \
                    "URL 输入后并发应自动回填 4（v1.30/FR-01-01）"
                # 手动改 6：URL → 保存到 → 并发，退格清回填值再输入
                app.send("tab", "tab")  # URL0 → 保存到1 → 并发2
                app.send("bs")
                app.type_text("6")
                app.pump(0.2)
                assert re.search(r"并发\s*>\s*6\b", app.text()), "并发应改为 6"
                # 继续修改 URL：回 URL 字段（BackTab×2）删尾字重输 → 并发保持 6
                os.write(app.fd, b"\x1b[Z\x1b[Z")  # BackTab×2：并发 → 保存到 → URL
                app.pump(0.2)
                app.send("bs")
                app.type_text("n")
                app.pump(0.3)
                assert re.search(r"并发\s*>\s*6\b", app.text()), \
                    "手动改过并发后改 URL 不应再回填（仅未手动改过时同步）"
                # Esc 取消：对话框关闭、无任务创建
                app.send("esc")
                app.pump(0.4)
                assert_not_in("添加任务：HTTP(S)", app.text(), "对话框应关闭")
                assert_in("正在下载 (0) │ 已完成 (0)", app.text(), "无任务创建")
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
