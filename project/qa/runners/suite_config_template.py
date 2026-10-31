#!/usr/bin/env python3
"""suite_config_template.py — QA 套件 · 01-config-template 配置模板自动生成（期号 01）。

对应规程：project/qa/01-config-template-qa.md（5 用例）。
端到端 UI 层验证：CLI/按键启动退出 + 磁盘产物观察（存在性/全注释/逐字节 cmp）+
TUI 默认值行为断言；不使用项目内部 API。
v1.10：模板内容契约基线 = 操作者钦定文本整文本（权威原文内嵌于
features/01-config-template.feature 场景 01 注释块）——本套件运行时从该文件
解析落盘基线文件（.work/tmp/qa-run/），按规程以 cmp 口径逐字节比对。
"""

from __future__ import annotations

import time

import os
import stat

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_in, assert_not_in,
    detail_text, wait_file_size, QA_FILE_SIZES,
)

PROJECT_DIR = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
FEATURE_PATH = os.path.join(PROJECT_DIR, "features", "01-config-template.feature")

FIVE_M = QA_FILE_SIZES["five-m.bin"]


def operator_template_text() -> str:
    """从 feature 场景 01 注释块解析操作者钦定模板原文（174 行，逐字节权威）。

    块起点 = 以「specifier v1.12）：」结尾的说明行；块终点 = 独立全角括号行
    「）」；内容行统一 2 空格缩进（feature 文件风格），空行即真实空行；
    EOF 以一个换行符收尾。
    """
    with open(FEATURE_PATH, encoding="utf-8") as f:
        lines = f.read().split("\n")
    start = next(i for i, l in enumerate(lines)
                 if l.rstrip().endswith("specifier v1.13）："))
    end = next(i for i, l in enumerate(lines) if i > start and l.strip() == "）")
    body = [l[2:] if l.startswith("  ") else l for l in lines[start + 1:end]]
    assert len(body) == 174, f"钦定原文应为 174 行，解析得 {len(body)}"
    for sep, decl in (("# ===== 公共配置 =====", "[common]"),
                      ("# ===== HTTP 引擎配置 =====", "[http]"),
                      ("# ===== BT 引擎配置 =====", "[bt]")):
        si = body.index(sep)
        assert body[si + 1] == "", f"{sep} 后应空行"
        assert body[si + 2] == decl, f"{sep} 后第二行应为表声明行 {decl}"
        assert body[si + 3] == "", "表声明行后应空行"
    assert not any(l.startswith("# ===== [") for l in body), "旧形态分隔头（含表名）退役"
    import re as _re
    assert not any(_re.match(r"^# [a-z_][a-z_0-9]*：", l) for l in body), \
        "v1.12：键注释首行不得再带键名前缀"
    active = [l for l in body if l.strip() and not l.startswith("#")]
    assert len(active) == 39, \
        f"v1.13 活跃行 = 36 键值 + 3 表声明，解析得 {len(active)}"
    assert sum(1 for l in body if l.startswith("# 取值范围")) == 36, \
        "v1.13：36 键取值范围注释保持"
    assert sum(1 for l in body if l.startswith("# [[common.proxy]]")) == 2, \
        "v1.13：proxy 双示例块保持注释（D63②）"
    return "\n".join(body) + "\n"


def write_baseline(env: Env) -> str:
    """钦定原文落盘为基线文件（规程环境前置：供 cmp 逐字节比对）。"""
    base = os.path.join(env.fixture_root, "config-template-baseline.toml")
    with open(base, "w", encoding="utf-8", newline="") as f:
        f.write(operator_template_text())
    return base


def cmp_bytes(a_path: str, b_path: str) -> bool:
    with open(a_path, "rb") as fa, open(b_path, "rb") as fb:
        return fa.read() == fb.read()


