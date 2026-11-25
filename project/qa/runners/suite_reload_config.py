#!/usr/bin/env python3
"""suite_reload_config.py — QA 套件 · 12-reload-config 配置热加载（期号 01，v1.57 可执行化）。

对应规程：project/qa/12-reload-config-qa.md（QA-RC-01..14，six-pack/QA 轮可执行化产物，
承接 hardender 轮移交欠账「12-reload-config 产品面 handler 可执行化」）。
端到端 UI 层验证：Ctrl+R 键注入（\\x12，与 Ctrl+C 同级全局分派）+ toast/徽标/详情断言
+ config.toml 运行期外部改写（与运行中 ezr 无 IPC，热加载唯一入口 = 键注入）
+ fixture/代理/tracker 日志对账；不使用项目内部 API。

判据口径（规程 v1.57/D81②）：成功 toast 恒为固定文案「✓ 配置已重新加载」，任何配置项
改动均不追加可选段（N/K/M 段全部退役）；失败臂「⚠ 配置热加载失败：<明细>」；
无主目录臂「无配置文件」。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

偏差登记（engineering.md「计划的估计数字与实测不符时以实测为准」）：
- 规程示例数字「队列栏 0/3」（RC-01/RC-03）按页签行「下载槽位 used/max」实测口径断言：
  RC-01/RC-03 常驻下载中任务 used=1（断言上限分量即时生效与逐次变化，used 分量按实测），
  RC-11 无任务场景 used=0 按字面 0/5 断言；RC-03 的「0/3」语义 = 0 等待 + 3 满槽
  （断言「排队第」行消失 + 下载槽位 3/3）。
- RC-07 max_retries 烘焙值 UI 观察通道 = 失败行 N/M 计数（1/9），块大小/并发经
  详情分块行与并发徽标 y 断言。

时序纪律：toast 3s 自动消失（D36），Ctrl+R 后在采样帧内捕获页脚；断言锚定最终态
常驻文本（徽标/详情/任务行）；崩溃重启以落盘注册表断言（registry.json 落库直读）。
"""

from __future__ import annotations

import os
import re
import sys
import time

QA_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, QA_DIR)

from harness import (  # noqa: E402
    MB, Env, EzrApp, Suite, TMP_ROOT, DUMMY_URL, QA_FILE_SIZES,
    add_task_via_dialog, add_task_via_proxy, assert_in,
    assert_not_in, conns_badge, detail_text, expected_content, open_add_dialog_via_cmd,
    proxy_dropdown_options, proxy_reqs, proxies_toml, read_file, registry_tasks,
    select_task_by_needle, start_proxy, stop_proxy,
)
from suite_bt_engine import (  # noqa: E402
    FILES, FSIZE, LtPeer, MiniTracker, _disk_file, _make_payload, _make_torrent,
    _tracker_log, _tab_to_button,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]
TWO_M = QA_FILE_SIZES["two-m.bin"]

TOAST_OK = "✓ 配置已重新加载"
TOAST_FAIL = "⚠ 配置热加载失败"
TOAST_NO_CFG = "无配置文件"
CTRL_R = "\x12"          # crossterm CONTROL+Char('r')，与 Ctrl+C 同级全局分派
SLOW = "?speed=300000"   # 门控速度：观察窗内任务不瞬时完成（toast 不被完成提示覆盖）


def _reload(app: EzrApp, env: Env, body: str, expect: str = TOAST_OK) -> None:
    """外部改写 config.toml + Ctrl+R + toast 断言（热加载唯一入口 = 键注入）。"""
    env.write_config(body)
    app.send(CTRL_R)
    assert app.wait_for(expect, 3), f"reload toast 未出现（期望 {expect}）"


def _slots(app: EzrApp) -> "tuple[int, int] | None":
    """页签行「下载槽位 used/max」（header.rs 渲染，used=占槽任务数）。"""
    m = re.search(r"下载槽位 (\d+)/(\d+)", app.text())
    return (int(m.group(1)), int(m.group(2))) if m else None


def _detail_dl(app: EzrApp) -> float:
    """选中任务详情「大小 X / Y」已下载字节（十进制；无读数 0.0，TP-01 权威量法）。

    目标文件创建即稀疏预分配全量（set_len），盘上尺寸不随进度增长——
    进度唯一可靠观测量 = 详情面板已下载字节（suite_throttle_proxy 同款）。
    """
    d = "\n".join(r[64:].rstrip() for r in app.screen.display)
    m = re.search(r"大小\s+([0-9.]+) (B|KB|MB|GB) /", d)
    if not m:
        return 0.0
    mult = {"B": 1, "KB": 1000, "MB": 1_000_000, "GB": 1_000_000_000}
    return float(m.group(1)) * mult[m.group(2)]


