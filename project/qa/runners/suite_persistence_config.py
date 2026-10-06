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
    wait_file_size, task_pct, QA_FILE_SIZES,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]
THREE_M = QA_FILE_SIZES["three-m.bin"]


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
        for i in range(1, 10):
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
                assert_in("任务 4", text, "全部 4 任务注册表还原")
                # 下载中任务重启后自动续传至完成（结果口径，避免速率竞速）
                assert app.wait_for("已完成 2", 90), "退出前下载中的任务应续传至完成"
                # 已完成历史（含顺序：small.bin 先于 three-m）在第二页签
                app.send("tab")
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
            env.write_config('max_speed = "1 MB/s"\n')
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
                assert_in("4 并发", d, "默认并发 4")
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
            env.write_config('block_size_http = "abc"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin?speed=900000"))
                assert app.wait_for("下载中", 8)
                assert_in("1 MB/块", detail_text(app), "非法块大小回退 1MB")
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise
            # ② download_slots = 0 → 5
            env.write_config("download_slots = 0\n")
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
            env.write_config("max_retries = -1\n")
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
            env.write_config('http_concurrency = 99\nmax_speed = "2 MB/s"\n')
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
            env.write_config('max_speed = "xyz"\n')
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
        """QA-PC-05 block_size_http=262144/2097152 下载 5MB：x/20·256KB/块 与 x/3·2MB/块。"""
        @self.case("QA-PC-05")
        def go(env: Env):
            # 每轮独立子目录：第二轮同 URL+同目录会被重复守卫拦截（QA-01-16）
            for seq, (bs, expect) in enumerate(((262144, "20"), (2097152, "3"))):
                sub = os.path.join(env.save_dir, f"bs{seq}")
                env.write_config(f"block_size_http = {bs}\n")
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
                    disp = "256 KB/块" if bs == 262144 else "2 MB/块"
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
            env.write_config('download_slots = 2\nmax_speed = "1 MB/s"\n')
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
            env.write_config('max_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            add_task_via_dialog(app, env, env.fixture.url("five-m.bin"))
            self._pause_at_progress(app, min_pct=30.0)
            app.send("space")
            assert app.wait_for("下载中", 5)
            app.pump(0.5)
            code = self._quit_and_check(app, "q")
            assert code == 0, f"Q 优雅退出码 0，得到 {code}"
            sc = os.path.join(env.save_dir, "five-m.bin.ezr")
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
        """QA-PC-09 Esc / Ctrl+C 退出：语义同 Q（停传、保断点、存注册表）无花屏（AC-9）。"""
        @self.case("QA-PC-09")
        def go(env: Env):
            # 每轮独立子目录：第二轮同 URL+同目录会被重复守卫拦截（QA-01-16）；
            # 引擎侧限速保证续传窗口（同 PC-08 口径）
            for seq, key in enumerate(("esc", "ctrl_c")):
                sub = os.path.join(env.save_dir, f"k{seq}")
                env.write_config('max_speed = "1 MB/s"\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                add_task_via_dialog(app, env, env.fixture.url("five-m.bin"),
                                    save_dir=sub)
                self._pause_at_progress(app, min_pct=20.0)
                app.send("space")
                assert app.wait_for("下载中", 5)
                app.pump(0.5)
                code = self._quit_and_check(app, key)
                assert code == 0, f"{key} 优雅退出码应为 0，得到 {code}"
                sc = os.path.join(sub, "five-m.bin.ezr")
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


if __name__ == "__main__":
    suite = PersistenceConfigSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
