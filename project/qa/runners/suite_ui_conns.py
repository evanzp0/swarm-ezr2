#!/usr/bin/env python3
"""suite_ui_conns.py — QA 套件 · 04-ui-conns UI 对齐 ezr-demo 定稿（期号 01）。

对应规程：project/qa/04-ui-conns-qa.md（18 用例）。
端到端 UI 层验证：伪终端画面/逐单元格配色断言 + 按键/鼠标注入（含 Ctrl 组合键与
SGR 鼠标）+ fixture 驱动真实状态；不使用项目内部 API。

权威参照：ezr-demo/docs/UI交互需求文档-定稿.md v1.4 与 ezr-demo 源码。
QA v138 会话新增（本套件为 specifier 规程的可执行化落产物，此前缺失）：
覆盖 FR-01-94…103（v1.13 定稿布局/流量图/并发连接面板/交互守卫链）与
FR-01-100…102（v1.14/v1.15 详情类型行、链接样式复制、速度同口径平滑）。

时序纪律：门控 Given（?speed= 慢速维持观察窗）；正探有界 wait_for、负探单发；
配色断言无视 NO_COLOR（产品强制彩色）；布局锚点动态定位（标题行/分隔符列），
不硬编码列位（engineering.md 区域锚定与面板箱体截取纪律）。
"""

from __future__ import annotations

import base64
import os
import re
import time

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_in, assert_not_in,
    chart_points, detail_text, expected_content, parse_speed, task_pct,
    wait_file_size, QA_FILE_SIZES,
)

LIGHT_BLUE = "7ab9f2"   # RGB(122,185,242) 下载淡蓝（峰值↓图例/链接字段名/连接速度）
MAGENTA = "d670d6"      # RGB(214,112,214) 峰值↑
YELLOW = "e4b23e"       # RGB(228,178,62) 协议徽标/算法名
DIM = "7e8d93"          # RGB(126,141,147) 普通字段名
SEL_BG = "162a30"       # RGB(22,42,48) 选中行深青背景
BLOCKS = "▁▂▃▄▅▆▇█"
BRAILLE = "".join(chr(c) for c in range(0x2800, 0x2900))


def _cell_bg(app: EzrApp, row: int, col: int) -> str | None:
    return app.screen.buffer[row][col].bg


def _cell_bold(app: EzrApp, row: int, col: int) -> bool:
    return bool(app.screen.buffer[row][col].bold)


def _cell_ul(app: EzrApp, row: int, col: int) -> bool:
    return bool(app.screen.buffer[row][col].underscore)


def _right_text(app: EzrApp, y: int, col: int = 64) -> str:
    """右栏箱体文本（buffer 列空间渲染，左栏 CJK 不影响起点）。

    pyte display 串会因宽字符缩短而错位（左栏 CJK 使 row[64:] 切进右栏
    中段）——直接走 buffer 单元格：跳过宽字符续位空格（data==""），得到
    仅右栏、起始列精确的文本。
    """
    parts: list[str] = []
    for x in range(col, app.screen.columns):
        d = app.screen.buffer[y][x].data
        if d:
            parts.append(d)
    return "".join(parts).rstrip()


def _find_in_row(app: EzrApp, y: int, text: str, start_col: int = 0) -> int | None:
    """在 row 的 buffer 列空间中查找 text，返回其首列号（CJK 宽字符列位还原）。

    列推进按单元格显示宽度（wcwidth）累加，天然跳过宽字符续位单元格；
    未命中返回 None。
    """
    import wcwidth as _ww
    n = len(text)
    x = start_col
    cols = app.screen.columns
    while x < cols:
        # 尝试在 x 匹配 text
        xi = x
        ok = True
        for ch in text:
            if xi >= cols or app.screen.buffer[y][xi].data != ch:
                ok = False
                break
            w = _ww.wcwidth(ch)
            xi += w if w > 0 else 1
        if ok:
            return x
        # 推进到下一列（按当前单元格宽度）
        d = app.screen.buffer[y][x].data
        w = _ww.wcwidth(d) if d else 1
        x += w if w and w > 0 else 1
    return None


def _title_row(app: EzrApp, needle: str) -> int | None:
    """面板标题行号（右栏文本含 needle 的第一行）。"""
    for i in range(app.screen.lines):
        if needle in _right_text(app, i) or needle in app.screen.display[i]:
            return i
    return None


def _sep_col(app: EzrApp) -> int | None:
    """头部 │ 分隔符列（行 1–3 同列出现的 │；列空间判定）。"""
    cand: list[int] = []
    for x in range(1, app.screen.columns - 1):  # 排除面板左右边框列
        if all(app.screen.buffer[y][x].data == "│" for y in (1, 2, 3)):
            cand.append(x)
    return cand[0] if cand else None


def _chart_bounds(app: EzrApp) -> tuple[int, int, int] | None:
    """图区 (top_row, left_col, right_col)（列空间；右缘含末个图列）。"""
    sep = _sep_col(app)
    if sep is None:
        return None
    # 图区 = 分隔符右侧 1 列空白之后、右缘 1 列空白之前（header.rs：chart_area
    # 宽 = chart_w，右缘留白列不含块字符）
    return 1, sep + 2, app.screen.columns - 3


def _header_field(app: EzrApp, label: str) -> str | None:
    """头部行 1 字段值（label 后至下一双空格字段标签前的文本）。"""
    row = app.screen.display[1]
    m = re.search(re.escape(label) + r"\s*(\S+(?: \S+)*?)(?:   |$)", row)
    return m.group(1).strip() if m else None


def _conns_data_rows(app: EzrApp) -> list[tuple[int, str | None, str | None]]:
    """并发连接面板数据行 [(序号, 速度或'-', 累计)]（右栏列空间，表头/提示行除外）。"""
    start = _title_row(app, "并发连接")
    if start is None:
        return []
    out: list[tuple[int, str | None, str | None]] = []
    for y in range(start + 1, app.screen.lines):
        seg = _right_text(app, y).replace("│", " ")
        m = re.match(r"\s*(\d+)\s+([\d.]+ [KMG]?B/s|-)\s+([\d.]+ [KMG]?B)\s*$", seg)
        if m:
            out.append((int(m.group(1)), m.group(2), m.group(3)))
    return out


def _click(app: EzrApp, row: int, col: int) -> None:
    """SGR 左键点击（0 基屏幕坐标 → 1 基 SGR 编码）。"""
    app.send(f"mclick:{col + 1}:{row + 1}")


def _label_pos(app: EzrApp, label: str) -> tuple[int, int] | None:
    """详情面板字段名 label 的 (row, col)（右栏列空间精确定位）。"""
    start = _title_row(app, "任务详情")
    if start is None:
        return None
    end = _title_row(app, "并发连接") or app.screen.lines
    for y in range(start, end):
        x = _find_in_row(app, y, label, start_col=64)
        if x is not None:
            return y, x
    return None


