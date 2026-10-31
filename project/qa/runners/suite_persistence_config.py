#!/usr/bin/env python3
"""suite_persistence_config.py — QA 套件 · 01-persistence-config 持久化配置与生命周期（期号 01）。

对应规程：project/qa/01-persistence-config-qa.md（9 用例）。
端到端 UI 层验证：CLI 启动/退出操作 + ~/.ezr 磁盘产物观察 + TUI 状态断言 + 双实例进程操作。
只经用户可达入口，不进入项目内部 API。

口径注记（QA-PC-04，已裁决）：http_concurrency 非法值原 Gherkin（99→4 回退）与
单元测试（999→64 钳制）矛盾；操作者裁决改规格随实现（钳制 1–64 上限），Gherkin 场景 04
与规程均已同步为回退/钳制语义，本套件断言与两者一致。
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time

from harness import (
    EZR_BIN, Env, EzrApp, Suite, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, detail_text, file_bytes, read_file,
    open_add_dialog_via_cmd, wait_file_size, task_pct, QA_FILE_SIZES,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]
THREE_M = QA_FILE_SIZES["three-m.bin"]


def _mini_torrent(name: str) -> bytes:
    """最小合法单文件 .torrent（5 字节 payload，20 字节伪 pieces）。"""
    buf = bytearray()
    buf += b"d8:announce14:udp://seed/ann"
    buf += b"4:infod6:lengthi5e4:name%d:" % len(name)
    buf += name.encode()
    buf += b"12:piece lengthi16384e6:pieces20:"
    buf += bytes(20)
    buf += b"ee"
    return bytes(buf)


class PersistenceConfigSuite(Suite):
    def __init__(self):
        super().__init__("01-persistence-config", port_base=41500)

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _pause_at_progress(app: EzrApp, min_pct: float, timeout: float = 40.0) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.2)
            pct = task_pct(app)
            if pct is not None and pct >= min_pct:
                break
        app.send("space")
        assert app.wait_for("已暂停", 5), "暂停后应显示已暂停"

    @staticmethod
    def _wait_sidecar(app: EzrApp, sc: str, timeout: float = 15.0) -> None:
        """等待 sidecar 首次周期落盘（2s 周期）。

        Space 暂停不触发 sidecar 写（快路径暂停可发生在首个 2s 周期之前），
        「Q 后 sidecar 保留」断言前必须等到底层已有周期落盘产物，否则
        慢速门控下的退出窗口会落在首刷之前（v138 QA 会话实测）。
        """
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.2)
            if file_bytes(sc) is not None:
                return
            time.sleep(0.1)
        raise AssertionError(f"{timeout}s 内 sidecar 未落盘: {sc}")

    @staticmethod
    def _quit_and_check(app: EzrApp, key: str) -> int:
        """按 key（q/esc/ctrl_c）优雅退出并回收退出码。"""
        app.send(key)
        deadline = time.time() + 8
        while time.time() < deadline:
            app.pump(0.1)
            if app.exit_code is not None:
                break
        code = app.exit_code
        app.close()
        return code if code is not None else -1

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in list(range(1, 10)) + [13]:
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-PC-01 预置各状态任务→优雅退出→重启：任务/历史/顺序与续传点还原。

        口径注记（已修正）：重启后默认页签为「正在下载」（全部非终态任务），
        已完成任务在第二页签——按 Tab 验证含历史在内的全部还原
        （Gherkin：全部任务（含已完成与已失败历史）与列表顺序完整还原）。
        """
        @self.case("QA-PC-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            add_task_via_dialog(app, env, env.fixture.url("small.bin"))
            assert app.wait_for("已完成", 10)
            add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
            assert app.wait_for("已失败", 10)
            add_task_via_dialog(app, env, env.fixture.url("five-m.bin?speed=800000"))
            self._pause_at_progress(app, min_pct=25.0)
            paused_pct = detail_text(app)
            add_task_via_dialog(app, env, env.fixture.url("three-m.bin?speed=800000"))
            assert app.wait_for("下载中", 8)
            code = self._quit_and_check(app, "q")
            assert code == 0, f"优雅退出码应为 0，得到 {code}"
            # 重启还原：默认页签含失败/暂停/下载中（自动续传）任务
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("ghost-404", 8), "失败任务应还原"
                text = app.text()
                assert_in("已失败", text, "失败状态保持")
                assert_in("已暂停", text, "暂停状态保持")
                # v1.13 页签行计数：1 已完成 + 3 非完成（失败/暂停/下载中）= 全部 4 任务
                assert_in("正在下载 (3)", text, "非完成 3 任务还原（v1.13 页签行口径）")
                assert_in("已完成 (1)", text, "已完成 1 任务还原")
                # 下载中任务重启后自动续传至完成（结果口径，避免速率竞速）
                assert app.wait_for("已完成 (2)", 90), "退出前下载中的任务应续传至完成（v1.13 页签行口径）"
                # 已完成历史（含顺序：small.bin 先于 three-m）在第二页签
                # （v1.30：Tab 改面板焦点，页签键 ] 承载切页）
                app.send("]")
                app.pump(0.3)
                text = app.text()
                assert_in("small.bin", text, "已完成历史应还原（第二页签）")
                assert_in("three-m.bin", text, "续传完成任务应还原（第二页签）")
                assert text.index("small.bin") < text.index("three-m.bin"), \
                    "列表顺序应与添加顺序一致"
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-PC-02 添加任务后检查主目录：.ezr/state 注册表；config 缺失或合法；sidecar。"""
        @self.case("QA-PC-02")
        def go(env: Env):
            # 引擎侧限速拉开下载窗：sidecar 随首块落盘，完成前有稳定观察窗
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin?speed=700000"))
                assert app.wait_for("下载中", 8)
                state_dir = os.path.join(env.home, ".ezr", "state")
                assert os.path.isdir(state_dir), "应存在 .ezr/state 目录"
                reg = os.path.join(state_dir, "registry.json")
                assert file_bytes(reg) is not None, "应存在注册表文件 registry.json"
                cfg = os.path.join(env.home, ".ezr", "config.toml")
                if file_bytes(cfg) is not None:
                    text = open(cfg, encoding="utf-8").read()
                    assert "==" not in text.replace("== ", ""), "配置应为合法 TOML"
                # sidecar 随首块落盘（有界轮询，完成前可观察；不预设固定延迟）
                sc = os.path.join(env.save_dir, "three-m.bin.ezr")
                deadline = time.time() + 10
                while time.time() < deadline and file_bytes(sc) is None:
                    app.pump(0.2)
                assert file_bytes(sc) is not None, "sidecar 应随目标文件"
                app.send("space")
                app.wait_for("已暂停", 5)
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-PC-03 删除 config 后下载：默认并发 4/槽位 5/块 1MB/download_dir 默认。"""
        @self.case("QA-PC-03")
        def go(env: Env):
            cfg = os.path.join(env.home, ".ezr", "config.toml")
            if file_bytes(cfg) is not None:
                os.remove(cfg)
            app = EzrApp(env.home, save_dir=None)  # 保存目录留空 → 走默认下载目录
            try:
                # 对话框目录字段留空（save_dir=""）→ 默认 download_dir
                add_task_via_dialog(app, env, env.fixture.url("five-m.bin?speed=900000"),
                                    save_dir="")
                assert app.wait_for("下载中", 8)
                d = detail_text(app)
                # v1.14：详情类型行无并发数，默认并发 4 经头部数据面字段核验
                assert app.wait_for("并发 4", 15), "默认并发 4（头部数据面）"
                assert_in("1 MB/块", d, "默认块 1MB")
                text = app.text()
                assert_in("下载槽位", text, "槽位栏存在")
                m = re.search(r"下载槽位 (\d+)/(\d+)", text)
                assert m and m.group(2) == "5", f"默认槽位 5: {m.group(0) if m else '未见'}"
                target = os.path.join(env.home, "Downloads", "five-m.bin")
                wait_file_size(target, FIVE_M, 90, app=app)
                assert_file_content(target, FIVE_M)
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-PC-04 非法配置回退/钳制：块 1MB/槽 5/重试 5/并发钳 64/不限速（规格已随实现）。"""
        @self.case("QA-PC-04")
        def go(env: Env):
            # ① block_size_http = abc → TOML 解析失败整个文件按默认 → 1MB/块
            env.write_config('[http]\nblock_size_http = "abc"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin?speed=900000"))
                assert app.wait_for("下载中", 8)
                assert_in("1 MB/块", detail_text(app), "非法块大小回退 1MB")
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ①' block_size_http = 256KB（v1.16/FR-01-104 钳制行）：生效值下限 1MB，
            #    小于 1MB 一律按 1MB 生效（配置文件是用户可达入口，端到端口径不变）
            env.write_config("[http]\nblock_size_http = 262144\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env,
                                    env.fixture.url("two-m.bin?speed=900000"))
                assert app.wait_for("下载中", 8)
                assert_in("1 MB/块", detail_text(app),
                          "256KB 应按 FR-01-104 下限钳制为 1MB 生效（v1.16）")
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ② download_slots = 0 → 5
            env.write_config("[common]\ndownload_slots = 0\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                app.wait_for("已完成", 10)
                m = re.search(r"下载槽位 \d+/(\d+)", app.text())
                assert m and m.group(1) == "5", f"非法槽位回退 5: {m.group(0) if m else '?'}"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ③ max_retries = -1 → TOML 非法 → 全默认（启动不报错，可正常下载）
            env.write_config("[http]\nmax_retries = -1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                app.wait_for("已完成", 10)
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ④ http_concurrency = 99 → 钳制 64（规格已随实现，Gherkin/规程已同步）
            # 口径：详情并发为「活跃连接数」随限速/块完成波动，不等于配置值；
            # 确定性口径 = sidecar 落盘的任务 concurrency（spec 钳制后值）。
            env.write_config('[http]\nhttp_concurrency = 99\n\n[common]\nmax_speed = "2 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("ten-m.bin"))
                assert app.wait_for("下载中", 8)
                # sidecar 每 2s 周期落盘（Space 暂停不写）：有界轮询等待首刷
                sc_path = os.path.join(env.save_dir, "ten-m.bin.ezr")
                deadline = time.time() + 10
                raw = None
                while time.time() < deadline:
                    app.pump(0.2)
                    raw = read_file(sc_path)
                    if raw is not None:
                        break
                assert raw is not None, "下载中应周期落盘 sidecar"
                sc = json.loads(raw.decode("utf-8"))
                conc = sc["task"]["concurrency"]
                assert conc == 64, f"并发应钳制 64: {conc}"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ⑤ max_speed = xyz → 解析 0 不限速
            env.write_config('[common]\nmax_speed = "xyz"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin"))
                wait_file_size(os.path.join(env.save_dir, "three-m.bin"), THREE_M,
                               30, app=app)
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise

    def _case_05(self):
        """QA-PC-05 block_size_http=4MB/2MB 下载 5MB：x/2·4 MB/块 与 x/3·2 MB/块。

        v1.16 同步（QA v138 会话）：原 256KB 行因 FR-01-104 生效值下限 1MB 改判
        4MB（规程 01-persistence-config-qa.md 同轮已同步）。
        """
        @self.case("QA-PC-05")
        def go(env: Env):
            # 每轮独立子目录：第二轮同 URL+同目录会被重复守卫拦截（QA-01-16）
            for seq, (bs, expect, disp) in enumerate(
                    ((4_194_304, "2", "4 MB/块"), (2_097_152, "3", "2 MB/块"))):
                sub = os.path.join(env.save_dir, f"bs{seq}")
                env.write_config(f"[http]\nblock_size_http = {bs}\n")
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(
                        app, env, env.fixture.url("five-m.bin?speed=900000"),
                        save_dir=sub)
                    assert app.wait_for("下载中", 8)
                    m = re.search(r"分块\s+\d+/(\d+) ·", detail_text(app))
                    assert m, f"详情应有分块行（bs={bs}）"
                    assert m.group(1) == expect, \
                        f"bs={bs} 分块总数应 {expect}: 得 {m.group(1)}"
                    assert_in(disp, detail_text(app), f"bs={bs} 块大小显示")
                    app.send("space")
                    app.wait_for("已暂停", 5)
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_06(self):
        """QA-PC-06 download_slots=2 添加 4 任务：前 2 下载；槽位栏 n/2；其余排队。"""
        @self.case("QA-PC-06")
        def go(env: Env):
            # 引擎侧限速：fixture ?speed 为单请求口径，多连接聚合后失效；
            # max_speed 保证任务占用槽位的时间窗（TP-01 权威口径）
            env.write_config('[common]\ndownload_slots = 2\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                for i in range(4):
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(f"three-m.bin?swapsize={3000000 + i}"))
                assert app.wait_for("下载槽位 2/2", 12), "应满载 2/2"
                text = app.text()
                assert_in("排队第", text, "第 3/4 个任务应排队")
                assert text.count("下载中") >= 2, "应恰有 2 个下载中（占满槽位）"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-PC-07 运行中二次启动：第二实例「ezr 已在运行」非零退出；首实例正常（AC-11）。"""
        @self.case("QA-PC-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("five-m.bin?speed=600000"))
                assert app.wait_for("下载中", 8)
                env_full = {
                    "HOME": env.home, "TERM": "xterm-256color",
                    "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                    "LANG": "C.UTF-8",
                }
                r = subprocess.run([EZR_BIN, "--version"], capture_output=True,
                                   text=True, env=env_full, timeout=15)
                # --version 快速路径不持锁；用真实启动（无参数）触发锁冲突
                r2 = subprocess.run([EZR_BIN], input="", capture_output=True,
                                    text=True, env=env_full, timeout=15)
                assert r2.returncode != 0, "第二实例应非零退出"
                assert "ezr 已在运行" in r2.stderr, \
                    f"第二实例应提示已在运行: {r2.stderr!r}"
                app.pump(0.5)
                assert_in("下载中", app.text(), "首实例应不受影响")
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-PC-08 下载至 30% Q→重启：断点保留、终端恢复、续传完成（AC-9）。"""
        @self.case("QA-PC-08")
        def go(env: Env):
            # 引擎侧限速：续传窗口多秒级，「下载中」徽标可被多帧捕获；
            # 1 MB/s 保证退出时任务仍在途（sidecar 未删除）
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            add_task_via_dialog(app, env, env.fixture.url("five-m.bin"))
            self._pause_at_progress(app, min_pct=30.0)
            app.send("space")
            assert app.wait_for("下载中", 5)
            sc = os.path.join(env.save_dir, "five-m.bin.ezr")
            self._wait_sidecar(app, sc)  # 等首个周期落盘，Q 时刻断点必然在盘
            code = self._quit_and_check(app, "q")
            assert code == 0, f"Q 优雅退出码 0，得到 {code}"
            assert file_bytes(sc) is not None, "Q 后 sidecar 应保留断点"
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("下载中", 15), "重启应续传"
                wait_file_size(os.path.join(env.save_dir, "five-m.bin"), FIVE_M,
                               90, app=app)
                assert_file_content(os.path.join(env.save_dir, "five-m.bin"), FIVE_M)
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-PC-09 Esc 主界面无操作 + Ctrl+C 优雅退出：停传、保断点、存注册表、无花屏。

        退出语义（01-tui-display-07 v1.30 口径，操作者 20261027 裁决定稿）：
        Esc 主界面无任何动作，退出仅 Q/Ctrl+C；Ctrl+C 全局直通 = 优雅退出，
        keys.rs 同 q 置 quit，AC-9 终端哨兵照常覆盖。
        01-persistence-config-09 feature 场景已按裁决同步修订
        （Esc 主界面无动作 + Ctrl+C 优雅退出）。
        """
        @self.case("QA-PC-09")
        def go(env: Env):
            # 每轮独立子目录：第二轮同 URL+同目录会被重复守卫拦截（QA-01-16）；
            # 引擎侧限速保证续传窗口（同 PC-08 口径）
            for seq, key in enumerate(("esc", "ctrl_c")):
                sub = os.path.join(env.save_dir, f"k{seq}")
                env.write_config('[common]\nmax_speed = "1 MB/s"\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                add_task_via_dialog(app, env, env.fixture.url("five-m.bin"),
                                    save_dir=sub)
                self._pause_at_progress(app, min_pct=20.0)
                app.send("space")
                assert app.wait_for("下载中", 5)
                sc = os.path.join(sub, "five-m.bin.ezr")
                self._wait_sidecar(app, sc)  # 等首个周期落盘（同 PC-08 口径）
                if key == "esc":
                    # v1.30：Esc 主界面无操作——应用不退出、任务继续下载；
                    # 收尾改以 Q 优雅退出验证断点保留（停传、存注册表同口径）
                    app.send("esc")
                    app.pump(1.0)
                    assert app.exit_code is None,                         "Esc 不应退出应用（v1.30 主界面无操作）"
                    assert app.wait_for("下载中", 5), "Esc 后任务应继续下载"
                    code = self._quit_and_check(app, "q")
                else:
                    code = self._quit_and_check(app, key)
                assert code == 0, f"{key} 轮优雅退出码应为 0，得到 {code}"
                assert file_bytes(sc) is not None, f"{key} 后 sidecar 应保留"
                # 重启无花屏：画面正常渲染并可恢复下载（续传至完成）
                app2 = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    assert app2.wait_for("下载中", 15), f"{key} 后重启应续传"
                    text = app2.text()
                    assert_in("EZR Downloader", text, f"{key} 后重启画面应正常渲染")
                    wait_file_size(os.path.join(sub, "five-m.bin"), FIVE_M,
                                   90, app=app2)
                    assert_file_content(os.path.join(sub, "five-m.bin"), FIVE_M)
                finally:
                    app2.graceful_quit()


    def _case_13(self):
        """QA-PC-13（v1.18 新增）全量 36 键合法非默认值生效冒烟（D62③）。

        零警告 + 可观察键逐项生效（download_dir 预填/落盘、download_slots、
        block_size_http、http_concurrency 回填、max_speed）+ BT 键预填生效
        （bt_concurrency、http/udp trackers）+ 下载完成产物完整。
        """
        @self.case("QA-PC-13")
        def go(env: Env):
            cfg_dir = os.path.join(env.home, "qa13-dl")
            cfg = (
                "[common]\n"
                f'download_dir = "{cfg_dir}"\n'
                "download_slots = 3\n"
                'max_speed = "2 MB/s"\n'
                "\n"
                '[[common.proxy]]\n'
                'name = "QA13-HTTP"\n'
                'type = "http"\n'
                'ip = "127.0.0.1"\n'
                "port = 18801\n"
                "\n"
                '[[common.proxy]]\n'
                'name = "QA13-SOCKS5"\n'
                'type = "socks5"\n'
                'ip = "127.0.0.2"\n'
                "port = 11802\n"
                'username = "qa13"\n'
                'password = "secret"\n'
                "\n"
                "[http]\n"
                "block_size_http = 2097152\n"
                "http_concurrency = 6\n"
                "max_retries = 7\n"
                "auto_retry = false\n"
                "backoff_initial = 4.5\n"
                "backoff_cap = 40.0\n"
                "progress_tick_ms = 120\n"
                "sidecar_flush_secs = 3\n"
                "throttle_burst_secs = 0.6\n"
                "throttle_sleep_ms = 25\n"
                "dns_race_grace_ms = 900\n"
                "\n"
                "[bt]\n"
                'http_trackers = ["http://cfg13a/ann", "http://cfg13b/ann"]\n'
                'udp_trackers = ["udp://cfg13u/ann"]\n'
                "bt_concurrency = 61\n"
                "peer_pipeline = 32\n"
                "upload_slots = 5\n"
                "upload_read_concurrency = 96\n"
                "idle_eviction_secs = 45\n"
                "connect_timeout_secs = 4\n"
                "handshake_timeout_secs = 11\n"
                "request_timeout_secs = 18\n"
                "requests_per_piece = 5\n"
                "rechoke_interval_secs = 9\n"
                "optimistic_interval_secs = 25\n"
                "progress_emit_min_interval_ms = 240\n"
                "tracker_numwant = 150\n"
                "peer_retry_base_secs = 6\n"
                "peer_retry_cap_secs = 100\n"
                "tracker_backoff_start_secs = 7\n"
                "tracker_backoff_cap_secs = 50\n"
                "udp_base_timeout_secs = 12\n"
                "udp_retries = 2\n"
                "seed_duration_secs = 1500.0\n"
            )
            env.write_config(cfg)
            app = EzrApp(env.home, save_dir=None)
            try:
                # 全量合法值 → 零警告（toast 3s 窗内断言）
                assert app.wait_for("EZR Downloader", 10), "应正常启动"
                assert_not_in("已忽略", app.text(),
                              "全量合法配置不得出现任何忽略类警告")
                # download_dir 预填 + http_concurrency 回填 + 下载落盘
                open_add_dialog_via_cmd(app, env.fixture.url("small.bin"))
                app.send("tab")
                # 字段超宽左截断显示（…+尾部）：按尾部可见段断言
                assert_in(cfg_dir[-16:], app.text(),
                          "保存目录字段应预填配置 download_dir（D62②，字段超宽左截断按尾部断言）")
                assert_in("6", app.text(), "并发应自动回填 http_concurrency=6")
                app.send("enter")
                assert app.wait_for("（无校验）", 60), "small.bin 应下载完成"
                assert file_bytes(os.path.join(cfg_dir, "small.bin")) is not None, \
                    "下载应落配置 download_dir（预填链生效）"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # BT 键生效：BT 对话框预填（bt_concurrency=61 + trackers 合并 + 目录预填）
            app = EzrApp(env.home, save_dir=None)
            try:
                tdir = os.path.join(env.home, "qa13-torrents")
                os.makedirs(tdir, exist_ok=True)
                tpath = os.path.join(tdir, "qa13.torrent")
                with open(tpath, "wb") as f:
                    f.write(_mini_torrent("qa13"))
                app.send("a")
                assert app.wait_for("请输入命令或链接", 5), "命令窗口应打开"
                app.type_text(tpath)
                app.send("enter")
                assert app.wait_for("添加任务：BT", 5), "应分流打开 BT 对话框"
                assert_in("61", app.text(), "并发应预填 [bt] bt_concurrency=61")
                assert_in("udp://cfg13u/ann", app.text(),
                          "trackers 应预填合并 [bt] udp_trackers")
                assert_in(cfg_dir[-16:], app.text(),
                          "保存到应预填配置 download_dir（D48，字段超宽左截断按尾部断言）")
                app.send("esc")
                app.pump(0.3)
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise


if __name__ == "__main__":
    suite = PersistenceConfigSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
