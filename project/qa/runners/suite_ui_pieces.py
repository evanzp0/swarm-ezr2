#!/usr/bin/env python3
"""suite_ui_pieces.py — QA 套件 · 05-ui-pieces P 键 pieces 分块弹窗（期号 01）。

对应规程：project/qa/05-ui-pieces-qa.md（7 用例；QA-UP-08 已随 v1.30 页脚收敛
退役，序号留空不重排）。QA v148 轮可执行化补齐（此前 05 规程无可执行套件）。

端到端 UI 层验证：伪终端逐单元格画面断言（色块 🟩🟦🟨⬜ 按宽字符列位还原）+
按键/鼠标注入 + fixture 驱动真实下载状态；不使用项目内部 API。

规程口径同步基线：v1.30（四色图例行、↑↓/B 回归键）、v1.31（未探测臂
!probed && total == 0 收窄）、v1.32（通用滚动条 ↑│║│↓、点击外部 = 无任何操作）、
v1.35（弹窗 68×20/内框 66×18/内容八段、网格每行 31 块、一屏 341 块/11 行——
QA v153-arch 交接批次同步；块大小口径沿 1 MB = 1024×1024，24 块计数不变）。

S 登记口径（环境判据，规程「通过准则」允许）：
- UP-04 前沿行垂直居中落位的逐行断言、↓ 箭头按压高亮/余辉的单元格级色彩断言：
  前沿落位依赖下载瞬时速率分配（块完成序随机），端到端窗口内不可稳定采样——
  前沿形态（上方🟩/前沿🟦/下方⬜）以聚合计数断言承载，精确行位由产品单测承载。
- UP-07 翻转零清屏（增量 diff 绘制）断言：需逐帧 diff 脚本挂钩（check_overlay_
  redraw 类），本套件以「开关/拖拽帧后画面内容正确」间接承载，帧级断言留
  demo 校验脚本（verify_ezr.py 同源手法）。
- UP-05 Ctrl+C 退出 + 重启回归臂：与 QA-TD-23（浮层内 Ctrl+C 直退）、
  QA-UC-11/12（重启后 ↑↓/B 守卫链）重复覆盖，本套件不再重复驱动（规程回归
  联动条款）。
- UP-06 ④ 臂 E2E 不可达：门槛第 4 臂（probed && supports && total == 0）
  需「已探测且 0 字节」——产品探测恒携 Range: bytes=0-，0 字节资源上按
  RFC 7233 恒 416 → 探测失败 probed=false → D35 未探测臂先中（套件④臂
  实测断言即此 toast）。第 4 臂由产品单测承载（pieces.rs 四拒绝臂单测，
  v148 轮登记，规程 UP-06 判据同步修订）。
"""

from __future__ import annotations

import os
import re
import tempfile
import time

from harness import (
    Env, EzrApp, Suite, TMP_ROOT, add_task_via_dialog, assert_in,
    assert_not_in, file_bytes, wait_file_size, QA_FILE_SIZES,
)

BLOCKS = ("🟩", "🟦", "🟨", "⬜")
LEGEND_LABELS = ("已下载", "正在下载", "部分下载", "未下载")


