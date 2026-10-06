#!/usr/bin/env python3
"""suite_modify_task.py — QA 套件 · 03-modify-task 任务修改对话框（期号 01）。

对应规程：project/qa/03-modify-task-qa.md（8 用例）。
端到端 UI 层验证：按键驱动（m/Tab/Enter/bs）+ 头部活跃连接数观测 + fixture 代理日志
+ 重启续传；不使用项目内部 API。
「立即生效」判定沿规程 D19 口径：不要求在途请求/在途块被打断重连；并发下调允许
块级过渡延迟（默认块 1 MB 秒级）。
焦点/字段口径（product dialogs.rs）：Add 六行 URL/保存到/并发/校验/校验码/代理
（确认 6 取消 7）；Modify 四行 并发0/校验1/校验码2/代理3（确认 4 取消 5）；
添加对话框校验算法默认 SHA-256（ck_type=3）。
"""

from __future__ import annotations

import os
import re
import time

from harness import (
    Env, EzrApp, Suite, TMP_ROOT, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, detail_text, expected_content, proxy_reqs,
    sha256_hex, start_proxy, stop_proxy, task_pct, wait_file_size,
    QA_FILE_SIZES,
)

TWELVE_M = QA_FILE_SIZES["twelve-m.bin"]
FIVE_M = QA_FILE_SIZES["five-m.bin"]
SMALL = 1_024


def _header_threads(app: EzrApp) -> int | None:
    """头部「并发线程 N」（活跃连接数，数据面判定）。"""
    m = re.search(r"并发线程 (\d+)", app.text())
    return int(m.group(1)) if m else None


def _open_modify(app: EzrApp) -> None:
    """选中任务按 m 打开修改对话框（焦点落并发字段）。"""
    app.send("m")
    assert app.wait_for("修改任务", 3), "修改对话框未打开"


def _confirm(app: EzrApp, tabs: int = 4) -> None:
    """从当前焦点 Tab×tabs 到确认按钮（焦点 4）并 Enter。

    并发字段（0）起步 tabs=4；校验码字段（2）起步 tabs=2。
    """
    app.send(*(["tab"] * tabs))
    app.send("enter")
    app.pump(0.3)


def _monotonic(pcts: list[float]) -> list[float]:
    return [(b - a) for a, b in zip(pcts, pcts[1:]) if b < a - 1e-9]


