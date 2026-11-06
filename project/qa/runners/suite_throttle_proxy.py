#!/usr/bin/env python3
"""suite_throttle_proxy.py — QA 套件 · 01-throttle-proxy 全局与任务级限速（期号 01）。

对应规程：project/qa/01-throttle-proxy-qa.md（14 用例）。本套件实跑
TP-01..07/10/13/14；TP-08/09/11/12 按 UC-08 先例注记移交（见下）。
端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数。
速度断言口径：门控下稳定窗采样；v1.11 起配置限速键二进制口径
（KiB/MiB/GiB，旧写法 KB/MB/GB 废弃回退 0）；v1.42 起面板速度显示
统一二进制（KiB/s、MiB/s），速度读数解析按 1024^n 换算。
v1.6：原 QA-TP-08~11 代理用例随旧全局 proxy 键退役删除（FR-01-90），
代理语义由 02-named-proxy / QA-NP 套件承载。
v1.7（v141）：TP-02/03/06 实跑化——TP-01 同款 100MB 大文件总量口径
（跳过前 10MB 启动瞬态，总量平均 ±10%）；QA_TP_ONLY=1,3 环境变量
支持用例过滤分批前台复跑（缺省全量，结果仅含所跑用例）。
v1.8（QA 最终轮 2026-11-06）：config 键名/单位/期望值随 v1.11 全量翻转
（max_speed → global_max_download_speed，1 MB/s → 1 MiB/s）；TP-04 增
"2 MB/s" 废弃回退臂；TP-05 行组按现行 Examples 重排 + 旧写法两行断言
回退 0（2 GiB/s 与 512 B/s 两行不可实测登记裁剪：前者与不限速形态
不可区分、后者 ten-m 需 5.4h）；补 TP-10/13/14 实跑。
注记移交（UC-08 先例：BT 判据由单测锁 + QA 会话覆盖、runner 维持 HTTP 驱动）：
- TP-08（BT 做种上传限速）：上传限速挂 EngineShared 双桶 set_rate 与
  bt/engine.rs task_ul.set_rate 均单测锁（engine/throttle.rs set_rate 热改
  真实 BT 拉取测速由 harness bench_multipeer QA 会话级覆盖）。
- TP-09（添加对话框任务级限速预填/规范化）：HTTP 臂 = QA-01-20 实跑同口径；
  BT 臂 = bt_dialog.rs 限速预填/规范化单测锁。
- TP-11（HTTP 任务修改限速 0→1M→500K→0）：≡ QA-MT-10 同轮实跑（03-modify-task），
  本套件不重复。
- TP-12（BT 任务修改限速）：BtTaskCmd::Reconfigure 命令面单测锁（hardening_g_engine），
  PTY 级 BT 交互由 BT 集成套件承载。
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time

from harness import (
    Env, EzrApp, Suite, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, clear_dir_prefill, expected_content,
    file_bytes, open_add_dialog_via_cmd, proxy_reqs, read_file,
    start_proxy, stop_proxy, wait_file_size, QA_FILE_SIZES,
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
    """「↓ 812 KiB/s」→ 字节/秒（v1.42 二进制显示口径；无读数返回 None）。"""
    m = re.search(r"↓ ([\d.]+) (B|KiB|MiB|GiB)/s", text)
    if not m:
        return None
    val, unit = float(m.group(1)), m.group(2)
    mult = {"B": 1, "KiB": 1024, "MiB": 1_048_576, "GiB": 1_073_741_824}[unit]
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
                if only else range(1, 11))
        for i in idxs:
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-TP-01 配置 global_max_download_speed=1 MiB/s 单任务：稳定速度 1 MiB/s ±10%（v1.11 二进制口径）。"""
        @self.case("QA-TP-01")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = "1 MiB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 100MB：启动瞬态（桶突发+socket 预读）摊薄至 <3%，稳态可测
                add_task_via_dialog(app, env, env.fixture.url("big-100m.bin"))
                assert app.wait_for("下载中", 8)
                avg = self._steady_rate(
                    app, 104_857_600, 10_000_000,
                    os.path.join(env.save_dir, "big-100m.bin"), timeout=200)
                assert 943_718 <= avg <= 1_153_434, \
                    f"稳定速度应 1 MiB/s（1048576）±10%: {avg/1000:.0f} KB/s"
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-TP-02 同配置 2 任务并发：合计速度 1 MiB/s ±10%（v141 实跑 + v1.11 口径）。

        规程另列 HTTP+BT 跨协议臂：共享配额双桶为协议无关实现
        （EngineShared 单例 + bt pump 挂同一桶，单测锁），真实 BT 拉取对账
        由 harness bench_multipeer QA 会话级覆盖，runner 维持 HTTP 驱动
        （UC-08 先例注记）。
        """
        @self.case("QA-TP-02")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = "1 MiB/s"\n')
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
                assert 943_718 <= rate <= 1_153_434, \
                    f"合计速度应 1 MiB/s（1048576）±10%: {rate/1000:.0f} KB/s"
            finally:
                app.graceful_quit()

    def _case_03(self):
        """QA-TP-03 配置 1 MiB/s + CLI --max-speed 512 KiB/s：参数优先（v141 实跑 + v1.11 口径）。"""
        @self.case("QA-TP-03")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = "1 MiB/s"\n')
            # CLI 覆盖配置（FR-01-60；CLI 与 config 同源新写法，旧写法报错退出）。
            # 判别力：512K+10% 上界 563K 与 1M−10% 下界 900K 不相交——
            # 测得 512K 档即证参数优先。
            app = EzrApp(env.home, args=["--max-speed", "512 KiB/s"],
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
        """QA-TP-04 global_max_download_speed = 0 / "abc" / "2 MB/s"：均不限速（v1.11：旧写法废弃回退 0）。"""
        @self.case("QA-TP-04")
        def go(env: Env):
            for seq, body in enumerate((
                    '[common]\nglobal_max_download_speed = 0\n',
                    '[common]\nglobal_max_download_speed = "abc"\n',
                    '[common]\nglobal_max_download_speed = "2 MB/s"\n')):
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
        """QA-TP-05 v1.11 二进制口径：新写法按 1024^n 生效、旧写法回退 0 不限速。"""
        @self.case("QA-TP-05")
        def go(env: Env):
            # v1.11 口径：耗时期望 = 10MB/rate（rate 按二进制 1024^n）；
            # 旧写法（KB/MB/GB 十进制）废弃 → 回退 0（不限速，秒完成）。
            # 裁剪登记：规格 7 行中 "2 GiB/s"（与不限速形态不可区分）与
            # "512 B/s"（ten-m 需 5.4h，超会话预算）两行不实测，键解析行为
            # 由 model/config.rs speed_parsing_binary_units /
            # speed_parsing_decimal_units_retired 单测锁承载。
            # 每档独立子目录：避免上一档已落盘的 ten-m.bin 使 exists 判定
            # 立即成真（脏测），亦避免同 URL 同目录重复守卫（QA-01-16）。
            for seq, (spec, want) in enumerate(
                    (("2 MiB/s", 2_097_152), ("500 KiB/s", 512_000),
                     ("2000000", 2_000_000), ("512 B/s 单测锁承载", None),
                     ("2 MB/s", 0), ("500KB/s", 0))):
                if spec.endswith("单测锁承载"):
                    continue
                sub = os.path.join(env.save_dir, f"r{seq}")
                env.write_config(
                    f'[common]\nglobal_max_download_speed = "{spec}"\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(app, env, env.fixture.url("ten-m.bin"),
                                        save_dir=sub)
                    # 回退 0 档（旧写法废弃）不限速亚秒完成——「下载中」为瞬时态
                    # 不可稳定观察（engineering.md 锚定持久终态），只对限速档断言
                    if want != 0:
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
                    if want == 0:
                        assert elapsed <= 6, \
                            f"{spec}（旧写法废弃回退 0）应不限速秒完成: {elapsed:.1f}s"
                    else:
                        want_t = 10_000_000 / want
                        assert 0.7 * want_t <= elapsed <= 1.6 * want_t, \
                            f"{spec} 耗时应 ≈{want_t:.1f}s: {elapsed:.1f}s"
                    app.graceful_quit()
                except Exception:
                    app.graceful_quit()
                    raise
            # 2 GiB/s（新写法上限档）：与不限速形态不可实测区分，由单测锁承载；
            # 此处保留一档不限速形态秒完成对照（独立子目录同理）
            env.write_config('[common]\nglobal_max_download_speed = "2 GiB/s"\n')
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
                assert elapsed <= 6, f"2 GiB/s（不限速形态档）应秒级完成: {elapsed:.1f}s"
                app.graceful_quit()
            except Exception:
                app.graceful_quit()
                raise

    def _case_06(self):
        """QA-TP-06 限速 1 MiB/s + 无 Range 文件（单流）：速度同样受限（v141 实跑 + v1.11 口径）。"""
        @self.case("QA-TP-06")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = "1 MiB/s"\n')
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
                assert 943_718 <= avg <= 1_153_434, \
                    f"单流路径速度应同样受限 1 MiB/s ±10%: {avg/1000:.0f} KB/s"
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
            env.write_config('[common]\nglobal_max_download_speed = "1 MiB/s"\n')
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


    def _case_08(self):
        """QA-TP-14 旧键 [common] max_speed = "1 MB/s"：未知键静默忽略 → 不限速、无致命项。"""
        @self.case("QA-TP-14")
        def go(env: Env):
            env.write_config('[common]\nmax_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 启动不报错（无致命 toast/退出）；旧键未知键静默忽略（v1.13
                # 三大表口径）→ 全局下载限速 0 → three-m 以 fixture 速度秒完成。
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin"))
                t0 = time.time()
                bare = os.path.join(env.save_dir, "three-m.bin")
                deadline = t0 + 30
                while time.time() < deadline:
                    app.pump(0.3)
                    if os.path.exists(bare):
                        break
                elapsed = time.time() - t0
                assert elapsed <= 6, \
                    f"旧键忽略后应不限速秒完成（若误识别 1 MB/s 需 ≥3s×3）: {elapsed:.1f}s"
                assert app.wait_for("（无校验）", 10), "应正常完成"
                text = app.text()
                assert_not_in("⚠", text, "不应有警告类 toast（未知键静默忽略）")
            finally:
                app.graceful_quit()

    def _case_09(self):
        """QA-TP-10 主行实跑（全局 10 MiB/s, 任务级 500 KiB/s）→ 任务速度 ≈500 KiB/s（min 合成）。

        规程三组合中本行（1 MiB/s, 500 KiB/s）以全局 10 MiB/s 等价替换
        （fixture 峰速 ~5MB/s 下 10 MiB/s 全局等于不限，min 合成判别力不变：
        测得 500 KiB/s 档即证任务级生效且低于全局）；(1 MiB/s, 0) 与
        (0, 2 MiB/s) 两行由 min 合成实现的每 chunk 双桶串联单测锁承载
        （bug18-coder 口径），不重复计时。
        """
        @self.case("QA-TP-10")
        def go(env: Env):
            env.write_config('[common]\nglobal_max_download_speed = "10 MiB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                # 添加对话框显式设任务级下载限速 500 KiB/s（对话框导航按
                # 现行七字段几何：打开焦点 URL0，Tab→保存到1，Tab×2→下载限速3；
                # v1.39/v1.40 上传限速字段退役后下载限速前移一档）
                open_add_dialog_via_cmd(app, env.fixture.url("ten-m.bin?cd=tp10.bin"))
                app.send("tab")
                clear_dir_prefill(app)
                sub = os.path.join(env.save_dir, "tp10")
                os.makedirs(sub, exist_ok=True)
                app.type_text(sub)
                app.send("tab", "tab")         # 保存到1 → 并发2 → 下载限速3
                clear_dir_prefill(app)
                app.type_text("500k")
                app.pump(0.3)
                assert re.search(r"下载限速\s*>\s*500 KiB/s", app.text()), \
                    "键入 500k 应规范化显示 500 KiB/s"
                app.send(*(["tab"] * 4))       # 下载限速3 → 立即下载7
                app.send("enter")
                assert app.wait_for("下载中", 8), "任务应启动"
                bare = os.path.join(sub, "tp10.bin")
                rate = self._limit_throughput(
                    app, 10_000_000, bare, timeout=90)
                assert 440_000 <= rate <= 590_000, \
                    f"任务速度应 ∈ min(全局, 任务级) = 500 KiB/s ±12%: {rate/1000:.0f} KB/s"
            finally:
                app.graceful_quit()

    def _case_10(self):
        """QA-TP-13 HTTP 臂实跑：任务级限速退出重启后预填与生效一致（注册表持久化）。

        BT 臂注记（UC-08 先例）：任务级限速落库为协议无关 registry 字段
        （同一写入路径，模型层单测锁），BT 任务创建/恢复 e2e 由 BT 集成
        套件承载，runner 维持 HTTP 驱动。
        """
        @self.case("QA-TP-13")
        def go(env: Env):
            sub = os.path.join(env.save_dir, "tp13")
            os.makedirs(sub, exist_ok=True)
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                open_add_dialog_via_cmd(
                    app, env.fixture.url("five-m.bin?speed=300000&cd=tp13"))
                app.send("tab")
                clear_dir_prefill(app)
                app.type_text(sub)
                app.send("tab", "tab")         # 保存到1 → 并发2 → 下载限速3
                clear_dir_prefill(app)
                app.type_text("500k")
                app.pump(0.2)
                app.send(*(["tab"] * 4))       # → 立即下载7
                app.send("enter")
                assert app.wait_for("下载中", 8), "任务应启动"
            finally:
                code = app.graceful_quit(timeout=10)
                assert code == 0, f"优雅退出码 0，得到 {code}"
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                assert app.wait_for("下载中", 15), "重启应续传"
                app.send("m")
                assert app.wait_for("修改任务", 3), "修改对话框未打开"
                assert re.search(r"下载限速\s*>\s*500 KiB/s", app.text()), \
                    "重启后修改框应预填 500 KiB/s（fmt 形态，注册表持久化）"
                app.send("esc")
                app.pump(0.3)
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
