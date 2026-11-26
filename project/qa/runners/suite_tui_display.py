#!/usr/bin/env python3
"""suite_tui_display.py — QA 套件 · 01-tui-display TUI 展示与交互（期号 01）。

对应规程：project/qa/01-tui-display-qa.md（25 用例 = QA-TD-01…28 缺 04/14/16；v1.34 补齐 25–28 可执行化）。
端到端 UI 层验证：伪终端画面/配色单元格断言 + 按键/鼠标（SGR）注入 + fixture 驱动真实状态。
配色断言与 NO_COLOR 无关（产品强制彩色）。环境判据：SGR 鼠标序列由 PTY 直接注入，
无真实鼠标也可验证（产品侧只依赖终端转义序列）。
"""

from __future__ import annotations

import os
import re
import time

from harness import (
    clear_dir_prefill,
    Env, EzrApp, Suite, DUMMY_URL, add_task_via_dialog, assert_in, assert_not_in,
    open_add_dialog_via_cmd,
    chart_points, detail_text, expected_content, file_bytes, parse_speed,
    wait_file_size, QA_FILE_SIZES,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]
LIGHT_BLUE = "7ab9f2"   # RGB(122,185,242) 链接字段名淡蓝（FR-01-101，UC-17 同源）
DIM = "7e8d93"          # 字段名暗灰（FR-01-101 UC-17 同源）


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
        # 对话框拖拽/关于窗口），QA v148 轮按规程可执行化补齐；
        # v1.34 新增 QA-TD-25..28（复制扩展/字段值弹窗/[✗]/未知尺寸），QA 20261122
        # 轮按规程可执行化补齐
        only = os.environ.get("QA_TD_ONLY", "").strip()
        if only:
            getattr(self, f"_case_{int(only):02d}")()
            return self.results
        for i in (1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 17,
                  18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28):
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
                for chip in ("A 命令", "D 删除", "M 修改", "Q 退出", "H 帮助"):
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
            env.write_config("[http]\nblock_size_http = 131072\n")
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
                    if re.search(r"↓ [1-9][\d.]* [KMG]?i?B/s", app.text()):
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
                # 完成 toast 可能被并发任务（恢复的 three-m 30s 慢速）恰同刻的
                # 完成 toast 顶替（QA 终验轮实测竞态，可见窗 ~0.2s）——toast 观测
                # 与落盘持久证据双臂锚定（断言锚定最终态常驻文本纪律）
                target_small = os.path.join(env.save_dir, "small.bin")
                try:
                    app.wait_for("（无校验）", 30)
                except AssertionError:
                    # toast 被同刻并发完成 toast 顶替时以落盘为持久证据
                    wait_file_size(target_small, SMALL_SIZE, 10, app=app)
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
                        app, env, self._slow(env, "two-m.bin", i, rate=8000))
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
        ]/[ 切页签（v1.30：Tab 不再切页签，G 图表快捷键随 FR-01-94
        流量图内嵌退役）；v1.9/FR-02-31 起 t/T 页签循环退役改 trackers 弹窗
        门控键（toast 拒绝且不开窗、页签不变）；H 弹出帮助窗口；O 弹出关于窗口（40×7，再按关闭，
        v1.32/FR-01-111）；Esc 主界面无动作（v1.30/FR-01-108）。
        """
        @self.case("QA-TD-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # A：五字段对话框（74×11 标题 ` 添加任务：HTTP(S) `，v1.32 更名）
                open_add_dialog_via_cmd(app, DUMMY_URL)
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
                # ]/[ 页签切换（v1.30：Tab 不再切页签；v1.9/FR-02-31 起 t/T 页签
                # 循环退役，页签切换仅 ]/[ 承担——T/t 改 trackers 弹窗门控键）
                app.send("]")
                app.pump(0.4)
                assert_in("（此页签下没有任务", app.text(),
                          "] 应切至已完成页签（空态提示）")
                app.send("[")
                app.pump(0.4)
                assert_in("three-m.bin", app.text(), "[ 应回正在下载页签")
                # T/t trackers 弹窗门控（FR-02-31/bug18 轮已批准语义）：toast 拒绝
                # 且不开窗、页签不变（toast 文案二选一 = 选中 HTTP 任务或未选中
                # 态，均在 FR-02-31 门控臂内；断言按 feature v1.42/FR-02-31 口径）
                app.send("T")
                app.pump(0.4)
                assert ("该任务没有 trackers（仅 BT 任务支持）" in app.text()
                        or "未选中任务，无法查看 trackers" in app.text()), \
                    "T 应 toast 拒绝（trackers 门控，FR-02-31）"
                assert_in("three-m.bin", app.text(),
                          "T 拒绝后应停留正在下载页签（页签循环退役）")
                assert_not_in("Trackers", app.text(),
                              "T 不应打开 trackers 窗口")
                app.send("t")
                app.pump(0.4)
                assert ("该任务没有 trackers（仅 BT 任务支持）" in app.text()
                        or "未选中任务，无法查看 trackers" in app.text()), \
                    "t 同口径 toast 拒绝（trackers 门控，FR-02-31）"
                assert_in("three-m.bin", app.text(), "t 拒绝后页签不变")
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

        触发路径同步（本轮）：原「任务已存在」toast 已随 v1.42/bug25 退役为
        「错误」模态窗口（01-add-task-16，D56）——模态窗不自动消失，不可承载
        toast 消失验证——改用「已添加任务」成功 toast（QA-01-20 同源路径，
        稳定可达）：出现、数秒后自动消失、无残留。
        """
        @self.case("QA-TD-09")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=500000"))
                assert app.wait_for("已添加任务", 6), "toast 应出现且清晰可读"
                assert app.wait_gone("已添加任务", 12), "toast 应数秒后自动消失"
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
                    m = re.search(r"全局 ↓ ([1-9][\d.]* [KMG]?i?B/s)", app.text())
                    if m:
                        header = m.group(0)
                        break
                assert header, "头部 ↓ 应显示真实速度（非零）"
                text = app.text()
                m = re.search(r"↑ ([\d.]+ [KMG]?i?B/s)", text)
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
                assert_in("按 A 打开命令窗口添加任务", app.text(), "空态提示（06-cmd-window-07）")
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
        """选中任务行（列表区，左 64 列）的速度读数（↓ X.X KiB/s，二进制族 v1.45）；无读数返回 None。

        跳过含「全局」的头部行与标题行；详情面板在右 64 列起，不参与匹配。
        （QA v148 轮注：本 helper 原置于 _case_14 与 _case_15 之间——_case_14
        随 v1.32 标题栏版本号退役删除时被连带切除，此处原位恢复。）
        """
        for row in app.screen.display:
            left = row[:64]
            if "全局" in left or "EZR Downloader" in left:
                continue
            m = re.search(r"↓ (\d+(?:\.\d+)? [KMG]?i?B/s)", left)
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
            env.write_config("[http]\nblock_size_http = 131072\n")
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
                    m = re.search(r"全局\s+↓ (\d+(?:\.\d+)? [KMG]?i?B/s)",
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
            m = re.match(r"\s*\d+\s+([\d.]+ [KMG]?i?B/s|-)\s+[\d.]+ [KMG]?B", t)
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

        偏差注记（QA 终验轮 20261112）：已下载字节观察锚按 v1.49 口径改读头部
        「会话已下载」（场景 12 规格 = 本次运行累计下载字节，同源于 Evt::Progress
        的 t.downloaded，块内实时推进）；原锚 = HTTP 明细行 `↓ 速度 <已下载>/<总>`
        尺寸对，已随 v1.49 明细行尺寸项整项退役（task_lines.rs:306 注记）。
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
                    m = re.search(r"全局\s+↓ (\d+(?:\.\d+)? [KMG]?i?B/s)", app.text())
                    conns = self._conn_speeds(app)
                    sm = re.search(r"会话已下载\s+([\d.]+ [KMG]?B)", app.text())
                    if ts is None or m is None or not conns or sm is None:
                        time.sleep(0.3)
                        continue
                    t = parse_speed(ts)
                    g = parse_speed(m.group(1))
                    if t <= 0:
                        # 传输中的零读数即被测缺陷形态（块内平台期），直接入样定罪
                        samples.append((0.0, g, conns,
                                        size_b(sm.group(1))))
                        break
                    samples.append((t, g, conns, size_b(sm.group(1))))
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
                open_add_dialog_via_cmd(app, DUMMY_URL)
                ly4, lx4 = border_cells("下载任务")
                assert app.cell_fg(ly4, lx4) != "green", \
                    "浮层打开时列表应降级暗框（FR-01-108）"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_19(self):
        """QA-TD-19 帮助窗口（v1.44 口径）：十二组 80 行、【并发连接】标志缩写 5 条目（FR-02-33/D69）、O 条目、四键关闭、位置记忆、模态。"""
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
                # 十二组分类逐字齐全（v1.44 内容 80 行 > 24 行窗口——滚动收集全文：
                # Home 起步 PgDn ±8 行步进至末页钳制，聚合各视口快照；
                # 滚动深度 80−20=60 ⌈60/8⌉=8 步，取 10 次冗余，末页钳制快照重复无害）
                parts: list[str] = []
                app.send("home")
                app.pump(0.3)
                parts.append(app.text())
                for _ in range(10):
                    app.send("pgdn")
                    app.pump(0.25)
                    parts.append(app.text())
                full = "\n".join(parts)
                for grp in ("【全局】", "【页签】", "【任务列表】", "【任务详情】",
                            "【字段值弹窗】", "【并发连接】", "【滚动条（通用组件）】",
                            "【弹窗", "【pieces 分块弹窗】", "【命令窗口（A）】",
                            "【BT 对话框（添加/修改：BT）】", "【trackers 弹窗（T）】"):
                    assert_in(grp, full, f"帮助应含 {grp}")
                # v1.32 内容点：O 条目、横向滚动条目、滚动条组四条目、拖拽文案
                # 逐字锚点 = help_lines 钦定文案（ui/help.rs，与单测同源）
                for frag in ("打开 / 关闭关于窗口（About）",
                             "横向滚动明细列（客户端/协议等超宽列）",
                             "拖拽滑块 ═║", "按住不放自动连发", "点击轨道",
                             "按住弹窗内空白处拖动移动窗口（滑块/边框除外）"):
                    assert_in(frag, full, f"帮助内容应含「{frag}」（v1.32）")
                # v1.44 增判据（FR-02-33/D69）：【并发连接】组标志缩写 5 条目逐字
                # 齐全（锚点 = help_lines 钦定文案，与单测/QA 判据同源；逐字命中
                # 同时验证条目描述在 70 宽窗口内框完整可见不截尾）
                for frag in ("标志 D / d", "D=正在下载 · d=欲下被 choke",
                             "标志 U / u", "U=正在上传 · u=欲传被 choke",
                             "标志 O / I", "O=乐观免 choke · I=入连（对方拨入）",
                             "标志 K", "K=对方未 choke 我方，且我方对其无需求",
                             "标志 ?", "?=我方未 choke 对方，且对方对我方无兴趣"):
                    assert_in(frag, full, f"帮助内容应含「{frag}」（v1.44/D69）")
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
                assert_in("A 命令 · D 删除 · M 修改 · Q 退出 · H 帮助", text,
                          "页脚 chips 逐字图例（v1.31）")
                # 点击 H chip（原地松开）= 开帮助，与键盘同口径
                #（chip 列位按 buffer 列空间取——页脚行含 CJK，pyte display
                # 串索引会左偏，串列作鼠标坐标会脱靶）
                def chip_pos(needle: str) -> tuple[int, int]:
                    # 自底向上扫（页脚在屏底；「A 命令」同文见于空态引导文案）
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
                ay, ax = chip_pos("A 命令")
                app.send(f"mclick:{ax + 2}:{ay + 1}")
                app.pump(0.15)
                app.send(f"mup:{ax + 2}:{ay + 1}")
                assert app.wait_for("请输入命令或链接", 3), "点击 A chip 应开命令窗口"
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
                open_add_dialog_via_cmd(app, DUMMY_URL)
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
                app.send("tab", "tab", "tab", "tab")  # URL0→保存到1→并发2→下载限速3→校验4（v1.38 限速字段插入）
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
                open_add_dialog_via_cmd(app, DUMMY_URL)
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
                    if opener == "A":
                        open_add_dialog_via_cmd(app, DUMMY_URL)
                    else:
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
    @staticmethod
    def _detail_row_span(app: EzrApp, label: str):
        """详情面板字段行 (row, label_col)：面板界「任务详情」→「并发连接」内定位。"""
        start = end = None
        for y in range(app.screen.lines):
            t = app.screen.display[y]
            if start is None and "任务详情" in t:
                start = y
            elif start is not None and "并发连接" in t:
                end = y
                break
        if start is None:
            return None
        hi = end if end is not None else app.screen.lines
        for y in range(start, hi):
            x = TuiDisplaySuite._find_col(app, y, label, start=64)
            if x is not None:
                return y, x
        return None

    @staticmethod
    def _click(app: EzrApp, row: int, col: int) -> None:
        app.send(f"mclick:{col + 1}:{row + 1}")

    @staticmethod
    def _cell_ul(app: EzrApp, row: int, col: int) -> bool:
        return bool(app.screen.buffer[row][col].underscore)

    @staticmethod
    def _cell_bg_red(app: EzrApp, row: int, col: int) -> bool:
        bg = app.screen.buffer[row][col].bg
        return bg in ("red", "cd0000", "800000") or (
            bg is not None and "cd0000" in str(bg))

    @staticmethod
    def _label_cells_styled(app: EzrApp, label: str, expect_ul: bool,
                            expect_fg: str | None = None) -> None:
        y, x = TuiDisplaySuite._detail_row_span(app, label)
        assert y is not None, f"字段名 {label} 应可见"
        import wcwidth as _ww
        xi = x
        for ch in label:
            cell = app.screen.buffer[y][xi]
            assert cell.data == ch, f"{label} 列位还原失败: {cell.data!r}"
            if expect_fg is not None:
                assert cell.fg == expect_fg, \
                    f"{label} 字段名配色应 {expect_fg}: {cell.fg}@{xi}"
            assert TuiDisplaySuite._cell_ul(app, y, xi) == expect_ul, \
                f"{label} 字段名下划线应 {expect_ul}@{xi}"
            w = _ww.wcwidth(ch)
            xi += w if w and w > 0 else 1
        pad_end = x + 9
        while xi < pad_end:
            assert not TuiDisplaySuite._cell_ul(app, y, xi), \
                f"{label} 填充空格不应有下划线@{xi}"
            xi += 1

    def _click_value_of(self, app: EzrApp, label: str, value_head: str) -> None:
        """点详情面板字段「值」文本（同 row、label 后列空间定位）开字段值弹窗。"""
        pos = self._detail_row_span(app, label)
        assert pos is not None, f"{label} 字段名应可见"
        vx = TuiDisplaySuite._find_col(app, pos[0], value_head, start=pos[1] + 1)
        assert vx is not None, f"{label} 值文本应可见"
        self._click(app, pos[0], vx)

    @staticmethod
    def _popup_span_from_content(app: EzrApp, needle: str):
        """字段值弹窗几何：按内容 needle 定位弹体行，向上找 ╭ 顶边框行。

        （弹窗标题文本与详情面板字段名同串冲突——panel_span 的 needle 首
        命中不可用；字段值弹窗体含完整值后半段，为唯一内容锚。）
        """
        for i in range(app.screen.lines):
            if TuiDisplaySuite._find_col(app, i, needle) is None:
                continue
            ty = None
            for yy in range(i, -1, -1):
                for xx in range(app.screen.columns):
                    if app.screen.buffer[yy][xx].data == "╭":
                        ty = yy
                        break
                if ty is not None:
                    break
            if ty is None:
                continue
            left = right = None
            for xx in range(app.screen.columns):
                if app.screen.buffer[ty][xx].data == "╭":
                    left = xx
                    break
            for xx in range(app.screen.columns - 1,
                            left if left is not None else 0, -1):
                if app.screen.buffer[ty][xx].data == "╮":
                    right = xx
                    break
            if left is None or right is None:
                continue
            bot = None
            for yy in range(ty + 1, app.screen.lines):
                if "╰" in app.screen.display[yy]:
                    bot = yy
                    break
            return ty, left, right, bot
        return None

    @staticmethod
    def _marquee_first_col(track_row: str) -> int | None:
        """进度条行走马灯填充段首列（非 ░ 填充字符首列；纯空轨 None）。"""
        import wcwidth as _ww
        x = 0
        first = None
        for ch in track_row:
            if ch != "░" and ch.strip() and ch not in ("│", "║", "┤", "├"):
                if first is None:
                    first = x
            x += _ww.wcwidth(ch) if _ww.wcwidth(ch) > 0 else 1
        return first

    def _case_25(self):
        """QA-TD-25 失败任务详情复制扩展（FR-01-101/D42）：字段名样式 + 失败/路径复制。

        已失败+提供校验码任务：字段名 = ID/创建/失败/路径（旧名退役）；校验/
        URL/失败/路径 四字段名淡蓝下划线，排队/ID/创建/类型/大小/分块 暗灰无
        下划线；点「失败」复制完整错误文案 + toast `已复制 失败原因`；点「路径」
        复制完整保存路径 + toast `已复制 保存路径`；URL/校验 同 QA-UC-18 口径；
        URL 值悬停无双下划线（break_links VS15 断链——静态单元格无叠加下划线）。
        失败路径 = fixture redirect=20 链：产品 reqwest Policy::limited(10) →
        「重定向次数超限」Fatal 即时失败（秒级，不涉 503 类 176s 重试链）。
        """
        @self.case("QA-TD-25")
        def go(env: Env):
            CK = "e3b0c44298fc1c149afbf4c8996fb924" \
                 "27ae41e4649b934ca495991b7852b855"
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?redirect=20"),
                    ck_value=CK)
                assert app.wait_for("已失败", 20), "重定向超限应秒级失败"
                assert_not_in("任务 ID", app.text(), "旧「任务 ID」名退役")
                assert_not_in("创建时间", app.text(), "旧「创建时间」名退役")
                assert_not_in("失败原因", app.text(), "旧「失败原因」名退役")
                for label in ("校验", "URL", "失败", "路径"):
                    self._label_cells_styled(app, label, expect_ul=True,
                                             expect_fg=LIGHT_BLUE)
                for label in ("ID", "创建", "类型", "大小", "分块"):
                    pos = self._detail_row_span(app, label)
                    if pos is None:
                        continue
                    y, x = pos
                    assert app.cell_fg(y, x) == DIM, \
                        f"{label} 字段名应暗灰: {app.cell_fg(y, x)}"
                    assert not self._cell_ul(app, y, x), \
                        f"{label} 字段名不应有下划线"
                pos = self._detail_row_span(app, "URL")
                assert pos is not None
                vx = self._find_col(app, pos[0], "http", start=pos[1] + 1)
                assert vx is not None, "URL 值应可见"
                for x2 in range(vx, min(vx + 32, app.screen.columns)):
                    assert not self._cell_ul(app, pos[0], x2), \
                        f"URL 值不应叠加链接下划线@{x2}"
                pos = self._detail_row_span(app, "失败")
                assert pos is not None
                self._click(app, pos[0], pos[1])
                assert app.wait_for("已复制 失败原因", 5), "点失败应 toast 复制"
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == "重定向次数超限", \
                    f"OSC52 应为完整失败文案: {payloads[-1:] if payloads else '[]'}"
                assert app.wait_gone("已复制 失败原因", 8)
                pos = self._detail_row_span(app, "路径")
                assert pos is not None
                self._click(app, pos[0], pos[1])
                assert app.wait_for("已复制 保存路径", 5), "点路径应 toast 复制"
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == os.path.join(
                    env.save_dir, "three-m.bin"), \
                    f"OSC52 应为完整路径: {payloads[-1:] if payloads else '[]'}"
                assert app.wait_gone("已复制 保存路径", 8)
                pos = self._detail_row_span(app, "URL")
                assert pos is not None
                self._click(app, pos[0], pos[1])
                assert app.wait_for("已复制 url", 5)
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == env.fixture.url(
                    "three-m.bin?redirect=20"), \
                    f"OSC52 应为详情展示 URL: {payloads[-1:] if payloads else '[]'}"
                assert app.wait_gone("已复制 url", 8)
                pos = self._detail_row_span(app, "校验")
                assert pos is not None
                self._click(app, pos[0], pos[1])
                assert app.wait_for("已复制 校验码", 5)
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == CK, \
                    f"OSC52 应为完整校验码: {payloads[-1:] if payloads else '[]'}"
            finally:
                app.graceful_quit()

    def _case_26(self):
        """QA-TD-26 字段值弹窗（FR-01-101/D42）：48×12 快照 wrap/Y 复制/滚动/关闭通道。

        点「校验」字段值开弹窗：48×12 圆角浮层、打开时快照完整值 wrap 折行
        无截断、Y 复制 + toast、短快照滚动键位钳制、Esc/[✗]/q 关闭且应用存活、
        点击外部无操作、拖拽随动 + 关闭即清（重开居中）、URL/失败/路径 同形态、
        无校验任务无校验字段行（字段不可用不打开）、弹窗内 A 消费为无动作、
        Ctrl+C 直接退出。
        """
        @self.case("QA-TD-26")
        def go(env: Env):
            CK = "e3b0c44298fc1c149afbf4c8996fb924" \
                 "27ae41e4649b934ca495991b7852b855"
            CKA = CK[:32]
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "three-m.bin", 26), ck_value=CK)
                assert app.wait_for("下载中", 8)
                self._click_value_of(app, "校验", "SHA-256")
                assert app.wait_for(CKA, 4), "点校验值应开字段值弹窗"
                g = self._popup_span_from_content(app, CKA)
                assert g is not None, "字段值弹窗几何应在位"
                ty, l, r, bot = g
                assert r - l == 47, f"字段值弹窗应 48 列（实测 {r - l + 1}）"
                assert bot is not None and bot - ty == 11, \
                    f"字段值弹窗应 12 行（实测 {bot - ty + 1}）"
                body = "\n".join(
                    app.screen.display[i][l:r + 1]
                    for i in range(ty + 1, bot))
                body_flat = body.replace(" ", "")
                assert CK[:42] in body_flat and CK[42:] in body_flat, \
                    "64 位校验码应完整呈现（wrap 折行无截断，内容区 42 列分断）"
                app.send("Y")
                assert app.wait_for("已复制 校验码", 5), "Y 应复制弹窗全部内容"
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == CK, \
                    f"Y OSC52 应为完整校验码: {payloads[-1:] if payloads else '[]'}"
                assert app.wait_gone("已复制 校验码", 8)
                for k in ("pgdn", "end", "home", "pgup", "up", "down"):
                    app.send(k)
                    app.pump(0.2)
                body_after = "\n".join(
                    app.screen.display[i][l:r + 1]
                    for i in range(ty + 1, bot)).replace(" ", "")
                assert body_after == body_flat, \
                    "短快照（2 行）滚动键位应钳制内容不变"
                app.send(f"mclick:{l + 3}:{ty + 1}",
                         f"mdrag:{l + 5}:{ty + 2}")
                app.pump(0.2)
                app.send(f"mup:{l + 5}:{ty + 2}")
                app.pump(0.4)
                g1 = self._popup_span_from_content(app, CKA)
                assert g1 is not None and g1[0] == ty + 1, \
                    f"弹窗标题栏拖拽应随动: {g1[0] if g1 else None} ≠ {ty + 1}"
                app.send("esc")
                app.pump(0.4)
                assert self._popup_span_from_content(app, CKA) is None, \
                    "Esc 应关闭弹窗"
                self._click_value_of(app, "校验", "SHA-256")
                assert app.wait_for(CKA, 4)
                g2 = self._popup_span_from_content(app, CKA)
                assert g2 is not None and g2[0] == ty, "重开应重新居中"
                ty2, l2, r2, bot2 = g2
                app.send("mclick:2:2")
                app.pump(0.15)
                app.send("mup:2:2")
                app.pump(0.4)
                assert_in(CKA, app.text(), "点外应无动作")
                app.send(f"mclick:{r2 - 1}:{ty2 + 1}")
                app.pump(0.15)
                app.send(f"mup:{r2 - 1}:{ty2 + 1}")
                app.pump(0.5)
                assert self._popup_span_from_content(app, CKA) is None, \
                    "[✗] 应关闭弹窗"
                self._click_value_of(app, "校验", "SHA-256")
                assert app.wait_for(CKA, 4)
                app.send("q")
                app.pump(0.4)
                assert self._popup_span_from_content(app, CKA) is None, \
                    "q 应关闭弹窗"
                self._click_value_of(app, "URL", "http")
                assert app.wait_for("http", 4), "点 URL 值应开 URL 弹窗"
                gu = self._popup_span_from_content(app, "http")
                assert gu is not None, "URL 弹窗几何应在位"
                assert gu[2] - gu[1] == 47, "URL 弹窗应 48 列"
                for k in ("pgdn", "end", "home", "up"):
                    app.send(k)
                    app.pump(0.2)
                assert_in("http", app.text(), "滚动后弹窗应存活")
                app.send("A")
                app.pump(0.3)
                assert self._popup_span_from_content(app, "http") is not None, \
                    "弹窗内 A 应被消费（弹窗不关不叠层）"
                app.send("esc")
                app.pump(0.4)
                add_task_via_dialog(app, env, self._slow(env, "two-m.bin", 26))
                assert app.wait_for("下载中", 8)
                assert self._detail_row_span(app, "校验") is None, \
                    "无校验任务不应有校验字段行"
                app.send("ctrl_c")
                deadline = time.time() + 6
                while time.time() < deadline and app.exit_code is None:
                    app.pump(0.1)
                assert app.exit_code == 0, "弹窗内 Ctrl+C 应直接退出"
                app = None
            finally:
                if app is not None:
                    app.graceful_quit()

    def _case_27(self):
        """QA-TD-27 七弹窗统一 [✗]（FR-01-114/D41）：几何/配色/按压/拖出/误触防护。

        添加/修改/删除/pieces/帮助/关于/字段值 七弹窗逐个：[✗] 3 列宽（U+2717、
        右角符内侧 1 列 + 左侧 padding、不覆盖边框线）；中括号与弹窗边框线同色
        （绿+粗）；✗ 常态 DIM2、按压仅前景红+粗（无背景反色）；原地松开 =
        关闭对应弹窗（对话框 = 取消不创建不改动）；拖出热区松开 = 取消不关闭；
        padding 列点击无动作不误触；下拉展开期点 [✗] 位置先收起下拉；
        弹窗过窄/与标题重叠时放弃绘制（P7，负几何注入不做）。
        """
        @self.case("QA-TD-27")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                open_add_dialog_via_cmd(app, DUMMY_URL)
                self._close_x_assert(app, "添加任务：HTTP(S)", "对话框")
                app.pump(0.3)
                for key, probe, span_probe, tag in (
                        ("h", "【全局】", "帮助", "帮助"),
                        ("o", "EZR Download v1.0.0", "About", "关于")):
                    app.send(key)
                    assert app.wait_for(probe, 4), f"{tag} 应打开"
                    self._close_x_assert(app, span_probe, tag,
                                         close_probe=probe)
                    app.pump(0.3)
                CK = "ab" * 32
                add_task_via_dialog(app, env, self._slow(env, "two-m.bin", 27),
                                    ck_value=CK)
                assert app.wait_for("下载中", 8)
                app.send("D")
                assert app.wait_for("删除任务", 4), "删除框应打开"
                self._close_x_assert(app, "删除任务", "删除框")
                app.pump(0.3)
                app.send("P")
                app.pump(0.6)
                if "分块状态" in app.text():
                    self._close_x_assert(app, "分块状态", "pieces",
                                         close_probe="分块状态")
                    app.pump(0.3)
                pos = self._detail_row_span(app, "校验")
                assert pos is not None
                self._click_value_of(app, "校验", "SHA-256")
                assert app.wait_for(CK[:32], 4)
                self._close_x_assert(
                    app, "校验", "字段值",
                    close_probe=CK[:32],
                    span=self._popup_span_from_content(app, CK[:32]))
                app.pump(0.3)
                app.send("M")
                app.pump(0.6)
                assert_in("修改任务", app.text(), "修改框应打开")
                gm = self._panel_span(app, "修改任务")
                assert gm is not None
                tym, lm, rm = gm
                app.send("tab", "tab")
                app.send("enter")
                app.pump(0.3)
                assert_in("NONE", app.text(), "校验下拉应展开")
                app.send(f"mclick:{rm - 1}:{tym + 1}")
                app.pump(0.15)
                app.send(f"mup:{rm - 1}:{tym + 1}")
                app.pump(0.4)
                assert_not_in("NONE", app.text(), "下拉展开期点 ✗ 应先收起下拉")
                assert_in("修改任务", app.text(), "对话框应保持打开")
                app.send("mclick:2:2")
                app.pump(0.15)
                app.send("mup:2:2")
                app.pump(0.4)
                app.send("esc")
                app.pump(0.4)
                app.send("M")
                app.pump(0.8)
                assert_in("修改任务", app.text(), "修改框重开应在位")
                self._close_x_assert(app, "修改任务", "修改框")
            finally:
                app.graceful_quit()

    def _close_x_assert(self, app: EzrApp, probe: str, tag: str,
                        close_probe: str | None = None,
                        span: tuple | None = None) -> None:
        """单弹窗 [✗]：几何/配色 + padding 无误触 + 拖出不关 + 按压态 + 原地松开。

        close_probe = 关闭后应消失的窗口体特有文本（帮助/关于等标题与页脚
        chips 同串时用内容组标题判关闭；缺省 = probe）。
        """
        close_probe = close_probe or probe
        app.send("mclick:2:2")
        app.pump(0.15)
        app.send("mup:2:2")
        app.pump(0.4)
        g = span if span is not None else self._panel_span(app, probe)
        assert g is not None, f"{tag} 弹窗几何应在位"
        ty, l, r = g[0], g[1], g[2]
        xc = r - 3
        cell = app.screen.buffer[ty][xc]
        assert cell.data == "✗", f"{tag} [✗] 应为 U+2717: {cell.data!r}"
        assert app.screen.buffer[ty][r - 4].data == "[", \
            f"{tag} [✗] 左括号应在位"
        assert app.screen.buffer[ty][r - 2].data == "]", \
            f"{tag} [✗] 右括号应在位"
        assert app.screen.buffer[ty][r - 1].data == "─", \
            f"{tag} [✗] 右侧应有 1 列边框线 padding（不覆盖边框）"
        border_fg = app.screen.buffer[ty][l].fg
        for bx in (r - 4, r - 2):
            bcell = app.screen.buffer[ty][bx]
            assert bcell.fg == border_fg, \
                f"{tag} 中括号应与边框线同色: {bcell.fg} ≠ {border_fg}"
            assert bcell.bold, f"{tag} 中括号应粗体"
        assert cell.fg != border_fg, \
            f"{tag} ✗ 常态应 DIM2 区分于边框绿: {cell.fg}"
        assert not cell.bold, f"{tag} ✗ 常态不应粗体"
        assert not self._cell_bg_red(app, ty, xc), \
            f"{tag} [✗] 常态无背景色"
        app.send(f"mclick:{r - 5 + 1}:{ty + 1}")
        app.pump(0.15)
        app.send(f"mup:{r - 5 + 1}:{ty + 1}")
        app.pump(0.4)
        assert_in(probe, app.text(), f"{tag} padding 列点击应无动作")
        app.send(f"mclick:{xc + 1}:{ty + 1}",
                 f"mdrag:{xc - 8}:{ty + 2}")
        app.pump(0.2)
        app.send(f"mup:{xc - 8}:{ty + 2}")
        app.pump(0.4)
        assert_in(probe, app.text(), f"{tag} 拖出热区松开不应关闭")
        app.send(f"mclick:{xc + 1}:{ty + 1}")
        app.pump(0.3)
        pcell = app.screen.buffer[ty][xc]
        assert pcell.data == "✗", f"{tag} 按压态 ✗ 应仍在位"
        assert pcell.fg in ("red", "cd0000", "e25c54"), \
            f"{tag} 按压态 ✗ 应前景红: {pcell.fg}"
        assert pcell.bold, f"{tag} 按压态 ✗ 应粗体"
        assert not self._cell_bg_red(app, ty, xc), \
            f"{tag} 按压态无背景反色"
        app.send(f"mup:{xc + 1}:{ty + 1}")
        app.pump(0.5)
        assert_not_in(close_probe, app.text(), f"{tag} 原地松开应关闭")
        assert_in("EZR Downloader", app.text(), f"{tag} 关闭后应用存活")

    def _case_28(self):
        """QA-TD-28 未知尺寸（无 Content-Length）：?%/走马灯/LT -/? B 与流结束恢复。

        fixture `streamy*` 文件名 = chunked 无 Content-Length（size_known=false）：
        下载中百分比 `?%`（右对齐）、进度条走马灯循环色块（连续采样 ≥2 相位
        确认循环）、任务行/统计行无尺寸项、剩余 `LT -`（v1.52 标签缩写）、
        详情大小行 `<已下载> / ? B`、分块行恒 `-`；已暂停进度条空轨道（全 ░）
        + `LT -`；流结束后详情大小行恢复实际大小、`?` 占位退役；支持续传对照
        任务百分比/进度条/ETA 不受影响。
        """
        @self.case("QA-TD-28")
        def go(env: Env):
            streamy = os.path.join(env.fixture_root, "streamy-five-m.bin")
            os.makedirs(env.fixture_root, exist_ok=True)
            if file_bytes(streamy) is None:
                with open(streamy, "wb") as f:
                    f.write(expected_content(QA_FILE_SIZES["five-m.bin"]))
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("streamy-five-m.bin?speed=200000"))
                assert app.wait_for("下载中", 8)
                rows = app.display_rows()
                lrow = next(r for r in rows if "streamy-five-m.bin" in r)
                assert "?%" in lrow, f"未知尺寸应显示 ?%: {lrow}"
                lidx = next(i for i, r in enumerate(rows)
                            if "streamy-five-m.bin" in r)
                block = "\n".join(rows[lidx:lidx + 4])
                assert "LT -" in block, \
                    f"任务块明细行剩余应显示 LT -: {block[:200]}"
                assert " MB" not in lrow.replace("five-m.bin", ""), \
                    f"行 1 不应显示尺寸项: {lrow}"
                pidx = lidx + 1
                track = rows[pidx]
                assert "░" in track, f"进度条行应有空轨道底: {track!r}"
                seq = []
                for _ in range(15):
                    app.pump(0.25)
                    rows = app.display_rows()
                    seg = self._marquee_first_col(rows[pidx])
                    if seg is not None:
                        seq.append(seg)
                assert len(set(seq)) >= 2, \
                    f"走马灯色块应随帧移动: {seq}"
                wraps = any(b < a for a, b in zip(seq, seq[1:]))
                assert wraps, f"走马灯应循环回绕（滑出后重新滑入）: {seq}"
                d = detail_text(app)
                assert "/ ? B" in d, "详情大小行应 <已下载> / ? B"
                pb = self._detail_row_span(app, "分块")
                assert pb is not None, "详情应有分块行"
                prow = app.screen.display[pb[0]]
                after_label = prow[pb[1] + 4:].strip()
                assert after_label.startswith("-"), \
                    f"未知尺寸任务分块行应恒 -: {after_label!r}"
                app.send("space")
                assert app.wait_for("已暂停", 5)
                rows = app.display_rows()
                lidx = next(i for i, r in enumerate(rows)
                            if "streamy-five-m.bin" in r)
                pblock = rows[lidx:lidx + 4]
                ptrack = next((r for r in pblock if "░" in r), None)
                assert ptrack is not None, \
                    f"暂停态应有进度条空轨道行: {pblock}"
                assert "█" not in ptrack and "▓" not in ptrack, \
                    f"暂停态进度条应空轨道: {ptrack!r}"
                assert "LT -" in "\n".join(pblock), \
                    f"暂停态剩余应 LT -: {pblock}"
                app.send("space")
                assert app.wait_for("下载中", 8)
                assert app.wait_gone("streamy-five-m.bin", 120), \
                    "streamy 任务应完成移出下载页签（120s）"
                app.send("]")
                app.pump(0.6)
                rows = app.display_rows()
                srows = [r for r in rows if "streamy-five-m.bin" in r]
                assert srows and "[已完成]" in srows[0], \
                    f"已完成页签应有 streamy 任务行: {srows[:1]}"
                d = detail_text(app)
                assert "/ ? B" not in d, "完成后 ? 占位应退役"
                assert "5.0 MB" in d, \
                    f"完成后大小行应恢复实际大小（详情尺寸十进制 MB 口径）: {[l for l in d.splitlines() if '大小' in l][:1]}"
                assert "?%" not in app.text(), "完成后 ?% 应退役"
                add_task_via_dialog(
                    app, env, env.fixture.url("two-m.bin?speed=400000"))
                assert app.wait_for("下载中", 8)
                eta_ok = False
                block2 = ""
                lrow = ""
                deadline = time.time() + 20
                while time.time() < deadline:
                    app.pump(0.5)
                    rows = app.display_rows()
                    lidx2 = next((i for i, r in enumerate(rows)
                                  if "two-m.bin" in r), None)
                    if lidx2 is None:
                        continue
                    block2 = "\n".join(rows[lidx2:lidx2 + 4])
                    if re.search(r"LT [0-9]", block2):
                        eta_ok = True
                        break
                lrow = next((r for r in rows if "two-m.bin" in r), "")
                assert eta_ok, f"对照任务 ETA 应有值（LT 后数字）: {block2[:200]}"
                assert "?%" not in block2, f"对照任务应数值百分比: {block2[:200]}"
                assert "%" in lrow, f"对照任务应有百分比: {lrow}"
                assert "LT" in block2, f"对照任务应显示 LT: {block2[:200]}"
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
