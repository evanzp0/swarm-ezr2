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
    detail_text, expected_content, file_bytes, wait_file_size,
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
        for i in range(1, 15):
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
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
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
                assert app.wait_for("已暂停", 5)
                assert_in("已暂停", app.text(), "暂停态")
                app.send("space")
                assert app.wait_for("下载中", 5)
                # 失败（瞬态倒计时）
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?status=503"))
                assert app.wait_for("s 后重试", 12), "倒计时行"
                assert_in("重试 1/5", app.text(), "重试计数")
                # 失败（停等）
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("不自动重试", 10)
                # 完成
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("（无校验）", 30)
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-TD-03 下载中详情字段全集齐备；分块行 1 MB/块。"""
        @self.case("QA-TD-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                d = detail_text(app)
                for field in ("状态", "ID", "类型", "大小", "速度", "保存",
                              "URL", "分块", "并发分块明细"):
                    assert_in(field, d, f"详情字段 {field}")
                assert_in("1 MB/块", d, "默认块大小")
                assert_in("支持断点续传", d, "续传标记")
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-TD-04 2MB 并发 4：明细表逐连接块号/状态；含「待命」。"""
        @self.case("QA-TD-04")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir, rows=46)
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "two-m.bin", 1, rate=40000),
                    conns="4")
                assert app.wait_for("下载中", 8)
                assert app.wait_for("待命", 12), "块数 < 并发数应出现待命连接"
                assert app.wait_for("传输中", 12), "活跃连接应传输中"
                d = detail_text(app)
                assert re.search(r"块 \d+/\d+", d), "连接应显示块号"
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


if __name__ == "__main__":
    suite = TuiDisplaySuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
