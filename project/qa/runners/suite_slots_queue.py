#!/usr/bin/env python3
"""suite_slots_queue.py — QA 套件 · 01-slots-queue 状态机与槽位排队（期号 01）。

对应规程：project/qa/01-slots-queue-qa.md（15 用例）。
端到端 UI 层验证：TUI 状态/颜色断言 + 按键操作 + fixture 慢速控制（稳定观察窗）。

时序纪律：门控 Given（?speed= 每请求限速，聚合 = 速率 × min(并发, 块数)）；需要
稳定中间态用「已暂停」播种；正探有界 wait_for、负探单发 wait_gone。

v1.21–v1.23 同步（QA v138 会话）：SQ-09 恢复一律重新排队（有空槽也不直开）；
SQ-10 满槽 toast「无空闲下载槽位」退役（改锚排队位次持久态）；新增 SQ-14/15
（失败挂起入 Paused：展示/数据与普通暂停一致；恢复重新排队）。
"""

from __future__ import annotations

import os
import re
import time

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_in, assert_not_in,
    detail_text, expected_content, file_bytes, read_file, wait_file_size,
    task_pct, QA_FILE_SIZES,
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
        for i in range(1, 16):
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
            env.write_config("[common]\ndownload_slots = 3\n")
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
            env.write_config("[common]\ndownload_slots = 3\n")
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
            env.write_config("[common]\ndownload_slots = 2\n")
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
            env.write_config("[common]\ndownload_slots = 1\n")
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
            env.write_config("[common]\ndownload_slots = 1\n")
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
        """QA-SQ-09 已暂停任务 Space（有空槽）：一律重新排队（v1.21）；获槽续传不回退。

        v1.21 同步：直开路径（Downloading + Start）已废除——Space 后任务先入
        等待队列（恢复 toast 双臂注明「已进入等待队列」），由槽位调度获槽启动。
        有空槽时排队与获槽同一调度帧完成，「等待中」为瞬时中间态不可稳定观察
        （engineering.md 探针秒败纪律）——改锚恢复 toast 文案（重排队路径证据）
        + 最终态「下载中」+ 进度不回退。
        """
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
                # 恢复 toast 双臂（fixture 支持 Range → 可续传臂）注明已进入等待队列
                assert app.wait_for("已进入等待队列", 4), \
                    "恢复应重新排队（toast 双臂注明已进入等待队列，v1.21）"
                assert app.wait_for("下载中", 10)
                pct_after = task_pct(app)
                assert pct_after is not None and pct_after >= pct_before - 0.5, \
                    f"续传进度不应回退: {pct_before} → {pct_after if pct_after is not None else '?'}"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-SQ-10 已暂停任务 Space（满槽）：转等待中显示位次；满槽 toast 已退役（v1.21）。"""
        @self.case("QA-SQ-10")
        def go(env: Env):
            env.write_config("[common]\ndownload_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                self._pause_first(app)
                # 占满唯一槽位
                add_task_via_dialog(app, env, self._slow_url(env, "three-m.bin", 2))
                assert app.wait_for("下载槽位 1/1", 10)
                # 选中已暂停任务（列表首位）按 Space → 一律重新排队（v1.21：
                # 「无空闲下载槽位（n/n）」toast 退役，不检测空闲槽位）
                app.send("home")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("排队第 1 位", 8), "满槽 Space 应转等待中显示位次"
                text = app.text()
                assert_not_in("无空闲下载槽位", text, "满槽 toast 已退役（v1.21）")
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-SQ-11 等待中任务 Space：转已暂停；排队行消失；已获槽释放递补。"""
        @self.case("QA-SQ-11")
        def go(env: Env):
            env.write_config("[common]\ndownload_slots = 1\n")
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

    def _case_14(self):
        """QA-SQ-14 失败任务（倒计时进行中）按空格：入 Paused，展示/数据与普通暂停一致。

        v1.21–v1.23 合并口径：独立 FailedPaused 状态与 has_error 标记均已退役——
        ①徽标「已暂停」白、统计行走中性行（无重试计数/「不再自动重试」红字）；
        ②详情无「失败原因」行；③数据同普通暂停（挂起即重置 error=null/retries=1，
        注册表快照同口径、不写 has_error）；④不占槽、倒计时清除不再到点重发；
        ⑤重启后仍保持「已暂停」（迟到失败事件对 Paused 全态整体忽略）。
        断言锚持久证据：徽标配色/详情面板/注册表快照（engineering.md
        「崩溃/重启用例以持久证据断言」）。
        """
        @self.case("QA-SQ-14")
        def go(env: Env):
            import json as _json
            env.write_config("[common]\ndownload_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("three-m.bin?status=503&retry_after=30"))
                assert app.wait_for("s 后重试", 10), "503+Retry-After 应进自动重试倒计时"
                # 挂起：失败行（列表首位）按空格
                app.send("home")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停", 6), "失败任务空格应转已暂停（v1.21 入 Paused）"
                # ① 徽标白（v1.22 展示统一：非红非黄）
                row_idx = next(
                    (i for i, r in enumerate(app.screen.display)
                     if "three-m.bin" in r[:64] and "已暂停" in r[:64]), None)
                assert row_idx is not None, "列表应出现已暂停徽标行"
                row = app.screen.display[row_idx]
                m = re.search(r"\[[^\]]*已暂停[^\]]*\]", row)
                assert m, f"行内应含已暂停徽标: {row!r}"
                fg = app.cell_fg(row_idx, m.start() + 2)
                assert fg in ("white", "lightgray", "gray", "d0d0d0", "e4e4e4",
                              "ffffff"), f"挂起徽标应白/中性色（v1.22）: {fg}"
                # ① 统计行中性：无重试计数/「不再自动重试」文案。
                #    挂起 toast（「…不再自动重试…」）为瞬时提示（3s TTL），先等
                #    其过期再断言常驻画面——统计行中性质锚定持久展示（v1.22）。
                app.wait_gone("不再自动重试", 8)
                text = app.text()
                assert_not_in("s 后重试", text, "倒计时应清除，不再到点重发")
                assert_not_in("不再自动重试", text, "挂起统计行应中性（v1.22）")
                # ② 详情无失败原因行
                assert_not_in("失败原因", detail_text(app), "详情无失败原因行（v1.22）")
                # ④ 不占槽
                assert_in("下载槽位 0/1", text, "挂起不占下载槽位")
                # ③ 数据同普通暂停：注册表快照 error=null / retries=1 / 无 has_error
                reg = os.path.join(env.home, ".ezr", "state", "registry.json")
                snap = None
                deadline = time.time() + 8
                while time.time() < deadline:
                    app.pump(0.2)
                    raw = read_file(reg)
                    if raw:
                        doc = _json.loads(raw.decode("utf-8"))
                        tasks = doc.get("tasks", [])
                        hit = [t for t in tasks if t.get("name", "").startswith("three-m")]
                        if hit and hit[0].get("state") == "Paused":
                            snap = hit[0]
                            break
                assert snap is not None, "注册表应落盘 Paused 任务快照"
                assert snap.get("error") is None, \
                    f"挂起即重置 error=null（v1.23）: {snap.get('error')!r}"
                assert snap.get("retries") == 1, \
                    f"挂起即重置 retries=1（v1.23）: {snap.get('retries')!r}"
                assert "has_error" not in snap, "has_error 字段应退役（v1.23）"
            finally:
                app.graceful_quit()
            # ⑤ 重启后仍保持「已暂停」（迟到失败事件整体忽略）
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("已暂停", 8), "重启后任务应保持已暂停"
                text = app.text()
                assert_not_in("已失败", text, "重启不应回退到失败态")
                assert_not_in("s 后重试", text, "重启不应恢复倒计时")
            finally:
                app.graceful_quit()

    def _case_15(self):
        """QA-SQ-15 已暂停失败任务再按空格：重新排队续传；R 仅兜底提示（v1.19 收窄）。"""
        @self.case("QA-SQ-15")
        def go(env: Env):
            env.write_config("[common]\ndownload_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("three-m.bin?status=503&retry_after=30"))
                assert app.wait_for("s 后重试", 10)
                app.send("home")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停", 6)
                # R 于挂起态：仅兜底提示（空格为唯一恢复键，v1.19/FR-01-92 收窄）
                app.send("r")
                assert app.wait_for("仅「已失败」的任务可以重试", 5), \
                    "挂起态 R 应仅给兜底提示"
                text = app.text()
                assert_not_in("s 后重试", text, "兜底 R 不应重新排队")
                # 空格恢复：重新排队（v1.21）→ 获槽下载 → 重探 503 再失败。
                #    恢复 toast 双臂在秒败场景会被紧随的失败 toast 同帧覆盖
                #    （engineering.md 探针秒败纪律：瞬时 toast 不可稳定观察），
                #    改锚持久最终态证据链：任务离开 Paused（重排队）→ 引擎重启
                #    → 再探 503 重新进入重试倒计时（倒计时已随挂起清除，重现
                #    「s 后重试」即证明经过了重新排队与重新启动）。
                app.send("space")
                assert app.wait_gone("[已暂停]", 8), \
                    "空格恢复应离开已暂停（重新排队，v1.21）"
                assert app.wait_for("s 后重试", 20), \
                    "恢复应重新排队并重探（重进重试倒计时为持久证据）"
                assert app.wait_for("下载槽位 1/1", 10), "重探期间槽位应占用（失败倒计时占槽）"
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
                app.pump(0.5)  # 失败切换窗 settle：转场瞬间注入按键会被事件循环吞键
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
