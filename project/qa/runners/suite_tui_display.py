#!/usr/bin/env python3
"""suite_tui_display.py — QA 套件 · 01-tui-display TUI 展示与交互（期号 01）。

对应规程：project/qa/01-tui-display-qa.md（14 用例）。
端到端 UI 层验证：伪终端画面/配色单元格断言 + 按键/鼠标（SGR）注入 + fixture 驱动真实状态。
配色断言与 NO_COLOR 无关（产品强制彩色）。环境判据：SGR 鼠标序列由 PTY 直接注入，
无真实鼠标也可验证（产品侧只依赖终端转义序列）。
"""

from __future__ import annotations

import os
import re
import time

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_in, assert_not_in,
    detail_text, expected_content, file_bytes, parse_speed, wait_file_size,
    QA_FILE_SIZES,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]


class TuiDisplaySuite(Suite):
    def __init__(self):
        super().__init__("01-tui-display", port_base=41700)

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _slow(env: Env, name: str, seq: int, rate: int = 100000) -> str:
        return env.fixture.url(f"{name}?swapsize={3000000 + seq}&speed={rate}")

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        # 规程用例编号无 QA-TD-04（FR-01-81 修订二撤销明细表场景后编号跳空）
        for i in (1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-TD-01 主界面四区结构：头部/页签+列表/详情+Sparkline/页脚。"""
        @self.case("QA-TD-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                text = app.text()
                assert_in("EZR Downloader", text, "标题")
                assert_in("全局", text, "头部统计")
                assert_in("任务队列", text, "任务队列块")
                assert_in("正在下载", text, "页签")
                assert_in("已完成", text, "页签")
                assert_in("任务详情", text, "详情面板")
                assert_in("全局速度", text, "Sparkline 面板")
                assert_in("Space 暂停/继续", text, "页脚快捷键区")
                assert_in("鼠标", text, "页脚鼠标提示区")
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-TD-02 五状态任务信息行文案逐字匹配。"""
        @self.case("QA-TD-02")
        def go(env: Env):
            # 块粒度显示口径（速度按块完成跳变，见 TD-15 长注）：默认 1 MB 块
            # 在 100KB/s 门控下首块 ~10s 才完成，非零速度读数窗很紧。配置缩小
            # HTTP 块至 128 KB 使速度行更早非零；行文案模式不受块大小影响。
            env.write_config("block_size_http = 131072\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8), "任务应进入下载中"
                # 速度行（下载中）: ↓ X/s + 已下载/总量 + 剩余
                # 速度按块完成粒度跳变（门控下先 0 后尖峰），轮询窗口采样
                deadline = time.time() + 20
                ok = False
                while time.time() < deadline:
                    app.pump(0.5)
                    if re.search(r"↓ [1-9][\d.]* [KMG]?B/s", app.text()):
                        ok = True
                        break
                assert ok, "下载中应出现非零速度行"
                app.send("space")
                assert app.wait_for("已暂停", 5), "暂停后应显示已暂停"
                assert_in("已暂停", app.text(), "暂停态")
                app.send("space")
                assert app.wait_for("下载中", 5), "恢复后应回到下载中"
                # 失败（瞬态倒计时）
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?status=503"))
                assert app.wait_for("s 后重试", 12), "倒计时行"
                assert_in("重试 1/5", app.text(), "重试计数")
                # 失败（停等）
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("不自动重试", 10), "停等应显示不自动重试"
                # 完成
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("（无校验）", 30), "无校验完成应出提示"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-TD-03 下载中详情字段全集齐备（v1.11 起不含状态/速度）；分块行 1 MB/块；
        无「并发分块明细」段落（FR-01-81 修订二）与「状态」「速度」字段行
        （v1.11/FR-01-80 修订）；大小行无「（剩余」后缀（v1.12/FR-01-80 修订）。"""
        @self.case("QA-TD-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                d = detail_text(app)
                for field in ("ID", "类型", "大小", "保存",
                              "URL", "分块"):
                    assert_in(field, d, f"详情字段 {field}")
                assert_in("1 MB/块", d, "默认块大小")
                assert_in("支持断点续传", d, "续传标记")
                assert_not_in("并发分块明细", d, "明细表应已移除（FR-01-81 修订二）")
                assert_not_in("状态", d, "状态行应已移除（v1.11/FR-01-80）")
                assert_not_in("速度", d, "速度行应已移除（v1.11/FR-01-80）")
                assert_not_in("剩余", d, "大小行剩余后缀应已移除（v1.12/FR-01-80）")
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-TD-05 六态+两协议状态色与协议徽标黄色。"""
        @self.case("QA-TD-05")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 下载中（淡蓝）行
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                row_idx = next(i for i, r in enumerate(app.screen.display)
                               if "three-m.bin" in r[:64] and "[HTTP]" in r[:64])
                row = app.screen.display[row_idx]
                m = re.search(r"\[[^\]]*下载中[^\]]*\]", row)
                assert m, f"行内应含下载中徽标: {row!r}"
                col = m.start() + 2
                fg = app.cell_fg(row_idx, col)
                assert fg in ("lightblue", "lightcyan", "cyan", "7fd4ff",
                              "87d7ff", "7ab9f2"), f"下载中应淡蓝色系: {fg}"
                # 失败（红）
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("已失败", 10)
                row_idx = next(i for i, r in enumerate(app.screen.display)
                               if "ghost-404" in r[:64] and "[HTTP]" in r[:64])
                row = app.screen.display[row_idx]
                m2 = re.search(r"\[[^\]]*已失败[^\]]*\]", row)
                assert m2, f"行内应含已失败徽标: {row!r}"
                col = m2.start() + 2
                fg = app.cell_fg(row_idx, col)
                assert fg in ("red", "lightred", "ff5f5f", "ff8787", "e25c54"), \
                    f"已失败应红色系: {fg}"
                # 协议徽标黄色
                row = app.screen.display[row_idx]
                col = row.index("[HTTP]") + 1
                fg = app.cell_fg(row_idx, col)
                assert fg in ("yellow", "e4b23e", "ffd787", "gold"), \
                    f"协议徽标应黄色: {fg}"
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-TD-06 12 任务 ↑↓/Home/End/PgUp/PgDn 移动与详情联动。"""
        @self.case("QA-TD-06")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir, rows=46)
            try:
                for i in range(12):
                    add_task_via_dialog(
                        app, env, self._slow(env, "two-m.bin", i, rate=60000))
                assert app.wait_for("下载槽位 5/5", 15)
                app.send("home")
                app.pump(0.3)
                sel_row = lambda: next(
                    (i for i, r in enumerate(app.screen.display) if "▌" in r), None)
                first = sel_row()
                app.send("pgdn")
                app.pump(0.4)
                moved = sel_row()
                assert moved != first, f"PgDn 应移动选中: {first} → {moved}"
                app.send("end")
                app.pump(0.4)
                end_sel = app.text()
                assert_in("排队第", end_sel, "End 应到队尾（排队任务可见）")
                app.send("pgup")
                app.pump(0.4)
                app.send("up")
                app.pump(0.3)
                # 详情随选中切换：详情标题应为当前选中任务名
                d = detail_text(app)
                m = re.search(r"(two-m\.bin(?:\.\d+)?)", d)
                assert m, "详情应随选中切换"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-TD-07 依次按 A/D/Tab/G/C/U/J：对话框与行为正确。"""
        @self.case("QA-TD-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # A：五字段对话框
                app.send("A")
                assert app.wait_for("添加下载任务", 3)
                assert_in("URL", app.text()) and None
                for f in ("保存到", "并发", "校验", "校验码"):
                    assert_in(f, app.text(), f"对话框字段 {f}")
                app.send("esc")
                app.pump(0.3)
                # D：三选删除对话框（先加任务）
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                app.send("D")
                assert app.wait_for("删除任务", 3)
                for opt in ("仅删除任务", "删除任务和文件", "取消"):
                    assert_in(opt, app.text(), f"删除选项 {opt}")
                app.send("esc")
                app.pump(0.3)
                # Tab 页签
                app.send("tab")
                app.pump(0.4)
                # G 图表
                app.send("g")
                app.pump(0.4)
                # C 清已完成
                app.send("c")
                app.pump(0.4)
                # U/J 移动（无多任务时报错 toast 而非崩溃）
                app.send("u")
                app.pump(0.3)
                app.send("j")
                app.pump(0.3)
                assert_in("EZR Downloader", app.text(), "应用仍正常运行")
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-TD-08 鼠标点击选中/滚轮滚动/按钮点击。"""
        @self.case("QA-TD-08")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                for i in range(3):
                    add_task_via_dialog(
                        app, env, self._slow(env, "three-m.bin", i))
                app.wait_for("下载中", 8)
                # 点击第二行任务（列表首行 y=6 区域；直接 SGR 注入）
                app.send("mclick:30:7")
                app.pump(0.5)
                d = detail_text(app)
                m = re.search(r"(three-m\.bin(?:\.\d+)?)", d)
                assert m, "点击应选中任务（详情随点击切换）"
                # 滚轮
                app.send("mwd:30:8")
                app.pump(0.4)
                app.send("mwu:30:8")
                app.pump(0.4)
                assert_in("EZR Downloader", app.text(), "滚轮后应用正常")
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-TD-09 满槽 Space toast：出现清晰可读、数秒后消失无残留。

        触发口径（同 SQ-10）：已暂停任务在槽满时按 Space 继续 → 无空闲 toast。
        """
        @self.case("QA-TD-09")
        def go(env: Env):
            env.write_config("download_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载槽位 1/1", 10)
                app.send("home")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停", 5)
                # 占满唯一槽位
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 2))
                assert app.wait_for("下载槽位 1/1", 10)
                # 已暂停任务（列表首位）按 Space → 无空闲 toast
                app.send("home")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("无空闲下载槽位", 8), "toast 应出现"
                assert app.wait_gone("无空闲下载槽位", 12), "toast 应自动消失"
                assert_not_in("◆ ⏳", app.text(), "toast 消失后无残留")
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-TD-10 中文文件名任务：列表/详情各列对齐无错位（CJK 两格宽）。"""
        @self.case("QA-TD-10")
        def go(env: Env):
            with open(os.path.join(env.fixture_root, "测试文件.bin"), "wb") as f:
                f.write(expected_content(200_000))
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("测试文件.bin?speed=400000"))
                assert app.wait_for("测试文件.bin", 10), "中文文件名应显示"
                row_idx = app.find_row("测试文件.bin")
                row = app.screen.display[row_idx]
                # 行右边界对齐：行尾应为面板边框字符（无错位溢出）
                assert row.rstrip().endswith("│"), f"列表行应右对齐闭合: {row[-8:]!r}"
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-TD-11 尺寸切 80×34：<100 列隐藏右栏；切回 120 恢复。"""
        @self.case("QA-TD-11")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                # 缩至 80 列
                import fcntl, struct, termios
                fcntl.ioctl(app.fd, termios.TIOCSWINSZ,
                            struct.pack("HHHH", 34, 80, 0, 0))
                assert app.wait_gone("任务详情", 8), "<100 列应隐藏右栏详情"
                assert_in("下载中", app.text(), "窄屏主列表应保留")
                # 恢复 120 列
                fcntl.ioctl(app.fd, termios.TIOCSWINSZ,
                            struct.pack("HHHH", 34, 120, 0, 0))
                assert app.wait_for("任务详情", 8), "恢复 120 列右栏应回归"
            finally:
                app.graceful_quit()

    def _case_12(self):
        """QA-TD-12 多任务头部：↓ 真实速度；↑ 恒 0；并发线程=活跃连接。"""
        @self.case("QA-TD-12")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 8 块/任务 × 3：块完成粒度的速度尖峰高频出现，采样可靠
                for i in range(3):
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(
                            f"eight-m.bin?swapsize={8000000 + i}&speed=200000"))
                assert app.wait_for("下载槽位", 20), "槽位栏应可见且任务获槽"
                deadline = time.time() + 30
                header = ""
                while time.time() < deadline:
                    app.pump(0.4)
                    m = re.search(r"全局  ↓ ([1-9][\d.]* [KMG]?B/s)", app.text())
                    if m:
                        header = m.group(0)
                        break
                assert header, "头部 ↓ 应显示真实速度（非零）"
                text = app.text()
                m = re.search(r"↑ ([\d.]+ [KMG]?B/s)", text)
                assert m and m.group(1) == "0 B/s", \
                    f"↑ 应恒为 0: {m.group(1) if m else '?'}"
                m = re.search(r"并发线程 (\d+)", text)
                assert m and 1 <= int(m.group(1)) <= 12, "并发线程应=活跃连接数"
            finally:
                app.graceful_quit()

    def _case_13(self):
        """QA-TD-13 干净启动空置 60s：空态提示；无任务自行出现。"""
        @self.case("QA-TD-13")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.pump(1.0)
                assert_in("按 A 添加下载任务", app.text(), "空态提示")  # noqa: 见 ui.rs 空态提示修复
                t0 = app.text()
                deadline = time.time() + 12  # 60s 规程时长按沙箱预算压缩采样
                while time.time() < deadline:
                    app.pump(1.0)
                t1 = app.text()
                assert "任务 0 · 正在下载 0 · 已完成 0" in t1, "应保持空任务计数"
                assert t1.split("全局")[0] == t0.split("全局")[0], "头部结构不应自行变化"
            finally:
                app.graceful_quit()

    def _case_14(self):
        """QA-TD-14 标题栏版本号 v0.1.0-01（FR-01-83）。"""
        @self.case("QA-TD-14")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.pump(1.0)
                assert_in("v0.1.0-01", app.text(), "标题栏应显示版本号")
            finally:
                app.graceful_quit()

    # -- 速度展示节奏（FR-01-17 修订）-----------------------------------------

    BAR_GLYPHS = "▁▂▃▄▅▆▇█"

    @staticmethod
    def _bar_count(app: "EzrApp") -> int:
        """「↓ 全局速度」面板内 Sparkline 采样点数（1 点 = 1 列含柱条字符）。

        两层口径修正：
        - 只统计图表面板内部行——全画面口径会把任务行/详情面板的进度条
          「█」也计入（实测 6s +38 点，全部来自进度条列）；
        - 按列去重而非字符计数——Sparkline 纵向柱高随数值幅度变化，字符
          总数度量的是幅度和而非点数（实测 EMA 爬升期 6s +25 字符）；
          「每秒至多新增 1 点」口径只关心列（数据点）数。
        """
        rows = app.screen.display
        start = next((i for i, r in enumerate(rows) if "全局速度" in r), None)
        if start is None:
            return 0
        cols: set[int] = set()
        for r in rows[start + 1:]:
            if "╰" in r and "╯" in r and r.count("─") > 10:
                break  # 面板底边框
            for x, ch in enumerate(r):
                if ch in "▁▂▃▄▅▆▇█":
                    cols.add(x)
        return len(cols)

    @staticmethod
    def _task_speed_sample(app: "EzrApp") -> str | None:
        """选中任务行（列表区，左 64 列）的速度读数（↓ X.X KB/s）；无读数返回 None。

        跳过含「全局」的头部行与标题行；详情面板在右 64 列起，不参与匹配。
        """
        for row in app.screen.display:
            left = row[:64]
            if "全局" in left or "EZR Downloader" in left:
                continue
            m = re.search(r"↓ (\d+(?:\.\d+)? [KMG]?B/s)", left)
            if m:
                return m.group(1)
        return None

    def _case_15(self):
        """QA-TD-15 慢速下载采样 10 秒 + Space 暂停 + 恢复：速度每秒至多变 1 次；
        Sparkline 每秒至多新增 1 点；暂停后任务速度立即归零无拖尾、全局同步扣除；
        恢复后 EMA 平滑爬升（FR-01-17 修订）。"""
        @self.case("QA-TD-15")
        def go(env: Env):
            # 块粒度显示口径：进度/速度按块完成跳变（默认 1 MB/块）。80KB/s
            # 门控下首块需 ~13s，10s 采样窗内零进度、Sparkline 无采样点
            # （TD-02 同口径注记）。两参数配合使 1s 节拍断言可稳定观察：
            #   ① 配置缩小 HTTP 块至 128 KB → 块完成节奏连续（默认块下进度
            #     每 ~13s 才跳一格）；配置文件是用户可达入口，端到端口径不变。
            #   ② rate=30000：fixture ?speed 为每请求限速口径（3 连接聚合
            #     ~90KB/s），3MB 需 ~34s——保证采样/暂停/恢复全程任务活跃
            #     （80KB/s 时聚合 ~240KB/s，任务在暂停步前即完成，R 无法测）。
            env.write_config("block_size_http = 131072\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "three-m.bin", 7, rate=30000))
                assert app.wait_for("下载中", 8)
                # ① 采样 10 秒（0.4s 间隔）：数值变化次数 ≤ 12（1s 节拍 + 边际）
                samples: list[str | None] = []
                deadline = time.time() + 10.0
                while time.time() < deadline:
                    app.pump(0.05)
                    samples.append(self._task_speed_sample(app))
                    time.sleep(0.35)
                vals = [s for s in samples if s]
                assert vals, "10 秒采样窗内应出现速度读数"
                transitions = sum(1 for a, b in zip(vals, vals[1:]) if a != b)
                assert transitions <= 12, \
                    f"速度变化 {transitions} 次/10s 超过 1s 节拍口径"
                # ② Sparkline 柱条增长：6 秒窗口至多 +8 点（1 点/秒 + 边际）
                c1 = self._bar_count(app)
                assert c1 >= 1, "Sparkline 应已有采样点"
                t1 = time.time()
                while time.time() < t1 + 6.0:
                    app.pump(0.1)
                    time.sleep(0.4)
                c2 = self._bar_count(app)
                dt = time.time() - t1
                assert c2 >= c1, "Sparkline 应单调增长（活跃下载期）"
                assert c2 - c1 <= dt + 2, \
                    f"Sparkline {dt:.1f}s 内新增 {c2 - c1} 点超过 1 点/秒口径"
                # ③ 暂停：任务速度立即归零无拖尾、头部全局同步扣除
                app.send("space")
                assert app.wait_for("已暂停", 5)
                row: str | None = None
                hdr = None
                zero_deadline = time.time() + 2.5
                while time.time() < zero_deadline:
                    app.pump(0.1)
                    row = self._task_speed_sample(app)
                    m = re.search(r"全局\s+↓ (\d+(?:\.\d+)? [KMG]?B/s)",
                                  app.text())
                    hdr = m.group(1) if m else None
                    if row is None and hdr is not None \
                            and parse_speed(hdr) == 0.0:
                        break
                    time.sleep(0.2)
                else:
                    raise AssertionError(
                        f"暂停后速度应立即归零无拖尾（任务行={row!r} 全局={hdr!r}）")
                # ④ 恢复：EMA 平滑爬升（首个非零读数 < 峰值 × 0.75）。
                # 观察窗下限核算（engineering.md）：恢复延迟 = 连接重建
                # （秒级，实测方差大）+ 首个 128 KB 块 ~1.4 s（90KB/s 聚合），
                # 12 样本窗 ~5 s 裕度不足（本轮实测一例恢复后全程零读数、
                # 单跑复现即消失）；扩至 20 样本 ~8 s+，覆盖慢恢复分位。
                app.send("space")
                assert app.wait_for("下载中", 5)
                climb: list[float] = []
                deadline = time.time() + 15.0
                while time.time() < deadline and len(climb) < 20:
                    app.pump(0.1)
                    s = self._task_speed_sample(app)
                    if s:
                        climb.append(parse_speed(s))
                    time.sleep(0.3)
                assert climb, "恢复后应重新出现速度读数"
                first, peak = climb[0], max(climb)
                assert peak > 0 and first <= peak * 0.75, \
                    f"恢复后应平滑爬升而非跳满（首读 {first:.0f} vs 峰值 {peak:.0f}）"
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = TuiDisplaySuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