class ModifyTaskSuite(Suite):
    def __init__(self):
        super().__init__("03-modify-task", port_base=42000)
        self._cur_app: "EzrApp | None" = None  # 失败现场快照用

    def _snapshot(self, env: Env, case_id: str) -> None:
        """失败现场：把当前画面写入 .work/tmp/qa-run/<case>.screen.txt。"""
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
    def _set_conns(app: EzrApp, value: str) -> None:
        """并发字段（焦点 0，Modify 打开即在此）清空重输。"""
        for _ in range(4):
            app.send("bs")
        app.type_text(value)

    @staticmethod
    def _edit_ck_value(app: EzrApp, value: str) -> None:
        """从并发 Tab×2 到校验码字段（焦点 2）清空重输（焦点留在 2）。"""
        app.send(*(["tab"] * 2))
        for _ in range(130):
            app.send("bs", gap=0.01)
        if value:
            app.type_text(value)

    def _wait_verify_ok(self, app: EzrApp, algo: str, timeout: float) -> None:
        """等待校验成功判定：完成 toast（校验通过）为主，已完成页签行为兜底。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.2)
            if f"{algo} 校验通过" in app.text():
                return
            time.sleep(0.2)
        # 兜底：切已完成页签核对常驻行
        app.send("tab")
        app.pump(0.4)
        assert_in(f"{algo} 校验成功", app.text(), "完成校验按新值判定")

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list:
        for i in range(1, 9):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-MT-01 按预填参数添加任务 → 选中按 m：四字段与当前值一致 + 焦点次序。"""
        @self.case("QA-MT-01")
        def go(env: Env):
            ck = sha256_hex(expected_content(SMALL))
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # 慢速任务保证 m 按下时仍在下载（非终态可修改）
                add_task_via_dialog(app, env, env.fixture.url("five-m.bin?speed=300000"),
                                    conns="3", ck_value=ck)
                assert app.wait_for("下载中", 8), "任务应在下载中"
                _open_modify(app)
                text = app.text()
                # 四字段与任务当前值一致（并发 3 / SHA-256 / 校验码 / 直连）
                assert re.search(r"并发\s*>\s*3\b", text), "并发预填应为 3"
                assert_in("SHA-256", text, "校验算法预填")
                assert_in(ck[-10:], text, "校验码预填（尾部显示口径，ui/dialog 行值截断规则）")
                assert_in("直连", text, "代理预填直连")
                # 焦点次序（功能口径）：并发起步 → Tab×5 到取消 → Enter 关闭不生效
                app.send(*(["tab"] * 5))
                app.send("enter")
                app.pump(0.4)
                assert "修改任务" not in app.text(), "取消 Enter 应关闭对话框"
                # 再次打开：参数未被取消路径改动
                _open_modify(app)
                assert re.search(r"并发\s*>\s*3\b", app.text()), "取消后并发不应变化"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-MT-02 下载中 1→4 与 4→1 各一轮：活跃连接数升降 + 字节单调不暂停。"""
        @self.case("QA-MT-02")
        def go(env: Env):
            # 相位 A：1→4（12MB 慢速保证观测窗）
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=400000"),
                    conns="1")
                assert app.wait_for("下载中", 8)
                app.pump(1.5)
                _open_modify(app)
                self._set_conns(app, "4")
                _confirm(app)
                assert app.wait_for("任务参数已更新", 3), "修改立即生效提示"
                # 上调：调度周期内活跃连接升至 ≥2（≤4），任务不暂停、字节单调
                peak, pcts, paused = 0, [], False
                deadline = time.time() + 9.0
                while time.time() < deadline:
                    app.pump(0.15)
                    t = _header_threads(app)
                    if t is not None:
                        peak = max(peak, t)
                    p = task_pct(app)
                    if p is not None:
                        pcts.append(p)
                    if "已暂停" in app.text():
                        paused = True
                        break
                    time.sleep(0.2)
                assert not paused, "上调过程任务不应暂停"
                assert 2 <= peak <= 4, f"上调后活跃连接应升至 2..4，实测峰值 {peak}"
                drops = _monotonic(pcts)
                assert not drops, f"进度应单调不减，回退点 {drops[:3]}"
            finally:
                app.graceful_quit()
            # 相位 B：4→1（独立子目录避免重复守卫；下调允许块级过渡）
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=400000"),
                    conns="4", save_dir=os.path.join(env.save_dir, "b"))
                assert app.wait_for("下载中", 8)
                app.pump(1.5)
                _open_modify(app)
                self._set_conns(app, "1")
                _confirm(app)
                assert app.wait_for("任务参数已更新", 3), "修改立即生效提示"
                # 下调：块级过渡后活跃连接降至 ≤1 并保持；进度单调、不暂停
                time.sleep(2.5)  # 过渡窗（1MB 块 @1.6MB/s ≈ 0.7s/块）
                low_streak, pcts, paused = 0, [], False
                deadline = time.time() + 8.0
                while time.time() < deadline:
                    app.pump(0.15)
                    t = _header_threads(app)
                    if t is None:
                        continue
                    if t > 1:
                        low_streak = 0
                    else:
                        low_streak += 1
                    p = task_pct(app)
                    if p is not None:
                        pcts.append(p)
                    if "已暂停" in app.text():
                        paused = True
                        break
                    time.sleep(0.2)
                assert not paused, "下调过程任务不应暂停"
                assert low_streak >= 5, \
                    f"下调后活跃连接应稳定 ≤1（连续低读数 {low_streak}）"
                drops = _monotonic(pcts)
                assert not drops, f"进度应单调不减，回退点 {drops[:3]}"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-MT-03 下载中直连→proxy-a→直连：新请求入目标日志、回切后停止增长。"""
        @self.case("QA-MT-03")
        def go(env: Env):
            px = start_proxy(env, "proxy-a", self._next_port())
            try:
                env.write_config(
                    '[[proxies]]\nname = "proxy-a"\ntype = "http"\n'
                    f'ip = "127.0.0.1"\nport = {px["port"]}\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                try:
                    add_task_via_dialog(
                        app, env, env.fixture.url("twelve-m.bin?speed=300000"))
                    assert app.wait_for("下载中", 8)
                    app.pump(1.0)
                    n0 = len(proxy_reqs(px))
                    # 直连 → proxy-a
                    _open_modify(app)
                    app.send(*(["tab"] * 3))  # 并发0→校验1→校验码2→代理3
                    app.send("enter")         # 展开代理下拉
                    assert app.wait_for("选择代理", 3), "代理下拉未展开"
                    app.send("down")          # 直连 → proxy-a（配置顺序第 1）
                    app.send("enter")         # 选中并关闭
                    app.pump(0.2)
                    _confirm(app, tabs=1)     # 代理3 → 确认4
                    assert app.wait_for("任务参数已更新", 3), "切代理应立即生效"
                    # 新请求出现在 proxy-a 日志（有界重试窗）
                    hit = False
                    deadline = time.time() + 8.0
                    while time.time() < deadline:
                        app.pump(0.2)
                        if len(proxy_reqs(px)) > n0:
                            hit = True
                            break
                        time.sleep(0.1)
                    assert hit, "切代理后 proxy-a 日志应出现新请求"
                    n1 = len(proxy_reqs(px))
                    # proxy-a → 直连
                    _open_modify(app)
                    app.send(*(["tab"] * 3))
                    app.send("enter")
                    assert app.wait_for("选择代理", 3)
                    app.send("home")          # 直连恒为首项
                    app.send("enter")
                    app.pump(0.2)
                    _confirm(app, tabs=1)
                    assert app.wait_for("任务参数已更新", 3), "切回直连应立即生效"
                    time.sleep(4.0)           # 观察窗：任务仍在途，日志不应再增长
                    app.pump(1.0)
                    n2 = len(proxy_reqs(px))
                    assert n2 == n1, \
                        f"切回直连后 proxy-a 日志不应增长（{n1} → {n2}）"
                    assert_in("下载中", app.text(), "切回直连后应继续下载")
                finally:
                    code = app.graceful_quit(timeout=10)
                    assert code == 0, f"优雅退出码 0，得到 {code}"
                # 重启续传至完成（退出时任务仍在途）
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                try:
                    assert app.wait_for("下载中", 15), "重启应续传"
                    wait_file_size(os.path.join(env.save_dir, "twelve-m.bin"),
                                   TWELVE_M, 90, app=app)
                finally:
                    app.graceful_quit()
                assert_file_content(
                    os.path.join(env.save_dir, "twelve-m.bin"), TWELVE_M)
            finally:
                stop_proxy(px)

    def _case_04(self):
        """QA-MT-04 改校验为正确/错误新值各一轮：完成校验按新值判定。"""
        @self.case("QA-MT-04")
        def go(env: Env):
            correct = sha256_hex(expected_content(TWELVE_M))
            wrong = ("0" * 64) if correct[0] != "0" else ("1" * 64)
            # 相位 A：正确新值 → 完成校验通过（子目录隔离两相位）
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=300000"),
                    save_dir=os.path.join(env.save_dir, "a"))
                assert app.wait_for("下载中", 8), "任务应在下载中"
                _open_modify(app)
                self._edit_ck_value(app, correct)  # 算法预选 SHA-256（添加默认）
                _confirm(app, tabs=2)
                assert app.wait_for("任务参数已更新", 3), "校验修改应生效"
                self._wait_verify_ok(app, "SHA-256", 75)
            finally:
                app.graceful_quit()
            # 相位 B：错误新值 → 校验失败（失败态行常驻，断言锚定最终态）
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=300000"),
                    save_dir=os.path.join(env.save_dir, "b"))
                assert app.wait_for("下载中", 8)
                _open_modify(app)
                self._edit_ck_value(app, wrong)
                _confirm(app, tabs=2)
                assert app.wait_for("任务参数已更新", 3), "校验修改应生效"
                assert app.wait_for("SHA-256 校验失败", 75), \
                    "错误校验码应判定校验失败"
            finally:
                app.graceful_quit()

    def _case_05(self):
        """QA-MT-05 清空校验码确定：完成无校验直接已完成；详情无校验行。"""
        @self.case("QA-MT-05")
        def go(env: Env):
            ck = sha256_hex(expected_content(FIVE_M))  # 与下载文件匹配
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=300000"),
                    conns="1", ck_value=ck)
                assert app.wait_for("下载中", 8), "任务应在下载中"
                d = detail_text(app)
                assert_in("校验", d, "前置：详情应有校验行")
                _open_modify(app)
                self._edit_ck_value(app, "")  # 清空校验码
                _confirm(app, tabs=2)
                assert app.wait_for("任务参数已更新", 3), "清空校验码应生效"
                assert app.wait_for("（无校验）", 35), "完成应无校验直接已完成"
                # 已完成页签选中该任务：详情应无校验行
                app.send("tab")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                d = detail_text(app)
                assert_not_in("校验", d, "详情应无校验行")
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-MT-06 SHA-256 配 8 位 hex：不生效 + 字段聚焦提示；修正后生效。"""
        @self.case("QA-MT-06")
        def go(env: Env):
            correct = sha256_hex(expected_content(FIVE_M))  # 与下载文件匹配
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=300000"),
                    conns="1")
                assert app.wait_for("下载中", 8), "任务应在下载中"
                _open_modify(app)
                self._edit_ck_value(app, "abcd1234")  # 8 位（SHA-256 需 64 位）
                _confirm(app, tabs=2)
                # 对话框不关、提示在案（位数口径文案）、任务无变化
                assert_in("修改任务", app.text(), "非法校验码对话框应保持打开")
                assert app.wait_for("校验码需为 64 位十六进制", 3), \
                    "应提示位数不符"
                # 修正（焦点已回收校验码字段=2）：清空重输正确值，不再 Tab
                for _ in range(130):
                    app.send("bs", gap=0.01)
                app.type_text(correct)
                _confirm(app, tabs=2)
                assert app.wait_for("任务参数已更新", 3), "修正后应生效"
                self._wait_verify_ok(app, "SHA-256", 20)
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-MT-07 已完成任务按 m：无对话框；toast 提示不可修改。"""
        @self.case("QA-MT-07")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, env.fixture.url("small.bin"))
                assert app.wait_for("已完成", 10)
                # 已完成任务在第二页签：切换并选中后再按 m
                app.send("tab")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                app.send("m")
                app.pump(0.5)
                assert_not_in("修改任务", app.text(), "已完成任务不应弹出修改对话框")
                assert app.wait_for("已完成任务不可修改", 3), \
                    "应有不可修改 toast 提示"
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-MT-08 修改并发/校验/代理 → 立即 Q → 重启：参数一致且按新参数续传。"""
        @self.case("QA-MT-08")
        def go(env: Env):
            ck = sha256_hex(expected_content(TWELVE_M))
            px = start_proxy(env, "proxy-a", self._next_port())
            try:
                env.write_config(
                    '[[proxies]]\nname = "proxy-a"\ntype = "http"\n'
                    f'ip = "127.0.0.1"\nport = {px["port"]}\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=300000"),
                    conns="2")
                assert app.wait_for("下载中", 8)
                _open_modify(app)
                self._set_conns(app, "3")       # 焦点 0：并发 2→3
                self._edit_ck_value(app, ck)    # Tab×2 → 校验码（算法预选 SHA-256）
                app.send("tab")                 # →代理（焦点 3）
                app.send("enter")
                assert app.wait_for("选择代理", 3), "代理下拉未展开"
                app.send("down")                # 直连 → proxy-a
                app.send("enter")
                app.pump(0.2)
                _confirm(app, tabs=1)           # 代理3 → 确认4
                assert app.wait_for("任务参数已更新", 3), "修改应立即生效"
                code = app.graceful_quit(timeout=10)
                assert code == 0, f"Q 优雅退出码 0，得到 {code}"
                # 重启：参数还原（详情 + 对话框预填），按新参数续传（经 proxy-a）
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                try:
                    assert app.wait_for("下载中", 15), "重启应续传"
                    d = detail_text(app)
                    assert_in("3 并发", d, "并发 3 应还原")
                    assert_in("校验", d, "校验行应还原")
                    n0 = len(proxy_reqs(px))
                    hit = False
                    deadline = time.time() + 10.0
                    while time.time() < deadline:
                        app.pump(0.2)
                        if len(proxy_reqs(px)) > n0:
                            hit = True
                            break
                        time.sleep(0.1)
                    assert hit, "续传请求应经 proxy-a"
                    _open_modify(app)
                    text = app.text()
                    assert re.search(r"并发\s*>\s*3\b", text), "对话框并发预填 3"
                    assert_in("proxy-a（http）", text, "对话框代理预填")
                    app.send("esc")
                    app.pump(0.3)
                    wait_file_size(
                        os.path.join(env.save_dir, "twelve-m.bin"), TWELVE_M, 90)
                    assert_file_content(
                        os.path.join(env.save_dir, "twelve-m.bin"), TWELVE_M)
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(px)


if __name__ == "__main__":
    suite = ModifyTaskSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
