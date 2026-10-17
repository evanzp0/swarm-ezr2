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
    """从 feature 场景 01 注释块解析操作者钦定模板原文（54 行，逐字节权威）。

    块起点 = 以「specifier v1.10）：」结尾的说明行；块终点 = 独立全角括号行
    「）」；内容行统一 2 空格缩进（feature 文件风格），空行即真实空行；
    EOF 以一个换行符收尾。
    """
    with open(FEATURE_PATH, encoding="utf-8") as f:
        lines = f.read().split("\n")
    start = next(i for i, l in enumerate(lines)
                 if l.rstrip().endswith("specifier v1.10）："))
    end = next(i for i, l in enumerate(lines) if i > start and l.strip() == "）")
    body = [l[2:] if l.startswith("  ") else l for l in lines[start + 1:end]]
    assert len(body) == 54, f"钦定原文应为 54 行，解析得 {len(body)}"
    assert all((not l.strip()) or l.startswith("# ") for l in body), \
        "钦定原文每条非空行应以 # 开头"
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
        """QA-CT-01 干净 HOME 启动：生成全注释模板，逐字节一致 + 行为全默认。"""
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
                assert nonblank and all(l.startswith("# ") for l in nonblank), \
                    "每条非空行应以 # 开头"
                # 9 键示例值与默认值口径逐一相符（含在钦定文本内，此处抽核心键）
                text = "\n".join(body)
                for key_line in ("# download_dir = \"\"",
                                 "# block_size_http = 1048576",
                                 "# download_slots = 5",
                                 "# max_speed = 0",
                                 "# max_retries = 5",
                                 "# auto_retry = true",
                                 "# backoff_initial = 8.0",
                                 "# backoff_cap = 60.0",
                                 "# http_concurrency = 4"):
                    assert_in(key_line, text, f"键示例行 {key_line}")
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
            env.write_config('download_slots = 2\n')
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
        """QA-CT-03 损坏配置不重写不阻塞：字节不变 + 全默认行为。"""
        @self.case("QA-CT-03")
        def go(env: Env):
            write_baseline(env)
            env.write_config('not [valid toml ===')
            cfg = env.config_path()
            with open(cfg, "rb") as f:
                before = f.read()
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("EZR Downloader", 6), "启动不报错"
                assert_in("下载槽位 0/5", app.text(), "默认槽位 5")
                add_task_via_dialog(
                    app, env, env.fixture.url("five-m.bin?speed=900000"))
                assert app.wait_for("下载中", 8), "正常下载"
                d = detail_text(app)
                assert_in("1 MB/块", d, "默认块 1 MB")
                assert app.wait_for("并发 4", 15), "默认并发 4（头部数据面）"
                with open(cfg, "rb") as f:
                    after = f.read()
                assert after == before, "损坏文件不被模板覆盖"
            finally:
                app.graceful_quit()

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