class UiConnsSuite(Suite):
    def __init__(self):
        super().__init__("04-ui-conns", port_base=42200)
        self._cur_app: "EzrApp | None" = None

    def _snapshot(self, env: Env, case_id: str) -> None:
        if self._cur_app is None:
            return
        try:
            from harness import TMP_ROOT
            with open(os.path.join(TMP_ROOT, f"{case_id}.screen.txt"), "w",
                      encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _slow(env: Env, name: str, seq: int, rate: int = 300000) -> str:
        return env.fixture.url(f"{name}?swapsize={9000000 + seq}&speed={rate}")

    @staticmethod
    def _detail_box(app: "EzrApp") -> str:
        """详情面板箱体文本（右栏列空间；任务详情标题行起、并发连接标题行止）。"""
        start = _title_row(app, "任务详情")
        if start is None:
            return ""
        end = _title_row(app, "并发连接") or app.screen.lines
        return "\n".join(_right_text(app, y) for y in range(start, end))

    def run(self) -> list:
        for i in (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    # -- 用例 -----------------------------------------------------------------

    def _case_01(self):
        """QA-UC-01 头部通栏 5 行定稿布局：标题行、无任务队列、列表占左列全高。"""
        @self.case("QA-UC-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 1))
                assert app.wait_for("下载中", 8)
                rows = app.screen.display
                # 标题行：` ◆ `（青色加粗）+ `EZR Downloader `（白色加粗）左对齐，
                # 名称前后各 1 列空白（padding 对称）；**不含版本号**
                #（v1.32/FR-01-94①：版本展示面迁移关于窗口 FR-01-111）
                assert_in("◆ EZR Downloader", rows[0], "标题行含产品名")
                assert_not_in("v0.1.0", rows[0], \
                    "标题行不应含版本号（v1.32，全屏唯一展示面 = 关于窗口）")
                assert rows[0].rstrip().endswith("╮"), \
                    f"头部应通栏全宽（右缘贴屏幕右缘）: {rows[0][-6:]!r}"
                # 左对齐 + 名称前后各 1 列空白：◆ 前恰 1 列空白（行首 ╭ 后）
                tcol = _find_in_row(app, 0, "EZR Downloader")
                assert tcol is not None and tcol < 25, \
                    f"标题应左对齐（产品名列 {tcol}）"
                # 峰值不出现在标题行（FR-01-94：峰值在内容行 1）
                assert "峰值" not in rows[0], "峰值不应写在标题行"
                # 头部 5 行：行 4 为底边框（─ 横线）
                assert set(rows[4].replace(" ", "")) <= set("─╰╯"), \
                    f"行 4 应为头部底边框: {rows[4][:20]!r}"
                # 任务队列面板撤销
                assert_not_in("任务队列", app.text(), "全屏不应存在「任务队列」面板")
                # 任务详情面板自头部正下方起（行 5）
                d = _title_row(app, "任务详情")
                assert d == 5, f"右栏详情应自头部正下方（行 5）起: {d}"
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-UC-02 头部字段单行顺序与峰值图例配色（单元格级）。"""
        @self.case("QA-UC-02")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 2))
                assert app.wait_for("下载中", 8)
                # 字段固定顺序单行：并发 → 会话已下载 → 全局 ↓↑ → 峰值 ↓↑
                # （列空间定位：display 串因 CJK 缩短，直接索引会错位）
                order = ["并发", "会话已下载", "全局", "峰值"]
                pos = []
                from_col = 0
                for lbl in order:
                    x = _find_in_row(app, 1, lbl, start_col=from_col)
                    assert x is not None, f"头部字段 {lbl} 应齐备"
                    pos.append(x)
                    from_col = x + 4
                assert pos == sorted(pos), f"字段顺序应为 并发→会话→全局→峰值: {pos}"
                # 峰值在内容行 1（非标题行边框）
                # 单元格配色：峰值 ↓ 淡蓝、峰值 ↑ 品红
                pk = pos[3]
                dlp = _find_in_row(app, 1, "↓", start_col=pk)
                ulp = _find_in_row(app, 1, "↑", start_col=dlp + 1)
                fg_dl = app.cell_fg(1, dlp)
                fg_ul = app.cell_fg(1, ulp)
                assert fg_dl == LIGHT_BLUE, f"峰值 ↓ 应为下载淡蓝 RGB(122,185,242): {fg_dl}"
                assert fg_ul == MAGENTA, f"峰值 ↑ 应为品红: {fg_ul}"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-UC-03 页签行并入头部、槽位内联左对齐、满槽黄色加粗、Tab 高亮。"""
        @self.case("QA-UC-03")
        def go(env: Env):
            env.write_config("download_slots = 2\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 3))
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 4))
                assert app.wait_for("下载槽位 2/2", 15)
                add_task_via_dialog(app, env, self._slow(env, "two-m.bin", 8))
                assert app.wait_for("排队第", 10)
                row = app.screen.display[3]
                # 行内容 正在下载 (n) │ 已完成 (n) │ 下载槽位 x/y（内联左对齐）
                m = re.search(r"正在下载 \(\d+\) │ 已完成 \(\d+\) │ 下载槽位 2/2", row)
                assert m, f"页签行应为定稿三段式且槽位内联: {row[:60]!r}"
                # 槽位段非行尾右对齐：槽位文本之后、分隔符之前应无正文（列空间）
                c = _find_in_row(app, 3, "2/2")
                sep = _sep_col(app)
                assert c is not None and sep is not None and c < sep - 4, \
                    f"槽位段应内联左对齐（紧跟页签）: 槽位列 {c} 分隔符列 {sep}"
                assert all(app.screen.buffer[3][x].data in (" ", "")
                           for x in range(c + 3, sep)), \
                    "槽位段与分隔符间应仅空白（非右对齐）"
                # 满槽黄色加粗（2/2 末字符格，列空间）
                assert app.cell_fg(3, c + 2) == YELLOW, \
                    f"满槽应为黄色: {app.cell_fg(3, c + 2)}"
                assert _cell_bold(app, 3, c + 2), "满槽应加粗"
                # ] 切换页签（v1.30 页签键改版：Tab 改面板焦点）：活动页签黑字青底加粗
                before = _find_in_row(app, 3, "正在下载")
                # 终端真彩口径：黑=RGB(0,0,0)、青=SGR 36 上抛 RGB 形态（00cdcd 实测）
                assert app.cell_fg(3, before + 1) in ("black", "000000") \
                    and _cell_bg(app, 3, before + 1) in ("cyan", "00cdcd"), \
                    f"活动页签应为黑字青底: fg={app.cell_fg(3, before + 1)} " \
                    f"bg={_cell_bg(app, 3, before + 1)}"
                app.send("]")
                app.pump(0.4)
                done = _find_in_row(app, 3, "已完成")
                assert done is not None and done > before, \
                    f"] 后已完成页签应在正在下载右侧: {done} vs {before}"
                assert app.cell_fg(3, done + 1) in ("black", "000000") \
                    and _cell_bg(app, 3, done + 1) in ("cyan", "00cdcd"), \
                    "] 后活动页签应切换为已完成（黑字青底）"
                # [ 切回（v1.30 页签键 ]/[/T 承载，规程 QA-UC-03 ]/[ 切换口径）
                app.send("[")
                app.pump(0.4)
                back = _find_in_row(app, 3, "正在下载")
                assert back is not None and back < done, \
                    f"[ 后应切回正在下载页签: {back} vs {done}"
                assert app.cell_fg(3, back + 1) in ("black", "000000") \
                    and _cell_bg(app, 3, back + 1) in ("cyan", "00cdcd"), \
                    "[ 后活动页签应切回正在下载（黑字青底）"
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-UC-04 │ 分隔符贯通全高、图区紧贴无空白（v1.32）、行 2 横线止于 │。"""
        @self.case("QA-UC-04")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 5))
                assert app.wait_for("下载中", 8)
                sep = _sep_col(app)
                assert sep is not None, "行 1–3 同列 │ 分隔符应存在"
                # v1.32/FR-01-94⑤：图区紧贴分隔符与右边框——分隔符右侧与
                # 图区右缘 0 列空白（与上下 padding 一致；旧「左右各 1 列
                # 空白」口径随 m3 改版退役）
                for y in (1, 3):
                    assert app.screen.buffer[y][sep - 1].data == " ", \
                        f"分隔符左侧应 1 列空白（行 {y}，字段区 padding）"
                for y in (1, 2, 3):
                    d = app.screen.buffer[y][sep + 1].data
                    assert d != "│" and (d == " " or d in BLOCKS), \
                        f"分隔符右侧应紧贴图区 0 列空白（行 {y}: {d!r}）"
                # 图区右缘紧贴右边框（0 列空白；非边框线字符）
                d = app.screen.buffer[1][app.screen.columns - 2].data
                assert d == " " or d in BLOCKS, \
                    f"图区右缘应紧贴右边框 0 列空白（实测 {d!r}）"
                # 行 2 横线贯通字段区、止于 │、图区无横线（buffer 列空间）
                for x in range(1, sep):
                    assert app.screen.buffer[2][x].data == "─", \
                        f"行 2 应为横线贯通字段区（列 {x}: {app.screen.buffer[2][x].data!r}）"
                assert app.screen.buffer[2][sep].data == "│", "横线应止于 │ 分隔符"
                for x in range(sep + 1, app.screen.columns - 1):
                    d = app.screen.buffer[2][x].data
                    assert d != "─", \
                        f"图区行 2 不应有横线（列 {x}）"
                    assert d == " " or d in BLOCKS, \
                        f"图区行 2 应仅块字符/空白（列 {x}: {d!r}）"
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-UC-05 单色面积流量图形态：3 行高、20% 宽、无断列、单色、无盲文、顶线部分块。"""
        @self.case("QA-UC-05")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 流量图按秒采样、窗口约 24 列——「每列均有块字符（无断列）」
                # 需窗口完全填充（≥24 个采样），观察窗下限 = 采样窗（engineering.md
                # 门控慢速场景先算观察窗下限）：大文件 + 慢速保证填充期任务仍活跃
                # （100MB @ ~1.2MB/s 聚合 ≈ 83s ≫ 24s 填充窗）。
                # 不走 _slow（其 swapsize 会把文件物化改写为 ~9MB，8s 即完成，
                # 观察窗内速度归零、图区排空）——原文件 100MB @ ~1.2MB/s ≈ 83s
                add_task_via_dialog(
                    app, env, env.fixture.url("big-100m.bin?speed=300000"))
                assert app.wait_for("下载中", 10)
                deadline = time.time() + 70.0
                filled = False
                while time.time() < deadline:
                    app.pump(0.2)
                    b = _chart_bounds(app)
                    # 「每列均有块字符」= 列空间判定（该列 3 行中任一行有块）；
                    # 面积柱自底向上生长，顶行只有高柱才有字符，逐格全称判定
                    # 会把正常波形误判为未填满
                    if b and all(
                            any(app.screen.buffer[y][x].data in BLOCKS
                                for y in (b[0], b[0] + 1, b[0] + 2))
                            for x in range(b[1], b[2] + 1)):
                        filled = True
                        break
                assert filled, "70s 内图区应被采样填满（每列均有块字符）"
                # 稳定窗：填充后再采 2s 断言形态不变量
                t1 = time.time()
                while time.time() < t1 + 2.0:
                    app.pump(0.15)
                top, left, right = _chart_bounds(app)
                width = right - left + 1
                expect_w = (app.screen.columns - 2) // 5
                assert abs(width - expect_w) <= 2, \
                    f"图宽应≈内容区 20%（{expect_w}±2）: {width}"
                cols_hit: set[int] = set()
                for y in (top, top + 1, top + 2):
                    for x in range(left, right + 1):
                        ch = app.screen.buffer[y][x].data
                        if ch in BLOCKS:
                            cols_hit.add(x)
                        else:
                            assert ch == " ", f"图区应无其他字符（{ch!r}@{x},{y}）"
                assert len(cols_hit) == width, \
                    f"每列均应有面积块字符（无断列）: {len(cols_hit)}/{width}"
                # 单色：全部块字符淡蓝
                for y in (top, top + 1, top + 2):
                    for x in range(left, right + 1):
                        if app.screen.buffer[y][x].data in BLOCKS:
                            fg = app.cell_fg(y, x)
                            assert fg == LIGHT_BLUE, \
                                f"图区应单色淡蓝: {fg}@{x},{y}"
                # 无盲文点阵字符
                chart_text = "".join(app.screen.buffer[y][x].data
                                     for y in (top, top + 1, top + 2)
                                     for x in range(left, right + 1))
                assert not any(ch in BRAILLE for ch in chart_text), \
                    "流量图不应使用盲文点阵字符"
                # 顶线含部分块（亚字符精度）
                top_seg = "".join(app.screen.buffer[top][x].data
                                  for x in range(left, right + 1))
                assert any(ch in "▁▂▃▄▅▆▇" for ch in top_seg), \
                    f"顶线应含部分块字符（亚格精度）: {top_seg!r}"
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-UC-06 头部字段真实语义：并发=连接总数、会话累计、全局和、峰值单调。"""
        @self.case("QA-UC-06")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 2 任务 × 并发 2 → 数据面连接总数 4
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 7), conns="2")
                assert app.wait_for("下载中", 8)
                add_task_via_dialog(
                    app, env, self._slow(env, "eight-m.bin", 8), conns="2")
                assert app.wait_for("下载中", 12)
                peak_seen: list[int] = []
                sess_seen: list[int] = []
                global_seen = 0.0
                deadline = time.time() + 20.0
                while time.time() < deadline:
                    app.pump(0.3)
                    text = app.text()
                    m = re.search(r"并发 (\d+)", text)
                    if m:
                        # 稳定读数窗内并发 = 4（两任务各 2 连接）
                        if "下载槽位 2/5" in text:
                            peak_seen.append(int(m.group(1)))
                    ms = re.search(r"会话已下载 ([\d.]+) ([KMG]?)B", text)
                    if ms:
                        mult = {"": 1.0, "K": 1e3, "M": 1e6, "G": 1e9}[ms.group(2)]
                        sess_seen.append(int(float(ms.group(1)) * mult))
                    mg = re.search(r"全局 ↓ ([1-9][\d.]* [KMG]?B/s)", text)
                    if mg:
                        global_seen = parse_speed(mg.group(1))
                    if len(peak_seen) >= 3 and len(sess_seen) >= 3 and global_seen > 0:
                        break
                    time.sleep(0.4)
                assert peak_seen and all(v == 4 for v in peak_seen), \
                    f"并发应为下载中任务连接总数 4: {peak_seen[:6]}"
                assert len(sess_seen) >= 2 and sess_seen[-1] > sess_seen[0], \
                    f"会话已下载应随进度增量累计: {sess_seen[:6]}"
                assert global_seen > 0, "全局 ↓ 应为各任务速度和（非零）"
                mu = re.search(r"↑ (\d+(?:\.\d+)? [KMG]?B/s)", app.text())
                assert mu and mu.group(1) == "0 B/s", f"↑ 应恒 0: {mu.group(1) if mu else '?'}"
                # 峰值 = 历史最大（与采样一致、单调不减）
                mp = re.search(r"峰值 ↓ (\d+(?:\.\d+)? [KMG]?B/s)", app.text())
                assert mp, "峰值 ↓ 应显示"
                assert parse_speed(mp.group(1)) >= global_seen * 0.5, \
                    "峰值应 ≥ 近期速度采样（历史最大口径）"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-UC-07 主体带定稿布局：左列 58%、详情定高 12 行（v1.20）、明细占余高。"""
        @self.case("QA-UC-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "three-m.bin", 9), ck_value="0" * 64)
                assert app.wait_for("下载中", 8)
                rows = app.screen.display
                d = _title_row(app, "任务详情")
                c = _title_row(app, "并发连接")
                assert d == 5 and c is not None, f"两面板标题应存在: d={d} c={c}"
                # 详情定高 12 行（10 内容 + 上下边框；v1.20 任务 ID/创建时间拆分）
                assert c - d == 12, f"详情面板应定高 12 行（v1.20）: {c - d}"
                # 并发明细面板占右列余高（至页脚上沿）
                footer_top = next(
                    (i for i in range(c + 1, len(rows))
                     if "↑↓" in rows[i] or "选择" in rows[i]), len(rows))
                assert footer_top - c >= 6, "明细面板应占右列余高"
                # 详情字段（FR-01-80 v1.11/v1.12 口径）
                dtext = detail_text(app)
                for f in ("任务 ID", "创建时间", "类型", "校验", "大小", "保存",
                          "URL", "分块"):
                    assert_in(f, dtext, f"详情字段 {f}")
                assert_not_in("状态", dtext, "详情无状态行（v1.11）")
                assert_not_in("速度", dtext, "详情无速度行（v1.11）")
                assert_not_in("（剩余", dtext, "大小行无剩余后缀（v1.12）")
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-UC-08 并发连接面板三列结构：活跃计数、表头、1 基升序、提示行、待命 '-'。"""
        @self.case("QA-UC-08")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 10), conns="4")
                assert app.wait_for("下载中", 8)
                # 活跃 x（1..4）：连接建立前标题已渲染「活跃 0」，先等首个非零
                deadline = time.time() + 12
                active = 0
                while time.time() < deadline:
                    app.pump(0.2)
                    m = re.search(r"活跃 (\d+)", _right_text(app, _title_row(app, "并发连接")))
                    if m and 1 <= int(m.group(1)) <= 4:
                        active = int(m.group(1))
                        break
                assert active, "12s 内应出现活跃连接（标题右侧活跃 x）"
                c = _title_row(app, "并发连接")
                # 表头三列：序号(4)/下载速度(8)/累计下载(8) 单空格分隔（列空间定位）
                xs = _find_in_row(app, c + 1, "序号", start_col=64)
                assert xs is not None, "表头应有序号列"
                xd = _find_in_row(app, c + 1, "下载速度", start_col=xs + 2)
                xc = _find_in_row(app, c + 1, "累计下载", start_col=xd + 4)
                assert xd == xs + 5 and xc == xd + 9, \
                    f"表头列宽应 4/8/8 单空格分隔: 序号@{xs} 速度@{xd} 累计@{xc}"
                # 数据行 1 基升序
                data = _conns_data_rows(app)
                ids = [r[0] for r in data]
                assert ids and ids == sorted(ids) and ids[0] == 1, \
                    f"数据行应按序号 1 基升序: {ids}"
                assert len(ids) <= 4, f"行数不应超并发数: {ids}"
                # v1.30/FR-01-96 修订：面板无底部提示行（全面板底部区域无
                # 提示文字——原「提示行随协议自适应」口径退役，交互说明收敛
                # 帮助窗口【并发连接】组）
                hint_found = any(
                    ("Ctrl+↑↓ 选择" in _right_text(app, y)
                     or "Ctrl+B" in _right_text(app, y))
                    for y in range(c + 1, app.screen.lines))
                assert not hint_found, "面板底部不应再有提示行（v1.30/P3 退役）"
                # 12 块文件 4 连接不至于全 idle，速度列至少一行非 '-'
                data = _conns_data_rows(app)
                assert any(v and v != "-" for _, v, _ in data), \
                    f"应有活跃连接速度读数: {data}"
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-UC-09 连接速度与累计量口径（FR-01-99/102）：平滑、单调、续传保留、R 清零。

        单会话驱动（跨会话重启会把任务按 sidecar 周期落盘口径恢复为重新下载，
        连接级累计随进程清零，「R 清零」判据必须在同会话内取旧值对照）：
        ①采样 5s 逐连接速度变化次数 ≤ 采样秒数（FR-01-102 每秒至多变 1 次）；
        ②累计下载单调递增（自本次开始下载起算）；③暂停→继续累计延续不清零；
        ④disconnect 瞬态失败 → R 重试 → 新一次下载连接累计从零重计。
        """
        @self.case("QA-UC-09")
        def go(env: Env):
            # disconnect=2500000：每请求 ~2.5MB 处被切 → 有进展瞬态失败。
            # 块必须大于切点（block_size_http=4MB，IC-11 同口径）——默认 1MB 块
            # 单请求恰一块、1MB < 2.5MB 切点，请求自然结束、disconnect 永不触发。
            env.write_config("block_size_http = 4194304\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env,
                    self._slow(env, "twelve-m.bin", 11, rate=200000) +
                    "&disconnect=2500000",
                    conns="3")
                assert app.wait_for("下载中", 8)
                assert app.wait_for("并发连接", 6)

                def cum_map() -> dict[int, float]:
                    out: dict[int, float] = {}
                    for i, v, c in _conns_data_rows(app):
                        if c and c != "-":
                            m = re.match(r"([\d.]+) ([KMG]?)B", c)
                            mult = {"": 1.0, "K": 1e3, "M": 1e6, "G": 1e9}[m.group(2)]
                            out[i] = float(m.group(1)) * mult
                    return out

                # ① 采样 5s：逐连接速度数值变化次数 ≤ 采样秒数（每秒至多变 1 次）
                samples: list[dict[int, str]] = []
                t1 = time.time()
                while time.time() < t1 + 5.0:
                    app.pump(0.1)
                    rows = {i: v for i, v, _ in _conns_data_rows(app)}
                    if rows:
                        samples.append(rows)
                    time.sleep(0.25)
                assert len(samples) >= 8, f"采样应足够密: {len(samples)}"
                for cid in samples[0]:
                    vals = [s[cid] for s in samples if cid in s]
                    changes = sum(1 for a, b in zip(vals, vals[1:]) if a != b)
                    assert changes <= 6.5, \
                        f"连接{cid}速度 5s 内变化 {changes} 次超 1s 节拍口径"
                # ② 累计下载单调递增（自本次开始下载起算）
                prev: dict[int, float] = {}
                deadline = time.time() + 30.0
                while time.time() < deadline:
                    app.pump(0.2)
                    cm = cum_map()
                    if cm:
                        for i, v in cm.items():
                            if i in prev:
                                assert v >= prev[i] - 1.0, \
                                    f"连接{i}累计应单调不减: {prev[i]:.0f} → {v:.0f}"
                        prev.update(cm)
                        if max(prev.values(), default=0) > 400_000:
                            break
                    if "s 后重试" in app.text():
                        break  # 瞬态失败提前到来（低速率下更早切请求）
                    time.sleep(0.3)
                assert prev, "应读到连接累计"
                peak_cum = max(prev.values())
                # ③ 暂停→继续（可续传）：连接累计在原值基础上延续累计（不清零，
                # 04-ui-conns-09 恢复臂；已探测任务引擎重启跳过 Probe 事件，
                # 实测 v138 QA 会话实证累计延续）。锚定：恢复后累计回到恢复前
                # 峰值水位并继续增长。
                app.send("space")
                assert app.wait_for("已暂停", 6), "暂停应生效"
                app.send("space")
                assert app.wait_gone("[已暂停]", 8), "空格恢复应离开已暂停（重新排队，v1.21）"
                assert app.wait_for("下载中", 15), "恢复后应开始传输"
                resumed_peak = 0.0
                deadline = time.time() + 12.0
                while time.time() < deadline:
                    app.pump(0.2)
                    cm = cum_map()
                    if cm:
                        resumed_peak = max(resumed_peak, max(cm.values()))
                        if resumed_peak >= peak_cum * 0.8:
                            break
                    time.sleep(0.3)
                assert resumed_peak >= peak_cum * 0.8, \
                    f"恢复后连接累计应延续（不清零臂）: 恢复前峰值 {peak_cum:.0f} vs " \
                    f"恢复后峰值 {resumed_peak:.0f}"
                # ④ 瞬态失败（disconnect 2.5MB 切断）→ R：重试 = 新的一次下载，
                # 连接累计从零重计（04-ui-conns-09 R 臂；QA v138 会话缺陷修复：
                # 已探测任务引擎重启跳过 Probe 事件、Queued 臂清零不触发，
                # requeue_failed 现显式清账本）
                assert app.wait_for("s 后重试", 120), \
                    "disconnect 应触发瞬态失败并进入自动重试倒计时"
                app.send("r")
                assert app.wait_gone("s 后重试", 10), "R 应转出重试倒计时（重新排队）"
                assert app.wait_for("下载中", 15), "R 后应重新开始下载"
                first_small = None
                deadline = time.time() + 6.0
                while time.time() < deadline and first_small is None:
                    app.pump(0.1)
                    cm = cum_map()
                    if cm:
                        first_small = max(cm.values())
                    time.sleep(0.1)
                assert first_small is not None and first_small < 300_000, \
                    f"R 重试后连接累计应从零重计: 首读 {first_small}"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-UC-10 明细可见性状态门槛：五态空明细 + 活跃 0；空面板滚轮穿透列表。"""
        @self.case("QA-UC-10")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 已完成（快文件）
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("已完成 (1)", 30), "small.bin 应完成（已完成态）"
                # 已失败
                add_task_via_dialog(app, env, env.fixture.url("ghost-404.bin"))
                assert app.wait_for("已失败", 10), "ghost 应失败（已失败态）"
                # 下载中 ×5（占满 5 槽；rate=20000 每请求门控——4 连接聚合
                # ~80KB/s，全程不完成，等待任务永不递补、列表静态）+
                # 等待 ×8（滚动余量；名称互异便于视口位移锚定）
                for i in range(4):
                    add_task_via_dialog(
                        app, env, self._slow(env, "twelve-m.bin", 20 + i, rate=20000))
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 24, rate=20000))
                assert app.wait_for("下载槽位 5/5", 20), "5 任务应占满 5 槽"
                # 等待任务同门控（暂停释放的槽位会递补等待任务——不限速则
                # 回环全速秒完、列表塌缩）：固定 3MB + 20KB/s，递补后也长活
                for n in ("eight-m.bin", "five-m.bin", "three-m.bin", "two-m.bin",
                          "one-m.bin", "half-m.bin", "ten-m.bin", "streamy.bin"):
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(
                            f"{n}?swapsize=3000000&speed=20000"))
                assert app.wait_for("排队第", 12), "第 6 任务应排队（等待中态）"
                # 校验中子判据：80KB/s 门控下无任务能在用例窗口内完成进入校验
                # 臂（完成瞬间窗口 <100ms 亦不可稳定轮询）——按规程 S 口径登记
                # （其余四态门槛已足证；校验中的空明细同形态由 UC-09 校验流程
                # 间接覆盖）
                # 已暂停：暂停首个下载任务（槽位由等待任务递补，等待态已断言）
                app.send("home")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停", 6), "暂停应生效（已暂停态）"
                # 五态逐个选中：明细空 + 居中「（无并发连接）」+ 活跃 0
                def check_empty(label: str):
                    text = app.text()
                    m = re.search(r"活跃 (\d+)", text)
                    assert m and m.group(1) == "0", f"[{label}] 活跃应为 0: {m and m.group(1)}"
                    assert_in("（无并发连接）", text, f"[{label}] 明细应空且居中提示")
                # 已暂停任务（home 后第 2 行）选中核查
                app.send("home")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                check_empty("已暂停")
                # 已失败任务（home 行）
                app.send("home")
                app.pump(0.3)
                check_empty("已失败")
                # 已完成任务：切已完成页签选中（v1.30 页签键 ]/[）
                app.send("]")
                app.pump(0.4)
                app.send("home")
                app.pump(0.3)
                check_empty("已完成")
                app.send("[")
                app.pump(0.4)
                # 空明细滚轮穿透：指针悬停明细面板滚动 → 列表视口位移
                # 穿透断言的滚动余量：列表视口受「选中可见」不变式钳制
                # （tick ⑧⑨：scroll ≤ selected）——选中末位等待任务（End 自动
                # 跟随滚到 max，其明细为空 → 滚轮穿透）后上滚。此时全部下载
                # 已完成/暂停/等待，列表静态 → 文本快照对比即滚动位移判据
                # （含速度行等的动态列表不可用相等对比）。
                # 穿透断言的滚动余量：列表视口受「选中可见」不变式钳制
                # （tick ⑧⑨：scroll ≤ selected）——选中末位等待任务（End 自动
                # 跟随滚到 max，其明细为空 → 滚轮穿透）后上滚。上滚前确认选中
                # 任务确实处于等待态（详情排队行），静态等待行文本随视口位移
                # 变化（排队位次数字随可见集合不同而不同）。
                c = _title_row(app, "并发连接")
                # 选中中部等待任务（End 后 Up×4）：「选中可见」不变式把视口钳在
                # [selected+1-vis, selected]，末位选中时视口钉死在 max、滚轮无
                # 位移；中部选中留出上滚余量（其明细为空 → 滚轮穿透滚列表）
                app.send("end")
                app.pump(0.5)
                app.send(*(["up"] * 4))
                app.pump(0.6)
                text_at_end = app.text()
                assert "排队第" in text_at_end, \
                    f"等待任务应可见: {text_at_end[-400:]!r}"
                before = self._list_snapshot(app)
                app.send(f"mwu:30:{c + 2}")
                app.pump(0.8)
                after = self._list_snapshot(app)
                assert before != after, \
                    f"空明细滚轮应穿透滚动任务列表（列表视口位移）\n" \
                    f"--- before ---\n" + "\n".join(r[:50] for r in before[:10])
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-UC-11 明细选择（v1.30/FR-01-97 改键）：↑↓（并发聚焦时）移动/高亮/
        跟随/钳制/切任务复位/状态门槛 toast/焦点移开不高亮。"""
        @self.case("QA-UC-11")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 12), conns="4")
                assert app.wait_for("下载中", 8)
                assert app.wait_for("并发连接", 6)
                c = _title_row(app, "并发连接")
                inner_x = _find_in_row(app, c + 1, "序号", start_col=64)
                assert inner_x is not None, "明细表头应可见"

                def sel_y():
                    return next(
                        (y for y in range(c + 2, min(c + 12, len(app.screen.display)))
                         if _cell_bg(app, y, inner_x) == SEL_BG), None)
                # v1.30 改键：明细选择 = ↑↓（仅并发面板聚焦时生效）——Tab×2 聚焦
                # （列表 → 详情 → 并发；FR-01-108 面板焦点模型）
                app.send("tab", "tab")
                app.pump(0.4)
                assert sel_y() is not None, "并发聚焦应显示内部选中行（仅聚焦时高亮）"
                # ↓ ×3：选中行下移（深青背景 + 首列白字）
                app.send("down", "down", "down")
                app.pump(0.5)
                sel_row = sel_y()
                assert sel_row is not None, "↓ 后应有深青背景选中行"
                assert app.cell_fg(sel_row, inner_x) in ("white", "ffffff"), \
                    f"选中行首列应白字: {app.cell_fg(sel_row, inner_x)}"
                # ↑：上移 1 格
                app.send("up")
                app.pump(0.4)
                sel2 = sel_y()
                assert sel2 == sel_row - 1, f"↑ 应上移 1 格: {sel_row} → {sel2}"
                # 边界钳制：连续 ↑×8 不越界（仍停在首行，不崩）
                app.send(*(["up"] * 8))
                app.pump(0.5)
                sel3 = sel_y()
                assert sel3 == c + 2, f"边界钳制应停在首行: {sel3} vs {c + 2}"
                # 焦点移开并发面板：面板不显示内部选中态（仅聚焦时高亮，v1.30）
                app.send("tab")
                app.pump(0.4)
                assert sel_y() is None, \
                    "焦点移开后明细不应显示选中高亮（仅聚焦时高亮，v1.30/FR-01-108）"
                app.send("tab", "tab")  # 回并发聚焦
                app.pump(0.4)
                # 切换任务后切回：选择复位首行（切任务需列表焦点：Tab 环绕回列表）
                app.send("tab")   # 并发 → 列表
                app.pump(0.3)
                app.send("down")
                app.pump(0.4)
                app.send("up")
                app.pump(0.4)
                app.send("tab", "tab")  # 列表 → 详情 → 并发
                app.pump(0.4)
                sel4 = sel_y()
                assert sel4 == c + 2, f"切任务后明细选择应复位首行: {sel4}"
                # 已暂停任务 ↓：状态门槛 toast（暂停走列表焦点空格）
                app.send("tab")   # 并发 → 列表
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停", 6)
                app.send("tab", "tab")  # → 并发聚焦
                app.pump(0.4)
                app.send("down")
                assert app.wait_for("当前状态无并发明细（仅下载中/做种中可选）", 5), \
                    "暂停态 ↓ 应给状态门槛 toast"
            finally:
                app.graceful_quit()

    def _case_12(self):
        """QA-UC-12 B 断开守卫链（v1.30 改键，不限焦点面板）：HTTP 拒绝 toast、
        状态门槛先行、浮层内不响应、Ctrl+C 全局直通。"""
        @self.case("QA-UC-12")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 13), conns="3")
                assert app.wait_for("下载中", 8)
                assert app.wait_for("并发连接", 6)
                n_rows = len(_conns_data_rows(app))
                # 下载中 HTTP 任务按 B（v1.30：断开改 B，不限焦点面板）：
                # 协议拒绝 toast（文案随键改 …（B））+ 行数不变
                app.send("b")
                assert app.wait_for("仅 BT 任务支持断开并发连接（B）", 5), \
                    "HTTP 下载中 B 应拒绝（v1.30 文案）"
                assert len(_conns_data_rows(app)) == n_rows, "拒绝断开行数应不变"
                # 已暂停任务按 B：状态门槛 toast 先于协议 toast
                app.send("space")
                assert app.wait_for("已暂停", 6)
                app.send("b")
                assert app.wait_for("当前状态无可断开的并发连接（仅下载中/做种中）", 5), \
                    "暂停态 B 应给状态门槛 toast（先于协议判断）"
                # 对话框打开：B 与 ↑↓ 不响应、Ctrl+C 全局直通退出（v1.31/D38）
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                app.send("b")
                app.pump(0.4)
                assert_in("添加任务：HTTP(S)", app.text(), "对话框内 B 不应生效")
                app.send("down")
                app.pump(0.4)
                assert_in("添加任务：HTTP(S)", app.text(), "对话框内 ↓ 应为字段导航（不生效断开）")
                app.send("ctrl_c")
                app.pump(0.6)
                assert app.exit_code is not None, "Ctrl+C 应退出应用"
            finally:
                app.close()

    def _case_13(self):
        """QA-UC-13 滚轮命中明细与穿透：悬停滚明细（列表不动）、空明细穿透滚列表。

        download_slots=2 造滚动余量：wheel16（16 连接，明细 16 行 > 视口）+
        1 个慢速下载占双槽 + 8 个排队任务（列表 10 项 > 视口 8 项）。
        断言锚「列表视口未动/已动」= 首个可见任务行名（engineering.md：
        列表内容（速度行）逐秒刷新，相等比较不可用，必须锚视口位）。
        """
        @self.case("QA-UC-13")
        def go(env: Env):
            env.write_config("http_concurrency = 16\ndownload_slots = 1\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 100MB 稀疏文件 → 100 块 ≥16 连接；明细行数超面板高
                # （不 swapsize：_slow 会把文件物化改小，16 连接秒完）
                add_task_via_dialog(
                    app, env, env.fixture.url("big-100m.bin?speed=150000"))
                assert app.wait_for("下载中", 10)
                # 9 个等待任务（slots=1 全部排队；同门控口径：暂停释放的槽位
                # 会递补等待任务，不限速则回环全速秒完、列表塌缩）
                for n in ("eight-m.bin", "five-m.bin", "three-m.bin", "two-m.bin",
                          "one-m.bin", "half-m.bin", "ten-m.bin", "small.bin",
                          "streamy.bin"):
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(f"{n}?swapsize=3000000&speed=20000"))
                assert app.wait_for("排队第", 15), "排队任务应可见（滚动余量）"
                # 回选 wheel16（添加期自动选中末位等待任务，其明细为空）
                app.send("home")
                app.pump(0.5)
                deadline = time.time() + 12
                while time.time() < deadline:
                    app.pump(0.2)
                    if len(_conns_data_rows(app)) >= 9:
                        break
                    time.sleep(0.2)
                c = _title_row(app, "并发连接")
                top0 = self._list_top_name(app)
                # 悬停明细面板滚轮：明细视口滚动、列表视口不动
                app.send(f"mwd:100:{c + 4}")
                app.pump(0.5)
                data = _conns_data_rows(app)
                assert data and data[0][0] > 1, \
                    f"悬停明细滚轮应滚动明细视口（首行序号 >1）: {data[:2]}"
                assert self._list_top_name(app) == top0, \
                    "悬停明细滚轮时任务列表视口不应滚动"
                # 暂停 wheel16（home 归位选中列表首项后再按空格——添加期自动
                # 选中末位排队任务，直接空格会暂停错对象）
                app.send("home")
                app.pump(0.5)
                app.send("space")
                assert app.wait_for("已暂停", 6)
                # 穿透断言（同 UC-10 口径）：End 后 Up×4 选中中部等待任务
                # （明细空 → 穿透；中部选中留出上滚余量，视口位移可观察）；
                # 等待态列表静态 → 文本快照对比即滚动位移判据
                app.send("end")
                app.pump(0.5)
                app.send(*(["up"] * 4))
                app.pump(0.6)
                before = self._list_snapshot(app)
                app.send(f"mwu:100:{c + 4}")
                app.pump(0.8)
                after = self._list_snapshot(app)
                assert before != after, \
                    "空明细滚轮应穿透滚动任务列表（列表视口位移）"
            finally:
                app.graceful_quit()

    @staticmethod
    def _left_text(app: "EzrApp", y: int, col: int = 64) -> str:
        """左栏箱体文本（buffer 列空间 0..col，右栏 CJK 不串入）。"""
        parts: list[str] = []
        for x in range(0, col):
            d = app.screen.buffer[y][x].data
            if d:
                parts.append(d)
        return "".join(parts)

    @classmethod
    def _list_snapshot(cls, app: "EzrApp") -> list[str]:
        """列表左栏文本快照（buffer 列空间 0..64；静态列表的滚动位移判定用）。"""
        c = _title_row(app, "并发连接")
        bottom = c if c is not None else app.screen.lines
        return [cls._left_text(app, y) for y in range(5, bottom)]

    @classmethod
    def _list_top_name(cls, app: "EzrApp") -> str | None:
        """列表首个可见任务行的文件名（buffer 列空间左 64 列）。"""
        c = _title_row(app, "并发连接")
        bottom = c if c is not None else app.screen.lines
        for y in range(5, bottom):
            r = cls._left_text(app, y)
            if "下载任务" in r:
                continue
            m = re.search(r"([\w.-]+\.(?:bin|pdf|gz|iso|zip))", r)
            if m:
                return m.group(1)
        return None

    def _case_14(self):
        """QA-UC-14 G 键紧凑布局：右栏收起列表满宽、流量图常显、页脚「面板」。"""
        @self.case("QA-UC-14")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 14, rate=300000))
                assert app.wait_for("下载中", 8)
                # 等首个流量图采样块（speed_hist 预填 90 零样本，首采样秒级）
                deadline = time.time() + 10.0
                while time.time() < deadline:
                    app.pump(0.2)
                    b = _chart_bounds(app)
                    if b and any(app.screen.buffer[y][x].data in BLOCKS
                                 for y in (b[0], b[0] + 1, b[0] + 2)
                                 for x in range(b[1], b[2] + 1)):
                        break
                else:
                    raise AssertionError("10s 内头部流量图应出现采样块")
                # 第一次 G：详情+明细收起、列表满宽、头部流量图仍在、页脚 G=面板
                app.send("g")
                app.pump(0.5)
                text = app.text()
                assert_not_in("任务详情", text, "详情面板应收起")
                assert_not_in("并发连接", text, "明细面板应收起")
                rows = app.screen.display
                list_row = rows[6]
                assert list_row.rstrip().endswith("│"), \
                    f"列表应占满整行（右缘闭合）: {list_row[-8:]!r}"
                # 头部流量图常显：分隔符与图区几何仍在（不受 G 影响）
                b = _chart_bounds(app)
                assert b is not None, "收起右栏后头部流量图应仍在（几何不撤销）"
                assert any(app.screen.buffer[y][x].data in BLOCKS
                           for y in (b[0], b[0] + 1, b[0] + 2)
                           for x in range(b[1], b[2] + 1)), \
                    "头部流量图应仍在绘制（活跃下载采样点）"
                # v1.30/P3 页脚收敛 + v1.31 双行 chips：页脚无 G 提示项
                #（「G 面板」判据随页脚收敛退役），仍为五枚 chips + toast 行
                footer = "".join(rows[-4:])
                assert not re.search(r"G\s*面板", footer), \
                    f"页脚不应再有 G 提示（v1.30/P3 退役）: {footer!r}"
                assert "A 添加" in footer and "H 帮助" in footer, \
                    f"页脚应仍为 chips 图例（v1.31）: {footer!r}"
                # 第二次 G：两面板恢复
                app.send("g")
                app.pump(0.5)
                text = app.text()
                assert_in("任务详情", text, "详情面板应恢复")
                assert_in("并发连接", text, "明细面板应恢复")
            finally:
                app.graceful_quit()

    def _case_15(self):
        """QA-UC-15 窄终端 80×34 降级：头部无图、页签贯通、列表满宽、明细不渲染。"""
        @self.case("QA-UC-15")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 15))
                assert app.wait_for("下载中", 8)
                # PTY 与 pyte buffer 同步缩放（harness EzrApp.resize）
                app.resize(80, 34)
                assert app.wait_gone("任务详情", 8), "<100 列右栏两面板不渲染"
                rows = app.screen.display
                # 头部无图：无贯通行 1–3 的 │ 分隔符（页签行内联 │ 合法）、无面积块
                assert _sep_col(app) is None, "窄屏头部不应有贯通分隔符"
                for y in (1, 2):
                    assert not any(ch in BLOCKS for ch in rows[y]), \
                        f"窄屏头部无流量图（行 {y}）"
                # 页签行贯通全宽（无图区让位）：槽位段仍内联
                assert re.search(r"下载槽位 \d+/\d+", rows[3]), "页签行应含槽位段"
                # 列表满宽：行 6 右缘闭合于屏幕右缘
                assert rows[6].rstrip().endswith("┤") or rows[6].rstrip().endswith("│"), \
                    f"列表应满宽: {rows[6][-6:]!r}"
                assert_not_in("并发连接", app.text(), "明细面板不渲染")
                # 无溢出错乱：各行右缘均为边框字符（含顶部 ╮ 角）
                for y in range(5, 30):
                    r = rows[y].rstrip()
                    assert r.endswith(("│", "┤", "╯", "┘", "╮")), \
                        f"行 {y} 右缘应闭合: {r[-4:]!r}"
                # 页脚精简（<100 列 9 项）：G 面板项被截断
                footer = "".join(rows[-4:])
                assert "面板" not in footer, "窄屏页脚应精简（G 面板项被截断）"
                # 切回 120 恢复
                app.resize(120, 34)
                assert app.wait_for("任务详情", 8), "恢复 120 列右栏应回归"
            finally:
                app.graceful_quit()

    def _case_16(self):
        """QA-UC-16 详情类型行不展示并发数（FR-01-100）：协议 · 续传说明，协议黄粗。"""
        @self.case("QA-UC-16")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "twelve-m.bin", 16), conns="4")
                assert app.wait_for("下载中", 8)
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 17))
                assert app.wait_for("下载中", 10)
                # 暂停第二个任务（有活跃连接的第一个任务保持下载中）
                app.send("down")
                app.pump(0.3)
                app.send("space")
                assert app.wait_for("已暂停", 6)
                app.send("up")
                app.pump(0.4)
                # 下载中任务（有活跃连接）类型行
                d = self._detail_box(app)
                m = re.search(r"类型\s*(HTTP|HTTPS) · (支持断点续传|不支持断点续传)", d)
                assert m, f"[下载中] 类型行应为 协议 · 续传说明: {d[:200]!r}"
                line = next((ln for ln in d.splitlines() if "类型" in ln), "")
                assert "并发" not in line, \
                    f"[下载中] 类型行不应含并发字样: {line!r}"
                pos = _label_pos(app, "类型")
                assert pos is not None
                y, x = pos
                px = _find_in_row(app, y, "HTTP", start_col=x)
                assert px is not None
                assert app.cell_fg(y, px + 1) == YELLOW, \
                    f"[下载中] 协议名应黄色: {app.cell_fg(y, px + 1)}"
                assert _cell_bold(app, y, px + 1), "[下载中] 协议名应加粗"
                # 已暂停任务类型行（同样无并发数）
                app.send("down")
                app.pump(0.4)
                d = self._detail_box(app)
                m = re.search(r"类型\s*(HTTP|HTTPS) · (支持断点续传|不支持断点续传)", d)
                assert m, f"[已暂停] 类型行: {d[:200]!r}"
                line = next((ln for ln in d.splitlines() if "类型" in ln), "")
                assert "并发" not in line, \
                    f"[已暂停] 类型行不应含并发字样: {line!r}"
            finally:
                app.graceful_quit()

    def _case_17(self):
        """QA-UC-17 详情字段名链接样式（FR-01-101）：校验/URL 淡蓝下划线、其余 DIM。"""
        @self.case("QA-UC-17")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, self._slow(env, "three-m.bin", 18),
                    ck_value="e3b0c44298fc1c149afbf4c8996fb924"
                             "27ae41e4649b934ca495991b7852b855")
                assert app.wait_for("下载中", 8)
                for label in ("校验", "URL"):
                    pos = _label_pos(app, label)
                    assert pos is not None, f"字段名 {label} 应可见"
                    y, x = pos
                    # 文字部分淡蓝 + 下划线（列空间：CJK 字段名占 2 单元格+续位）
                    import wcwidth as _ww
                    xi = x
                    for ch in label:
                        cell = app.screen.buffer[y][xi]
                        assert cell.data == ch, \
                            f"{label} 列位还原失败: {cell.data!r} ≠ {ch!r}"
                        fg = cell.fg
                        assert fg == LIGHT_BLUE, \
                            f"{label} 字段名应淡蓝: {fg}@{xi}"
                        assert _cell_ul(app, y, xi), \
                            f"{label} 字段名应带下划线@{xi}"
                        w = _ww.wcwidth(ch)
                        xi += w if w and w > 0 else 1
                    # 对齐填充空格无下划线（pad_right 至 9 列显示宽）
                    pad_end = x + 9
                    while xi < pad_end:
                        assert not _cell_ul(app, y, xi), \
                            f"{label} 填充空格不应有下划线@{xi}"
                        xi += 1
                # 其余字段名 DIM 无下划线
                for label in ("任务 ID", "类型", "大小", "保存", "分块"):
                    pos = _label_pos(app, label)
                    if pos is None:
                        continue
                    y, x = pos
                    fg = app.cell_fg(y, x)
                    assert fg == DIM, f"{label} 字段名应为暗灰: {fg}"
                    assert not _cell_ul(app, y, x), f"{label} 字段名不应有下划线"
            finally:
                app.graceful_quit()

    def _case_18(self):
        """QA-UC-18 点击字段名复制与 toast（FR-01-101/D27）：OSC52 载荷 + 守卫链。"""
        @self.case("QA-UC-18")
        def go(env: Env):
            CK = ("e3b0c44298fc1c149afbf4c8996fb924"
                  "27ae41e4649b934ca495991b7852b855")
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, self._slow(env, "three-m.bin", 19),
                                    ck_value=CK)
                assert app.wait_for("下载中", 8)
                url = env.fixture.url(
                    f"three-m.bin?swapsize={9000000 + 19}&speed=300000")
                # ① 点击「URL」字段名：剪贴板=详情展示 URL、toast=已复制 url
                pos = _label_pos(app, "URL")
                assert pos is not None, "URL 字段名应可见"
                _click(app, pos[0], pos[1])
                assert app.wait_for("已复制 url", 5), "URL 点击应 toast 已复制 url"
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == url, \
                    f"OSC52 载荷应为详情展示 URL: {payloads[-1:] if payloads else '[]'}"
                # ② 点击「校验」字段名：剪贴板=完整 64 位校验码（非截断展示值）
                pos = _label_pos(app, "校验")
                assert pos is not None, "校验字段名应可见"
                _click(app, pos[0], pos[1])
                assert app.wait_for("已复制 校验码", 5), "校验点击应 toast 已复制 校验码"
                payloads = app.osc52_payloads()
                assert payloads and payloads[-1] == CK, \
                    f"OSC52 载荷应为完整校验码: {payloads[-1:] if payloads else '[]'}"
                # ③ 无校验任务：原校验行区域无热区（点击无动作）
                add_task_via_dialog(app, env, self._slow(env, "two-m.bin", 20))
                assert app.wait_for("下载中", 10)
                app.send("down")
                app.pump(0.4)
                assert_not_in("已复制", app.text(), "切换任务后 toast 应已消退")
                n_before = len(app.osc52_payloads())
                # 校验字段名不应存在（无校验任务）
                assert _label_pos(app, "校验") is None, "无校验任务不应有校验字段行"
                # 点击原校验行区域（URL 行上方一格，即校验行历史位置）
                pos_url = _label_pos(app, "URL")
                _click(app, pos_url[0] - 1, pos_url[1])
                app.pump(0.5)
                assert len(app.osc52_payloads()) == n_before, "无热区点击不应复制"
                assert_not_in("已复制", app.text(), "无热区点击不应 toast")
                # ④ G 收起右栏后原位置点击：无动作（热区随面板收起清空）
                app.send("up")
                app.pump(0.4)  # 回到有校验码任务
                app.send("g")
                app.pump(0.5)
                assert_not_in("任务详情", app.text(), "右栏应收起")
                n_before = len(app.osc52_payloads())
                _click(app, pos_url[0], pos_url[1])
                app.pump(0.5)
                assert len(app.osc52_payloads()) == n_before, "收起后点击不应复制"
                assert_not_in("已复制", app.text(), "收起后点击不应 toast")
                app.send("g")
                app.pump(0.5)
                # ⑤ 对话框内点击「URL」字段名：被对话框分支消费、剪贴板与 toast 不变
                app.send("A")
                assert app.wait_for("添加任务：HTTP(S)", 3)
                n_before = len(app.osc52_payloads())
                _click(app, pos_url[0], pos_url[1])
                app.pump(0.5)
                assert len(app.osc52_payloads()) == n_before, "对话框内点击不应复制"
                assert_in("添加任务：HTTP(S)", app.text(), "对话框应保持打开")
                assert_not_in("已复制", app.text(), "对话框内点击不应 toast")
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = UiConnsSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
