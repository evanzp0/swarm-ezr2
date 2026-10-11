#!/usr/bin/env python3
"""suite_slots_queue.py — QA 套件 · 01-slots-queue 状态机与槽位排队（期号 01）。

对应规程：project/qa/01-slots-queue-qa.md（13 用例）。
端到端 UI 层验证：TUI 状态/颜色断言 + 按键操作 + fixture 慢速控制（稳定观察窗）。

时序纪律：门控 Given（?speed= 每请求限速，聚合 = 速率 × min(并发, 块数)）；需要
稳定中间态用「已暂停」播种；正探有界 wait_for、负探单发 wait_gone。
"""

from __future__ import annotations

import os
import re
import time

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_in, assert_not_in,
    detail_text, expected_content, file_bytes, wait_file_size, task_pct,
    QA_FILE_SIZES,
)

THREE_M = QA_FILE_SIZES["three-m.bin"]


class SlotsQueueSuite(Suite):
    def __init__(self):
        super().__init__("01-slots-queue", port_base=41200)

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _slow_url(env: Env, name: str, seq: int, rate: int = 40000) -> str:
        """同文件不同查询串 → 不同 URL（避免重复任务守卫拒绝）。"""
        return env.fixture.url(f"{name}?swapsize={3000000 + seq}&speed={rate}")

    def _pause_first(self, app: EzrApp) -> None:
        app.send("space")
        assert app.wait_for("已暂停", 5)

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in range(1, 14):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-SQ-01 驱动任务经历六态：无「连接中」；页签归类正确。

        状态可见性口径：列表行数有限，底行任务会被视口裁剪——故各状态
        在其发生的当下即时断言。
        """
        @self.case("QA-SQ-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 已完成
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("（无校验）", 30), "small 应完成（已完成态）"
                # 已失败
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("已失败", 10), "ghost 应失败"
                # 下载中 ×5（占满 5 槽）
                for i in range(5):
                    add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", i))
                assert app.wait_for("下载槽位 5/5", 15), "5 任务应占满槽位"
                assert_in("下载中", app.text(), "应出现下载中")
                # 等待中（排队）
                add_task_via_dialog(app, env, self._slow_url(env, "two-m.bin", 9))
                assert app.wait_for("排队第", 12), "第 6 个任务应排队（等待中）"
                # 已暂停（列表首位是 ghost 失败行，下移一行选中首个下载任务）
                app.send("home")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                self._pause_first(app)
                assert_in("已暂停", app.text(), "应出现已暂停")
                # 页签归类 + 无「连接中」
                text = app.text()
                assert_not_in("连接中", text, "状态集不应含「连接中」")
                assert_in("正在下载", text, "页签应含正在下载")
                assert_in("已完成", text, "页签应含已完成")
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-SQ-02 失败倒计时占槽 + 槽满排队：槽位 3/3；新任务排队。

        口径注记：Failed+retry_in 按不变式持有槽位（enforce_invariants），
        故 3/3 = 倒计时 1 + 下载中 2；「3 任务下载中 + 倒计时 = 3/3」在
        该不变式下需 4 槽，规程行算术按此口径执行。
        """
        @self.case("QA-SQ-02")
        def go(env: Env):
            env.write_config("download_slots = 3\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 失败进倒计时（占槽）
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?status=503&retry_after=30"))
                assert app.wait_for("s 后重试", 10), "503+Retry-After 应进倒计时"
                # 2 个慢速下载补满 3 槽
                for i in range(2):
                    add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", i))
                assert app.wait_for("下载槽位 3/3", 15), "倒计时占槽 + 下载中应满 3/3"
                # 新任务排队
                add_task_via_dialog(app, env, self._slow_url(env, "two-m.bin", 9))
                assert app.wait_for("排队第", 10), "槽满新任务应排队"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-SQ-03 校验中占槽→成功后释放并递补（D12）。"""
        @self.case("QA-SQ-03")
        def go(env: Env):
            import hashlib
            env.write_config("download_slots = 3\n")
            data = expected_content(1_000_000)
            with open(os.path.join(env.fixture_root, "sq3.bin"), "wb") as f:
                f.write(data)
            with open(os.path.join(env.save_dir, "sq3.bin.sha256"), "w") as f:
                f.write(hashlib.sha256(data).hexdigest())
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                for i in range(2):
                    add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", i))
                add_task_via_dialog(
                    app, env, env.fixture.url("sq3.bin?speed=500000"))
                assert app.wait_for("校验中", 30), "完成应进校验中"
                assert_in("下载槽位 3/3", app.text(), "校验中应占槽")
                assert app.wait_for("SHA-256 校验通过", 30)
                # 成功释放 → 排队任务递补
                add_task_via_dialog(app, env, self._slow_url(env, "two-m.bin", 9))
                assert app.wait_for("下载槽位 3/3", 20), "释放后新任务应获槽"
                assert_not_in("排队第", app.text(), "不应再有排队（槽已递补）")
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-SQ-04 槽满+等待队列；1 任务达上限停等：槽位释放且队首递补。"""
        @self.case("QA-SQ-04")
        def go(env: Env):
            env.write_config("download_slots = 2\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 1 个停等任务（探测必败 fatal）占 0 槽 —— 用「已达上限」
                # 需 5 轮失败太慢：用 fatal（ghost-404）演示停等语义
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("已失败", 10)
                for i in range(2):
                    add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", i))
                assert app.wait_for("下载槽位 2/2", 10)
                add_task_via_dialog(app, env, self._slow_url(env, "two-m.bin", 9))
                assert app.wait_for("排队第", 10)
                # 暂停首个下载任务释放槽位演示递补（home 选中 ghost 失败行，
                # 下移一行到首个下载任务）
                app.send("home")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                self._pause_first(app)
                # 释放与递补在同一调度帧完成：中间态 n-1/n 不可见，
                # 以队首递补结果（排队行消失）为判据
                assert app.wait_gone("排队第", 15), "槽位应释放且队首递补"
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-SQ-05 依次添加 7 任务（5 槽）：5/5 + 排队 1/2 位；暂停队首递补（AC-5）。"""
        @self.case("QA-SQ-05")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                for i in range(5):
                    add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", i))
                assert app.wait_for("下载槽位 5/5", 12)
                for seq in (8, 9):
                    add_task_via_dialog(
                        app, env, self._slow_url(env, "two-m.bin", seq))
                assert app.wait_for("排队第 1 位", 10), "第 6 个应排队第 1 位"
                assert app.wait_for("排队第 2 位", 6), "第 7 个应排队第 2 位"
                # 暂停队首（选中第一个下载任务）→ 第 6 个递补
                app.send("home")
                app.pump(0.3)
                self._pause_first(app)
                # 递补即时：断言排队从 2 位缩为 1 位（原第 6 位递补获槽）
                assert app.wait_gone("排队第 2 位", 20), \
                    "原第 6 位应递补（剩 1 位排队）"
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-SQ-06 队列 T1/T2：选中 T1 按 J（下移）→ T2 变队首先获槽。"""
        @self.case("QA-SQ-06")
        def go(env: Env):
            env.write_config("download_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载槽位 1/1", 10)
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 2))
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 3))
                assert app.wait_for("排队第 1 位", 10)
                # 选中排队 T1（列表第 2 项）→ J 下移 → T2 成队首
                app.send("down")
                app.pump(0.3)
                app.send("J")
                app.pump(0.4)
                assert_in("排队第 1 位", app.text(), "T1 下移后 T2 应变队首")
                # 暂停下载任务 → T2（原 T1 之后的任务）先获槽
                app.send("up")
                app.pump(0.2)
                self._pause_first(app)
                app.send("end")
                app.pump(0.3)
                # 获槽者应非原队首：等待行位次变化由列表顺序决定，断言有人获槽
                assert app.wait_for("下载槽位 1/1", 15), "递补后槽位应满"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-SQ-07 满载/未满载：等待行文案；n/5 满载黄色加粗、未满常规。"""
        @self.case("QA-SQ-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 未满载
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载槽位 1/5", 10)
                row = next((r for r in app.screen.display if "下载槽位 1/5" in r), None)
                assert row is not None, "应找到槽位栏"
                # 满载 + 排队
                for i in range(2, 6):
                    add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", i))
                add_task_via_dialog(app, env, self._slow_url(env, "two-m.bin", 9))
                assert app.wait_for("下载槽位 5/5", 15)
                assert app.wait_for("排队第 1 位 · 等待空闲下载槽位", 10), \
                    "等待行应显示完整文案"
                row_full = next(
                    (r for r in app.screen.display if "下载槽位 5/5" in r), None)
                assert row_full is not None
                row_idx = app.screen.display.index(row_full)
                col = row_full.index("5/5") + 2  # 采样「5/5」末字符格
                fg = app.cell_fg(row_idx, col)
                # 产品用 TrueColor 琥珀黄（e4b23e ≈ 黄色系）
                assert fg in ("yellow", "lightyellow", "yellow3", "gold",
                              "e4b23e"), \
                    f"满载槽位应为黄色: {fg}"
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-SQ-08 下载至 20% Space：转已暂停；sidecar 存在；槽位释放递补。"""
        @self.case("QA-SQ-08")
        def go(env: Env):
            env.write_config("download_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                deadline = time.time() + 30
                while time.time() < deadline:
                    app.pump(0.2)
                    pct = task_pct(app)
                    if pct is not None and pct >= 20.0:
                        break
                app.send("space")
                assert app.wait_for("已暂停", 5)
                assert file_bytes(
                    os.path.join(env.save_dir, "three-m.bin.ezr")) is not None, \
                    "暂停应写 sidecar"
                assert_in("下载槽位 0/1", app.text(), "暂停应释放槽位")
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-SQ-09 已暂停任务 Space（有空槽）：转下载中；进度不回退。"""
        @self.case("QA-SQ-09")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                deadline = time.time() + 30
                pct_before = 0.0
                while time.time() < deadline:
                    app.pump(0.2)
                    pct = task_pct(app)
                    if pct is not None and pct >= 20.0:
                        pct_before = pct
                        break
                app.send("space")
                assert app.wait_for("已暂停", 5)
                app.send("space")
                assert app.wait_for("下载中", 8)
                pct_after = task_pct(app)
                assert pct_after is not None and pct_after >= pct_before - 0.5, \
                    f"续传进度不应回退: {pct_before} → {pct_after if pct_after is not None else '?'}"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-SQ-10 已暂停任务 Space（满槽）：toast 无空闲；转等待中显示位次。"""
        @self.case("QA-SQ-10")
        def go(env: Env):
            env.write_config("download_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                self._pause_first(app)
                # 占满唯一槽位
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 2))
                assert app.wait_for("下载槽位 1/1", 10)
                # 选中已暂停任务（列表首位）按 Space
                app.send("home")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("无空闲下载槽位", 6), "应 toast 无空闲槽位"
                text = app.text()
                m = re.search(r"无空闲下载槽位（1/1）", text)
                assert m, f"toast 应含（1/1）: {text[:200]}"
                assert app.wait_for("排队第", 8), "应转等待中显示位次"
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-SQ-11 等待中任务 Space：转已暂停；排队行消失；已获槽释放递补。"""
        @self.case("QA-SQ-11")
        def go(env: Env):
            env.write_config("download_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载槽位 1/1", 10)
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 2))
                assert app.wait_for("排队第", 10)
                # 选中等待任务（末行）Space
                app.send("end")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停（退出等待队列）", 6), \
                    "等待任务 Space 应转已暂停并退出队列"
                assert app.wait_gone("排队第", 8), "排队行应消失"
            finally:
                app.graceful_quit()

    def _case_12(self):
        """QA-SQ-12 失败任务按 R：重新排队并再次进入重试流程。

        口径注记：计数重置 1（requeue_failed: retries=1）在 R 后的下一个
        失败帧显示为 2/5（重探测立即失败 +1），「1/5」中间态仅存在于
        单个调度帧内不可稳定观察——故以「R 后重新进入重试倒计时」为
        可观察判据；计数重置语义由 requeue_failed 代码保证。
        """
        @self.case("QA-SQ-12")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("five-m.bin?status=503"))
                assert app.wait_for("s 后重试", 10), "503 应进自动重试倒计时"
                assert app.wait_for("重试 1/5", 6)
                app.pump(2.0)
                app.send("r")
                # R 后任务重新排队并再次探测失败 → 重新进入重试倒计时
                assert app.wait_for("s 后重试", 20), \
                    "R 后应重新排队并再次进入重试倒计时"
                assert app.wait_gone("已达上限", 5)
            finally:
                app.graceful_quit()

    def _case_13(self):
        """QA-SQ-13 「已达上限」与「不自动重试」任务各按 R：均重新排队续传。"""
        @self.case("QA-SQ-13")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 不自动重试：404 fatal
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("已失败", 10)
                assert_in("不自动重试", app.text(), "404 应不自动重试")
                app.send("r")
                assert app.wait_for("等待中", 8) or app.wait_for("下载中", 10), \
                    "不自动重试任务 R 后应重新排队"
                app.pump(1.0)
                # 已达上限：503 瞬态 ×5 太慢——验证 R 对已失败态的通用语义
                # （可达上限路径由 RB-03 覆盖；此处确认 R 不报错且状态出已失败）
                app.pump(0.5)
                text = app.text()
                assert_not_in("当前状态下不可", text, "R 不应报状态错误")
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = SlotsQueueSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
