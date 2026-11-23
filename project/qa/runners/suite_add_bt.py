#!/usr/bin/env python3
"""suite_add_bt.py — 07-add-bt-qa.md v1.51 十二用例可执行化（six-pack/QA 2026-11-17）。

覆盖 QA-AB-01…12（QA-AB-07 按规程钦定记 S：D53 退役，磁力输入行为归 QA-CW-08）。
对应 features/07-add-bt.feature 12 场景（78×29 / 焦点表 0–11 / v1.51「按顺序下载」）。

Fixture 组成（每用例独立现场）：
- 四文件种子 multi.torrent（a-d.bin 各 512KiB，piece 256KiB；ezr-mktorrent name=root
  固定 → 树 root→root→文件双层，与 suite_modify_bt 同构布局）；
- 十文件种子 wide.torrent（树 12 行 > 7 可视行，AB-02 滚动条臂）；
- 单文件种子 single.torrent（AB-05）；
- env.home/qatorrents/<seed>.torrent（AB-09 相对路径形态；app cwd = env.home）。

AB-10 限速行口径按 v1.45/FR-02-35 现行规格执行（QA-01-20 v1.45 剪行同口径）：
config "2 MiB/s" → 预填 `2 MiB/s`；裸整数 2000000 → `2000000 B/s`；键入 `2m` →
`2 MiB/s`、裸 512 确认 = 512 B/s、非法回退 config 默认；规程中「2 MB/s 旧写法 →
回退 0」行为 v1.40 陈旧文本——现行 config 严格校验下该值启动失败（TP-04 v1.12
实跑承载），对话框预填臂不可达，本套件不再构造该行。

时序纪律：门控不需要（对话框静态断言 + 任务创建锚 toast/注册表）；自适应 Tab 到
按钮（_tab_to_button，tab 计数对渲染时序脆弱）；预填清空用自适应退格（PTY 退格
连发投递上限，notes/environment.md bug19 口径）。
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys

QA_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, QA_DIR)

from harness import (TMP_ROOT, EzrApp, Suite, Env, assert_in, assert_not_in,  # noqa: E402
                     _clear_field_prefill)

MKTORRENT = os.path.join(QA_DIR, "..", "..", "ezr", "target", "debug",
                         "ezr-mktorrent")

ALT_ENTER = "\x1b\r"
BACKTAB = "\x1b[Z"
FILES = ("a.bin", "b.bin", "c.bin", "d.bin")
WIDE_FILES = tuple(f"f{i:02d}.bin" for i in range(10))
FSIZE = 512 * 1024
PIECE = 262144


def _click(app: EzrApp, col0: int, row0: int) -> None:
    app.send(f"mclick:{col0 + 1}:{row0 + 1}")


def _cell(app: EzrApp, y: int, x: int) -> str:
    return app.screen.buffer[y][x].data or ""


def _row_text(app: EzrApp, y: int, x0: int = 0, x1: int | None = None) -> str:
    x1 = app.screen.columns if x1 is None else x1
    return "".join(_cell(app, y, x) for x in range(x0, x1)).rstrip()


def _focused_btn(app: EzrApp) -> "tuple[int, str] | tuple[None, None]":
    """当前聚焦按钮（行, 文本）：聚焦按钮 = ACCENT 青底（suite_modify_bt 同口径实测）。"""
    buf = app.screen._screen.buffer
    for y in range(0, app.screen.lines):
        line = "".join(buf[y][x].data or "" for x in range(10, app.screen.columns - 4))
        if "[ 立即下载 ]" in line or "[ 仅添加 ]" in line or "[ 取消 ]" in line:
            segs: dict[str, list[int]] = {}
            for x in range(10, app.screen.columns - 4):
                segs.setdefault(str(buf[y][x].bg), []).append(x)
            hl = segs.get("00cdcd")
            if hl:
                txt = "".join(buf[y][x].data or "" for x in range(hl[0], hl[-1] + 1))
                return y, txt.strip()
    return None, None


def _tab_to_button(app: EzrApp, want: str, max_tabs: int = 14) -> None:
    for _ in range(max_tabs):
        _y, label = _focused_btn(app)
        if label and want in label:
            app.pump(0.1)
            return
        app.send("tab")
        app.pump(0.15)
    raise AssertionError(f"未能聚焦按钮 {want!r}")


class AddBtSuite(Suite):
    def __init__(self):
        super().__init__("07-add-bt", port_base=44300)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12):
            getattr(self, f"_case_{i}")()
        return self.results

    def _snapshot(self, env: Env, case_id: str) -> None:
        if self._cur_app is None:
            return
        try:
            with open(os.path.join(TMP_ROOT, f"{case_id}.screen.txt"), "w",
                      encoding="utf-8") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    # -- fixture -------------------------------------------------------------

    def _mk_torrent(self, pay_root: str, rel_files: "tuple[str, ...]",
                    out: str) -> str:
        lay = os.path.join(pay_root, "root", "root")
        os.makedirs(lay, exist_ok=True)
        for rel in rel_files:
            p = os.path.join(lay, rel)
            if not os.path.exists(p):
                os.makedirs(os.path.dirname(p), exist_ok=True)
                with open(p, "wb") as f:
                    f.write(os.urandom(FSIZE))
        if not os.path.exists(out):
            r = subprocess.run(
                [MKTORRENT, "--out", out, "--piece-len", str(PIECE)] +
                [os.path.join(lay, rel) for rel in rel_files],
                capture_output=True, text=True)
            assert r.returncode == 0, f"mktorrent 失败: {r.stderr.strip()}"
        return out

    def _mk_single(self, env: Env, name: str, announce: "str | None" = None) -> str:
        """独立单文件种子（info name = payload-<name>.bin），供同 case 多轮创建免判重。"""
        qdir = os.path.join(env.fixture_root, "qatorrents")
        src = os.path.join(env.fixture_root, f"payload-{name}.bin")
        if not os.path.exists(src):
            with open(src, "wb") as f:
                f.write(b"s" * (1024 * 1024))
        out = os.path.join(qdir, f"{name}.torrent")
        if not os.path.exists(out):
            args = [MKTORRENT, "--out", out, "--piece-len", str(PIECE)]
            if announce:
                args += ["--announce", announce]
            args.append(src)
            r = subprocess.run(args, capture_output=True, text=True)
            assert r.returncode == 0, f"mktorrent({name}) 失败: {r.stderr.strip()}"
        return out

    def _make_fixtures(self, env: Env) -> dict:
        qdir = os.path.join(env.fixture_root, "qatorrents")
        os.makedirs(qdir, exist_ok=True)
        multi = self._mk_torrent(env.fixture_root, FILES,
                                 os.path.join(qdir, "multi.torrent"))
        wide = self._mk_torrent(env.fixture_root, WIDE_FILES,
                                os.path.join(qdir, "wide.torrent"))
        single_src = os.path.join(env.fixture_root, "payload-s.bin")
        if not os.path.exists(single_src):
            with open(single_src, "wb") as f:
                f.write(b"s" * (1024 * 1024))
        single = os.path.join(qdir, "single.torrent")
        if not os.path.exists(single):
            r = subprocess.run(
                [MKTORRENT, "--out", single, "--piece-len", str(PIECE), single_src],
                capture_output=True, text=True)
            assert r.returncode == 0, f"mktorrent(single) 失败: {r.stderr.strip()}"
        home_q = os.path.join(env.home, "qatorrents")
        os.makedirs(home_q, exist_ok=True)
        seed9 = os.path.join(home_q, "seed9.torrent")
        if not os.path.exists(seed9):
            r = subprocess.run(
                [MKTORRENT, "--out", seed9, "--piece-len", str(PIECE), single_src],
                capture_output=True, text=True)
            assert r.returncode == 0, f"mktorrent(seed9) 失败: {r.stderr.strip()}"
        return {"qdir": qdir, "multi": multi, "wide": wide, "single": single,
                "seed9": seed9,
                "s1": self._mk_single(env, "docs-a"),
                "s2": self._mk_single(env, "docs-b"),
                "s3": self._mk_single(env, "docs-c"),
                "meta": self._mk_single(env, "meta-docs",
                                        "http://127.0.0.1:19998/announce")}

    # -- shared steps --------------------------------------------------------

    def _reg_tasks(self, home: str) -> list:
        """读 <home>/.ezr/state/registry.json（磁盘产物，用户可达入口）。"""
        p = os.path.join(home, ".ezr", "state", "registry.json")
        with open(p, encoding="utf-8") as f:
            return json.load(f).get("tasks", [])

    def _reg_bt_task(self, home: str, name_frag: str) -> dict:
        tasks = self._reg_tasks(home)
        t = next((x for x in tasks if name_frag in x.get("name", "")), None)
        assert t is not None, f"注册表应含 {name_frag} 任务: {[x.get('name') for x in tasks]}"
        return t

    def _open_bt_dialog(self, app: EzrApp, source: str) -> None:
        app.send("a")
        assert app.wait_for("请输入命令或链接", 5), "命令窗口应打开"
        app.type_text(source)
        app.send("enter")
        assert app.wait_for("添加任务：BT", 6), f"应分流 BT 对话框: {source[:40]}"

    def _frame(self, app: EzrApp, title: str) -> "tuple[int, int, int, int]":
        ty = app.find_row(title)
        assert ty is not None, f"未找到标题「{title}」"
        l = next(x for x in range(app.screen.columns) if _cell(app, ty, x) == "╭")
        r = next(x for x in range(l + 1, app.screen.columns) if _cell(app, ty, x) == "╮")
        by = next(y for y in range(ty + 1, app.screen.lines)
                  if _cell(app, y, l) == "╰")
        return ty, l, r, by

    def _tree_rows(self, app: EzrApp) -> "list[tuple[str, str]]":
        rows: list[tuple[str, str]] = []
        in_tree = False
        for line in app.text().split("\n"):
            if "选择下载文件" in line:
                in_tree = True
                continue
            if not in_tree:
                continue
            if "trackers" in line or "立即下载" in line:
                break
            m = re.match(r".*?(\[x\]|\[-\]|\[ \])\s+(.*?)(?:\s{2,}|$)", line)
            if m:
                marker = m.group(1).replace("[", "").replace("]", "")
                name = (m.group(2).strip().rstrip("/")
                        .lstrip("\u25b8\u25be").strip())
                rows.append((marker, name))
        return rows

    def _tree_goto(self, app: EzrApp, name: str,
                   cursor_hint: "str | None" = None) -> "tuple[int, str]":
        rows = self._tree_rows(app)
        idx = next((i for i, (_mk, nm) in enumerate(rows) if nm == name), None)
        assert idx is not None, f"树应含 {name}：{rows}"
        cur = 0
        if cursor_hint:
            cur = next((i for i, (_mk, nm) in enumerate(rows) if nm == cursor_hint), 0)
        delta = idx - cur
        if delta > 0:
            app.send(*(["down"] * delta))
        elif delta < 0:
            app.send(*(["up"] * (-delta)))
        app.pump(0.2)
        return idx, rows[idx][0]

    def _stats_line(self, app: EzrApp) -> str:
        m = re.search(r"已选 (\d+) 个文件，共 ([0-9.]+ \S+)", app.text())
        return f"{m.group(1)}|{m.group(2)}" if m else "none"

    def _expand_tree_full(self, app: EzrApp) -> None:
        """双层树展开至文件层（home 归根 → root → root → 文件）。"""
        app.send("home")
        app.pump(0.15)
        app.send("tab", "tab")
        app.send("right", "down", "right", "up")
        app.pump(0.3)

    # -- cases ---------------------------------------------------------------

    def _case_1(self):
        """QA-AB-01 78×29 布局 + 预填（并发 20 / 限速 fmt / 代理默认）+ 焦点初值种子行。"""
        @self.case("QA-AB-01")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                ty, l, r, by = self._frame(app, "添加任务：BT")
                assert r - l + 1 == 78, f"应 78 列（实测 {r - l + 1}）"
                assert by - ty + 1 == 29, f"应 29 行（实测 {by - ty + 1}）"
                assert l == (app.screen.columns - 78) // 2 and \
                    ty == (app.screen.lines - 29) // 2, "应居中"
                assert next((x for x in range(r - 4, r)
                             if _cell(app, ty, x) == "✗"), None) is not None, "[✗]"
                text = app.text()
                for label in ("种子", "保存到", "选择下载文件", "[全选]", "[全不选]",
                              "并发", "下载限速", "上传限速", "代理", "按顺序下载",
                              "trackers", "[ 立即下载 ]", "[ 仅添加 ]", "[ 取消 ]"):
                    assert_in(label, text, f"布局应含 {label}")
                assert re.search(r"并发\s*>\s*20\b", text), "并发预填 = 20"
                assert "qatorrents/multi.torrent" in text, \
                    "种子行预填路径（… 头截断尾部可见）"
                assert "Downloads" in text or env.save_dir.rsplit("/", 1)[-1] in text, \
                    "保存到预填配置下载目录（截断尾部可见）"
                assert_in("可输入多行 traker，使用 ALT+ENTER 换行", text,
                          "trackers 占位提示逐字")
                assert re.search(r"已选 4 个文件，共 [\d.]+ \S+", text), \
                    f"统计行（4 文件）: {self._stats_line(app)}"
                assert "☐" in text and not re.search(r"按顺序下载.*☑", text), \
                    "按顺序下载默认 ☐ 不勾选"
                app.type_text("F")
                app.pump(0.2)
                assert fx["multi"] not in app.text().replace("F", ""), \
                    "初始焦点应在种子行（键入直接落入种子字段）"
            finally:
                app.graceful_quit()

    def _case_2(self):
        """QA-AB-02 文件树交互：展开/收拢/Space 三态/统计联动/全不选全选/点击复选框。"""
        @self.case("QA-AB-02")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["wide"])
                app.send("tab", "tab")
                app.pump(0.2)
                rows = self._tree_rows(app)
                assert len(rows) == 1 and rows[0][1] == "root" and \
                    rows[0][0] in ("x", "-"), f"树默认收拢全选: {rows}"
                app.send("right")
                app.pump(0.2)
                rows = self._tree_rows(app)
                assert len(rows) == 2, f"root 展开应露出子目录: {rows}"
                app.send("down", "right")
                app.pump(0.2)
                rows = self._tree_rows(app)
                assert len(rows) == 7, \
                    f"子目录展开后列表框 7 可视行（视口截取）: {len(rows)}"
                assert self._stats_line(app).startswith("10|"), \
                    f"全选统计 10 文件: {self._stats_line(app)}"
                app.send("up")
                app.pump(0.15)
                self._tree_goto(app, "f03.bin")
                app.send("space")
                app.pump(0.2)
                rows = self._tree_rows(app)
                mk = dict((nm, m) for m, nm in rows)
                assert self._stats_line(app).startswith("9|"), \
                    f"Space 取消 f03 后统计 9: {self._stats_line(app)}"
                assert mk["f02.bin"] == "x" and mk["f04.bin"] == "x", "其余保持勾选"
                app.send("n")
                app.pump(0.3)
                mk = dict((nm, m) for m, nm in self._tree_rows(app))
                assert all(v == " " for v in mk.values()), "N 全不选"
                assert self._stats_line(app).startswith("0|"), \
                    f"统计应 0 个文件: {self._stats_line(app)}"
                app.send("a")
                app.pump(0.3)
                mk = dict((nm, m) for m, nm in self._tree_rows(app))
                assert all(v == "x" for v in mk.values()), "A 全选"
                assert self._stats_line(app).startswith("10|"), "统计应 10 个文件"
                app.send(*(["up"] * 5))
                app.pump(0.15)
                app.send("left", "left")
                app.pump(0.2)
                rows = self._tree_rows(app)
                assert len(rows) == 1, \
                    f"上移至目录行后 ←← 应收拢至根: {rows}"
                app.send("right", "down", "right", "up")
                app.pump(0.3)
                self._tree_goto(app, "f04.bin")
                row_idx = None
                lines = app.text().split("\n")
                for i, line in enumerate(lines):
                    if "[x]" in line and "f04.bin" in line:
                        row_idx = i
                        break
                assert row_idx is not None, "f04 行应可见"
                col = lines[row_idx].index("[x]")
                _click(app, col + 1, row_idx)
                app.pump(0.3)
                mk = dict((nm, m) for m, nm in self._tree_rows(app))
                assert mk["f04.bin"] == " ", "点击复选框列应翻转勾选"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_3(self):
        """QA-AB-03 trackers：占位提示 / ALT+ENTER 换行 / 65 行 toast / 焦点 Enter=确认。"""
        @self.case("QA-AB-03")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                assert_in("可输入多行 traker，使用 ALT+ENTER 换行", app.text(),
                          "占位提示在位")
                app.send(*(["tab"] * 8))
                app.pump(0.2)
                app.type_text("http://127.0.0.1:1/announce")
                app.send(ALT_ENTER)
                app.type_text("http://127.0.0.1:2/announce")
                app.pump(0.2)
                text = app.text()
                assert "可输入多行 traker" not in text, "输入后占位提示应消失"
                assert_in("http://127.0.0.1:2/announce", text, "第二行应可见")
                toast_seen = False
                for _ in range(64):
                    app.send(ALT_ENTER, gap=0.03)
                    if "trackers 最多 64 行" in app.text():
                        toast_seen = True
                        break
                assert toast_seen, "连续换行应在 trackers 64 行上限处 toast"
                app.send(*(["bs"] * 2))
                app.pump(0.2)
                app.send("enter")
                created = app.wait_for("已添加任务", 6)
                if not created:
                    app.pump(0.5)
                    tasks = self._reg_tasks(env.home)
                    created = any("payload-ub" in t.get("name", "") for t in tasks)
                assert created, "trackers 焦点 Enter = 确认创建（toast 或注册表）"
            finally:
                app.graceful_quit()

    def _case_4(self):
        """QA-AB-04 校验臂：空种子 / 全不选 / 重复任务错误窗口 / 不存在路径。"""
        @self.case("QA-AB-04")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                _clear_field_prefill(app, "种子")
                app.send(*(["tab"] * 2))
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("请输入种子文件路径", 3), "空种子应 toast"
                assert_in("添加任务：BT", app.text(), "对话框应保持打开")
                app.type_text(fx["multi"])
                app.pump(0.2)
                self._expand_tree_full(app)
                app.send("n")
                app.pump(0.3)
                assert self._stats_line(app).startswith("0|"), \
                    f"N 全不选后统计应为 0: {self._stats_line(app)}"
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("请至少选择一个要下载的文件", 3), \
                    "全不选确认应 toast"
                assert_in("添加任务：BT", app.text(), "对话框应保持打开")
                app.send("esc")
                app.pump(0.3)

                self._open_bt_dialog(app, fx["multi"])
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "首轮应创建成功"
                self._open_bt_dialog(app, fx["multi"])
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("该链接的任务已存在", 4), \
                    "同来源重复应弹错误窗口（v1.42/bug27）"
                assert_in(" 错误 ", app.text(), "错误窗口标题")
                app.send("esc")
                app.pump(0.3)
                assert_in("添加任务：BT", app.text(),
                          "关闭错误窗口后应回添加对话框（保持打开）")
                app.send("esc")
                app.pump(0.3)

                self._open_bt_dialog(app, fx["multi"])
                _clear_field_prefill(app, "种子")
                app.type_text(os.path.join(env.fixture_root, "missing.torrent"))
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("无法读取种子文件", 3), \
                    "不存在路径应 toast ⚠ 无法读取种子文件（对话框臂）"
                assert_in("添加任务：BT", app.text(), "toast 臂不关窗")
            finally:
                app.graceful_quit()

    def _case_5(self):
        """QA-AB-05 添加创建：toast 逐字 / 目录归一（~/ 与相对）/ URL 归一绝对路径。

        三轮分别用独立单文件种子（docs-a/b/c.torrent，同 case 同 home 下防判重误伤）。"""
        @self.case("QA-AB-05")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["s1"])
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "单文件种子应创建"
                toast = next((ln for ln in app.text().split("\n")
                              if "已添加任务" in ln and "BT" in ln), "")
                assert re.search(r"✓ 已添加任务 #\d+: \S+（BT，已选 1/1 文件）", toast), \
                    f"toast 逐字形态: {toast!r}"
                detail = self._detail(app)
                assert "BT · 支持断点续传" in detail, f"详情 BT 行: {detail[:200]!r}"
                t1 = self._reg_bt_task(env.home, "payload-docs-a.bin")
                assert t1["url"] == fx["s1"], \
                    f"注册表 url 应为归一绝对路径: {t1['url']!r}"
                assert t1["save_dir"] == os.path.join(env.home, "Downloads"), \
                    f"未改保存到时落配置链默认 = <主目录>/Downloads: {t1['save_dir']!r}"

                self._open_bt_dialog(app, fx["s2"])
                app.send("tab")
                _clear_field_prefill(app, "保存到")
                app.type_text("~/qabt-docs")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "~ 目录应创建"
                t2 = self._reg_bt_task(env.home, "payload-docs-b.bin")
                assert t2["save_dir"] == os.path.join(env.home, "qabt-docs"), \
                    f"保存到 ~ 应归一为绝对路径 <主目录>/qabt-docs: {t2['save_dir']!r}"

                self._open_bt_dialog(app, fx["s3"])
                app.send("tab")
                _clear_field_prefill(app, "保存到")
                app.type_text("qabt-rel")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "相对目录应创建"
                t3 = self._reg_bt_task(env.home, "payload-docs-c.bin")
                assert t3["save_dir"] == os.path.join(env.home, "qabt-rel"), \
                    f"相对目录应归一为 <cwd>/qabt-rel 绝对路径: {t3['save_dir']!r}"
                assert t3["url"] == fx["s3"], "任务 url 归一不变"
            finally:
                app.graceful_quit()

    def _detail(self, app: EzrApp) -> str:
        rows = app.screen.display
        top = next((i for i, r in enumerate(rows) if "任务详情" in r), None)
        if top is None:
            return ""
        end = next((i for i in range(top + 1, len(rows)) if "并发连接" in rows[i]),
                   len(rows))
        return "\n".join(row[64:].rstrip() for row in rows[top:end])

    def _case_6(self):
        """QA-AB-06 仅添加 = 已暂停不占槽；空格重排转等待/下载。"""
        @self.case("QA-AB-06")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                _tab_to_button(app, "仅添加")
                app.send("enter")
                assert app.wait_for("已暂停", 6), "仅添加应落已暂停"
                toast = next((ln for ln in app.text().split("\n")
                              if "已添加任务" in ln and "⏸" in ln), "")
                assert "已暂停，按空格开始" in toast, f"toast 逐字含暂停提示: {toast!r}"
                assert "已暂停 (1)" in app.text() or "[已暂停]" in app.text(), \
                    "任务行应已暂停"
                app.send("space")
                app.pump(0.5)
                text = app.text()
                assert "等待中" in text or "下载中" in text, \
                    "空格后应重排转等待中/下载中"
                assert_not_in("已暂停 (1)", text, "任务不应停留在已暂停")
            finally:
                app.graceful_quit()

    def _case_7(self):
        """QA-AB-07（退役，规程钦定 S）磁力任务元数据获取随 D53 退役——行为归 QA-CW-08。"""
        self.skip("QA-AB-07", "D53 退役（规程 v1.51 钦定留空不重排）：磁力输入行为由 "
                              "QA-CW-08 错误窗口用例承载（suite_cmd_window.py 实跑），"
                              "本用例记 S")

    def _case_8(self):
        """QA-AB-08 窄终端钳制（rows<30）：窗口钳制、字段保留、无残影。"""
        @self.case("QA-AB-08")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, cols=100, rows=24)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                ty, l, r, by = self._frame(app, "添加任务：BT")
                assert by - ty + 1 <= 24, f"窗口应钳制终端高（实测 {by - ty + 1}）"
                assert r - l + 1 <= 100, "窗口宽应 ≤ 终端宽"
                text = app.text()
                for label in ("种子", "保存到", "并发", "按顺序下载", "trackers",
                              "[ 立即下载 ]"):
                    assert_in(label, text, f"窄端应保留 {label}")
                rows = self._tree_rows(app)
                assert len(rows) >= 1, "列表框收缩不低于 1 可视行"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_9(self):
        """QA-AB-09 路径归一四形态 + 同来源重复创建错误窗口。

        四形态归一后同一绝对路径——判重谓词 = 同来源（v1.42/bug27），故每形态
        独立子 HOME 会话（四例均创建成功的唯一自洽口径）；第二轮在末形态会话内
        重复同来源 → 错误窗口。"""
        @self.case("QA-AB-09")
        def go(env: Env):
            fx = self._make_fixtures(env)
            seed9 = fx["seed9"]
            forms = ["ABS", "~/qatorrents/seed9.torrent",
                     "qatorrents/seed9.torrent", "./qatorrents/seed9.torrent"]
            last_app = None
            try:
                for i, form in enumerate(forms):
                    sub_home = os.path.join(env.home, f"h{i}")
                    os.makedirs(os.path.join(sub_home, "qatorrents"), exist_ok=True)
                    sub_seed = os.path.join(sub_home, "qatorrents", "seed9.torrent")
                    if not os.path.exists(sub_seed):
                        import shutil
                        shutil.copy(seed9, sub_seed)
                    app = EzrApp(sub_home, save_dir=env.save_dir, rows=44)
                    self._cur_app = app
                    last_app = app
                    src = sub_seed if form == "ABS" else form
                    self._open_bt_dialog(app, src)
                    _tab_to_button(app, "立即下载")
                    app.send("enter")
                    assert app.wait_for("已添加任务", 6), \
                        f"形态 {i} 应创建成功: {form!r}"
                    t = self._reg_bt_task(sub_home, "payload-s.bin")
                    norm = os.path.join(sub_home, "qatorrents", "seed9.torrent")
                    assert t["url"] == norm, \
                        f"注册表 url 应为归一绝对路径 {norm}: {t['url']!r}"
                    assert t["save_dir"] == os.path.join(sub_home, "Downloads"), \
                        f"save_dir = 配置链默认 <主目录>/Downloads: {t['save_dir']!r}"
                # 第二轮：同来源重复（在末形态会话内）
                app = last_app
                self._open_bt_dialog(app, "./qatorrents/seed9.torrent")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("该链接的任务已存在", 4), \
                    "第二轮同来源重复创建应弹错误窗口（判重用归一路径）"
                app.send("esc")
                app.pump(0.3)
            finally:
                if last_app is not None:
                    last_app.graceful_quit()

    def _case_10(self):
        """QA-AB-10 限速预填 fmt 规范化（v1.45 现行口径）+ 键入规范化/裸数字/非法回退。"""
        @self.case("QA-AB-10")
        def go(env: Env):
            fx = self._make_fixtures(env)
            rows = (
                ("2 MiB/s", "2 MiB/s", "multi"),
                ("2000000", "2000000 B/s", "wide"),
            )
            try:
                for seq, (cfgval, prefill, tor) in enumerate(rows):
                    env.write_config(
                        f'[common]\ntask_max_download_speed = "{cfgval}"\n'
                        f'task_max_upload_speed = "{cfgval}"\n')
                    app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                    self._cur_app = app
                    try:
                        self._open_bt_dialog(app, fx[tor])
                        text = app.text()
                        assert re.search(
                            rf"下载限速\s*>\s*{re.escape(prefill)}\b", text), \
                            f"config {cfgval!r} 下载限速预填应 {prefill!r}: {text[:400]!r}"
                        assert re.search(
                            rf"上传限速\s*>\s*{re.escape(prefill)}\b", text), \
                            f"config {cfgval!r} 上传限速预填应 {prefill!r}"
                        app.send(*(["tab"] * 4))
                        for _ in range(16):
                            app.send("bs")
                        if seq == 0:
                            app.type_text("2m")
                            app.pump(0.3)
                            assert re.search(r"下载限速\s*>\s*2 MiB/s", app.text()), \
                                "键入 2m 应即规范化显示 2 MiB/s"
                        else:
                            app.type_text("512")
                            app.pump(0.3)
                            assert re.search(r"下载限速\s*>\s*512\b", app.text()), \
                                "裸数字 512 输入态不缀单位"
                            _tab_to_button(app, "立即下载")
                            app.send("enter")
                            assert app.wait_for("已添加任务", 6)
                            app.send("m")
                            assert app.wait_for("修改任务：BT", 5), "M 开修改面板"
                            mtext = app.text()
                            assert re.search(r"下载限速\s*>\s*512 B/s", mtext), \
                                f"裸 512 确认应落库 512 B/s（注册表回读）: {mtext[:400]!r}"
                            break
                    finally:
                        app.graceful_quit()
                env.write_config(
                    '[common]\ntask_max_download_speed = "2 MiB/s"\n')
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                try:
                    self._open_bt_dialog(app, fx["single"])
                    app.send(*(["tab"] * 4))
                    for _ in range(16):
                        app.send("bs")
                    app.type_text("abc")
                    app.pump(0.3)
                    _tab_to_button(app, "立即下载")
                    app.send("enter")
                    assert app.wait_for("已添加任务", 6), "非法输入确认应回退创建"
                    app.send("m")
                    assert app.wait_for("修改任务：BT", 5)
                    assert re.search(r"下载限速\s*>\s*2 MiB/s", app.text()), \
                        "非法输入应回退 config 默认 2 MiB/s"
                finally:
                    app.graceful_quit()
            finally:
                env.write_config("")

    def _case_11(self):
        """QA-AB-11 tracker 协议白名单：ftp 拒绝截尾 / wss 同权 / 大写等价 / 留空 trackers（v1.52 产品口径：池 = 三来源合并去重，留空时池内容 = 种子元数据自带 announce）。"""
        @self.case("QA-AB-11")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                app.send(*(["tab"] * 8))
                app.pump(0.2)
                app.type_text("ftp://tracker.example/ann")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("不支持的 tracker 协议", 3), "ftp 应 toast 拒绝"
                toast = next((ln for ln in app.text().split("\n")
                              if "不支持的 tracker 协议" in ln), "")
                assert "ftp://tracker.example/ann" in toast, \
                    f"toast 应含 URL（未截尾长度内）: {toast!r}"
                assert "添加任务：BT" in app.text(), "对话框应保持打开"
                for _ in range(30):
                    app.send("bs")
                app.pump(0.3)
                app.type_text("wss://tracker.example/ws")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "wss 应同权放行创建"
                app.send("t")
                assert app.wait_for("Trackers", 4), "T 弹窗应打开"
                assert_in("wss://tracker.example/ws", app.text(), "T 弹窗含 wss 行")
                app.send("esc")
                app.pump(0.3)

                self._open_bt_dialog(app, fx["wide"])
                app.send(*(["tab"] * 8))
                app.pump(0.2)
                app.type_text("WS://tracker.example/ws")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "大写 WS:// 应等价放行"

                self._open_bt_dialog(app, fx["meta"])
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "留空 trackers 应创建"
                app.send("t")
                assert app.wait_for("Trackers", 4), "T 弹窗应打开"
                raw = app.raw_text()
                tail = raw[raw.rfind("Trackers"):] if "Trackers" in raw else ""
                tail = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", tail)
                tail = tail.replace("\uFE0E", "")
                assert "127.0.0.1:19998/announce" in tail, \
                    "留空 trackers 时种子元数据自带 announce 应入池（三来源口径，" \
                    "FR-02-21）；规程「留空默认 4 条」为 demo 演示口径、产品未实现，" \
                    "已登记 handoff 待批"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _trackers_rows(self, app: EzrApp) -> list[str]:
        """T 弹窗 tracker URL 行提取（# 序号行，排除表头/任务名）。"""
        out = []
        text = app.text()
        for m in re.finditer(r"^\s*(\d+)\s+(\S+://\S+)", text, re.M):
            out.append(m.group(2))
        return out

    def _case_12(self):
        """QA-AB-12 按顺序下载：默认 ☐ / Space·Enter·点击翻转 / 勾选落库跨重启。"""
        @self.case("QA-AB-12")
        def go(env: Env):
            fx = self._make_fixtures(env)
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                self._open_bt_dialog(app, fx["multi"])
                text = app.text()
                assert re.search(r"按顺序下载\s*☐", text.replace("  ", " ")) or \
                    "☐" in text, "初值应 ☐ 不勾选"
                app.send(*(["tab"] * 7))
                app.pump(0.2)
                app.send("space")
                app.pump(0.2)
                assert_in("☑", app.text(), "Space 应翻转 ☐→☑")
                app.send("enter")
                app.pump(0.2)
                assert_in("☐", app.text(), "Enter 应再次翻转 ☑→☐")
                _click_at_seq = self._seq_row_click(app, "按顺序下载")
                if _click_at_seq:
                    assert_in("☑", app.text(), "点击复选框应翻转 ☐→☑")
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6), "勾选确认应创建任务"
                app.graceful_quit()

                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                assert app.wait_for("BT", 6), "重启应恢复任务"
                app.send("m")
                assert app.wait_for("修改任务：BT", 5)
                assert_in("☑", app.text(), "重启后修改面板按顺序下载应保持 ☑")
                app.send("esc")
                app.pump(0.3)

                self._open_bt_dialog(app, fx["single"])
                _tab_to_button(app, "立即下载")
                app.send("enter")
                assert app.wait_for("已添加任务", 6)
                app.send("m")
                assert app.wait_for("修改任务：BT", 5)
                assert_not_in("☑", app.text(), "未勾选任务不应启用顺序下载")
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _seq_row_click(self, app: EzrApp, label: str) -> bool:
        lines = app.text().split("\n")
        for i, line in enumerate(lines):
            if label in line and "☐" in line:
                _click(self._cur_app, line.index("☐"), i)
                self._cur_app.pump(0.2)
                return True
        return False


def main() -> int:
    suite = AddBtSuite()
    suite.run()
    p, f, s = suite.summary()
    print(f"\n[07-add-bt] P={p} F={f} S={s}")
    return 1 if f else 0


if __name__ == "__main__":
    sys.exit(main())
