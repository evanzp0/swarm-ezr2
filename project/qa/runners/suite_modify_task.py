#!/usr/bin/env python3
"""suite_modify_task.py — QA 套件 · 03-modify-task 任务修改对话框（期号 01）。

对应规程：project/qa/03-modify-task-qa.md（10 用例；v1.38 限速字段插入几何已全量同步 + MT-10 补齐）。
端到端 UI 层验证：按键驱动（m/Tab/Enter/bs）+ 头部活跃连接数观测 + fixture 代理日志
+ 重启续传；不使用项目内部 API。
「立即生效」判定沿规程 D19 口径：不要求在途请求/在途块被打断重连；**下调即时收缩
（v1.16/D30 改判）**——确定后约 1 秒内连接数降至目标值（不等块边界）。
焦点/字段口径（product dialogs.rs）：Add 六行 URL/保存到/并发/校验/校验码/代理
（v1.38 限速字段插入后：URL0/保存到1/并发2/下载限速3/校验4/校验码5/代理6，立即下载 7 仅添加 8 取消 9）；
Modify 五行 并发0/下载限速1/校验2/校验码3/代理4（确认 5 取消 6，v1.38 限速字段插入后）；
v1.32：校验下拉 8 项默认 NONE（校验码禁输入），设/改校验值须先在下拉选算法。

v1.17 同步（QA v138 会话）：新增 QA-MT-09（对话框粘贴，FR-01-06 v1.17 修订）——
Add/Modify 文本字段同权接收 bracketed paste：校验码仅 hex ≤128 位（CAP_CK_HEX）、
并发仅数字 ≤2 位（CAP_CONNS_DIGITS）、控制字符净化；下拉展开与按钮焦点粘贴被忽略。
"""

from __future__ import annotations

import os
import re
import time

from harness import (
    Env, EzrApp, Suite, TMP_ROOT, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, conns_badge, detail_text, expected_content,
    proxy_reqs, sha256_hex, start_proxy, stop_proxy, task_pct,
    wait_file_size, QA_FILE_SIZES,
)

TWELVE_M = QA_FILE_SIZES["twelve-m.bin"]
FIVE_M = QA_FILE_SIZES["five-m.bin"]
SMALL = 1_024


def _active_conns(app: EzrApp) -> int | None:
    """并发连接面板标题徽标 x（v1.34/FR-01-96：` x/y 项 `——x = 活跃明细行数）。

    注意口径（v1.34 修订）：头部「并发 N」= 下载中任务正在连接 + 已连接的
    **活跃并发总数**（FR-01-81 v1.34，与徽标 x、明细行数严格同源，is_active
    单源；待命/已断开不计——旧「连接总数/速度 > 0」双口径退役）；
    QA-MT-02 观测升降级调度效果以徽标 x 为准（harness.conns_badge 区域锚定）。
    """
    b = conns_badge(app)
    return b[0] if b is not None else None


def _open_modify(app: EzrApp) -> None:
    """选中任务按 m 打开修改对话框（焦点落并发字段）。"""
    app.send("m")
    assert app.wait_for("修改任务", 3), "修改对话框未打开"


