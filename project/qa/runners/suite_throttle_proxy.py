#!/usr/bin/env python3
"""suite_throttle_proxy.py — QA 套件 · 01-throttle-proxy 全局限速（期号 01）。

对应规程：project/qa/01-throttle-proxy-qa.md（7 用例）。
端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数。
速度断言口径：门控下稳定窗采样，十进制 1 MB=1000 KB。
v1.6：原 QA-TP-08~11 代理用例随旧全局 proxy 键退役删除（FR-01-90），
代理语义由 02-named-proxy / QA-NP 套件承载。
v1.7（v141）：TP-02/03/06 实跑化——TP-01 同款 100MB 大文件总量口径
（跳过前 10MB 启动瞬态，总量平均 ±10%）；QA_TP_ONLY=1,3 环境变量
支持用例过滤分批前台复跑（缺省全量，结果仅含所跑用例）。
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, expected_content, file_bytes, proxy_reqs,
    read_file, start_proxy, stop_proxy, wait_file_size, QA_FILE_SIZES,
)

FIVE_M = QA_FILE_SIZES["five-m.bin"]


def _detail_dl(app: EzrApp) -> float:
    """选中任务详情「大小 X / Y」已下载字节（十进制；无读数 0.0）。

    v1.13 定稿布局右栏列 64 起切片（detail_text 同款）；读数为字节级
    实时口径（FR-01-105），供稳态窗时刻锚定（TP-01 权威量法两端）。
    """
    d = "\n".join(r[64:].rstrip() for r in app.screen.display)
    m = re.search(r"大小\s+([0-9.]+) (B|KB|MB|GB) /", d)
    if not m:
        return 0.0
    mult = {"B": 1, "KB": 1000, "MB": 1_000_000, "GB": 1_000_000_000}
    return float(m.group(1)) * mult[m.group(2)]


def _parse_speed_text(text: str) -> float | None:
    """「↓ 812 KB/s」→ 字节/秒（十进制口径；无读数返回 None）。"""
    m = re.search(r"↓ ([\d.]+) (B|KB|MB|GB)/s", text)
    if not m:
        return None
    val, unit = float(m.group(1)), m.group(2)
    mult = {"B": 1, "KB": 1000, "MB": 1_000_000, "GB": 1_000_000_000}[unit]
    return val * mult


class ThrottleProxySuite(Suite):
    def __init__(self):
        super().__init__("01-throttle-proxy", port_base=41300)

    # -- 公共操作 -------------------------------------------------------------

    @staticmethod
    def _avg_speed(app: EzrApp, seconds: float = 10.0,
                   save_dir: str | None = None) -> float | None:
        """采样全局 ↓ 均值（字节/秒；含 0 读数，None 跳过）。"""
        samples: list[float] = []
        deadline = time.time() + seconds
        while time.time() < deadline:
            app.pump(0.4)
            v = _parse_speed_text(app.text())
            if v is not None:
                samples.append(v)
        return sum(samples) / len(samples) if samples else None

    @staticmethod
    def _limit_throughput(app: EzrApp, total_bytes: int, bare_path: str,
                          timeout: float = 60.0) -> float:
        """总量口径限速测量：total_bytes / 完成耗时。

        显示速度按块完成粒度跳变、文件预分配大小恒定、详情累计值为块阶梯——
        唯有「总字节 / 完成总耗时」两端精确（耗时由完成 rename 锚定）。
        """
        t0 = time.time()
        deadline = t0 + timeout
        while time.time() < deadline:
            app.pump(0.3)
            if os.path.exists(bare_path):
                return total_bytes / max(0.1, time.time() - t0)
        raise AssertionError(f"{timeout}s 内未完成: {bare_path}")

    @staticmethod
    def _steady_rate(app: EzrApp, total_bytes: int, skip_bytes: int,
                     bare_path: str, timeout: float = 90.0) -> float:
        """稳态速率：跳过启动瞬态（桶突发+socket 预读）后的（字节/秒）。

        (total - skip) / (完成时刻 - 已下载 skip 字节时刻)；两端均为精确累计。
        """
        t_skip = None
        t0 = time.time()
        deadline = t0 + timeout
        while time.time() < deadline:
            app.pump(0.2)
            if t_skip is None and _detail_dl(app) >= skip_bytes:
                t_skip = time.time()
            if os.path.exists(bare_path):
                t_done = time.time()
                return (total_bytes - skip_bytes) / max(0.1, t_done - t_skip)
        raise AssertionError(f"{timeout}s 内未完成: {bare_path}")

    # 实例管理收口共享 harness（QA DRY 条款：跨套件共享操作上提）
    _start_proxy = staticmethod(start_proxy)
    _proxy_reqs = staticmethod(proxy_reqs)
    _stop_proxy = staticmethod(stop_proxy)


    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        # QA_TP_ONLY=1,3 → 只跑 _case_01/_case_03。沙箱前台单命令 10min 上限
        # 对策：本套件全量实跑 >13min 时分批复跑（v141）；缺省全量，
        # 结果仅含所跑用例（汇总口径由调用方合并各批）。
        only = os.environ.get("QA_TP_ONLY", "").strip()
        idxs = ([int(x) for x in only.split(",") if x.strip().isdigit()]
                if only else range(1, 8))
        for i in idxs:
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-TP-01 配置 max_speed=1 MB/s 单任务：稳定速度 1 MB/s ±10%（AC-7）。"""
        @self.case("QA-TP-01")
        def go(env: Env):
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 100MB：启动瞬态（桶突发+socket 预读）摊薄至 <3%，稳态可测
                add_task_via_dialog(app, env, env.fixture.url("big-100m.bin"))
                assert app.wait_for("下载中", 8)
                avg = self._steady_rate(
                    app, 104_857_600, 10_000_000,
                    os.path.join(env.save_dir, "big-100m.bin"), timeout=200)
                assert 900_000 <= avg <= 1_100_000, \
                    f"稳定速度应 1 MB/s ±10%: {avg/1000:.0f} KB/s"
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-TP-02 同配置 2 任务并发：合计速度 1 MB/s ±10%（v141 实跑）。"""
        @self.case("QA-TP-02")
        def go(env: Env):
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # TP-01 同款口径对合计流量：100MB×2 使启动瞬态（桶突发+
                # socket 预读）摊薄至 <3%。?cd= 区分命名（FR-01-02）+
                # 独立子目录，规避同 URL 重复守卫（QA-01-16）且落盘路径可判。
                dirs = {n: os.path.join(env.save_dir, f"r{n}")
                        for n in ("a", "b")}
                for n in sorted(dirs):
                    os.makedirs(dirs[n], exist_ok=True)
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(f"big-100m.bin?cd=tp02{n}.bin"),
                        save_dir=dirs[n])
                    assert app.wait_for("下载中", 8), f"任务 {n} 应启动"
                total = 2 * 104_857_600
                bares = {n: os.path.join(dirs[n], f"tp02{n}.bin")
                         for n in dirs}
                # 组合稳态窗：每任务跳过 10MB 启动瞬态（TP-01 对称）；
                # home/down 逐轮读双任务详情（选中行 ▌ + 详情「大小」读数）。
                # 读数为显示口径（滞后实际 ≤1MB 块粒度），对 180MB 稳态量
                # 误差 ≤1.1%，远小于 ±10% 容差。
                dl = {"a": 0.0, "b": 0.0}
                t_skip = None
                combined = 0.0
                t0 = time.time()
                deadline = t0 + 330
                while time.time() < deadline and t_skip is None:
                    app.send("home")
                    app.pump(0.25)
                    dl["a"] = max(dl["a"], _detail_dl(app))
                    app.send("down")
                    app.pump(0.25)
                    dl["b"] = max(dl["b"], _detail_dl(app))
                    if dl["a"] >= 10_000_000 and dl["b"] >= 10_000_000:
                        t_skip = time.time()
                        combined = dl["a"] + dl["b"]
                assert t_skip is not None, (
                    f"330s 内双任务未同达 10MB 稳态窗: {dl}")
                t_done = None
                while time.time() < deadline:
                    app.pump(0.3)
                    if all(os.path.exists(bares[n]) for n in bares):
                        t_done = time.time()
                        break
                assert t_done is not None, "双任务未在时限内完成"
                rate = (total - combined) / max(0.1, t_done - t_skip)
                assert 900_000 <= rate <= 1_100_000, \
                    f"合计速度应 1 MB/s ±10%: {rate/1000:.0f} KB/s"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-TP-03 配置 1 MB/s + CLI --max-speed 512 KB/s：参数优先（v141 实跑）。"""
        @self.case("QA-TP-03")
        def go(env: Env):
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            # CLI 覆盖配置（FR-01-60）。判别力：512K+10% 上界 563K 与
            # 1M−10% 下界 900K 不相交——测得 512K 档即证参数优先。
            app = EzrApp(env.home, args=["--max-speed", "512 KB/s"],
                         save_dir=env.save_dir)
            try:
                # TP-01 同款 100MB 大文件总量口径（跳过前 10MB 启动瞬态）
                add_task_via_dialog(app, env, env.fixture.url("big-100m.bin"))
                assert app.wait_for("下载中", 8)
                avg = self._steady_rate(
                    app, 104_857_600, 10_000_000,
                    os.path.join(env.save_dir, "big-100m.bin"), timeout=300)
                assert 460_800 <= avg <= 563_200, \
                    (f"稳定速度应 512 KB/s ±10%（参数优先于配置 1 MB/s）: "
                     f"{avg/1000:.0f} KB/s")
            finally:
                app.graceful_quit()

    def _case_04(self):
        """QA-TP-04 max_speed=0 / "abc"：分别为不限速 / 回退默认不限速。"""
        @self.case("QA-TP-04")
        def go(env: Env):
            for seq, body in enumerate(('[common]\nmax_speed = "0"\n', '[common]\nmax_speed = "abc"\n')):
                env.write_config(body)
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(
                        app, env,
                        env.fixture.url(f"three-m.bin?swapsize={3000000+seq}"))
                    # 无门控瞬时完成：以完成 toast 验证正常启动
                    assert app.wait_for("（无校验）", 30), f"{body!r} 应正常启动"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise

    def _case_05(self):
        """QA-TP-05 十进制口径：各限速档的 ten-m 总耗时单调符合 10MB/rate。"""
        @self.case("QA-TP-05")
        def go(env: Env):
            # 十进制口径：1MB=10^6B。耗时期望 = 10MB/rate；启动瞬态
            # （±0.3s 固定 + 突发）以 0.7× 下限、1.6× 上限吸收，
            # 绝对 ±10% 口径由 TP-01 权威兑底。
            # 每档独立子目录：避免上一档已落盘的 ten-m.bin 使 exists 判定
            # 立即成真（脏测），亦避免同 URL 同目录重复守卫（QA-01-16）。
            for seq, (spec, want) in enumerate(
                    (("500KB/s", 500_000), ("2 MB/s", 2_000_000),
                     ("2 mb/s", 2_000_000), ("2000000 B/s", 2_000_000))):
                sub = os.path.join(env.save_dir, f"r{seq}")
                env.write_config(f'[common]\nmax_speed = "{spec}"\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(app, env, env.fixture.url("ten-m.bin"),
                                        save_dir=sub)
                    assert app.wait_for("下载中", 8), f"{spec} 应正常启动"
                    t0 = time.time()
                    bare = os.path.join(sub, "ten-m.bin")
                    deadline = t0 + 90
                    while time.time() < deadline:
                        app.pump(0.3)
                        if os.path.exists(bare):
                            break
                    assert os.path.exists(bare), f"{spec} 应完成"
                    elapsed = time.time() - t0
                    want_t = 10_000_000 / want
                    assert 0.7 * want_t <= elapsed <= 1.6 * want_t, \
                        f"{spec} 耗时应 ≈{want_t:.1f}s: {elapsed:.1f}s"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise
            # 1GB/s：不限速形态，秒完成（独立子目录同理）
            env.write_config('[common]\nmax_speed = "1GB/s"\n')
            sub = os.path.join(env.save_dir, "rfree")
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("ten-m.bin"),
                                    save_dir=sub)
                t0 = time.time()
                bare = os.path.join(sub, "ten-m.bin")
                deadline = t0 + 30
                while time.time() < deadline:
                    app.pump(0.3)
                    if os.path.exists(bare):
                        break
                elapsed = time.time() - t0
                assert elapsed <= 6, f"1GB/s（不限速形态）应秒级完成: {elapsed:.1f}s"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise

    def _case_06(self):
        """QA-TP-06 限速 1 MB/s + 无 Range 文件（单流）：速度同样受限（v141 实跑）。"""
        @self.case("QA-TP-06")
        def go(env: Env):
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # ?norange=1：fixture 忽略 Range 回 200 全量（有 Content-Length）
                # → 引擎单流路径（探测 bytes=0- 即单流本体，FR-01-10）；
                # 落盘名取 URL 末段 big-100m.bin。TP-01 同款 100MB 总量口径。
                add_task_via_dialog(app, env,
                                    env.fixture.url("big-100m.bin?norange=1"))
                assert app.wait_for("不支持断点续传", 8), "详情应显示不支持断点续传"
                avg = self._steady_rate(
                    app, 104_857_600, 10_000_000,
                    os.path.join(env.save_dir, "big-100m.bin"), timeout=300)
                assert 900_000 <= avg <= 1_100_000, \
                    f"单流路径速度应同样受限 1 MB/s ±10%: {avg/1000:.0f} KB/s"
                # 证据面（DE-02 同款判据）：无 Range 项不得有分块 Range 形态
                reqs = [r for r in env.fixture.requests()
                        if "big-100m" in r["path"]]
                assert reqs, "fixture 未收到 big-100m 请求"
                ranged = [r for r in reqs
                          if r["range"] and r["range"] != "bytes=0-"]
                assert not ranged, \
                    f"无 Range 项不应有分块 Range 数据请求: {ranged[:3]}"
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-TP-07 检查全部界面区块与页脚：无限速控件；布局不变。"""
        @self.case("QA-TP-07")
        def go(env: Env):
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin"))
                assert app.wait_for("下载中", 8)
                text = app.text()
                assert_in("EZR Downloader", text, "标题")
                assert_in("全局", text, "头部")
                # 页脚（v1.13/FR-01-107 页脚收敛：提示词 chips 化，「暂停」属
                # footer.rs 退役词表——旧「Space 暂停/继续」断言随版退役）
                assert_in("A 命令 · D 删除 · M 修改 · Q 退出 · H 帮助", text,
                          "页脚命令 chips（v1.13 收敛定版）")
                assert_not_in("限速", text.replace("max_speed", ""), "无限速控件")
                assert_in("峰值 ↓", text, "头部流量图区块不变（v1.13 内嵌无标题，峰值图例）")
                assert_in("并发连接", text, "并发连接面板区块不变")
            finally:
                app.graceful_quit()

if __name__ == "__main__":
    suite = ThrottleProxySuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