class PiecesSuite(Suite):
    def __init__(self):
        super().__init__("05-ui-pieces", port_base=42100)
        self._cur_app: "EzrApp | None" = None

    def _snapshot(self, env: Env, case_id: str) -> None:
        if self._cur_app is None:
            return
        try:
            path = os.path.join(TMP_ROOT, f"{case_id}.screen.txt")
            with open(path, "w", encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _span(app: EzrApp, needle: str = "分块状态"):
        """浮层几何 (标题行, 左边界列, 右边界列)；未打开返回 None。"""
        for i in range(app.screen.lines):
            x = PiecesSuite._find(app, i, needle)
            if x is None:
                continue
            left = right = None
            for c in range(x - 1, -1, -1):
                if app.screen.buffer[i][c].data == "╭":
                    left = c
                    break
            for c in range(x + 1, app.screen.columns):
                if app.screen.buffer[i][c].data == "╮":
                    right = c
                    break
            if left is not None and right is not None:
                return i, left, right
        return None

    @staticmethod
    def _find(app: EzrApp, y: int, needle: str) -> int | None:
        """buffer 列空间多字符查找（宽字符列位还原）。"""
        import wcwidth as _ww
        x = 0
        while x < app.screen.columns:
            xi, ok = x, True
            for ch in needle:
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

    def _open_pieces(self, app: EzrApp) -> tuple[int, int, int]:
        app.send("p")
        g = self._wait_span(app)
        assert g is not None, "P 应打开分块弹窗"
        return g

    @staticmethod
    def _wait_span(app: EzrApp, timeout: float = 3.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.1)
            g = PiecesSuite._span(app)
            if g is not None:
                return g
        return None

    @staticmethod
    def _close_pieces(app: EzrApp, key: str = "esc") -> None:
        app.send(key)
        deadline = time.time() + 3
        while time.time() < deadline:
            app.pump(0.1)
            if PiecesSuite._span(app) is None:
                return
        raise AssertionError(f"{key} 应关闭分块弹窗")

    @staticmethod
    def _grid_counts(app: EzrApp, ty: int, l: int, r: int) -> dict:
        """网格区四色块计数（图例行与底边框之间；宽字符按格计数）。"""
        counts = {b: 0 for b in BLOCKS}
        # 内框：标题行 ty 之下、底边框 ╰ 之上；图例行之下为网格
        grid_rows = []
        bottom = next(i for i in range(ty + 1, app.screen.lines)
                      if app.screen.buffer[i][l].data == "╰")
        for y in range(ty + 1, bottom):
            row = "".join(app.screen.buffer[y][c].data or ""
                          for c in range(l + 1, r))
            if "已下载" in row or "正在下载" in row:  # 图例行
                continue
            if "分块" in row or "块大小" in row or "HTTP ·" in row:
                continue
            if any(b in row for b in BLOCKS):
                grid_rows.append(y)
        for y in grid_rows:
            for c in range(l + 1, r):
                d = app.screen.buffer[y][c].data
                if d in counts:
                    counts[d] += 1
        return counts

    def _legend_visible(self, app: EzrApp, ty: int, l: int, r: int) -> bool:
        """四色图例行判据（v1.30：位于弹窗底部、底边框之上）。"""
        bottom = next((i for i in range(ty + 1, app.screen.lines)
                       if app.screen.buffer[i][l].data == "╰"), None)
        if bottom is None:
            return False
        for y in range(ty + 1, bottom):
            row = "".join(app.screen.buffer[y][c].data or ""
                          for c in range(l + 1, r))
            if all(b in row for b in BLOCKS) and \
                    all(lb in row for lb in LEGEND_LABELS):
                return True
        return False

    def _thumb(self, app: EzrApp, ty: int, l: int, r: int) -> tuple[int, int] | None:
        """弹窗右边界列滚动条滑块 (首, 末) 行；无滑块 None。"""
        first = last = None
        for y in range(ty + 1, app.screen.lines):
            d = app.screen.buffer[y][r].data
            if d == "║":
                if first is None:
                    first = y
                last = y
        return (first, last) if first is not None else None

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list:
        for i in (1, 2, 3, 4, 5, 6, 7):
            getattr(self, f"_case_0{i}")()
        return self.results

    def _case_01(self):
        """QA-UP-01 下载中按 P：68×20 居中、八段/元信息/四色图例（v1.35）、56×16 钳制。"""
        @self.case("QA-UP-01")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 25165824 B = 24 MiB → 24 块（块大小 1 MiB；24000000 仅 23 块）
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        "five-m.bin?swapsize=25165824&speed=400000"),
                    conns="4")
                assert app.wait_for("下载中", 8)
                app.pump(1.0)
                ty, l, r = self._open_pieces(app)
                # v1.35/FR-01-106：弹窗 68×20（64×21→67×23→68×20 呼吸行 +
                # 加宽 3 列 + 高度收缩；内框 66×18，内容八段）
                assert r - l == 67, f"弹窗应 68 列（实测 {r - l + 1}）"
                bottom = next(i for i in range(ty + 1, app.screen.lines)
                              if app.screen.buffer[i][l].data == "╰")
                assert bottom - ty == 19, f"弹窗应 20 行（实测 {bottom - ty + 1}）"
                # 居中：120 列下左右边距各 ≈26（(120-68)/2 = 26）
                assert abs(l - (app.screen.columns - 68) // 2) <= 1, \
                    f"弹窗应水平居中（左界 {l}）"
                text = app.text()
                assert_in("分块状态", text, "标题")
                # v1.30 四色图例行（替代原快捷键提示行）
                assert self._legend_visible(app, ty, l, r), \
                    "底部图例行应显示四色说明（v1.30）"
                assert_not_in("PgUp/PgDn 滚动 · Esc/P 关闭", text,
                              "旧快捷键提示行应已退役（v1.30）")
                # 元信息：HTTP · 块大小 1 MB · x/24 块 · 并发 4
                meta = None
                for y in range(ty + 1, ty + 5):
                    row = "".join(app.screen.buffer[y][c].data or ""
                                  for c in range(l + 1, r))
                    if "块大小" in row:
                        meta = row
                        break
                assert meta is not None and re.search(
                    r"HTTP · 块大小 1 MB · \d+/24 块 · 并发 4", meta), \
                    f"元信息行应含 协议/块大小/x/24 块/并发 4: {meta!r}"
                self._close_pieces(app)
                app.graceful_quit()
                # 56×16 窄终端：弹窗钳制为 56×16、四段完整
                app = EzrApp(env.home, save_dir=env.save_dir, cols=56, rows=16)
                self._cur_app = app
                assert app.wait_for("下载中", 8)
                ty2, l2, r2 = self._open_pieces(app)
                assert r2 - l2 <= 55, f"窄端弹窗应 ≤56 列（实测 {r2 - l2 + 1}）"
                assert self._legend_visible(app, ty2, l2, r2), \
                    "窄端图例行应完整（四段随内框收缩）"
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-UP-02 已暂停（有进度）与已完成任务网格：🟩/🟨/⬜ 分布 + 图例恒显。"""
        @self.case("QA-UP-02")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 已完成：12MB（12 块）快速完成 → 全 🟩
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?swapsize=12000000"))
                assert app.wait_for("已完成", 30), "12MB 应完成"
                # 已暂停（有进度）：24 MiB（24 块）慢速下载至 ~30% 暂停
                # （百分比取同屏 <99% 的最大读数——已完成任务的 100% 不计入）
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        "five-m.bin?swapsize=25165824&speed=400000"),
                    save_dir=os.path.join(env.save_dir, "p"))
                assert app.wait_for("下载中", 8)
                deadline = time.time() + 45
                pct = 0.0
                while time.time() < deadline:
                    app.pump(0.2)
                    vals = [float(x) for x in
                            re.findall(r"(\d+\.\d)%", app.text())]
                    vals = [v for v in vals if v < 99.0]
                    if vals:
                        pct = max(vals)
                        if pct >= 30.0:
                            break
                app.send("space")
                assert app.wait_for("已暂停", 6)
                # 暂停任务弹窗：🟩 若干、无 🟦、其余 🟨/⬜
                ty, l, r = self._open_pieces(app)
                counts = self._grid_counts(app, ty, l, r)
                assert counts["🟩"] >= 1, f"暂停任务应有已下载块: {counts}"
                assert counts["🟦"] == 0, f"暂停任务不应有在传块: {counts}"
                assert counts["🟩"] + counts["🟨"] + counts["⬜"] == 24, \
                    f"网格应覆盖全部 24 块: {counts}"
                assert self._legend_visible(app, ty, l, r), \
                    "图例行恒显（与任务状态无关，v1.30）"
                self._close_pieces(app)
                # 已完成任务（第二页签）：全 🟩
                app.send("]")
                app.pump(0.4)
                app.send("home")
                app.pump(0.3)
                ty2, l2, r2 = self._open_pieces(app)
                counts2 = self._grid_counts(app, ty2, l2, r2)
                assert counts2["🟩"] == 12 and counts2["🟦"] == 0 \
                    and counts2["🟨"] == 0, f"已完成应全 🟩（12 块）: {counts2}"
                assert self._legend_visible(app, ty2, l2, r2), "图例行恒显"
                self._close_pieces(app)
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-UP-03 下载中网格：🟦 在传块（含零头）+ 暂停后 🟦 清零、🟩 计数不变。"""
        @self.case("QA-UP-03")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 24 MiB = 24 块；等 ≥20%：4 并发均分带宽下 4 块同刻完成于
                # ~16.7%——15% 臂实测全在传 0 完成（{'🟩':0,'🟦':4}），20% 保绿 ≥3
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        "five-m.bin?swapsize=25165824&speed=300000"),
                    conns="4")
                assert app.wait_for("下载中", 8)
                deadline = time.time() + 45
                while time.time() < deadline:
                    app.pump(0.2)
                    vals = [float(x) for x in
                            re.findall(r"(\d+\.\d)%", app.text())]
                    vals = [v for v in vals if v < 99.0]
                    if vals and max(vals) >= 20.0:
                        break
                ty, l, r = self._open_pieces(app)
                counts = self._grid_counts(app, ty, l, r)
                green_before = counts["🟩"]
                assert counts["🟦"] >= 1, \
                    f"下载中应有在传块（🟦 优先于 🟨，含落盘零头）: {counts}"
                assert green_before >= 3, \
                    f"应有 ≥3 已完成块（规程：前 3 块 🟩）: {counts}"
                assert self._legend_visible(app, ty, l, r), "图例恒显"
                self._close_pieces(app)
                # 暂停：🟦 清零（零头转 🟨/无零头转 ⬜），🟩 分布不变
                app.send("space")
                assert app.wait_for("已暂停", 6)
                ty2, l2, r2 = self._open_pieces(app)
                counts2 = self._grid_counts(app, ty2, l2, r2)
                assert counts2["🟦"] == 0, f"暂停后不应有 🟦: {counts2}"
                assert counts2["🟩"] == green_before, \
                    f"暂停后 🟩 分布不变（{green_before} → {counts2['🟩']}）: {counts2}"
                self._close_pieces(app)
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-UP-04 520 块弹窗：通用滚动条（v1.32）、键盘滚动、观察窗不自动跟随。"""
        @self.case("QA-UP-04")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 2000 块：2 GiB 稀疏真实文件（非 swapsize——fixture 对 swapsize
                # 物化进内存，2 GiB 会 OOM；truncate 稀疏文件 0 磁盘占用，
                # fixture 按需 seek 供字节；ezr 目标文件 set_len 同为稀疏）
                # 可见 11 行（一屏 341 块，v1.35 68×20）→ 滚程 54 行，
                # ↓×3/PgDn 8 行/End/Home 均有行程
                # （520 块版仅 2 行滚程，PgDn 断言无从驱动——规程同步修订）
                big = os.path.join(env.fixture_root, "piece-big.bin")
                if not os.path.exists(big):
                    with open(big, "wb") as f:
                        f.truncate(2097152000)
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("piece-big.bin?speed=400000"),
                    conns="4")
                assert app.wait_for("下载中", 8)
                deadline = time.time() + 20
                while time.time() < deadline:
                    app.pump(0.2)
                    if re.search(r"[1-9]\d*\.\d%", app.text()):
                        break
                ty, l, r = self._open_pieces(app)
                # 右边界列通用滚动条：↑ / │ 轨道 / ║ 滑块（恒绿加粗）/ ↓
                cells = "".join(app.screen.buffer[y][r].data
                                for y in range(ty + 1, app.screen.lines))
                assert "↑" in cells and "↓" in cells and "║" in cells \
                    and "│" in cells, \
                    f"右缘应出现通用滚动条（↑│║↓，v1.32/FR-01-109）: {cells!r}"
                thumb0 = self._thumb(app, ty, l, r)
                assert thumb0 is not None, "滑块应在位"
                # 观察窗：6s 后滑块位不变（网格不自动跟随下载前沿）
                time.sleep(6.0)
                app.pump(0.5)
                assert self._thumb(app, ty, l, r) == thumb0, \
                    "6s 观察窗内网格不应自动跟随（前沿定位为打开时刻快照）"
                # ↓×3 逐行下滚 → 滑块下移
                app.send(*(["down"] * 3))
                app.pump(0.5)
                thumb1 = self._thumb(app, ty, l, r)
                assert thumb1 is not None and thumb1[0] > thumb0[0], \
                    f"↓×3 应下滚（滑块 {thumb0} → {thumb1}）"
                # PgDn 下滚 8 行
                app.send("pgdn")
                app.pump(0.5)
                thumb2 = self._thumb(app, ty, l, r)
                assert thumb2 is not None and thumb2[0] > thumb1[0], \
                    f"PgDn 应下滚 8 行（{thumb1} → {thumb2}）"
                # End 跳末页钳制（滑块贴底）
                app.send("end")
                app.pump(0.5)
                thumb3 = self._thumb(app, ty, l, r)
                assert thumb3 is not None and thumb3[0] > thumb2[0], \
                    f"End 应跳末页（{thumb2} → {thumb3}）"
                app.send("end")
                app.pump(0.4)
                assert self._thumb(app, ty, l, r) == thumb3, \
                    "End 重复应钳制不越界"
                # Home 回首行（滑块回顶）
                app.send("home")
                app.pump(0.5)
                thumb4 = self._thumb(app, ty, l, r)
                assert thumb4 is not None and thumb4[0] < thumb3[0], \
                    f"Home 应回首行（{thumb3} → {thumb4}）"
                # PgUp 上滚 8 行（自顶钳制：位置不变）
                app.send("pgup")
                app.pump(0.4)
                assert self._thumb(app, ty, l, r) == thumb4, \
                    "首行 PgUp 应钳制不越界"
                self._close_pieces(app)
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-UP-05 弹窗模态：功能键无动作；q/Q/Esc 只关窗不退出（P 重开验存活）。"""
        @self.case("QA-UP-05")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        "five-m.bin?swapsize=24000000&speed=400000"),
                    conns="4")
                assert app.wait_for("下载中", 8)
                self._open_pieces(app)
                # 功能键全部被消费：弹窗不关、无对话框、应用存活
                for key in ("space", "r", "a", "d", "m", "c", "tab", "g",
                            "u", "j"):
                    app.send(key)
                    app.pump(0.25)
                    assert self._span(app) is not None, \
                        f"{key} 不应关闭弹窗（模态消费）"
                    t = app.text()
                    assert_not_in("添加任务：HTTP(S)", t, f"{key} 不应开框")
                    assert_not_in("删除任务", t, f"{key} 不应开删除框")
                assert_in("下载中", app.text(), "任务应仍下载中")
                # q / Q / Esc 均只关弹窗且应用存活（P 重开验）
                for key in ("q", "Q", "esc"):
                    self._close_pieces(app, key)
                    assert_in("EZR Downloader", app.text(), f"{key} 后应用存活")
                    self._open_pieces(app)
                self._close_pieces(app)
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-UP-06 逐臂拒绝 toast（弹窗不打开）：未选中/未探测/不支持续传/0 字节。

        各臂独立 HOME/save（同 case 内臂间会话恢复会互染：②臂 5 个慢速任务
        持久化后 ③臂重启即恢复占满槽位，新任务永远未探测 → 误入未探测臂）。
        """
        @self.case("QA-UP-06")
        def go(env: Env):
            def arm_env(tag: str) -> tuple[str, str]:
                home = tempfile.mkdtemp(prefix=f"qa-up6-{tag}-", dir=TMP_ROOT)
                save = tempfile.mkdtemp(prefix=f"qa-up6-{tag}-dl-", dir=TMP_ROOT)
                return home, save

            # ① 空列表（无任务）
            home, save = arm_env("a1")
            app = EzrApp(home, save_dir=save)
            self._cur_app = app
            try:
                app.pump(0.6)
                app.send("p")
                assert app.wait_for("未选中任务，无法查看分块", 3), \
                    "空列表 P 应 toast 未选中"
                assert self._span(app) is None, "弹窗不应打开"
                app.graceful_quit()
                # ② 未探测（5 槽占满，第 6 任务等待中未预取）
                home, save = arm_env("a2")
                app = EzrApp(home, save_dir=save)
                self._cur_app = app
                for i in range(5):
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(
                            f"five-m.bin?swapsize={8000000 + i}&speed=20000"),
                        save_dir=os.path.join(save, f"s{i}"))
                assert app.wait_for("下载槽位 5/5", 20), "5 任务应占满槽位"
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("five-m.bin?swapsize=9000000&speed=20000"),
                    save_dir=os.path.join(save, "s5"))
                assert app.wait_for("等待中", 8), "第 6 任务应排队（未探测）"
                app.send("p")
                assert app.wait_for("任务尚未探测，暂无分块信息", 3), \
                    "未探测任务 P 应 toast（D35：!probed && total == 0）"
                assert self._span(app) is None, "弹窗不应打开"
                app.graceful_quit()
                # ③ 不支持续传（norange → 单流整段下载）
                home, save = arm_env("a3")
                app = EzrApp(home, save_dir=save)
                self._cur_app = app
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        "five-m.bin?swapsize=3000000&speed=400000&norange=1"),
                    save_dir=save)
                assert app.wait_for("下载中", 8)
                app.send("p")
                assert app.wait_for(
                    "该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息",
                    3), "不支持续传任务 P 应 toast"
                assert self._span(app) is None, "弹窗不应打开"
                app.graceful_quit()
                # ④ 0 字节（swapsize=0）：实测终态为「已失败」——产品探测用
                # Range: bytes=0-，0 字节资源上恒 416（RFC 7233：空文件任意
                # 区间不可满足）→ probed=false → D35 未探测臂先中。门槛第 4 臂
                # （total==0 且已探测）端到端不可达，由产品单测承载（pieces.rs
                # 四拒绝臂单测）——规程 UP-06 ④ 判据已同步修订。
                # （勿用 wait_for("已完成")：会误匹配页签头「已完成 (0)」）
                home, save = arm_env("a4")
                app = EzrApp(home, save_dir=save)
                self._cur_app = app
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?swapsize=0"),
                    save_dir=save)
                assert app.wait_for("已失败", 10), "0 字节任务应到终态"
                app.pump(0.3)
                app.send("p")
                assert app.wait_for("任务尚未探测，暂无分块信息", 3), \
                    "0 字节任务（探测 416 未遂）P 应 toast 未探测臂"
                assert self._span(app) is None, "弹窗不应打开"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-UP-07 弹窗滚轮/拖拽/重开居中/点击外部无操作（v1.32 模态统一口径）。"""
        @self.case("QA-UP-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url(
                        "five-m.bin?swapsize=520000000&speed=400000"),
                    conns="4")
                assert app.wait_for("下载中", 8)
                deadline = time.time() + 20
                while time.time() < deadline:
                    app.pump(0.2)
                    if re.search(r"[1-9]\d*\.\d%", app.text()):
                        break
                ty, l, r = self._open_pieces(app)
                thumb0 = self._thumb(app, ty, l, r)
                # 列表首行快照（滚轮不应穿透任务列表）
                def list_head():
                    return "".join(app.screen.buffer[6][c].data or ""
                                   for c in range(1, 40))
                head0 = list_head()
                # 滚轮向下 ×3 = 网格下滚 6 行（每滚 ±2 行）
                for _ in range(3):
                    app.send(f"mwd:{l + 20}:{ty + 6}")
                    app.pump(0.25)
                app.pump(0.3)
                thumb1 = self._thumb(app, ty, l, r)
                assert thumb0 is not None and thumb1 is not None \
                    and thumb1[0] > thumb0[0], \
                    f"滚轮 ×3 应下滚网格（滑块 {thumb0} → {thumb1}）"
                assert list_head() == head0, "滚轮不应穿透滚动任务列表"
                # 左键按住弹窗内拖动：窗口随动且钳制终端内
                app.send(f"mclick:{l + 3}:{ty + 1}",
                         f"mdrag:{l + 6}:{ty + 2}", f"mdrag:{l + 8}:{ty + 3}")
                app.pump(0.2)
                app.send(f"mup:{l + 8}:{ty + 3}")
                app.pump(0.4)
                g2 = self._span(app)
                assert g2 is not None and g2[0] == ty + 2, \
                    f"拖拽应随动（{ty} → {g2 and g2[0]}）"
                # P 关闭重开：回到居中位（拖拽位置不保留，与帮助窗口不同；
                # v1.35 68 列：center = (120-68)//2 = 26）
                self._close_pieces(app, "p")
                ty3, l3, r3 = self._open_pieces(app)
                center = (app.screen.columns - 68) // 2
                assert abs(l3 - center) <= 1, \
                    f"重开应回居中位（左界 {l3}，期望 ≈{center}；拖拽位置不保留）"
                # 点击弹窗外部 = 无任何操作（v1.32 模态统一口径：不关闭不穿透）
                sel_before = [(y, x) for y in range(app.screen.lines)
                              for x in range(0, 64)
                              if app.screen.buffer[y][x].data == "▌"]
                app.send("mclick:2:2")
                app.pump(0.15)
                app.send("mup:2:2")
                app.pump(0.4)
                assert self._span(app) is not None, "点击外部不应关闭弹窗（v1.32）"
                sel_after = [(y, x) for y in range(app.screen.lines)
                             for x in range(0, 64)
                             if app.screen.buffer[y][x].data == "▌"]
                assert sel_before == sel_after, "点击外部不应穿透任务列表"
                # 关闭通道 P/Esc/q/Q（规程口径；Esc 已在别臂覆盖）
                self._close_pieces(app, "p")
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = PiecesSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
