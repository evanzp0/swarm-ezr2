#!/usr/bin/env python3
"""suite_integrity_check.py — QA 套件 · 01-integrity-check 完整性校验（期号 01）。

对应规程：project/qa/01-integrity-check-qa.md（11 用例）。
端到端 UI 层验证：TUI 状态/校验文案断言 + 磁盘伴随文件预置 + CLI 参数 + fixture 日志。

口径注记（QA-IC-06，已裁决）：多伴随文件并存按算法表声明顺序取最先；Gherkin 示例
原点名 Adler-32 与实现 CHECKSUM_ALGOS 声明序（MD5 在前）矛盾，操作者裁决改规格随实现——
Gherkin 场景 06 已改为 MD5 优先，本套件断言与规格、实现一致。
"""

from __future__ import annotations

import hashlib
import os
import re
import time
import zlib

from harness import (
    EZR_BIN, Env, EzrApp, Suite, TMP_ROOT as TMP_ROOT2, add_task_via_dialog,
    assert_file_content, assert_in, assert_not_in, detail_text,
    expected_content, file_bytes, wait_file_size, QA_FILE_SIZES,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]

# 7 算法：名称、hex 位数、hashlib/zlib 计算器（与产品 CHECKSUM_ALGOS 声明序一致：
# MD5 → SHA-1 → SHA-224 → SHA-256 → SHA-384 → SHA-512 → Adler-32）
ALGOS = [
    ("MD5", 32, lambda d: hashlib.md5(d).hexdigest()),
    ("SHA-1", 40, lambda d: hashlib.sha1(d).hexdigest()),
    ("SHA-224", 56, lambda d: hashlib.sha224(d).hexdigest()),
    ("SHA-256", 64, lambda d: hashlib.sha256(d).hexdigest()),
    ("SHA-384", 96, lambda d: hashlib.sha384(d).hexdigest()),
    ("SHA-512", 128, lambda d: hashlib.sha512(d).hexdigest()),
    ("Adler-32", 8, lambda d: format(zlib.adler32(d) & 0xFFFFFFFF, "08x")),
]