def _confirm(app: EzrApp, tabs: int = 5) -> None:
    """从当前焦点 Tab×tabs 到确认按钮（焦点 5）并 Enter。

    v1.38 限速字段插入后：并发字段（0）起步 tabs=5；校验码字段（3）与
    代理字段（4）起步 tabs=2/1 不变（确认 = 焦点 5）。
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
        """设/改校验为 SHA-256 并重输校验码（焦点留在校验码 2）。

        v1.32/FR-01-01 语义：校验下拉默认 NONE（无校验任务修改框预填 NONE）
        且 NONE 选中时校验码禁输入——设值必须先在校验字段（焦点 1）展开下拉
        选算法：Home 归 NONE（选中即改值、校验码实时清空）→ ↓×4 到 SHA-256
        （与预填算法无关的确定性路径）→ Enter 关下拉（焦点留 2）→ Tab 到
        校验码（焦点 3，v1.38 限速字段插入后）→ 清残值重输。
        """
        app.send("tab", "tab")          # 并发0 → 下载限速1 → 校验2（v1.38）
        app.send("enter")               # 展开算法下拉
        app.send("home")                # 归 NONE（清校验码）
        app.send(*(["down"] * 4))       # NONE→MD5→SHA-1→SHA-224→SHA-256
        app.send("enter")               # 关下拉（焦点留校验 2）
        app.send("tab")                 # 校验2 → 校验码3
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
        for i in range(1, 11):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-MT-01 按预填参数添加任务 → 选中按 m：五字段与当前值一致 + 焦点次序（v1.38 五行几何）。"""
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
                # 五字段与任务当前值一致（并发 3 / 下载限速 0 / SHA-256 / 校验码 / 直连）
                assert re.search(r"并发\s*>\s*3\b", text), "并发预填应为 3"
                assert re.search(r"下载限速\s*>\s*0\b", text), \
                    "下载限速预填应为 0（任务未设限速，v1.38 五行几何）"
                assert_in("SHA-256", text, "校验算法预填")
                assert_in(ck[-10:], text, "校验码预填（尾部显示口径，ui/dialog 行值截断规则）")
                assert_in("直连", text, "代理预填直连")
                # 焦点次序（功能口径）：并发起步 → Tab×6 到取消（焦点 6）→ Enter 关闭不生效
                app.send(*(["tab"] * 6))
                app.send("enter")
                app.pump(0.4)
                assert "修改任务" not in app.text(), "取消 Enter 应关闭对话框"
                # 再次打开：参数未被取消路径改动
                _open_modify(app)
                assert re.search(r"并发\s*>\s*3\b", app.text()), "取消后并发不应变化"
                app.send("esc")
                app.pump(0.3)
                # v1.32 NONE 预填臂（规程第二相）：无校验任务按 m → 预填 NONE、
                # 校验码输入框空且禁输入（逐键拦截）
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?speed=100000"),
                    save_dir=os.path.join(env.save_dir, "t2"))
                assert app.wait_for("下载中", 8), "无校验任务应在下载中"
                app.send("down")   # 选中第二行（新任务；已在末行时钳制原地）
                app.pump(0.3)
                _open_modify(app)
                text = app.text()
                assert_in("NONE", text, "无校验任务算法应预填 NONE（v1.32）")
                app.send(*(["tab"] * 3))   # 并发0 → 下载限速1 → 校验2 → 校验码3（v1.38）
                app.type_text("abcd1234")
                app.pump(0.3)
                assert_not_in("abcd", app.text(), "NONE 选中时校验码应禁输入")
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
                    t = _active_conns(app)
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
            # 相位 B：4→1（独立子目录避免重复守卫；v1.16/D30：下调即时收缩）
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
                # v1.16/D30 即时收缩：确定后约 1 秒内（进度帧粒度）降至 ≤1，
                # 不等块边界（1MB 块 @1.6MB/s ≈ 0.7s/块——若仍按块边界收缩，
                # 超额读数会持续到下一块完成，必红）。超额读数只允许出现在
                # 确认后 1.0s 宽限窗内（帧粒度 + 确认 toast 渲染延迟）；其后
                # 连续低读数稳定，进度单调、不暂停。
                confirm_t = time.time()
                late_high = []      # 宽限窗外的 >1 读数（应为空）
                low_streak, pcts, paused = 0, [], False
                deadline = time.time() + 8.0
                while time.time() < deadline:
                    app.pump(0.15)
                    t = _active_conns(app)
                    if t is None:
                        continue
                    if t > 1:
                        low_streak = 0
                        if time.time() - confirm_t > 1.0:
                            late_high.append((round(time.time() - confirm_t, 2), t))
                    else:
                        low_streak += 1
                    p = task_pct(app)
                    if p is not None:
                        pcts.append(p)
                    if "已暂停" in app.text():
                        paused = True
                        break
                    time.sleep(0.15)
                assert not paused, "下调过程任务不应暂停"
                assert not late_high, \
                    f"下调应即时收缩（1s 内降至 ≤1，实测宽限窗外超额读数 {late_high[:4]}）"
                assert low_streak >= 5, \
                    f"下调后活跃连接应稳定 ≤1（连续低读数 {low_streak}）"
                drops = _monotonic(pcts)
                assert not drops, f"进度应单调不减（被中断块块内已写字节保留），回退点 {drops[:3]}"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-MT-03 下载中直连→proxy-a→直连：新请求入目标日志、回切后停止增长。"""
        @self.case("QA-MT-03")
        def go(env: Env):
            px = start_proxy(env, "proxy-a", self._next_port())
            try:
                env.write_config(
                    '[common]\n\n[[common.proxy]]\nname = "proxy-a"\ntype = "http"\n'
                    f'ip = "127.0.0.1"\nport = {px["port"]}\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                try:
                    # big-100m：每连接限速口径（×4 conns = 1.2MB/s，100MB ≈ 83s）
                    # ——观测窗（hit ≤8s，块耗 3.5s）与重启续传窗（83s ≫ 用例
                    # 时长 ~17s）同时满足；twelve-m 在该口径下 10s 下完，重启
                    # 断言与完成竞速（本会话实录）。
                    add_task_via_dialog(
                        app, env, env.fixture.url("big-100m.bin?speed=300000"))
                    assert app.wait_for("下载中", 8)
                    app.pump(1.0)
                    n0 = len(proxy_reqs(px))
                    # 直连 → proxy-a
                    _open_modify(app)
                    app.send(*(["tab"] * 4))  # 并发0→下载限速1→校验2→校验码3→代理4（v1.38）
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
                    app.send(*(["tab"] * 4))  # 并发0→…→代理4（v1.38 五行几何）
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
                    wait_file_size(
                        os.path.join(env.save_dir, "big-100m.bin"),
                        104_857_600, 150, app=app)
                finally:
                    app.graceful_quit()
                assert_file_content(
                    os.path.join(env.save_dir, "big-100m.bin"),
                    104_857_600)
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
        """QA-MT-05 v1.32 双臂：NONE 臂 = 清除校验唯一路径；SHA-256 留空臂 = 提交被拒。

        规程 v1.32 重写口径：①NONE 臂——算法选 NONE（校验码实时清空禁输入）
        确定 = 清除校验，完成无校验直接已完成、详情无校验期望行；②SHA-256
        留空臂——保持算法清空校验码文本确定被拒，toast
        `⚠ SHA-256 校验码不可为空（需 64 位十六进制）` + 聚焦校验码字段
        （原「清空校验码 = 清除校验」路径退役）。
        """
        @self.case("QA-MT-05")
        def go(env: Env):
            ck = sha256_hex(expected_content(FIVE_M))  # 与下载文件匹配
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # -- 臂①：NONE 清除路径 ------------------------------------
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=300000"),
                    conns="1", ck_value=ck)
                assert app.wait_for("下载中", 8), "任务应在下载中"
                d = detail_text(app)
                assert_in("校验", d, "前置：详情应有校验行")
                _open_modify(app)
                app.send("tab", "tab")     # 并发0 → 下载限速1 → 校验2（v1.38）
                app.send("enter")          # 展开算法下拉
                app.send("home")           # 选 NONE（选中即改值、校验码清空）
                app.send("enter")          # 关下拉（焦点留校验 2）
                app.send("tab")            # 校验2 → 校验码3（应空且禁输入）
                app.type_text("ff" * 8)
                app.pump(0.2)
                assert_not_in("ffffff", app.text(), "NONE 选中校验码应禁输入")
                app.send("enter")          # 校验码3 Enter = 确认（v1.38）
                assert app.wait_for("任务参数已更新", 3), "NONE 清除应生效"
                assert app.wait_for("（无校验）", 35), "完成应无校验直接已完成"
                # 已完成页签选中该任务：详情应无校验行（v1.30 页签键 ]）
                app.send("]")
                app.pump(0.3)
                app.send("down")
                app.pump(0.3)
                d = detail_text(app)
                assert_not_in("校验", d, "详情应无校验行")
                app.graceful_quit()
                # -- 臂②：SHA-256 留空提交被拒 ------------------------------
                sub2 = os.path.join(env.save_dir, "b2")
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?speed=100000"),
                    save_dir=sub2)
                assert app.wait_for("下载中", 8), "臂②任务应在下载中"
                _open_modify(app)
                self._edit_ck_value(app, "")  # 选 SHA-256、清空校验码文本
                _confirm(app, tabs=2)
                assert app.wait_for("校验码不可为空", 3), \
                    "SHA-256 留空确定应被拒（v1.32 必填）"
                assert_in("修改任务", app.text(), "被拒后对话框应保持打开")
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
                # 修正（焦点已回收校验码字段=3，v1.38）：清空重输正确值，不再 Tab
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
                # 已完成任务在第二页签：切换并选中后再按 m（v1.30 页签键 ]）
                app.send("]")
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
                    '[common]\n\n[[common.proxy]]\nname = "proxy-a"\ntype = "http"\n'
                    f'ip = "127.0.0.1"\nport = {px["port"]}\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=300000"),
                    conns="2")
                assert app.wait_for("下载中", 8)
                _open_modify(app)
                self._set_conns(app, "3")       # 焦点 0：并发 2→3
                self._edit_ck_value(app, ck)    # 内部导航至校验码3（算法预选 SHA-256）
                app.send("tab")                 # →代理（焦点 4，v1.38）
                app.send("enter")
                assert app.wait_for("选择代理", 3), "代理下拉未展开"
                app.send("down")                # 直连 → proxy-a
                app.send("enter")
                app.pump(0.2)
                _confirm(app, tabs=1)           # 代理4 → 确认5（v1.38）
                assert app.wait_for("任务参数已更新", 3), "修改应立即生效"
                code = app.graceful_quit(timeout=10)
                assert code == 0, f"Q 优雅退出码 0，得到 {code}"
                # 重启：参数还原（详情 + 对话框预填），按新参数续传（经 proxy-a）
                app = EzrApp(env.home, save_dir=env.save_dir)
                self._cur_app = app
                try:
                    assert app.wait_for("下载中", 15), "重启应续传"
                    d = detail_text(app)
                    # v1.14：详情类型行无并发数，并发还原经头部数据面字段核验
                    assert app.wait_for("并发 3", 15), "并发 3 应还原（头部数据面）"
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


    def _case_09(self):
        """QA-MT-09 修改对话框粘贴（v1.17/FR-01-06 修订）：字段过滤、容量钳制、门控忽略。

        ①校验码字段粘贴含杂质长摘要（控制字符/非 hex 混入，>128 位）→ 仅接收
        hex、容量钳 128 位（CAP_CK_HEX，与逐键过滤一致；显示为尾部截断口径）；
        ②并发字段粘贴混合数字字母 → 仅接收数字 ≤2 位（CAP_CONNS_DIGITS）；
        ③校验下拉展开时粘贴被忽略（字段无变化）；④按钮焦点粘贴被忽略、无副作用。
        断言锚：字段渲染值（校验码尾部 10 位截断口径，QA-MT-01 同源）。
        """
        @self.case("QA-MT-09")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=300000"),
                    conns="3")
                assert app.wait_for("下载中", 8)
                # ② 并发字段（焦点 0）粘贴混合数字字母：仅数字 ≤2 位
                _open_modify(app)
                os.write(app.fd, b"\x1b[200~" + b"4a9b2c" + b"\x1b[201~")
                app.pump(0.4)
                text = app.text()
                assert re.search(r"并发\s*>\s*34\b", text), \
                    f"并发粘贴应仅接收数字（3+4 → 34）: {text[:200]!r}"
                # v1.40 粘贴满容量 = 重启录入口径（QA-01-21 同源）：再粘
                # xyz12 → 过滤数字 12 → 清空重输显示 12（2 位封顶）
                os.write(app.fd, b"\x1b[200~" + b"xyz12" + b"\x1b[201~")
                app.pump(0.4)
                text = app.text()
                assert re.search(r"并发\s*>\s*12\b", text), \
                    f"满容量粘贴应重启录入（过滤后 12，2 位封顶）: {text[:200]!r}"
                # ① 校验码字段（焦点 2）粘贴含杂质超长摘要：先选 SHA-256
                #    （v1.32：NONE 选中时校验码禁输入，粘贴同拦截）再粘贴
                self._edit_ck_value(app, "")
                junk = ("中\t文\n" + "zz" + "a1B2c3D4" * 20)  # 控制字符+非hex+hex 160 位
                os.write(app.fd, b"\x1b[200~" + junk.encode() + b"\x1b[201~")
                app.pump(0.5)
                # 净化+过滤+钳制：控制字符剔除、非 hex 剔除、hex 前 128 位
                hex_only = "".join(c for c in junk if c.isascii()
                                   and c.isalnum() and c in "0123456789abcdefABCDEF")
                expect = hex_only[:128][-10:]
                assert_in(expect, app.text(),
                          f"校验码粘贴应仅收 hex ≤128 位（尾部 10 位 {expect}）")
                assert_not_in("中文", app.text(), "控制字符/非 hex 不应进入字段")
                # ③ 校验下拉展开时粘贴被忽略：焦点自校验码（2）Tab 环绕回校验（1），
                #    Enter 展开下拉；粘贴后下拉仍开、字段值不变（无副作用）。
                #    （v1.34 二批修订：文本字段内方向键 = 光标移动（Up 行首），
                #    不跳字段焦点——旧「up 上移」导航锚点退役，焦点导航只走
                #    Tab 环绕；v1.38 限速字段插入后七站：校验码3→代理4→确认5
                #    →取消6→并发0→下载限速1→校验2）
                #    （画面全屏对比不可用——对话框外列表速度行逐秒刷新，属合法变化）
                app.send(*(["tab"] * 6))  # 焦点 3 环绕一周到 2（校验，v1.38 七站）
                app.pump(0.2)
                app.send("enter")  # 展开校验下拉
                assert app.wait_for("SHA-512", 3), "校验下拉应展开"
                os.write(app.fd, b"\x1b[200~" + b"deadbeef9999" + b"\x1b[201~")
                app.pump(0.4)
                after = app.text()
                assert_in("SHA-512", after, "粘贴后下拉应仍展开（忽略粘贴）")
                assert_in(expect, after, "粘贴后校验码字段值不应变化（无副作用）")
                app.send("esc")    # Esc 收起下拉（焦点不变）
                app.pump(0.3)
                # ④ 按钮焦点粘贴被忽略且无副作用：Tab×3 至确认钮（焦点 5），
                #    粘贴不得触发按钮；随后 Esc 关闭（不生效改动，任务参数不变）
                app.send(*(["tab"] * 3))  # 校验2 → 校验码3 → 代理4 → 确认5
                app.pump(0.2)
                os.write(app.fd, b"\x1b[200~" + b"9999ff" + b"\x1b[201~")
                app.pump(0.4)
                after = app.text()
                assert_in("修改任务", after, "粘贴不得触发确认（对话框仍开）")
                assert re.search(r"并发\s*>\s*12\b", after), \
                    "按钮焦点粘贴不得改动并发字段值（②重启录入后为 12）"
                app.send("esc")    # Esc 关闭对话框（粘贴/改动不生效）
                app.pump(0.4)
                assert_in("下载中", app.text(), "任务应继续下载（改动未生效）")
            finally:
                app.graceful_quit()


    def _case_10(self):
        """QA-MT-10（v1.37/38/41 同步补齐）下载中改任务级下载限速：规范化、非法拒绝、清空回填、fmt 回显。

        规程口径（交互语义相；限速引擎有效性由 TP-10 实跑承载不在此重复）：
        ①下载中按 m → 下载限速键入 1M 即规范化显示 `1 MiB/s` → 确认 toast
        「任务参数已更新」且任务保持下载中 → 重开预填 `1 MiB/s`（注册表落盘
        经 UI 回读证据）；②非法值 abc 确认：toast 逐字
        `⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）` + 对话框保持
        （焦点回下载限速字段，产品 dialogs.rs 拒绝路径）；③清空直接确认 =
        回填 config 任务级默认（task_max_download_speed = "2 MiB/s" → 预填
        `2 MiB/s`，v1.38 反转：原「留空保持」退役）；④任务限速 512（裸
        整数）重开预填显示 `512 B/s`（fmt 回显，非 1024 整倍数显裸 N B/s）。
        产品单测锚：modify_dialog_prefills_applies_and_rejects_completed /
        modify_confirm_empty_limit_backfills_config_default /
        modify_window_shows_bare_number_as_b_per_s（dialogs.rs）。
        """
        @self.case("QA-MT-10")
        def go(env: Env):
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # -- ① 键入规范化 + 确认生效 + 注册表落盘（UI 回读）--------
                add_task_via_dialog(
                    app, env, env.fixture.url("twelve-m.bin?speed=200000"))
                assert app.wait_for("下载中", 8), "任务应在下载中"
                app.pump(1.0)
                _open_modify(app)
                app.send("tab")            # 并发0 → 下载限速1（v1.38）
                app.type_text("1M")
                app.pump(0.3)
                assert re.search(r"下载限速\s*>\s*1 MiB/s", app.text()), \
                    "键入 1M 应即规范化显示 1 MiB/s"
                _confirm(app, tabs=4)      # 下载限速1 → 确认5
                assert app.wait_for("任务参数已更新", 3), "限速修改应生效"
                assert_in("下载中", app.text(), "任务应保持下载中（不重启不排队）")
                _open_modify(app)
                assert re.search(r"下载限速\s*>\s*1 MiB/s", app.text()), \
                    "重开应预填 1 MiB/s（注册表落盘 UI 回读）"
                # -- ② 非法值确认被拒（toast 逐字 + 对话框保持）-----------
                app.send("tab")            # 重开焦点回并发0 → 下载限速1
                for _ in range(16):
                    app.send("bs")
                app.type_text("abc")
                app.send("enter")          # 下载限速1 Enter = 确认路径
                assert app.wait_for("限速值非法", 3), "非法值应被拒"
                assert_in("⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）",
                          app.text(), "toast 应逐字匹配 v1.37 统一文案")
                assert_in("修改任务", app.text(), "被拒后对话框应保持打开")
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()
            # -- ③ 清空确认回填 config 任务级默认（v1.38 反转）-----------
            env.write_config('[common]\ntask_max_download_speed = "2 MiB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            self._cur_app = app
            try:
                # speed=60000（每连接口径 ×4 = 240KB/s → 12.5s）：覆盖 ③④
                # 两相位；添加不自动选中新任务——down 显式选中（twelve-m
                # 恢复占行 1、three-m 行 2；已在末行时钳制原地，MT-01 臂2 同款）
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?speed=60000"),
                    save_dir=os.path.join(env.save_dir, "c"))
                assert app.wait_for("下载中", 8), "臂③任务应在下载中"
                app.send("down")
                app.pump(0.3)
                _open_modify(app)
                app.send("tab")            # → 下载限速1（预填 2 MiB/s）
                for _ in range(16):
                    app.send("bs")
                app.send("enter")          # 清空直接确认 = 回填 config 默认
                assert app.wait_for("任务参数已更新", 3), "清空确认应生效"
                _open_modify(app)
                assert re.search(r"下载限速\s*>\s*2 MiB/s", app.text()), \
                    "清空确认应回填 config 任务级默认（2 MiB/s，v1.38）"
                # -- ④ 裸整数 512 落库后 fmt 回显 -------------------------
                app.send("tab")            # 并发0 → 下载限速1
                for _ in range(16):
                    app.send("bs")
                app.type_text("512")
                app.pump(0.2)
                assert re.search(r"下载限速\s*>\s*512\b", app.text()), \
                    "输入态裸数字 512 不缀单位（输入态显示口径）"
                app.send(*(["tab"] * 4))   # 下载限速1 → 确认5
                app.send("enter")
                assert app.wait_for("任务参数已更新", 3), "512 落库应生效"
                _open_modify(app)
                assert re.search(r"下载限速\s*>\s*512 B/s", app.text()), \
                    "重开预填应 fmt 回显 512 B/s（非 1024 整倍数显裸 N B/s）"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

if __name__ == "__main__":
    suite = ModifyTaskSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