def _wait_detail_dl(app: EzrApp, target: float, timeout: float = 40.0) -> None:
    """轮询详情已下载字节直至 ≥ target（窗口期持续泵 PTY）。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        app.pump(0.1)
        if _detail_dl(app) >= target:
            return
    raise AssertionError(
        f"{timeout}s 内详情已下载未达 {target}（当前 {_detail_dl(app)}）")


def _avg_rate(app: EzrApp, window_s: float) -> float:
    """详情已下载字节增速（B/s）：窗口期持续泵 PTY（防阻塞式轮询塞满输出缓冲）。

    带宽注记：fixture ?speed 门控为每连接独立限速（多连接聚合速率 ≈ N×门控），
    全局/任务级限速由令牌桶聚合钳制；短窗口受块尾效应与显示量化影响读数偏低，
    断言窗口 ≥8s、带宽 ±15%（相邻限速档位相差 ≥2×，判别力不减）。
    """
    s0 = _detail_dl(app)
    t0 = time.time()
    while time.time() - t0 < window_s:
        app.pump(0.1)
        time.sleep(0.02)
    dt = max(time.time() - t0, 1e-6)
    return (_detail_dl(app) - s0) / dt


def _wait_file_content(path: str, size: int, app: EzrApp,
                       timeout: float = 90.0) -> None:
    """字节级完成锚（泵轮询）：产物与 fixture 模式逐字节一致即完成。"""
    want = expected_content(size)
    deadline = time.time() + timeout
    while time.time() < deadline:
        app.pump(0.2)
        data = read_file(path)
        if data is not None and data == want:
            return
    raise AssertionError(f"{timeout}s 内产物未逐字节一致: {path}")


def _make_payload2(root: str) -> str:
    """第二份 BT payload（同布局异内容 → 异 infohash，t2 任务身份独立）。"""
    pay = os.path.join(root, "payload2")
    lay = os.path.join(pay, "root", "root")
    os.makedirs(lay, exist_ok=True)
    for name in FILES:
        with open(os.path.join(lay, name), "wb") as f:
            f.write(bytes((i * 7 + 11) % 251 for i in range(FSIZE)))
    return pay


class ReloadConfigSuite(Suite):
    def __init__(self):
        super().__init__("12-reload-config", port_base=47000)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in range(1, 15):
            getattr(self, f"_case_{i:02d}")()
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

    def _new_app(self, env: Env, rows: int = 44,
                 env_extra: "dict[str, str | None] | None" = None) -> EzrApp:
        app = EzrApp(env.home, save_dir=env.save_dir, rows=rows,
                     env_extra=env_extra)
        self._cur_app = app
        return app

    def _add_bt(self, app: EzrApp, torrent: str) -> None:
        """命令窗口 A → 种子路径 → BT 对话框 → 立即下载（用户可达唯一入口）。"""
        assert app.wait_for("下载槽位", 12), "应用应启动就绪"
        app.send("a")
        assert app.wait_for("请输入命令或链接", 5)
        app.type_text(torrent)
        app.send("enter")
        assert app.wait_for("添加任务：BT", 6)
        _tab_to_button(app, "立即下载")
        app.send("enter")
        assert app.wait_for("已添加任务", 6)

    def _registry_wait(self, app: EzrApp, env: Env, needle: str, want: str,
                       timeout: float = 6.0) -> None:
        """落盘注册表断言（带泵轮询窗）：等 save_registry 落库后 proxy == want。

        轮询期间持续泵 PTY（阻塞式等待会塞满输出缓冲使被测应用停摆）。
        """
        deadline = time.time() + timeout
        last = None
        while time.time() < deadline:
            app.pump(0.1)
            try:
                t = next((t for t in registry_tasks(env.home)
                          if needle in t["url"]), None)
                if t is not None:
                    last = t.get("proxy")
                    if last == want:
                        return
            except Exception:
                pass
            time.sleep(0.1)
        raise AssertionError(
            f"注册表 {timeout}s 内未落库 {want}（needle={needle}, proxy={last}）")

    # -- QA-RC-01 -------------------------------------------------------------
    def _case_01(self):
        """主界面与全部浮层（添加/帮助/命令窗）内 Ctrl+R 全局可达、浮层不关、槽位即时生效。"""
        @self.case("QA-RC-01")
        def go(env: Env):
            env.write_config("[common]\ndownload_slots = 5\n")
            app = self._new_app(env)
            try:
                assert app.wait_for("EZR Downloader", 6)
                add_task_via_dialog(app, env, env.fixture.url(f"five-m.bin{SLOW}"))
                assert app.wait_for("下载中", 8)
                assert_in("下载槽位 1/5", app.text(), "初值 5")

                _reload(app, env, "[common]\ndownload_slots = 3\n")
                assert app.wait_for("下载槽位 1/3", 3), "主界面 reload 即时生效"

                open_add_dialog_via_cmd(app, DUMMY_URL)
                _reload(app, env, "[common]\ndownload_slots = 8\n")
                assert app.wait_for("下载槽位 1/8", 3), "添加对话框内 reload 生效"
                assert_in("添加任务：HTTP(S)", app.text(), "浮层不关闭")
                app.send("esc")
                app.pump(0.3)

                app.send("h")
                assert app.wait_for("【全局】", 3)
                _reload(app, env, "[common]\ndownload_slots = 4\n")
                assert app.wait_for("下载槽位 1/4", 3), "帮助窗口内 reload 生效"
                assert_in("【全局】", app.text(), "帮助窗不关闭")
                app.send("esc")
                app.pump(0.3)

                app.send("A")
                assert app.wait_for("请输入命令或链接", 3)
                _reload(app, env, "[common]\ndownload_slots = 6\n")
                assert app.wait_for("下载槽位 1/6", 3), "命令窗口内 reload 生效"
                assert_in("请输入命令或链接", app.text(), "命令窗不关闭")
                app.send("esc")
                app.pump(0.3)

                assert_in("下载中", app.text(), "运行中任务不中断")
                assert_not_in("已失败", app.text())
            finally:
                app.graceful_quit()

    # -- QA-RC-02 -------------------------------------------------------------
    def _case_02(self):
        """global_max_download_speed 热加载：运行中任务立即按新限速重定目标且不中断。"""
        @self.case("QA-RC-02")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = 0\n')
            app = self._new_app(env)
            try:
                add_task_via_dialog(app, env,
                                    env.fixture.url("big-100m.bin?speed=2000000"))
                assert app.wait_for("下载中", 8)
                _wait_detail_dl(app, 8 * MB, 60)  # 越过启动瞬态
                r1 = _avg_rate(app, 5.0)
                assert r1 >= 1_600_000, \
                    f"门控 2 MB/s 基线实测 {r1/1e6:.2f} MB/s"
                _reload(app, env, '[common]\nglobal_max_download_speed = "512 KiB/s"\n')
                _wait_detail_dl(app, _detail_dl(app) + 3 * MB, 60)
                r2 = _avg_rate(app, 15.0)
                assert 430_000 <= r2 <= 600_000, \
                    f"热加载后应按 512 KiB/s 重定目标: {r2/1000:.0f} KB/s"
                assert_not_in("s 后重试", app.text(), "无失败重试")
                assert_not_in("已失败", app.text(), "任务不中断")
            finally:
                app.graceful_quit()

    # -- QA-RC-03 -------------------------------------------------------------
    def _case_03(self):
        """download_slots 热加载双向：等待任务按列表顺序立即启动；收缩不抢占、不回收。

        任务节奏注记：fixture ?speed 门控为每连接独立限速（块大小 1MB，四连接聚合
        ≈ N×门控），门控取 80 KB/s 拉长单任务生命周期，保证 reload 时队列深度
        （1 下载 2 等待）与 3/3 满槽窗口 ≥10s 可稳定观察。
        """
        @self.case("QA-RC-03")
        def go(env: Env):
            env.write_config("[common]\ndownload_slots = 1\n")
            app = self._new_app(env)
            try:
                assert app.wait_for("EZR Downloader", 6)
                for name in ("five-m.bin", "one-m.bin", "half-m.bin"):
                    add_task_via_dialog(app, env,
                                        env.fixture.url(f"{name}?speed=80000"))
                assert app.wait_for("下载槽位 1/1", 8), "1 下载"
                assert app.wait_for("排队第", 10), "2 等待（排队位次）"

                _reload(app, env, "[common]\ndownload_slots = 3\n")
                assert app.wait_for("下载槽位 3/3", 15), "等待任务按列表顺序立即启动"
                assert_not_in("排队第", app.text(), "0 等待（规程 0/3 语义）")

                _reload(app, env, "[common]\ndownload_slots = 1\n")
                text = app.text()
                assert text.count("下载中") >= 3, "3 个运行任务均不被抢占"
                assert_not_in("已失败", text)

                assert app.wait_for("已完成 (1)", 60), "等待某任务自然结束"
                text = app.text()
                assert text.count("下载中") >= 2, "其余任务继续下载"
                assert _slots(app) == (2, 1), \
                    f"空槽不补位且不回收（used 2 > max 1）: {_slots(app)}"
                assert_not_in("排队第", text, "无等待任务自动启动")
            finally:
                app.graceful_quit()

    # -- QA-RC-04 -------------------------------------------------------------
    def _case_04(self):
        """auto_retry 两相位热加载：首次失败按初值语义，热加载后下一次失败按新值语义。

        断连参数注记：disconnect=65536（块内中截，截断点 < 残余片长恒为网络错误）
        ——disconnect ≥ 残余片长时截断落在响应体末尾，请求完成与对端重置竞速，
        失败判定非确定（实测 disconnect=1000000 重试轮可静默完成）；有进展重置
        计数，不触发 max_retries 上限。
        """
        @self.case("QA-RC-04")
        def go(env: Env):
            env.write_config("[http]\nauto_retry = true\n")
            app = self._new_app(env)
            try:
                add_task_via_dialog(app, env,
                                    env.fixture.url("five-m.bin?disconnect=65536"))
                assert app.wait_for("s 后重试", 20), \
                    "初值 true：自动重试倒计时语义"

                _reload(app, env, "[http]\nauto_retry = false\n")
                assert app.wait_for("不自动重试", 40), \
                    "热加载后下一次失败判定按 false 语义（直接停等无倒计时）"

                _reload(app, env, "[http]\nauto_retry = true\n")
                app.send("r")
                assert app.wait_for("s 后重试", 25), \
                    "热加载后重试路径按 true 语义恢复自动倒计时"
            finally:
                app.graceful_quit()

    # -- QA-RC-05 -------------------------------------------------------------
    def _case_05(self):
        """命名代理端点热切：存量连接不打断、新连接走新端点、任务完成字节完整。"""
        @self.case("QA-RC-05")
        def go(env: Env):
            pa = start_proxy(env, "rc05-p1", self._next_port())
            pb = start_proxy(env, "rc05-p2", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "p1", "type": "http",
                     "ip": "127.0.0.1", "port": pa["port"]},
                ]))
                app = self._new_app(env)
                try:
                    add_task_via_proxy(app, env,
                                       env.fixture.url(f"five-m.bin{SLOW}"), 1)
                    assert app.wait_for("下载中", 8)
                    deadline = time.time() + 20
                    while time.time() < deadline and not any(
                            "five-m" in r["target"] for r in proxy_reqs(pa)):
                        app.pump(0.2)
                    assert any("five-m" in r["target"] for r in proxy_reqs(pa)), \
                        "p1 端点应在案"
                    assert not any("five-m" in r["target"] for r in proxy_reqs(pb)), \
                        "p2 端点切换前无流量"

                    _reload(app, env, proxies_toml([
                        {"name": "p1", "type": "http",
                         "ip": "127.0.0.1", "port": pb["port"]},
                    ]))
                    deadline = time.time() + 30
                    while time.time() < deadline and not any(
                            "five-m" in r["target"] for r in proxy_reqs(pb)):
                        app.pump(0.2)
                    assert any("five-m" in r["target"] for r in proxy_reqs(pb)), \
                        "新端点连接来源应在案（代理 fixture 日志为唯一事实来源）"
                    assert_not_in("s 后重试", app.text(), "存量连接不打断")
                    _wait_file_content(os.path.join(env.save_dir, "five-m.bin"),
                                       FIVE_M, app)
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(pa)
                stop_proxy(pb)

    # -- QA-RC-06 -------------------------------------------------------------
    def _case_06(self):
        """引用消失臂（删除 p1）：任务落库转直连、下拉重建无旧名、重启不复活、重加不复挂。

        时序注记：修改框检查全程在任务 a 下载中打开（'m' 对已完成任务不开修改框，
        实测 20261125）；任务 a 门控 20 KB/s（四连接聚合 ≈80 KB/s，5 MB ≈ 62.5s），
        覆盖两段会话全部检查窗；直连继续判据 = fixture 直连拨号在案 + 无失败转态
        （规程不要求 a 在用例内完成，字节完整性由 QA-RC-05 承载）。
        """
        @self.case("QA-RC-06")
        def go(env: Env):
            pa = start_proxy(env, "rc06-p1", self._next_port())
            pb = start_proxy(env, "rc06-p2", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "p1", "type": "http",
                     "ip": "127.0.0.1", "port": pa["port"]},
                    {"name": "p2", "type": "http",
                     "ip": "127.0.0.1", "port": pb["port"]},
                ]))
                app = self._new_app(env)
                try:
                    sub_a = os.path.join(env.save_dir, "a")
                    sub_b = os.path.join(env.save_dir, "b")
                    add_task_via_proxy(
                        app, env, env.fixture.url("big-100m.bin?speed=300000"), 1,
                        save_dir=sub_a)
                    add_task_via_proxy(
                        app, env, env.fixture.url("two-m.bin?speed=80000"), 2,
                        save_dir=sub_b)
                    assert app.text().count("下载中") >= 2, "两任务均下载中"
                    deadline = time.time() + 20
                    while time.time() < deadline and not any(
                            "big-100m" in r["target"] for r in proxy_reqs(pa)):
                        app.pump(0.2)
                    assert any("big-100m" in r["target"] for r in proxy_reqs(pa)), \
                        "p1 端点应在案（任务 a）"

                    _reload(app, env, proxies_toml([
                        {"name": "p2", "type": "http",
                         "ip": "127.0.0.1", "port": pb["port"]},
                    ]))
                    assert_not_in("引用的代理已删除", app.text(),
                                  "引用消失不追加专属提示段（D82② M 段退役）")
                    self._registry_wait(app, env, "big-100m", "direct")
                    n_fix = len(env.fixture.requests())
                    deadline = time.time() + 30
                    while time.time() < deadline and not any(
                            "big-100m" in r.get("path", "")
                            for r in env.fixture.requests()[n_fix:]):
                        app.pump(0.2)
                    assert any("big-100m" in r.get("path", "")
                               for r in env.fixture.requests()[n_fix:]), \
                        "fixture 直连来源应在案（任务 a 重拨直连）"
                    assert_not_in("s 后重试", app.text(), "无失败转态")
                    assert_not_in("已失败", app.text())
                    assert_in("下载中", app.text(), "任务 a 直连继续")

                    text = proxy_dropdown_options(app)
                    assert_in("直连", text)
                    assert_in("p2（http）", text, "下拉 = 直连 + p2")
                    assert_not_in("p1（http）", text, "p1 不再出现")

                    select_task_by_needle(app, "big-100m")
                    app.send("m")
                    assert app.wait_for("修改任务", 3), "修改对话框未打开"
                    text = app.text()
                    assert_in("直连", text, "修改框代理预填直连")
                    assert_not_in("p1（http）", text, "p1 不被重新注入")
                    app.send("esc")
                    app.pump(0.3)
                finally:
                    app.graceful_quit()

                app = self._new_app(env)
                try:
                    assert app.wait_for("下载中", 10), "重启任务还原"
                    self._registry_wait(app, env, "big-100m", "direct")
                    select_task_by_needle(app, "big-100m")
                    app.send("m")
                    assert app.wait_for("修改任务", 3), "重启后修改框未打开"
                    text = app.text()
                    assert_in("直连", text, "重启复查预填直连、旧名不复活")
                    assert_not_in("p1（http）", text)
                    app.send("esc")
                    app.pump(0.3)

                    _reload(app, env, proxies_toml([
                        {"name": "p1", "type": "http",
                         "ip": "127.0.0.1", "port": pa["port"]},
                        {"name": "p2", "type": "http",
                         "ip": "127.0.0.1", "port": pb["port"]},
                    ]))
                    text = proxy_dropdown_options(app)
                    assert_in("p1（http）", text, "重加 p1 重现两对话框下拉")

                    select_task_by_needle(app, "big-100m")
                    app.send("m")
                    assert app.wait_for("修改任务", 3), "重加后修改框未打开"
                    assert_in("直连", app.text(), "重加后任务保持直连不复挂")
                    app.send("esc")
                    app.pump(0.3)

                    app.send(CTRL_R)
                    assert app.wait_for(TOAST_OK, 3), "第二次 Ctrl+R 固定文案"
                    assert_not_in("引用的代理已删除", app.text())
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(pa)
                stop_proxy(pb)

    # -- QA-RC-07 -------------------------------------------------------------
    def _case_07(self):
        """B 类快照键（block_size_http/http_concurrency/max_retries）：存量烘焙、新任务新值。"""
        @self.case("QA-RC-07")
        def go(env: Env):
            env.write_config(
                "[common]\n"
                f'download_dir = "{env.save_dir}"\n'
                "\n[http]\n"
                "block_size_http = 1048576\n"
                "http_concurrency = 4\n"
                "max_retries = 5\n"
            )
            app = self._new_app(env)
            try:
                add_task_via_dialog(app, env, env.fixture.url(f"five-m.bin{SLOW}"))
                assert app.wait_for("下载中", 8)
                select_task_by_needle(app, "five-m")
                assert_in("1 MB/块", detail_text(app), "任务 a 块大小烘焙值")
                b = conns_badge(app)
                assert b is not None and b[1] == 4, \
                    f"任务 a 并发面板 y=4: {b}"

                _reload(app, env,
                        "[common]\n"
                        f'download_dir = "{env.save_dir}"\n'
                        "\n[http]\n"
                        "block_size_http = 2097152\n"
                        "http_concurrency = 8\n"
                        "max_retries = 9\n")

                add_task_via_dialog(app, env,
                                    env.fixture.url("three-m.bin?disconnect=1200000"))
                assert app.wait_for("1/9", 20), "任务 b 重试上限按新值烘焙（失败行 N/M）"
                select_task_by_needle(app, "three-m")
                assert_in("2 MB/块", detail_text(app), "任务 b 块大小按新值")
                b = conns_badge(app)
                assert b is not None and b[1] == 8, \
                    f"任务 b 并发面板 y=8: {b}"

                select_task_by_needle(app, "five-m")
                assert_in("1 MB/块", detail_text(app), "任务 a 维持烘焙值")

                open_add_dialog_via_cmd(app, DUMMY_URL)
                assert re.search(r"并发\s*>\s*8\b", app.text()), "添加对话框并发预填 8"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    # -- QA-RC-08 -------------------------------------------------------------
    def _case_08(self):
        """C 类键（throttle_burst_secs/listen_port）：热加载固定文案无重启段、监听启动绑定一次。"""
        @self.case("QA-RC-08")
        def go(env: Env):
            tr = MiniTracker(self._next_port(),
                             os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
            seeder = None
            app = None
            try:
                assert tr.wait_ready()
                log_path = os.path.join(env.fixture_root, "tr.jsonl")
                pay = _make_payload(env.fixture_root)
                torrent = _make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
                seeder = LtPeer("rc08-seed", self._next_port(), torrent, pay)
                assert seeder.wait_total_done(4 * FSIZE, 25), "seeder 校验超时"
                def bt_cfg(burst: str = "0.5", port: int = 51444) -> str:
                    return (
                        "[common]\n"
                        f'download_dir = "{env.save_dir}"\n'
                        "task_max_download_speed = 200000\n"
                        "\n[http]\n"
                        f"throttle_burst_secs = {burst}\n"
                        "\n[bt]\n"
                        f'trackers = ["http://127.0.0.1:{tr.port}/announce"]\n'
                        f"listen_port = {port}\n"
                    )
                env.write_config(bt_cfg())
                app = self._new_app(env)
                self._add_bt(app, torrent)
                assert app.wait_for("下载中", 12)
                deadline = time.time() + 20
                rows = []
                while time.time() < deadline:
                    rows = [r for r in _tracker_log(log_path)
                            if r["peer_id"].startswith("-EZ2")]
                    if rows and any(r["port"] == 51444 for r in rows):
                        break
                    app.pump(0.3)
                assert rows and any(r["port"] == 51444 for r in rows), \
                    f"t1 announce 来源 = 51444: {[r['port'] for r in rows]}"

                _reload(app, env, bt_cfg(burst="1.5"))
                assert_not_in("重启", app.text(), "C 类改动不再呈现重启提示段（D81③）")

                src = lambda n: os.path.join(pay, "root", "root", n)
                deadline = time.time() + 120
                done = False
                while time.time() < deadline:
                    if all(_disk_file(env.save_dir, n) and
                           open(_disk_file(env.save_dir, n), "rb").read() ==
                           open(src(n), "rb").read() for n in FILES):
                        done = True
                        break
                    app.pump(0.5)
                assert done, "t1 传输无异常（产物逐字节一致）"

                _reload(app, env, bt_cfg(burst="1.5", port=51555))
                app.send(CTRL_R)
                assert app.wait_for(TOAST_OK, 3), "第三次 Ctrl+R（配置未变）固定文案"
                assert_not_in("重启", app.text(), "无任何可选段")

                n0 = len(_tracker_log(log_path))
                pay2 = _make_payload2(env.fixture_root)
                torrent2 = _make_torrent(pay2, f"http://127.0.0.1:{tr.port}/announce")
                self._add_bt(app, torrent2)
                deadline = time.time() + 30
                new_started = []
                while time.time() < deadline:
                    new_started = [r for r in _tracker_log(log_path)[n0:]
                                   if r["event"] == "started"]
                    if new_started:
                        break
                    app.pump(0.5)
                assert new_started, "t2 应发起 started announce"
                assert all(r["port"] == 51444 for r in new_started), \
                    f"listen_port 启动绑定一次（t2 announce 来源仍 51444）: " \
                    f"{[r['port'] for r in new_started]}"
            finally:
                if app is not None:
                    app.graceful_quit()
                if seeder is not None:
                    seeder.shutdown()
                tr.stop()

    # -- QA-RC-09 -------------------------------------------------------------
    def _case_09(self):
        """D 类失败保护：任一键非法整体拒绝、旧配置逐通道原样保留、修正后生效。

        终端口径注记：逐键文案判据要求完整键点路径（common.download_slots）可见，
        而页脚 toast 在窄终端被渲染层截断（120 列下实测仅见「键 common」）——
        本用例以 160 列启动（产品实证支持范围 90–190 列，v1.48 注记），
        其余用例仍为规程默认 120×44。
        """
        @self.case("QA-RC-09")
        def go(env: Env):
            env.write_config('[common]\n'
                             'download_slots = 5\n'
                             'global_max_download_speed = "1 MiB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44, cols=160)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, env.fixture.url("big-100m.bin"))
                assert app.wait_for("下载中", 8)
                _wait_detail_dl(app, 6 * MB, 60)
                r0 = _avg_rate(app, 8.0)
                assert 820_000 <= r0 <= 1_300_000, \
                    f"初值 1 MiB/s 基线: {r0/1e6:.2f} MB/s"

                _reload(app, env, '[common]\n'
                                  'download_slots = "abc"\n'
                                  'global_max_download_speed = "1 MiB/s"\n',
                        expect=TOAST_FAIL)
                # 逐键文案断言：页脚 toast 含完整键点路径（160 列不截断）。
                text = app.text()
                assert_in("common.download_slots", text, "逐键文案语义")
                assert _slots(app) == (1, 5), f"槽位旧值保留: {_slots(app)}"
                r1 = _avg_rate(app, 8.0)
                assert 820_000 <= r1 <= 1_300_000, \
                    f"限速旧值保留: {r1/1e6:.2f} MB/s"
                assert_in("下载中", app.text(), "无半新半旧中间态")

                _reload(app, env, '[common]\n'
                                  'download_slots = "abc"\n'
                                  'global_max_download_speed = "2 MiB/s"\n',
                        expect=TOAST_FAIL)
                assert_in("common.download_slots", app.text(), "整体拒绝")
                r2 = _avg_rate(app, 8.0)
                assert 820_000 <= r2 <= 1_300_000, \
                    f"合法键也不生效: {r2/1e6:.2f} MB/s"

                _reload(app, env, '[common]\n'
                                  'download_slots = 3\n'
                                  'global_max_download_speed = "2 MiB/s"\n')
                assert app.wait_for("下载槽位 1/3", 3), "全部合法后槽位生效"
                _wait_detail_dl(app, _detail_dl(app) + 4 * MB, 60)
                r3 = _avg_rate(app, 10.0)
                assert 1_700_000 <= r3 <= 2_400_000, \
                    f"全部合法后限速生效: {r3/1e6:.2f} MB/s"
            finally:
                app.graceful_quit()

    # -- QA-RC-10 -------------------------------------------------------------
    def _case_10(self):
        """CLI --max-speed 覆盖贯穿热加载：文件值改动不冲掉 CLI 值。"""
        @self.case("QA-RC-10")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = "1 MiB/s"\n')
            app = EzrApp(env.home, args=["--max-speed", "256 KiB/s"],
                         save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                add_task_via_dialog(app, env, env.fixture.url("big-100m.bin"))
                assert app.wait_for("下载中", 8)
                _wait_detail_dl(app, 3 * MB, 90)
                r1 = _avg_rate(app, 10.0)
                assert 215_000 <= r1 <= 310_000, \
                    f"CLI 256 KiB/s 基线: {r1/1000:.0f} KB/s"
                _reload(app, env,
                        '[common]\nglobal_max_download_speed = "2 MiB/s"\n')
                r2 = _avg_rate(app, 10.0)
                assert 215_000 <= r2 <= 310_000, \
                    f"热加载后仍 CLI 256 KiB/s（不被文件值冲掉）: {r2/1000:.0f} KB/s"
                assert_not_in("s 后重试", app.text())
                assert_not_in("已失败", app.text())
            finally:
                app.graceful_quit()

    # -- QA-RC-11 -------------------------------------------------------------
    def _case_11(self):
        """删除 config.toml 后热加载：默认配置整体生效、模板不重新生成、代理下拉仅直连。"""
        @self.case("QA-RC-11")
        def go(env: Env):
            env.write_config(
                "[common]\n"
                "download_slots = 2\n"
                "\n[[common.proxy]]\n"
                'name = "p1"\n'
                'type = "http"\n'
                'ip = "127.0.0.1"\n'
                "port = 1\n"
            )
            app = self._new_app(env)
            try:
                assert app.wait_for("下载槽位 0/2", 8), "初值 2（无任务 used=0）"
                cfg = env.config_path()
                os.remove(cfg)
                app.send(CTRL_R)
                assert app.wait_for(TOAST_OK, 3)
                assert app.wait_for("下载槽位 0/5", 3), "默认配置整体生效"
                assert not os.path.exists(cfg), "config.toml 未被重新生成（D81①）"
                text = proxy_dropdown_options(app)
                assert_in("直连", text)
                assert_not_in("p1（http）", text, "代理下拉仅剩直连")
            finally:
                app.graceful_quit()

    # -- QA-RC-12 -------------------------------------------------------------
    def _case_12(self):
        """无主目录环境：Ctrl+R toast「无配置文件」、应用不退出、配置保持默认可继续操作。"""
        @self.case("QA-RC-12")
        def go(env: Env):
            app = EzrApp(env.home, rows=44, env_extra={
                "HOME": None, "USERPROFILE": None, "EZR_HOME": None,
            })
            self._cur_app = app
            try:
                assert app.wait_for("EZR Downloader", 6), "无主目录启动不崩溃"
                app.send(CTRL_R)
                assert app.wait_for(TOAST_NO_CFG, 4), "toast 无配置文件"
                app.pump(1.5)
                assert_in("EZR Downloader", app.text(), "应用不退出")
                app.send("A")
                assert app.wait_for("请输入命令或链接", 4), "任务列表可继续操作"
                app.send("esc")
                app.pump(0.3)
            finally:
                app.graceful_quit()

    # -- QA-RC-13 -------------------------------------------------------------
    def _case_13(self):
        """帮助窗口【全局】组 Ctrl+R 条目 + 帮助打开时可热加载且窗口不关闭。"""
        @self.case("QA-RC-13")
        def go(env: Env):
            app = self._new_app(env)
            try:
                assert app.wait_for("EZR Downloader", 6)
                app.send("h")
                assert app.wait_for("【全局】", 3)
                assert_in("重新加载配置文件（热加载，无需重启）", app.text(),
                          "【全局】组含 Ctrl+R 条目")
                app.send("pgdn", "pgdn")
                app.pump(0.3)
                assert_not_in("重新加载配置文件（热加载，无需重启）", app.text(),
                              "帮助窗口可滚动（条目滚出视口）")
                app.send("home")
                app.pump(0.3)
                assert_in("重新加载配置文件（热加载，无需重启）", app.text(),
                          "滚动还原后条目仍在（窗口尺寸不变）")
                app.send(CTRL_R)
                assert app.wait_for(TOAST_OK, 3), "帮助打开时 Ctrl+R 触发热加载"
                assert_in("【全局】", app.text(), "帮助窗口不关闭")
                app.send("esc")
                app.pump(0.3)
                assert app.wait_gone("【全局】", 3), "Esc 关闭帮助窗"
            finally:
                app.graceful_quit()

    # -- QA-RC-14 -------------------------------------------------------------
    def _case_14(self):
        """引用消失臂（改名 p1→p1x→p1）：落库直连、浮层内热加载下拉即时重建、不自动复挂。

        时序注记：与 QA-RC-06 同口径——任务 a 门控 20 KB/s 覆盖两段会话检查窗，
        修改框在下载中打开且浮层保持跨越第二次热加载；改名回 p1 后浮层内下拉
        即时重建（v1.59/D82③）。
        """
        @self.case("QA-RC-14")
        def go(env: Env):
            pa = start_proxy(env, "rc14-p1", self._next_port())
            pb = start_proxy(env, "rc14-p2", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "p1", "type": "http",
                     "ip": "127.0.0.1", "port": pa["port"]},
                    {"name": "p2", "type": "http",
                     "ip": "127.0.0.1", "port": pb["port"]},
                ]))
                app = self._new_app(env)
                try:
                    sub_a = os.path.join(env.save_dir, "a")
                    sub_b = os.path.join(env.save_dir, "b")
                    add_task_via_proxy(
                        app, env, env.fixture.url("big-100m.bin?speed=300000"), 1,
                        save_dir=sub_a)
                    add_task_via_proxy(
                        app, env, env.fixture.url("two-m.bin?speed=80000"), 2,
                        save_dir=sub_b)
                    assert app.text().count("下载中") >= 2, "两任务均下载中"
                    deadline = time.time() + 20
                    while time.time() < deadline and not any(
                            "big-100m" in r["target"] for r in proxy_reqs(pa)):
                        app.pump(0.2)
                    assert any("big-100m" in r["target"] for r in proxy_reqs(pa)), \
                        "p1 端点应在案（任务 a）"

                    _reload(app, env, proxies_toml([
                        {"name": "p1x", "type": "http",
                         "ip": "127.0.0.1", "port": pa["port"]},
                        {"name": "p2", "type": "http",
                         "ip": "127.0.0.1", "port": pb["port"]},
                    ]))
                    assert_not_in("引用的代理已删除", app.text(),
                                  "旧名消失不追加专属提示段（D82②③）")
                    self._registry_wait(app, env, "big-100m", "direct")
                    n_fix = len(env.fixture.requests())
                    deadline = time.time() + 30
                    while time.time() < deadline and not any(
                            "big-100m" in r.get("path", "")
                            for r in env.fixture.requests()[n_fix:]):
                        app.pump(0.2)
                    assert any("big-100m" in r.get("path", "")
                               for r in env.fixture.requests()[n_fix:]), \
                        "fixture 直连来源应在案（任务 a 重拨直连）"
                    assert_not_in("s 后重试", app.text())
                    assert_not_in("已失败", app.text())

                    text = proxy_dropdown_options(app)
                    assert_in("直连", text)
                    assert_in("p1x（http）", text, "p1x 按配置顺序出现")
                    assert_in("p2（http）", text)
                    assert_not_in("p1（http）", text, "p1 消失")

                    select_task_by_needle(app, "big-100m")
                    app.send("m")
                    assert app.wait_for("修改任务", 3), "修改对话框未打开"
                    assert_in("直连", app.text(), "修改框预填直连（无旧名）")

                    env.write_config(proxies_toml([
                        {"name": "p1", "type": "http",
                         "ip": "127.0.0.1", "port": pa["port"]},
                        {"name": "p2", "type": "http",
                         "ip": "127.0.0.1", "port": pb["port"]},
                    ]))
                    app.send(CTRL_R)
                    assert app.wait_for(TOAST_OK, 3), "浮层打开时热加载"
                    assert_in("修改任务", app.text(), "浮层不关")

                    app.send(*(["tab"] * 4))   # 并发(0)→限速(1)→校验(2)→校验码(3)→代理(4)
                    app.send("enter")
                    assert app.wait_for("选择代理", 3), "代理下拉未展开"
                    text = app.text()
                    assert_in("p1（http）", text, "下拉即时重建 = 直连 + p1 + p2")
                    assert_in("p2（http）", text)
                    assert_not_in("p1x（http）", text)
                    app.send("esc")
                    app.pump(0.2)
                    assert_in("直连", app.text(), "预填仍直连（不自动复挂，D82③）")
                    app.send("esc")
                    app.pump(0.3)
                finally:
                    app.graceful_quit()

                app = self._new_app(env)
                try:
                    assert app.wait_for("下载中", 10), "重启任务还原"
                    self._registry_wait(app, env, "big-100m", "direct")
                    select_task_by_needle(app, "big-100m")
                    app.send("m")
                    assert app.wait_for("修改任务", 3), "重启后修改框未打开"
                    assert_in("直连", app.text(), "重启复查预填直连")
                    assert_not_in("p1x", app.text())
                    app.send("esc")
                    app.pump(0.3)
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(pa)
                stop_proxy(pb)


if __name__ == "__main__":
    suite = ReloadConfigSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