class ConfigTemplateSuite(Suite):
    def __init__(self):
        super().__init__("01-config-template", port_base=41900)

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list:
        for i in range(1, 6):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-CT-01 干净 HOME 启动：生成 v1.13 活跃模板，逐字节一致 + 解析等价默认。"""
        @self.case("QA-CT-01")
        def go(env: Env):
            baseline = write_baseline(env)
            cfg = env.config_path()
            assert not os.path.exists(cfg), "前置：配置文件应不存在"
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("EZR Downloader", 6), "启动画面"
                assert os.path.exists(cfg), "config.toml 应已生成"
                assert cmp_bytes(cfg, baseline), "模板与钦定文本应逐字节一致"
                with open(cfg, encoding="utf-8") as f:
                    body = f.read().split("\n")
                nonblank = [l for l in body if l.strip()]
                active = [l for l in nonblank if not l.startswith("#")]
                assert len(active) == 39, \
                    f"v1.13 活跃行 = 36 键值 + 3 表声明，实得 {len(active)}"
                # 36 键示例值与默认值口径逐一相符（v1.11 三表版抽验全集）
                text = "\n".join(body)
                for key_line in ("download_dir = \"\"",
                                 "download_slots = 5",
                                 "max_speed = 0",
                                 "block_size_http = 1048576",
                                 "http_concurrency = 4",
                                 "max_retries = 5",
                                 "auto_retry = true",
                                 "backoff_initial = 8.0",
                                 "backoff_cap = 60.0",
                                 "progress_tick_ms = 100",
                                 "sidecar_flush_secs = 2",
                                 "throttle_burst_secs = 0.5",
                                 "throttle_sleep_ms = 20",
                                 "dns_race_grace_ms = 1000",
                                 "http_trackers = []",
                                 "udp_trackers = []",
                                 "bt_concurrency = 20",
                                 "peer_pipeline = 64",
                                 "upload_slots = 4",
                                 "upload_read_concurrency = 128",
                                 "idle_eviction_secs = 60",
                                 "connect_timeout_secs = 3",
                                 "handshake_timeout_secs = 10",
                                 "request_timeout_secs = 20",
                                 "requests_per_piece = 4",
                                 "rechoke_interval_secs = 10",
                                 "optimistic_interval_secs = 30",
                                 "progress_emit_min_interval_ms = 250",
                                 "tracker_numwant = 200",
                                 "peer_retry_base_secs = 5",
                                 "peer_retry_cap_secs = 120",
                                 "tracker_backoff_start_secs = 8",
                                 "tracker_backoff_cap_secs = 60",
                                 "udp_base_timeout_secs = 15",
                                 "udp_retries = 3",
                                 "seed_duration_secs = 1800",
                                 "# [[common.proxy]]"):
                    assert_in(key_line, text, f"键示例行 {key_line}")
                assert_not_in("# [[proxies]]", text, "旧 [[proxies]] 表头退役")
                # v1.10 最小注释集：无凭证规则注释行、无解释性语义注释行
                assert_not_in("必填", text, "无凭证规则注释行（FR-01-93⑤ 废止）")
                assert_not_in("IPv6", text, "无 IPv6 语义注释行")
                assert_not_in("代理自身走 TLS", text, "无三类型语义注释行")
                # 行为与缺失文件一致（默认值生效）：槽位 5
                assert_in("下载槽位 0/5", app.text(), "默认槽位 5")
                # 添加任务验证并发 4 / 块 1 MB
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=900000"))
                assert app.wait_for("下载中", 8), "任务应下载（启动不受模板影响）"
                d = detail_text(app)
                # v1.14：详情类型行无并发数，默认并发 4 经头部数据面字段核验
                assert app.wait_for("并发 4", 15), "默认并发 4（头部数据面）"
                assert_in("1 MB/块", d, "默认块 1 MB")
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-CT-02 已有合法配置不重写：字节与 mtime 不变 + 槽位 2 生效。"""
        @self.case("QA-CT-02")
        def go(env: Env):
            write_baseline(env)  # 保持基线落盘口径一致（本用例不比对）
            env.write_config('[common]\ndownload_slots = 2\n')
            cfg = env.config_path()
            with open(cfg, "rb") as f:
                before = f.read()
            mtime_before = os.stat(cfg).st_mtime_ns
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("EZR Downloader", 6)
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=200000"))
                assert app.wait_for("下载中", 8)
                add_task_via_dialog(
                    app, env, env.fixture.url("three-m.bin?speed=200000"),
                    save_dir=os.path.join(env.save_dir, "t2"))
                assert app.wait_for("下载槽位 2/2", 8), "两任务占满槽位 2"
                add_task_via_dialog(
                    app, env, env.fixture.url("two-m.bin?speed=200000"),
                    save_dir=os.path.join(env.save_dir, "t3"))
                assert app.wait_for("等待中", 8), "第 3 个任务应排队（等待中）"
                with open(cfg, "rb") as f:
                    after = f.read()
                assert after == before, "配置文件应字节级不变（不重写不追加）"
                assert os.stat(cfg).st_mtime_ns == mtime_before, \
                    "mtime 不应因启动/生成动作变化"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-CT-03（v1.13 翻转）损坏配置拒绝启动：非零退出 + 报错含路径与原因 + 文件不变 + 修复恢复。"""
        @self.case("QA-CT-03")
        def go(env: Env):
            write_baseline(env)
            env.write_config('not [valid toml ===')
            cfg = env.config_path()
            with open(cfg, "rb") as f:
                before = f.read()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                deadline = time.time() + 8
                while app.exit_code is None and time.time() < deadline:
                    app.pump(0.2)
                assert app.exit_code is not None, \
                    "损坏配置应拒绝启动（进程应退出，D63①）"
                assert app.exit_code != 0, f"退出码应非零，实得 {app.exit_code}"
                tail = app.text()[-2000:]
                assert_in("配置文件解析失败", tail, "报错含解析失败语义")
                assert_in("config.toml", tail, "报错含配置文件路径")
            finally:
                app.pump(0.2)
            with open(cfg, "rb") as f:
                after = f.read()
            assert after == before, "损坏文件不被重写、不被模板覆盖"
            env.write_config('[common]\ndownload_slots = 3\n')
            app2 = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app2.wait_for("EZR Downloader", 6), "修复为合法 TOML 后应正常启动"
                assert_in("下载槽位 0/3", app2.text(), "修复文件生效（槽位 3）")
            finally:
                app2.graceful_quit()

    def _case_04(self):
        """QA-CT-04 配置目录只读：启动正常、无 config.toml 生成、全默认。"""
        @self.case("QA-CT-04")
        def go(env: Env):
            write_baseline(env)
            ezr_dir = os.path.join(env.home, ".ezr")
            os.makedirs(ezr_dir, exist_ok=True)
            os.chmod(ezr_dir, stat.S_IRUSR | stat.S_IXUSR)  # 555
            try:
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    assert app.wait_for("EZR Downloader", 6), "启动正常不报错"
                    assert_in("下载槽位 0/5", app.text(), "行为全默认（槽位 5）")
                    cfg = os.path.join(ezr_dir, "config.toml")
                    assert not os.path.exists(cfg), "只读目录不应生成 config.toml"
                finally:
                    app.graceful_quit()
            finally:
                os.chmod(ezr_dir, stat.S_IRWXU)  # 755 清理
                cfg = os.path.join(ezr_dir, "config.toml")
                assert not os.path.exists(cfg) or os.path.getsize(cfg) >= 0, \
                    "清理后状态一致"

    def _case_05(self):
        """QA-CT-05 EZR_HOME 重定位：$X/config.toml 生成且逐字节一致。"""
        @self.case("QA-CT-05")
        def go(env: Env):
            baseline = write_baseline(env)
            xdir = os.path.join(env.fixture_root, "ezr-home-x")
            os.makedirs(xdir, exist_ok=True)
            app = EzrApp(env.home, save_dir=env.save_dir,
                         env_extra={"EZR_HOME": xdir})
            try:
                assert app.wait_for("EZR Downloader", 6)
                xc = os.path.join(xdir, "config.toml")
                assert os.path.exists(xc), "$X/config.toml 应已生成"
                assert cmp_bytes(xc, baseline), "重定位模板与钦定文本逐字节一致"
                assert not os.path.exists(env.config_path()), \
                    "$HOME/.ezr/ 下不应生成 config.toml"
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = ConfigTemplateSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
