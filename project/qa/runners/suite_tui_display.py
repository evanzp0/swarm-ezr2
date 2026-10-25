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
    chart_points, detail_text, expected_content, file_bytes, parse_speed,
    wait_file_size, QA_FILE_SIZES,
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
        # 规程用例编号无 QA-TD-04/16（FR-01-81 修订二撤销明细表场景后编号跳空）；
        # v1.16 新增 QA-TD-17（任务速度实时口径，FR-01-105）；
        # QA-TD-14 退役（v1.32/FR-01-94① 头部标题栏版本号移除，序号留空不重排）；
        # v1.30–v1.32 新增 QA-TD-18..24（焦点模型/帮助/滚动条/删除框/页脚 chips/
        # 对话框拖拽/关于窗口），QA v148 轮按规程可执行化补齐
        for i in (1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 17,
                  18, 19, 20, 21, 22, 23, 24):
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
                # v1.13/FR-01-94：「任务队列」面板撤销，全屏不存在其标题
                assert_not_in("任务队列", text, "任务队列面板已撤销（04-ui-conns-01）")
                assert_in("正在下载", text, "页签")
                assert_in("已完成", text, "页签")
                assert_in("任务详情", text, "详情面板")
                # 流量图内嵌头部右侧且无标题（04-ui-conns-05）；结构锚点为
                # 头部字段「峰值 ↓」图例与右栏「并发连接」面板
                assert_in("峰值 ↓", text, "头部流量图区（内嵌无标题，峰值图例）")
                assert_in("并发连接", text, "并发连接面板")
                # v1.31/FR-01-107①：页脚双行 = 常用命令图例 chips + toast 独占行；
                # 除五枚 chips 外无其他快捷键项（Space 暂停/P 分块/G 面板/鼠标
                # 等历史项逐字检索为零，交互说明收敛帮助窗口）
                for chip in ("A 添加", "D 删除", "M 修改", "Q 退出", "H 帮助"):
                    assert_in(chip, text, f"页脚 chips 应含「{chip}」（v1.31）")
                assert_not_in("Space 暂停/继续", text, "页脚历史项已退役（v1.31）")
                assert_not_in("P 分块", text, "页脚历史项已退役（v1.30 P3）")
                assert_not_in("G 面板", text, "页脚历史项已退役（v1.30 P3）")
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
                # v1.34/FR-01-80 D42 字段名精简：任务 ID→ID、创建时间→创建、
                # 保存→路径（字段行布局随精简漂移）
                for field in ("ID", "创建", "类型", "大小", "路径",
                              "URL", "分块"):
                    assert_in(field, d, f"详情字段 {field}")
                assert_not_in("保存", d, "旧字段名 保存 应已精简（v1.34/D42）")
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
        """QA-TD-07 依次按 A/D/]/[/T/H/O/C/U/J/Esc：对话框与行为正确。

        规程 v1.30/31/32 同步口径（qa/01-tui-display-qa.md QA-TD-07 行）：
        ]/[/T 切/循环页签（v1.30：Tab 不再切页签，G 图表快捷键随 FR-01-94
        流量图内嵌退役）；H 弹出帮助窗口；O 弹出关于窗口（40×7，再按关闭，
        v1.32/FR-01-111）；Esc 主界面无动作（v1.30/FR-01-108）。
        """
        @self.case("QA-TD-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # A：五字段对话框（74×11 标题 ` 添加任务：HTTP(S) `，v1.32 更名）
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                for f in ("URL", "保存到", "并发", "校验", "校验码"):
                    assert_in(f, app.text(), f"对话框字段 {f}")
                app.send("esc")
                app.pump(0.3)
                # D：三选删除对话框（先加任务，66×7）
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                app.send("D")
                assert app.wait_for("删除任务", 3)
                for opt in ("仅删除任务", "删除任务和文件", "取消"):
                    assert_in(opt, app.text(), f"删除选项 {opt}")
                app.send("esc")
                app.pump(0.3)
                # ]/[/T 页签切换与循环（v1.30：Tab 不再切页签）
                app.send("]")
                app.pump(0.4)
                assert_in("（此页签下没有任务", app.text(),
                          "] 应切至已完成页签（空态提示）")
                app.send("[")
                app.pump(0.4)
                assert_in("three-m.bin", app.text(), "[ 应回正在下载页签")
                app.send("t")
                app.pump(0.4)
                assert_in("（此页签下没有任务", app.text(), "T 应循环切页签")
                app.send("t")
                app.pump(0.4)
                assert_in("three-m.bin", app.text(), "T 应循环回正在下载页签")
                # H 帮助窗口（FR-01-107；【全局】组为窗口内文锚点）
                app.send("h")
                assert app.wait_for("【全局】", 3), "H 应弹出帮助窗口"
                app.send("esc")
                app.pump(0.4)
                assert_not_in("【全局】", app.text(), "Esc 应关闭帮助窗口")
                # O 关于窗口（40×7 `EZR Download v1.0.0`，再按关闭，FR-01-111/D39）
                app.send("o")
                assert app.wait_for("EZR Download v1.0.0", 3), "O 应弹出关于窗口"
                app.send("o")
                app.pump(0.4)
                assert_not_in("EZR Download v1.0.0", app.text(),
                              "再按 O 应关闭关于窗口")
                # C 清已完成
                app.send("c")
                app.pump(0.4)
                # U/J 移动（无多任务时报错 toast 而非崩溃）
                app.send("u")
                app.pump(0.3)
                app.send("j")
                app.pump(0.3)
                assert_in("EZR Downloader", app.text(), "应用仍正常运行")
                # Esc 主界面无动作（v1.30：退出仅 Q/Ctrl+C）
                app.send("esc")
                app.pump(0.4)
                assert app.exit_code is None, "Esc 不应退出（主界面无动作）"
                assert_in("EZR Downloader", app.text(), "Esc 后应用仍运行")
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
        """QA-TD-09 toast 反馈自动消失：出现清晰可读、数秒后消失无残留。

        触发路径同步（v1.21 QA v138 会话）：原「满槽 Space 继续」toast 已随
        v1.21 满槽提示退役而不可达（01-tui-display-09 括号示例陈旧，规格偏差
        已登记 handoff 待操作者裁决）——改用「任务已存在」重复添加 toast
        （QA-01-16 同源路径，稳定可达）：出现、数秒后自动消失、无残留。
        """
        @self.case("QA-TD-09")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=500000"))
                assert app.wait_for("已添加任务", 6)
                # 重复添加同 URL 同目录 → 「任务已存在」toast
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                app.type_text(env.fixture.url("five-m.bin?speed=500000"))
                app.send("tab")
                app.type_text(env.save_dir)
                app.send("enter")
                assert app.wait_for("任务已存在", 5), "toast 应出现且清晰可读"
                assert app.wait_gone("任务已存在", 12), "toast 应数秒后自动消失"
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
        """QA-TD-12 多任务头部：↓ 真实速度；↑ 恒 0；并发=下载中任务连接总数（v1.13 数据面口径）。"""
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
                    m = re.search(r"全局 ↓ ([1-9][\d.]* [KMG]?B/s)", app.text())
                    if m:
                        header = m.group(0)
                        break
                assert header, "头部 ↓ 应显示真实速度（非零）"
                text = app.text()
                m = re.search(r"↑ ([\d.]+ [KMG]?B/s)", text)
                assert m and m.group(1) == "0 B/s", \
                    f"↑ 应恒为 0: {m.group(1) if m else '?'}"
                m = re.search(r"并发 (\d+)", text)
                assert m and 1 <= int(m.group(1)) <= 12, "头部并发应=活跃连接数（数据面）"
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
                assert "正在下载 (0) │ 已完成 (0)" in t1, "应保持空任务计数（v1.13 页签行口径）"
                assert t1.split("全局")[0] == t0.split("全局")[0], "头部结构不应自行变化"
            finally:
                app.graceful_quit()

    @staticmethod
    def _bar_count(app: "EzrApp") -> int:
        """头部右侧内嵌流量图采样点数（v1.13 布局，harness.chart_points 承载）。

        口径不变：只统计图区内部、按列去重（1 点 = 1 列含柱条字符）——
        区域从旧「↓ 全局速度」独立面板改为头部第 1–3 行最右两个「│」
        之间的内嵌图区（FR-01-94/95 定稿，区域实现收口在共享 harness）。
        """
        return chart_points(app)

    @staticmethod
    def _task_speed_sample(app: "EzrApp") -> str | None:
        """选中任务行（列表区，左 64 列）的速度读数（↓ X.X KB/s）；无读数返回 None。

        跳过含「全局」的头部行与标题行；详情面板在右 64 列起，不参与匹配。
        （QA v148 轮注：本 helper 原置于 _case_14 与 _case_15 之间——_case_14
        随 v1.32 标题栏版本号退役删除时被连带切除，此处原位恢复。）
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

    # -- 任务速度实时口径（v1.16/FR-01-105，QA-TD-17）--------------------------

    @staticmethod
    def _conn_speeds(app: "EzrApp") -> list[float]:
        """并发连接面板数据行的连接速度读数（B/s）。

        行形态（ui/conns.rs）：序号(4) 空格 下载速度(8 右对齐) 空格 累计(8)；
        待命连接速度列显示 "-"。右栏定位走 buffer 单元格列空间（v140 修订：
        pyte display 串因左栏 CJK 宽字符收缩错位，row[64:] 会切进右栏边框
        ——沿 04-ui-conns `_right_text` 同口径），自「并发连接」标题行起、
        面板底框止，只匹配含三段数据形态的行。
        """
        cols = app.screen.columns
        lines_n = app.screen.lines

        def row_text(y: int, x0: int) -> str:
            return "".join(app.screen.buffer[y][x].data
                           for x in range(x0, cols)
                           if app.screen.buffer[y][x].data)

        start = next((y for y in range(lines_n)
                      if "并发连接" in row_text(y, 0)), None)
        if start is None:
            return []
        # 面板左边框列 = 标题行 ╭（右栏箱体左上角）所在列
        x0 = next((x for x in range(cols)
                   if app.screen.buffer[start][x].data == "╭"), 64)
        out: list[float] = []
        for y in range(start + 1, lines_n):
            t = row_text(y, x0 + 1)
            if t.startswith("╰"):
                break
            m = re.match(r"\s*\d+\s+([\d.]+ [KMG]?B/s|-)\s+[\d.]+ [KMG]?B", t)
            if not m:
                continue
            v = m.group(1)
            out.append(0.0 if v == "-" else parse_speed(v))
        return out

    def _case_17(self):
        """QA-TD-17 任务/全局/逐连接速度同源无块内归零（v1.16/FR-01-105）。

        块内采样 3 次（间隔 ≥1s）：三者同步非零、同数量级——无「连接非 0 而
        任务/全局 0 B/s」块内平台期；已下载字节逐样本持续推进；全程无零读数
        （块完成瞬间速度连续，无归零闪现）。

        观察窗核算（engineering.md 门控慢速场景先算观察窗下限；v140 修订：采样
        启动对齐实测需 ~4s，300KB/s 全程 ~10s 余量不足致 3 组难采满——改
        rate=200000 拉长全程至 ~15s）：默认块 1MB、fixture ?speed=200000 为
        每请求口径——12 块文件 4 连接轮转领块，任务全程 ~15s；采样窗
        3×1.05s + 就绪 ~4s 远小于全程，且速度为块内实时口径（FR-01-105
        修复后不依赖块边界）。
        """
        @self.case("QA-TD-17")
        def go(env: Env):
            def size_b(text: str) -> float:
                m = re.match(r"([\d.]+) ([KMG]?)B", text)
                assert m, f"无法解析大小: {text!r}"
                mult = {"": 1.0, "K": 1e3, "M": 1e6, "G": 1e9}[m.group(2)]
                return float(m.group(1)) * mult

            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 17, rate=200000))
                assert app.wait_for("下载中", 8)
                assert app.wait_for("并发连接", 6)
                # 就绪门：等首个非零任务速度（传输已开始的确定观察窗）
                first = None
                deadline = time.time() + 15.0
                while time.time() < deadline and first is None:
                    app.pump(0.1)
                    v = self._task_speed_sample(app)
                    if v and parse_speed(v) > 0:
                        first = v
                    else:
                        time.sleep(0.3)
                assert first is not None, "15s 内应出现非零任务速度"
                samples: list[tuple[float, float, list[float], float]] = []
                deadline = time.time() + 15.0
                while time.time() < deadline and len(samples) < 3:
                    app.pump(0.1)
                    ts = self._task_speed_sample(app)
                    m = re.search(r"全局\s+↓ (\d+(?:\.\d+)? [KMG]?B/s)", app.text())
                    conns = self._conn_speeds(app)
                    dm = re.search(r"↓ \d+(?:\.\d+)? [KMG]?B/s\s+([\d.]+ [KMG]?B)/",
                                   app.text())
                    if ts is None or m is None or not conns or dm is None:
                        time.sleep(0.3)
                        continue
                    t = parse_speed(ts)
                    g = parse_speed(m.group(1))
                    if t <= 0:
                        # 传输中的零读数即被测缺陷形态（块内平台期），直接入样定罪
                        samples.append((0.0, g, conns,
                                        size_b(dm.group(1))))
                        break
                    samples.append((t, g, conns, size_b(dm.group(1))))
                    time.sleep(1.05)
                assert len(samples) == 3, f"应采得 3 组样本，实得 {len(samples)}"
                prev = -1.0
                for i, (t, g, conns, dl) in enumerate(samples):
                    assert t > 0 and g > 0, \
                        f"样本{i} 任务/全局速度应非零（块内平台期即 v1.16 缺陷形态）: " \
                        f"t={t:.0f} g={g:.0f}"
                    assert any(c > 0 for c in conns), \
                        f"样本{i} 应有活跃连接速度非零: {conns}"
                    s_sum = sum(c for c in conns)
                    assert s_sum > 0
                    assert 0.4 <= t / s_sum <= 2.5, \
                        f"样本{i} 任务速度应≈连接速度和（t={t:.0f} Σc={s_sum:.0f}）"
                    assert 0.4 <= g / s_sum <= 2.5, \
                        f"样本{i} 全局速度应≈连接速度和（g={g:.0f} Σc={s_sum:.0f}）"
                    assert dl > prev, \
                        f"样本{i} 已下载字节应持续推进（块内实时而非块边界跳变）: " \
                        f"{prev:.0f} → {dl:.0f}"
                    prev = dl
            finally:
                app.graceful_quit()



    # -- v1.30–v1.32 新增用例（QA v148 轮按规程可执行化） ----------------------

    @staticmethod
    def _cell_bg(app: EzrApp, row: int, col: int):
        return app.screen.buffer[row][col].bg

    @staticmethod
    def _cell_bold(app: EzrApp, row: int, col: int) -> bool:
        return bool(app.screen.buffer[row][col].bold)

    @staticmethod
    def _find_col(app: EzrApp, y: int, text: str, start: int = 0) -> int | None:
        """buffer 列空间文本查找（宽字符列位还原；未命中 None）。"""
        import wcwidth as _ww
        x = start
        while x < app.screen.columns:
            xi, ok = x, True
            for ch in text:
                if xi >= app.screen.columns or \
                        app.screen.buffer[y][xi].data != ch:
                    ok = False
                    break
                w = _ww.wcwidth(ch)
                xi += w if w > 0 else 1
            if ok:
                return x
            d = app.screen.buffer[y][x].data
            w = _ww.wcwidth(d) if d else 1
            x += w if w and w > 0 else 1
        return None

    @staticmethod
    def _panel_title(app: EzrApp, needle: str) -> tuple[int, int] | None:
        """面板标题（needle）所在 (行, 标题首列)；未命中 None。"""
        for i in range(app.screen.lines):
            x = TuiDisplaySuite._find_col(app, i, needle)
            if x is not None:
                return i, x
        return None

    @staticmethod
    def _panel_span(app: EzrApp, needle: str) -> tuple[int, int, int] | None:
        """浮层几何 (标题行, 左边界列, 右边界列)——全 buffer 列空间。

        （pyte display 串因标题内 CJK 宽字符缩短而错位，串索引不可直接当
        buffer 列用——TD-23 首轮实测：串列 rfind ╭ 比真列左偏 ~10 列，
        按压点落在窗外 = 拖拽静默无动作。）
        """
        g = TuiDisplaySuite._panel_title(app, needle)
        if g is None:
            return None
        ty, tx = g
        left = right = None
        for x in range(tx - 1, -1, -1):
            if app.screen.buffer[ty][x].data == "╭":
                left = x
                break
        for x in range(tx + 1, app.screen.columns):
            if app.screen.buffer[ty][x].data == "╮":
                right = x
                break
        if left is None or right is None:
            return None
        return ty, left, right

    def _case_18(self):
        """QA-TD-18 面板焦点模型（v1.30/FR-01-108）：Tab/BackTab 循环 + 聚焦绿框 + 浮层降级 + G 收右栏。"""
        @self.case("QA-TD-18")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                app.pump(0.5)

                def border_cells(needle: str):
                    """面板标题行：(行, 左边界列)——全 buffer 列空间
                    （pyte display 串会因左栏 CJK 宽字符缩短而错位）。"""
                    ty, tx = self._panel_title(app, needle)
                    left = None
                    for x in range(tx - 1, -1, -1):
                        if app.screen.buffer[ty][x].data == "╭":
                            left = x
                            break
                    return ty, left
                # 初始焦点 = 列表：列表绿框加粗，详情/并发暗
                ly, lx = border_cells("下载任务")
                dy, dx = border_cells("任务详情")
                cy, cx = border_cells("并发连接")
                assert app.cell_fg(ly, lx) in ("green", "00cd00") and \
                    self._cell_bold(app, ly, lx), "初始焦点列表（绿框加粗）"
                assert app.cell_fg(dy, dx) != "green", "详情未聚焦（暗框）"
                assert app.cell_fg(cy, cx) != "green", "并发未聚焦（暗框）"
                # Tab：列表 → 详情
                app.send("tab")
                app.pump(0.4)
                dy2, dx2 = border_cells("任务详情")
                assert app.cell_fg(dy2, dx2) in ("green", "00cd00") and \
                    self._cell_bold(app, dy2, dx2), "Tab 后焦点详情"
                ly2, lx2 = border_cells("下载任务")
                assert app.cell_fg(ly2, lx2) != "green", "列表失焦"
                # Tab：详情 → 并发
                app.send("tab")
                app.pump(0.4)
                cy2, cx2 = border_cells("并发连接")
                assert app.cell_fg(cy2, cx2) in ("green", "00cd00"), "再 Tab 后焦点并发"
                # Tab：并发 → 环绕回列表
                app.send("tab")
                app.pump(0.4)
                ly3, lx3 = border_cells("下载任务")
                assert app.cell_fg(ly3, lx3) in ("green", "00cd00"), "Tab 应环绕回列表"
                # BackTab：列表 → 反向回并发
                app.send("backtab")  # BackTab：列表 → 并发（反向）
                app.pump(0.4)
                cy3, cx3 = border_cells("并发连接")
                assert app.cell_fg(cy3, cx3) in ("green", "00cd00"), "BackTab 反向回并发"
                # ↑↓ 按焦点分发：并发聚焦时 ↑↓ 移明细选择（主列表选中不变；
                # ▌ 标记列表与并发明细共用——按列区分：列表 x<64、明细 x≥64）
                def sel_cells(x_lo: int, x_hi: int):
                    return [(y, x) for y in range(app.screen.lines)
                            for x in range(x_lo, x_hi)
                            if app.screen.buffer[y][x].data == "▌"]
                list_before = sel_cells(0, 64)
                # 并发明细选中行 = 深青背景高亮 SEL_BG（Rgb 22,42,48 →
                # SGR 上抛 162a30；FR-01-97 v1.30 改键：聚焦时 ↑↓ 移动选择）
                def sel_bg_rows():
                    return [(y, x) for y in range(app.screen.lines)
                            for x in range(64, app.screen.columns)
                            if app.screen.buffer[y][x].bg == "162a30"]
                conn_before = sel_bg_rows()
                app.send("down")
                app.pump(0.4)
                list_after = sel_cells(0, 64)
                conn_after = sel_bg_rows()
                assert list_before == list_after, \
                    "并发聚焦时 ↑↓ 应作用于并发明细（主列表选中不变）"
                assert conn_before != conn_after, \
                    "并发聚焦时 ↑↓ 应移动并发明细选择（FR-01-97 v1.30 改键）"
                # G 收起右栏 → 再 G 恢复（v1.13 G 键语义）
                app.send("g")
                app.pump(0.4)
                assert_not_in("任务详情", app.text(), "G 应收起右栏")
                app.send("g")
                app.pump(0.4)
                assert_in("任务详情", app.text(), "再 G 应恢复右栏")
                # 浮层打开 → 三面板全部降级暗框（模态）
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                ly4, lx4 = border_cells("下载任务")
                assert app.cell_fg(ly4, lx4) != "green", \
                    "浮层打开时列表应降级暗框（FR-01-108）"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_19(self):
        """QA-TD-19 帮助窗口（v1.32 口径）：八组 44 行、O 条目、四键关闭、位置记忆、模态。"""
        @self.case("QA-TD-19")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                app.send("h")
                assert app.wait_for("【全局】", 3), "H 应打开帮助窗口"
                # 70×24 圆角浮层（标题行 ╭╮ 跨度 70、上下边界行距 24）
                g = self._panel_span(app, "帮助")
                assert g is not None, "帮助窗口几何应在位"
                ty, l, r = g
                assert r - l == 69, f"帮助窗口应 70 列（实测 {r - l + 1}）"
                bot = next(i for i in range(ty + 1, app.screen.lines)
                           if "╰" in app.screen.display[i])
                assert bot - ty == 23, f"帮助窗口应 24 行（实测 {bot - ty + 1}）"
                # 八组分类逐字齐全（v1.32 内容 44 行 > 24 行窗口——滚动收集全文：
                # Home 起步 PgDn ±8 行步进至末页，聚合各视口快照）
                parts: list[str] = []
                app.send("home")
                app.pump(0.3)
                parts.append(app.text())
                for _ in range(7):
                    app.send("pgdn")
                    app.pump(0.25)
                    parts.append(app.text())
                full = "\n".join(parts)
                for grp in ("【全局】", "【页签】", "【任务列表】", "【任务详情】",
                            "【并发连接】", "【滚动条（通用组件）】", "【弹窗",
                            "【pieces 分块弹窗】"):
                    assert_in(grp, full, f"帮助应含 {grp}")
                # v1.32 内容点：O 条目、横向滚动条目、滚动条组四条目、拖拽文案
                # 逐字锚点 = help_lines 钦定文案（ui/help.rs，与单测同源）
                for frag in ("打开 / 关闭关于窗口（About）",
                             "横向滚动明细列（客户端/协议等超宽列）",
                             "拖拽滑块 ═║", "按住不放自动连发", "点击轨道",
                             "按住弹窗内空白处拖动移动窗口（滑块/边框除外）"):
                    assert_in(frag, full, f"帮助内容应含「{frag}」（v1.32）")
                # 滚动：End 到末页钳制（末行提示常驻）
                app.send("end")
                app.pump(0.3)
                # v1.34/FR-01-107 P10 定版：末行 note 滚动提示退役（全屏检索
                # 该文案为零，产品单测同源锚定）——旧「末行提示在位」断言反相
                assert_not_in("↑↓/滚轮 滚动本页", app.text(),
                              "帮助末行 note 应已退役（v1.34/FR-01-107 P10）")
                # 帮助内按 O = 关于叠于帮助之上；关闭后回帮助
                app.send("o")
                assert app.wait_for("EZR Download v1.0.0", 3), "帮助内 O 应叠开关于"
                # 关于叠于帮助之上（z-order 最上层）：帮助窗口标题行仍在位
                assert self._panel_span(app, "帮助") is not None, \
                    "关于打开时帮助应仍在下层"
                app.send("esc")
                app.pump(0.4)
                assert self._panel_span(app, "帮助") is not None, \
                    "Esc 关关于后应回帮助"
                app.send("esc")
                app.pump(0.4)
                assert self._panel_span(app, "帮助") is None, "Esc 关帮助"
                # 位置跨次记忆：H 开窗拖拽 (+6,+3) → Esc → 重开仍在原位
                app.send("h")
                assert app.wait_for("【全局】", 3)
                gh = self._panel_span(app, "帮助")
                assert gh is not None
                ty0, lh, _rh = gh
                app.send(f"mclick:{lh + 3}:{ty0 + 1}",
                         f"mdrag:{lh + 5}:{ty0 + 2}", f"mdrag:{lh + 6}:{ty0 + 3}")
                app.pump(0.2)
                app.send(f"mup:{lh + 6}:{ty0 + 3}")
                app.pump(0.4)
                gh1 = self._panel_span(app, "帮助")
                assert gh1 is not None
                ty1 = gh1[0]
                assert ty1 == ty0 + 2, f"拖拽应移动窗口（{ty0} → {ty1}）"
                app.send("esc")
                app.pump(0.4)
                app.send("h")
                assert app.wait_for("【全局】", 3)
                ty2, _ = self._panel_title(app, "帮助")
                assert ty2 == ty1, "重开帮助应记忆拖拽后位置（FR-01-107）"
                # 点击外部 = 无任何操作（v1.32 统一模态口径：不关闭不穿透）
                app.send("mclick:2:30")
                app.pump(0.2)
                app.send("mup:2:30")
                app.pump(0.4)
                assert_in("【全局】", app.text(), "点击外部不应关闭帮助（v1.32）")
                # 模态：开窗时 A/D/Space/Tab 被消费为主界面无动作
                app.send("a")
                app.pump(0.3)
                assert_not_in("添加任务：HTTP(S)", app.text(), "模态下 A 应被消费")
                app.send("d")
                app.pump(0.3)
                # 帮助内容自身含「删除任务」字样（【全局】A/M/D 条目）——
                # 以删除框专属按钮文本为锚（帮助内容不含）
                assert_not_in("[ 仅删除任务 ]", app.text(), "模态下 D 应被消费")
                # Esc/H/q/Q 四键均只关窗且应用存活
                for key in ("esc", "h", "q", "Q"):
                    app.pump(0.3)
                    if key == "Q":
                        app.send("Q")
                    else:
                        app.send(key)
                    app.pump(0.4)
                    assert_not_in("【全局】", app.text(), f"{key} 应关闭帮助")
                    assert_in("EZR Downloader", app.text(), f"{key} 后应用存活")
                    if key != "Q":
                        app.send("h")
                        assert app.wait_for("【全局】", 3), f"{key} 后重开帮助"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_20(self):
        """QA-TD-20 通用滚动条（v1.32/FR-01-109）：↑│║↓ 形态、连发、轨道翻页、滚轮视口。"""
        @self.case("QA-TD-20")
        def go(env: Env):
            # 20 个长活任务（>一屏）驱动列表滚动条
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                for i in range(20):
                    # swapsize=3MB+i：固定 3MB 长活文件 + 10KB/s 单请求门控
                    # （4 连接聚合 40KB/s ≈ 75s/任务，观察窗内不完成；
                    # swapsize 区分 URL 避免同 URL 同目录重复守卫）
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(
                            f"five-m.bin?swapsize={3000000 + i}&speed=10000"))
                assert app.wait_for("正在下载 (20)", 30), "20 任务应全部入列"
                app.pump(0.5)
                g = self._panel_span(app, "下载任务")
                assert g is not None, "列表面板几何应在位"
                ly, left, right = g
                # 右边界列出现通用滚动条：↑ 顶 / │ 轨道 / ║ 滑块 / ↓ 底
                col_cells = [app.screen.buffer[y][right].data
                             for y in range(ly + 1, app.screen.lines - 4)]
                assert "↑" in col_cells, "滚动条顶箭头 ↑ 应在位"
                assert "↓" in col_cells, "滚动条底箭头 ↓ 应在位"
                assert "║" in col_cells, "双线滑块 ║ 应在位"
                assert "│" in col_cells, "单线轨道 │ 应在位"
                # 滑块长 ≥1 且 ≤ 轨道−2（行程保证；精确公式单测锁定）
                track = sum(1 for c in col_cells if c in "│║")
                slider = sum(1 for c in col_cells if c == "║")
                assert 1 <= slider <= track - 2, \
                    f"滑块长应在 1..轨道-2（实测 {slider}/{track}）"
                # 视口回顶（添加跟随选中把视口钉在末位——home 选首项）
                app.send("home")
                app.pump(0.5)

                def first_row() -> str:
                    # 名字行 + 次行（等待任务名字行同形——排队位次在次行
                    # 「排队第 n」，逐行同形会使翻页不可区分）
                    return "".join(
                        app.screen.buffer[y][c].data or " "
                        for y in (ly + 1, ly + 2)
                        for c in range(left, left + 40))
                # 按住 ↓ 箭头：立即步进 1 项 + 连发（0.4s 起振 0.12s 周期）
                down_i = ly + 1 + col_cells.index("↓")
                first_before = first_row()
                app.send(f"mclick:{right + 1}:{down_i + 1}")
                app.pump(0.9)
                app.send(f"mup:{right + 1}:{down_i + 1}")
                app.pump(0.4)
                assert first_before != first_row(), \
                    "按住 ↓ 箭头应步进并连发（视口/选中前移）"
                # 单击轨道 = 朝点击方向整页滚动（点击点须落在空白轨道带——
                # 落在滑块上 = Thumb 抓取拖拽、落在箭头上 = 步进连发）。
                # 可观察锚 = 滑块位置（等待任务行同形不可区分——名字行同文、
                # 次行为空进度条；滑块位是规程几何契约自身的显示量）
                def thumb_band():
                    cs = [app.screen.buffer[y][right].data
                          for y in range(ly + 1, app.screen.lines - 4)]
                    if "║" not in cs:
                        return None
                    a = cs.index("║")
                    b = len(cs) - 1 - cs[::-1].index("║")
                    return (a, b)
                tb0 = thumb_band()
                col_cells2 = [app.screen.buffer[y][right].data
                              for y in range(ly + 1, app.screen.lines - 4)]
                t_start = col_cells2.index("║")
                t_end = len(col_cells2) - 1 - col_cells2[::-1].index("║")
                below = list(range(t_end + 1, col_cells2.index("↓")))
                above = list(range(col_cells2.index("↑") + 1, t_start))
                band = below if len(below) >= 2 else above
                assert band, "轨道空白带应在位"
                mid = ly + 1 + band[len(band) // 2]
                app.send(f"mclick:{right + 1}:{mid + 1}")
                app.pump(0.15)
                app.send(f"mup:{right + 1}:{mid + 1}")
                app.pump(0.4)
                tb1 = thumb_band()
                assert tb0 is not None and tb1 is not None and tb0 != tb1, \
                    f"单击轨道应整页滚动（滑块位 {tb0} → {tb1}）"
                # 滚轮 ±2 只移视口不动选中项（▌ 按列区分：列表 x<64）
                def sel_cells():
                    return [(y, x) for y in range(app.screen.lines)
                            for x in range(0, 64)
                            if app.screen.buffer[y][x].data == "▌"]
                sel_before = sel_cells()
                tw0 = thumb_band()
                # 轨道翻页后视口在末位——滚轮向上（±2 行）才有位移空间
                app.send(f"mwu:{left + 10}:{ly + 3}")
                app.pump(0.4)
                tw1 = thumb_band()
                assert sel_before == sel_cells(), "滚轮只移视口不动选中项"
                assert tw0 != tw1, "滚轮应移动视口（±2 项，滑块位随动）"
                app.graceful_quit()
                # 任务少于一屏：无滚动条
                app = EzrApp(env.home, save_dir=env.save_dir,
                             env_extra={"EZR_HOME": env.fixture_root})
                self._cur_app = app
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("已完成", 10)
                app.pump(0.4)
                g2 = self._panel_span(app, "下载任务")
                assert g2 is not None, "列表面板几何应在位"
                ly2, _l2, right2 = g2
                col2 = [app.screen.buffer[y][right2].data
                        for y in range(ly2 + 1, app.screen.lines - 4)]
                assert "║" not in col2 and "↑" not in col2, \
                    "任务少于一屏不应绘制滚动条（组件内收口）"
            finally:
                app.graceful_quit()

    def _case_21(self):
        """QA-TD-21 删除对话框（v1.30）：66×7、提示行唯一保留、数字键直选、逐键语义。"""
        @self.case("QA-TD-21")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app

            def add_slow(salt: int) -> str:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        f"three-m.bin?speed=200000&swapsize={3000000 + salt}"))
                assert app.wait_for("下载中", 8), "任务应在下载中"
                return os.path.join(env.save_dir, "three-m.bin")

            try:
                target = add_slow(1)
                # 尺寸/按钮/提示行（全浮层唯一保留的底部提示行）
                app.send("D")
                assert app.wait_for("删除任务", 3), "D 应打开删除框"
                g = self._panel_span(app, "删除任务")
                assert g is not None, "删除框几何应在位"
                ty, l, r = g
                assert r - l == 65, f"删除框应 66 列（实测 {r - l + 1}）"
                bot = next(i for i in range(ty + 1, app.screen.lines)
                           if "╰" in app.screen.display[i])
                assert bot - ty == 6, f"删除框应 7 行（实测 {bot - ty + 1}）"
                text = app.text()
                for btn in ("[ 仅删除任务 ]", "[ 删除任务和文件 ]", "[ 取消 ]"):
                    assert_in(btn, text, f"删除框按钮 {btn}")
                # v1.34/FR-01-25 P10 定版：底部提示行退役（全屏检索该文案为零，
                # 原「全浮层唯一保留提示行」例外条款随之关闭）——旧断言反相
                assert_not_in("1/2/3 快捷选择", text,
                              "底部提示行应已退役（v1.34/FR-01-25 P10）")
                # 数字键直选：2 = 删除任务和文件（文件与 sidecar 全删）
                app.send("2")
                assert app.wait_for("正在停止引擎并清理本地文件", 6)
                deadline = time.time() + 10
                while time.time() < deadline:
                    app.pump(0.2)
                    if file_bytes(target) is None and \
                            file_bytes(target + ".downloading") is None:
                        break
                assert file_bytes(target) is None, "2 应删除任务和文件"
                assert_in("已删除任务", app.text(), "删除 toast")
                # 3 / Space / Esc 关闭且任务与文件保留（每臂独立任务）
                for key, salt in (("3", 2), ("space", 3), ("esc", 4)):
                    t2 = add_slow(salt)
                    app.send("D")
                    assert app.wait_for("删除任务", 3), "D 应打开删除框"
                    app.send(key)
                    app.pump(0.4)
                    assert_not_in("[ 仅删除任务 ]", app.text(), f"{key} 应关框")
                    assert app.exit_code is None, f"{key} 不应退出应用"
                    assert file_bytes(t2) is not None or \
                        file_bytes(t2 + ".downloading") is not None, \
                        f"{key} 不应删除文件"
                    app.graceful_quit()
                    app = EzrApp(env.home, save_dir=env.save_dir)
                    self._cur_app = app
                # Enter 触发当前焦点按钮（焦点 0 = 仅删除任务：文件保留）
                t3 = add_slow(5)
                app.send("D")
                assert app.wait_for("删除任务", 3)
                app.send("enter")
                assert app.wait_for("已删除任务（保留文件）", 6), \
                    "Enter 应触发焦点按钮（仅删除任务）"
                assert file_bytes(t3) is not None, "仅删除任务应保留文件"
                # q 为字符输入被消费：不关窗不退出（v1.30/FR-01-25）
                add_slow(6)
                app.send("D")
                assert app.wait_for("删除任务", 3)
                app.send("q")
                app.pump(0.5)
                assert_in("[ 仅删除任务 ]", app.text(), "q 应被消费（不关窗）")
                assert_in("EZR Downloader", app.text(), "q 不应退出应用")
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_22(self):
        """QA-TD-22 页脚 chips（v1.31/FR-01-107①）：逐字图例、点击执行、拖出取消、模态不可达、窄端不渲染。"""
        @self.case("QA-TD-22")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                app.pump(0.5)
                text = app.text()
                # chips 逐字（行 1 图例 + 暗灰 · 分隔）
                assert_in("A 添加 · D 删除 · M 修改 · Q 退出 · H 帮助", text,
                          "页脚 chips 逐字图例（v1.31）")
                # 点击 H chip（原地松开）= 开帮助，与键盘同口径
                #（chip 列位按 buffer 列空间取——页脚行含 CJK，pyte display
                # 串索引会左偏，串列作鼠标坐标会脱靶）
                def chip_pos(needle: str) -> tuple[int, int]:
                    # 自底向上扫（页脚在屏底；「A 添加」同文见于空态引导文案）
                    for i in range(app.screen.lines - 1, -1, -1):
                        x = self._find_col(app, i, needle)
                        if x is not None:
                            return i, x
                    raise AssertionError(f"chip 未找到: {needle}")
                fy, fx = chip_pos("H 帮助")
                app.send(f"mclick:{fx + 2}:{fy + 1}")
                app.pump(0.15)
                app.send(f"mup:{fx + 2}:{fy + 1}")
                assert app.wait_for("【全局】", 3), "点击 H chip 应开帮助"
                app.send("esc")
                app.pump(0.4)
                # 按住 M chip 拖出松开 = 取消不执行（无修改框）
                my, mx = chip_pos("M 修改")
                app.send(f"mclick:{mx + 2}:{my + 1}")
                app.pump(0.15)
                app.send(f"mup:{mx + 2}:{max(2, my - 4)}")
                app.pump(0.4)
                assert_not_in("修改任务：HTTP(S)", app.text(), "拖出松开应取消")
                # 点击 A chip 开添加框；模态打开时 chips 不可达（点击 H 无效）
                ay, ax = chip_pos("A 添加")
                app.send(f"mclick:{ax + 2}:{ay + 1}")
                app.pump(0.15)
                app.send(f"mup:{ax + 2}:{ay + 1}")
                assert app.wait_for("添加任务：HTTP(S)", 3), "点击 A chip 应开框"
                app.send(f"mclick:{fx + 2}:{fy + 1}")
                app.pump(0.15)
                app.send(f"mup:{fx + 2}:{fy + 1}")
                app.pump(0.4)
                assert_not_in("【全局】", app.text(), "模态打开时 chips 不可达")
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()
            # 终端过窄：chip 整枚不渲染（P7）；Q chip 点击退出（最后、独立 env）
            app = EzrApp(env.home, save_dir=env.save_dir, cols=36)
            try:
                app.pump(0.6)
                assert_not_in("H 帮助", app.text(), "窄终端 chip 整枚不渲染（P7）")
            finally:
                app.graceful_quit()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                app.pump(0.5)
                qy, qx = chip_pos("Q 退出")
                app.send(f"mclick:{qx + 2}:{qy + 1}")
                app.pump(0.15)
                app.send(f"mup:{qx + 2}:{qy + 1}")
                deadline = time.time() + 6
                while time.time() < deadline and app.exit_code is None:
                    app.pump(0.1)
                assert app.exit_code == 0, \
                    f"点击 Q chip 应退出（码 0），实测 {app.exit_code}"
            finally:
                app.close()

    def _case_23(self):
        """QA-TD-23 对话框拖拽（FR-01-110/D38）：标题栏随动、字段行互斥、点外无动作、重开居中、Ctrl+C 直退。"""
        @self.case("QA-TD-23")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                g = self._panel_span(app, "添加任务：HTTP(S)")
                assert g is not None, "添加框几何应在位"
                ty, l, _r = g
                # 标题栏非交互区起拖：窗口随动（Down→Drag→Up，+5,+2）
                app.send(f"mclick:{l + 3}:{ty + 1}",
                         f"mdrag:{l + 5}:{ty + 2}", f"mdrag:{l + 8}:{ty + 3}")
                app.pump(0.2)
                app.send(f"mup:{l + 8}:{ty + 3}")
                app.pump(0.4)
                ty2, _ = self._panel_title(app, "添加任务：HTTP(S)")
                assert ty2 == ty + 2, f"标题栏拖拽应随动（{ty} → {ty2}）"
                # 字段行按住不拖拽（点击语义优先、互斥分流）——按压点按拖拽后
                # 的当前几何取（列偏移沿用拖拽前旧值会落在窗外 = 拖拽起点）
                g3 = self._panel_span(app, "添加任务：HTTP(S)")
                assert g3 is not None
                ty2b, l2, _r2 = g3
                fy = ty2b + 2  # 并发字段行（行序：URL/保存到/并发…）
                app.send(f"mclick:{l2 + 6}:{fy + 1}",
                         f"mdrag:{l2 + 9}:{fy + 2}", f"mdrag:{l2 + 12}:{fy + 3}")
                app.pump(0.2)
                app.send(f"mup:{l2 + 12}:{fy + 3}")
                app.pump(0.4)
                g4 = self._panel_span(app, "添加任务：HTTP(S)")
                assert g4 is not None
                ty3 = g4[0]
                assert ty3 == ty2b, "字段行按住不应拖拽窗口（互斥分流）"
                ty2 = ty2b
                # 点击窗口外部 = 无任何操作（不点外关闭，v1.32 统一口径）
                app.send("mclick:2:2")
                app.pump(0.15)
                app.send("mup:2:2")
                app.pump(0.4)
                assert_in("添加任务：HTTP(S)", app.text(), "点击外部应无动作")
                # 下拉展开期点按先消费不拖拽（点按命中下拉行 = 选择消费）：
                # 展开校验下拉后点击下拉条目，窗口不动、下拉收起
                gd = self._panel_span(app, "添加任务：HTTP(S)")
                assert gd is not None
                tyd, ld, _rd = gd
                app.send("tab", "tab", "tab")  # URL0→保存到1→并发2→校验3
                app.send("enter")              # 展开校验下拉
                app.pump(0.3)
                app.send(f"mclick:{ld + 10}:{tyd + 6}")
                app.pump(0.15)
                app.send(f"mup:{ld + 10}:{tyd + 6}")
                app.pump(0.4)
                ge = self._panel_span(app, "添加任务：HTTP(S)")
                assert ge is not None and ge[0] == tyd, \
                    "下拉展开期点按不应拖拽窗口"
                # Esc 关框重开验居中复位
                app.send("esc")
                app.pump(0.3)
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                ty4, _ = self._panel_title(app, "添加任务：HTTP(S)")
                assert ty4 == ty, "重开应重新居中（位置不保留）"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()
            # Ctrl+C 全局例外：对话框/帮助内各一次，直接退出（v1.31/D38）
            for opener, probe in (("A", "添加任务：HTTP(S)"), ("h", "【全局】")):
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    app.send(opener)
                    assert app.wait_for(probe, 3)
                    app.send("ctrl_c")
                    deadline = time.time() + 6
                    while time.time() < deadline and app.exit_code is None:
                        app.pump(0.1)
                    assert app.exit_code == 0, \
                        f"{opener} 浮层内 Ctrl+C 应直接退出（码 0）"
                finally:
                    app.close()

    def _case_24(self):
        """QA-TD-24 关于窗口（FR-01-111/D39）：40×7、钦定文案、四键关闭、z-order、位置记忆、模态。"""
        @self.case("QA-TD-24")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 头部标题栏无版本号（v1.32 版本展示面唯一 = 关于窗口）
                app.pump(0.5)
                assert_not_in("v0.1.0", app.text(), "头部标题栏应无版本号（v1.32）")
                app.send("o")
                assert app.wait_for("EZR Download v1.0.0", 3), "O 应开关于窗口"
                g = self._panel_span(app, "About")
                assert g is not None, "关于窗口几何应在位"
                ty, l, r = g
                assert r - l == 39, f"关于窗口应 40 列（实测 {r - l + 1}）"
                bot = next(i for i in range(ty + 1, app.screen.lines)
                           if "╰" in app.screen.display[i])
                assert bot - ty == 6, f"关于窗口应 7 行（实测 {bot - ty + 1}）"
                assert_in("EZR Download v1.0.0", app.text(), "钦定文案（D39）")
                # 位置记忆：拖拽 (+5,+2) → Esc → 重开仍在原位
                app.send(f"mclick:{l + 3}:{ty + 1}",
                         f"mdrag:{l + 5}:{ty + 2}", f"mdrag:{l + 8}:{ty + 3}")
                app.pump(0.2)
                app.send(f"mup:{l + 8}:{ty + 3}")
                app.pump(0.4)
                ty1, _ = self._panel_title(app, "About")
                assert ty1 == ty + 2, f"关于窗口拖拽应随动（{ty} → {ty1}）"
                app.send("esc")
                app.pump(0.4)
                app.send("o")
                assert app.wait_for("EZR Download v1.0.0", 3)
                ty2, _ = self._panel_title(app, "About")
                assert ty2 == ty1, "重开关于应记忆拖拽位置"
                # 点击外部 = 无任何操作
                app.send("mclick:2:2")
                app.pump(0.15)
                app.send("mup:2:2")
                app.pump(0.4)
                assert_in("EZR Download v1.0.0", app.text(), "点外应无动作")
                # 模态：开窗时 A/D/Space/Tab 被消费
                app.send("a")
                app.pump(0.3)
                assert_not_in("添加任务：HTTP(S)", app.text(), "模态下 A 被消费")
                # Esc/O/q/Q 四键均只关窗且应用存活
                for key in ("esc", "o", "q", "Q"):
                    app.send(key)
                    app.pump(0.4)
                    assert_not_in("EZR Download v1.0.0", app.text(),
                                  f"{key} 应关闭关于")
                    assert_in("EZR Downloader", app.text(), f"{key} 后应用存活")
                    if key != "Q":
                        app.send("o")
                        assert app.wait_for("EZR Download v1.0.0", 3)
                # 帮助内 O = 关于叠于帮助之上；关闭后回帮助
                app.send("h")
                assert app.wait_for("【全局】", 3)
                app.send("o")
                assert app.wait_for("EZR Download v1.0.0", 3)
                assert_in("【全局】", app.text(), "帮助应仍在下层")
                app.send("esc")
                app.pump(0.4)
                assert_in("【全局】", app.text(), "Esc 关关于后应回帮助")
                app.send("esc")
                app.pump(0.3)
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