class IntegrityCheckSuite(Suite):
    def __init__(self):
        super().__init__("01-integrity-check", port_base=41600)
        self._cur_app = None

    def _snapshot(self, env: Env, case_id: str) -> None:
        if self._cur_app is None:
            return
        try:
            with open(os.path.join(TMP_ROOT2, f"{case_id}.screen.txt"), "w") as f:
                f.write(self._cur_app.text())
        except Exception:
            pass

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _make_file(env: Env, name: str, size: int = 1_000_000) -> bytes:
        data = expected_content(size)
        with open(os.path.join(env.fixture_root, name), "wb") as f:
            f.write(data)
        return data

    @staticmethod
    def _cli(env: Env, *args: str):
        import subprocess
        return subprocess.run(
            [EZR_BIN, *args], input="", capture_output=True, text=True,
            env={"HOME": env.home, "TERM": "xterm-256color",
                 "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                 "LANG": "C.UTF-8"}, timeout=15)

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in range(1, 12):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-IC-01 CLI -x 七算法前缀码：详情算法依次 Adler-32…SHA-512。"""
        @self.case("QA-IC-01")
        def go(env: Env):
            data = self._make_file(env, "ic01.bin", 1_000_000)
            cases = [
                (f"adler32={ALGOS[6][2](data)}", "Adler-32"),
                (f"md5={ALGOS[0][2](data)}", "MD5"),
                (f"sha1={ALGOS[1][2](data)}", "SHA-1"),
                (f"sha224={ALGOS[2][2](data)}", "SHA-224"),
                (f"sha256={ALGOS[3][2](data)}", "SHA-256"),
                (f"sha384={ALGOS[4][2](data)}", "SHA-384"),
                (f"sha512={ALGOS[5][2](data)}", "SHA-512"),
            ]
            for xarg, algo in cases:
                app = EzrApp(env.home, save_dir=env.save_dir,
                             args=[env.fixture.url("ic01.bin?speed=400000"),
                                   "-d", env.save_dir, "-x", xarg])
                try:
                    # 快文件的「已添加」toast 会被完成 toast 覆盖：以
                    # 「{算法} 校验通过」完成 toast 为锚（算法名随文案校验）
                    assert app.wait_for(f"{algo} 校验通过", 60), \
                        f"{xarg.split('=')[0]} 应完成并 {algo} 校验通过"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_02(self):
        """QA-IC-02 显式 MD5 优先于伴随 .sha256：显示「MD5 校验成功」。"""
        @self.case("QA-IC-02")
        def go(env: Env):
            data = self._make_file(env, "ic02.bin", 1_000_000)
            with open(os.path.join(env.save_dir, "ic02.bin.sha256"), "w") as f:
                f.write(hashlib.sha256(data).hexdigest())
            # CLI 显式 -x md5=…：伴随 .sha256 并存仍显式优先（来源优先级 D3）
            app = EzrApp(env.home, save_dir=env.save_dir,
                         args=[env.fixture.url("ic02.bin?speed=400000"),
                               "-d", env.save_dir,
                               "-x", f"md5={hashlib.md5(data).hexdigest()}"])
            try:
                assert app.wait_for("MD5 校验通过", 60), \
                    "显式 MD5 应优先于伴随 .sha256"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-IC-03 分别预置 7 算法伴随（含 .SHA256 大写）：各显示校验成功。"""
        @self.case("QA-IC-03")
        def go(env: Env):
            for idx, (name, need, calc) in enumerate(ALGOS):
                fname = f"ic03-{idx}.bin"
                data = self._make_file(env, fname, 1_000_000)
                suffix = name.replace("-", "").lower()  # sha256 / adler32 / md5…
                if name == "SHA-256" and idx == 3:
                    suffix = "SHA256"  # 大写后缀形态（大小写不敏感）
                with open(os.path.join(env.save_dir, f"{fname}.{suffix}"), "w") as f:
                    f.write(calc(data))
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(
                        app, env, env.fixture.url(f"{fname}?speed=400000"))
                    assert app.wait_for(f"{name} 校验通过", 60), \
                        f"{fname}.{suffix} 应显示 {name} 校验通过"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_04(self):
        """QA-IC-04 伴随内容两形态：裸 hex / 「hex  文件名」均识别。"""
        @self.case("QA-IC-04")
        def go(env: Env):
            for form in ("bare", "withname"):
                fname = f"ic04-{form}.bin"
                data = self._make_file(env, fname, 1_000_000)
                digest = hashlib.sha256(data).hexdigest()
                content = digest if form == "bare" else \
                    f"{digest}  {fname}"
                with open(os.path.join(env.save_dir, f"{fname}.sha256"), "w") as f:
                    f.write(content)
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(
                        app, env, env.fixture.url(f"{fname}?speed=400000"))
                    assert app.wait_for("SHA-256 校验通过", 60), \
                        f"{form} 形态应校验通过"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_05(self):
        """QA-IC-05 .sha256 内容 32 位（位数不符）：无效提醒并按无校验完成。"""
        @self.case("QA-IC-05")
        def go(env: Env):
            self._make_file(env, "ic05.bin", 400_000)
            with open(os.path.join(env.save_dir, "ic05.bin.sha256"), "w") as f:
                f.write("a" * 32)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("ic05.bin?speed=900000"))
                assert app.wait_for("无校验", 60), "位数不符应按无校验完成"
                # toast 提醒（位数不符无效）可能出现且已消失；不强断言其文案
                assert_not_in("校验成功", app.text(), "不应出现校验成功")
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-IC-06 多伴随并存按算法表声明序取最先（MD5 优先，规格已随实现）。"""
        @self.case("QA-IC-06")
        def go(env: Env):
            data = self._make_file(env, "ic06.bin", 400_000)
            with open(os.path.join(env.save_dir, "ic06.bin.adler32"), "w") as f:
                f.write(ALGOS[6][2](data))
            with open(os.path.join(env.save_dir, "ic06.bin.md5"), "w") as f:
                f.write(ALGOS[0][2](data))
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("ic06.bin?speed=400000"))
                # 实现口径：算法表声明序 MD5 在前 → MD5 校验
                assert app.wait_for("MD5 校验", 60), \
                    "并存时应按声明序取 MD5（实现口径，矛盾待裁决）"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-IC-07 正确 SHA-256：校验中占槽→已完成；收尾三件套。"""
        @self.case("QA-IC-07")
        def go(env: Env):
            data = self._make_file(env, "ic07.bin", FIVE_M)
            with open(os.path.join(env.save_dir, "ic07.bin.sha256"), "w") as f:
                f.write(hashlib.sha256(data).hexdigest())
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("ic07.bin?speed=600000"))
                assert app.wait_for("校验中", 30), "完成应先进校验中（占槽）"
                text = app.text()
                assert_in("下载槽位 1/5", text, "校验中应占槽")
                assert app.wait_for("SHA-256 校验通过", 30), "应校验通过"
                assert app.wait_gone("校验中", 15)
                target = os.path.join(env.save_dir, "ic07.bin")
                assert file_bytes(target) is not None, "裸名文件应存在"
                assert file_bytes(target + ".downloading") is None
                assert file_bytes(target + ".ezr") is None, "sidecar 应删除"
                assert_in("下载槽位 0/5", app.text(), "完成后应释放槽位")
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-IC-08 错误 SHA-256：已失败校验失败不自动重试并释放槽位（AC-6）。"""
        @self.case("QA-IC-08")
        def go(env: Env):
            data = self._make_file(env, "ic08.bin", 1_000_000)
            bad = "0" * 64
            with open(os.path.join(env.save_dir, "ic08.bin.sha256"), "w") as f:
                f.write(bad)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("ic08.bin?speed=900000"))
                assert app.wait_for("SHA-256 校验失败", 60), \
                    "错误期望值应校验失败"
                text = app.text()
                assert_in("内容与校验值不符", text, "失败原因应含内容与校验值不符")
                assert_in("不自动重试", text, "校验失败应不自动重试")
                assert_in("下载槽位 0/5", text, "校验失败应释放槽位")
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-IC-09 失败后 R→修正伴随→R：重新校验通过转已完成（AC-6）。"""
        @self.case("QA-IC-09")
        def go(env: Env):
            data = self._make_file(env, "ic09.bin", 1_000_000)
            bad = "0" * 64
            with open(os.path.join(env.save_dir, "ic09.bin.sha256"), "w") as f:
                f.write(bad)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("ic09.bin?speed=900000"))
                assert app.wait_for("SHA-256 校验失败", 60)
                # 首次 R：期望值仍是坏的（伴随未变）→ 仍失败。
                # 「computed=」为失败 toast 专有文案（列表行常驻会瞬时命中），
                # 以它等待第二次校验失败，避免与首轮失败竞速
                ts_before = time.time()
                app.send("r")
                assert app.wait_for("重新校验", 6), "R 应触发重新校验"
                app.pump(1.0)
                reqs = [r for r in env.fixture.requests()
                        if "ic09.bin" in r["path"] and r["range"]
                        and r["range"] != "bytes=0-"
                        and r["ts"] / 1000 >= ts_before - 1]
                assert not reqs, "重新校验不应发生块数据 Range 请求"
                assert app.wait_for("computed=", 30), "坏期望值 R 后仍失败"
                # 修正伴随 → R → 通过（R 前确认任务已回到「已失败」停等态）
                self._cur_app = app
                assert app.wait_for("不自动重试", 10), "第二次失败应停等"
                with open(os.path.join(env.save_dir, "ic09.bin.sha256"), "w") as f:
                    f.write(hashlib.sha256(data).hexdigest())
                app.send("r")
                assert app.wait_for("SHA-256 校验通过", 30), \
                    "修正伴随后 R 应校验通过转已完成"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-IC-10 无显式值无伴随：直接已完成显示无校验不经校验中。"""
        @self.case("QA-IC-10")
        def go(env: Env):
            self._make_file(env, "ic10.bin", 1_000_000)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env, env.fixture.url("ic10.bin?speed=600000"))
                assert app.wait_for("下载完成", 30), "应直接下载完成"
                assert app.wait_for("无校验", 10), "应显示无校验"
                text = app.text()
                assert_not_in("校验中", text, "不应进入校验中")
                assert_not_in("校验成功", text, "不应出现校验成功")
                assert_in("下载槽位 0/5", text, "完成应释放槽位")
            finally:
                app.graceful_quit()

    def _case_11(self):
        """QA-IC-11 fixture Range 短响应：网络类瞬态失败→自动重试续传补齐→完成。

        口径注记（已裁决）：「文件大小不符」终态与续传补齐在引擎块记账不变式下互斥
        （块须标完成才触发大小校验，而标完成的洞无法续传补齐）。操作者裁决改规程
        文案随实现：短响应按网络类瞬态处理，自动重试续传补齐后完成；Gherkin 场景 11
        与规程均已同步，「大小不符」文案正常路径不可达。
        """
        @self.case("QA-IC-11")
        def go(env: Env):
            env.write_config("block_size_http = 4194304\n")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(
                    app, env,
                    env.fixture.url("eight-m.bin?disconnect=2000000"))
                # 短响应（每请求切 2MB）→ 有进展瞬态失败 → 自动重试倒计时
                assert app.wait_for("s 后重试", 20), \
                    "短响应应失败并进入自动重试倒计时"
                assert app.wait_for("下载中", 30), "退避后应自动重试续传"
                target = os.path.join(env.save_dir, "eight-m.bin")
                wait_file_size(target, QA_FILE_SIZES["eight-m.bin"], 90, app=app)
                resumed = [r for r in env.fixture.requests()
                           if r["range"] and "eight-m.bin" in r["path"]]
                starts = [int(re.match(r"bytes=(\d+)-", r["range"]).group(1))
                          for r in resumed if re.match(r"bytes=(\d+)-", r["range"])]
                assert any(s >= 2_000_000 for s in starts), \
                    f"应有 ≈2MB 起点的续传补齐请求: {sorted(starts)}"
                assert_file_content(target, QA_FILE_SIZES["eight-m.bin"])
            finally:
                app.graceful_quit()


if __name__ == "__main__":
    suite = IntegrityCheckSuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
