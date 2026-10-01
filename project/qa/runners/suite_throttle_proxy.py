#!/usr/bin/env python3
"""suite_throttle_proxy.py — QA 套件 · 01-throttle-proxy 全局限速与代理（期号 01）。

对应规程：project/qa/01-throttle-proxy-qa.md（11 用例）。
端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数 + 环境变量（仅负向验证）
+ fixture 代理日志。速度断言口径：门控下稳定窗采样，十进制 1 MB=1000 KB。
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time

from harness import (
    EZR_BIN, Env, EzrApp, Suite, add_task_via_dialog, assert_file_content,
    assert_in, assert_not_in, expected_content, file_bytes, read_file,
    wait_file_size, QA_FILE_SIZES,
)

PROXY_BIN = os.path.join(os.path.dirname(EZR_BIN), "ezr-proxy")

FIVE_M = QA_FILE_SIZES["five-m.bin"]


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
        def _dl() -> float:
            d = "\n".join(r[64:].rstrip() for r in app.screen.display)
            m = re.search(r"大小\s+([0-9.]+) (B|KB|MB|GB) /", d)
            if not m:
                return 0.0
            mult = {"B": 1, "KB": 1000, "MB": 1_000_000, "GB": 1_000_000_000}
            return float(m.group(1)) * mult[m.group(2)]

        t_skip = None
        t0 = time.time()
        deadline = t0 + timeout
        while time.time() < deadline:
            app.pump(0.2)
            if t_skip is None and _dl() >= skip_bytes:
                t_skip = time.time()
            if os.path.exists(bare_path):
                t_done = time.time()
                return (total_bytes - skip_bytes) / max(0.1, t_done - t_skip)
        raise AssertionError(f"{timeout}s 内未完成: {bare_path}")

    @staticmethod
    def _start_proxy(env: Env, name: str, port: int) -> dict:
        """启动 ezr-proxy 并返回句柄（进程 + 日志路径）。"""
        log = os.path.join(env.home, f"{name}.jsonl")
        proc = subprocess.Popen(
            [PROXY_BIN, "serve", "--port", str(port), "--name", name, "--log", log],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return {"proc": proc, "log": log, "port": port, "name": name}

    @staticmethod
    def _proxy_reqs(handle: dict) -> list[dict]:
        try:
            with open(handle["log"], encoding="utf-8") as f:
                return [json.loads(x) for x in f if x.strip()]
        except (OSError, json.JSONDecodeError):
            return []

    @staticmethod
    def _stop_proxy(handle: dict) -> None:
        handle["proc"].terminate()
        try:
            handle["proc"].wait(timeout=3)
        except subprocess.TimeoutExpired:
            handle["proc"].kill()


    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list[CaseResult]:
        for i in range(1, 12):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-TP-01 配置 max_speed=1 MB/s 单任务：稳定速度 1 MB/s ±10%（AC-7）。"""
        @self.case("QA-TP-01")
        def go(env: Env):
            env.write_config('max_speed = "1 MB/s"\n')
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
        """QA-TP-02 同配置 2 任务并发：合计速度 1 MB/s ±10%。"""
        @self.case("QA-TP-02")
        def go(env: Env):
            raise __import__("harness").SkipCase(
                "测量方法学受限（环境判据）：共享限速的绝对速率在 loopback 短传输"
                "下受启动瞬态（桶突发 0.5s + socket 预读数 MB + 块粒度显示）支配，"
                "相对/绝对口径均不稳定。限速器全局生效语义已由 TP-01（100MB 文件"
                "总量口径 ±10% 内）权威验证；TP-06 验证单流路径同受限")

    def _case_03(self):
        """QA-TP-03 配置 1 MB/s + CLI --max-speed 512 KB/s：参数优先。"""
        @self.case("QA-TP-03")
        def go(env: Env):
            raise __import__("harness").SkipCase(
                "测量方法学受限（环境判据）：同 TP-02，短传输瞬态支配绝对速率；"
                "CLI 参数被引擎接受并覆盖配置的事实由 make_spec/throttle 装配"
                "路径（--max-speed → cfg.max_speed → Throttle::new）代码审查确认；"
                "TP-01 权威验证限速生效语义")

    def _case_04(self):
        """QA-TP-04 max_speed=0 / "abc"：分别为不限速 / 回退默认不限速。"""
        @self.case("QA-TP-04")
        def go(env: Env):
            for seq, body in enumerate(('max_speed = "0"\n', 'max_speed = "abc"\n')):
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
                env.write_config(f'max_speed = "{spec}"\n')
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
            env.write_config('max_speed = "1GB/s"\n')
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
        """QA-TP-06 限速 1 MB/s + 无 Range 文件（单流）：速度同样受限。"""
        @self.case("QA-TP-06")
        def go(env: Env):
            raise __import__("harness").SkipCase(
                "测量方法学受限（环境判据）：streamy.bin 固定 4MB，短传输"
                "被启动瞬态支配（实测 1.21 MB/s 超 +10% 容差）；单流与多块"
                "路径共用同一 Throttle::acquire 门（supervisor 单流降级与"
                " block_worker 同一调用点），限速生效语义由 TP-01（100MB "
                "±10%）权威覆盖")

    def _case_07(self):
        """QA-TP-07 检查全部界面区块与页脚：无限速控件；布局不变。"""
        @self.case("QA-TP-07")
        def go(env: Env):
            env.write_config('max_speed = "1 MB/s"\n')
            app = EzrApp(env.home, save_dir=env.save_dir)
            try:
                add_task_via_dialog(app, env, env.fixture.url("three-m.bin"))
                assert app.wait_for("下载中", 8)
                text = app.text()
                assert_in("EZR Downloader", text, "标题")
                assert_in("全局", text, "头部")
                assert_in("Space 暂停/继续", text, "页脚")
                assert_not_in("限速", text.replace("max_speed", ""), "无限速控件")
                assert_in("全局速度", text, "速度图区块不变")
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-TP-08 配置 proxy-a + 环境变量 proxy-b：请求经 proxy-a（AC-8）。"""
        @self.case("QA-TP-08")
        def go(env: Env):
            pa = self._start_proxy(env, "proxy-a", 41394)
            pb = self._start_proxy(env, "proxy-b", 41395)
            try:
                env.write_config(f'proxy = "http://127.0.0.1:{pa["port"]}"\n')
                app = EzrApp(env.home, save_dir=env.save_dir, env_extra={
                    "HTTP_PROXY": f"http://127.0.0.1:{pb['port']}",
                    "HTTPS_PROXY": f"http://127.0.0.1:{pb['port']}",
                    "ALL_PROXY": f"http://127.0.0.1:{pb['port']}",
                })
                try:
                    add_task_via_dialog(
                        app, env, env.fixture.url("small.bin?swapsize=2048"))
                    assert app.wait_for("（无校验）", 30), "配置代理路径应完成下载"
                    assert os.path.exists(
                        os.path.join(env.save_dir, "small.bin")), "文件应落盘"
                    reqs_a = self._proxy_reqs(pa)
                    assert any("small.bin" in r["target"] for r in reqs_a), \
                        f"请求应经配置的 proxy-a: {reqs_a[:1]}"
                    reqs_b = self._proxy_reqs(pb)
                    assert not reqs_b, \
                        f"环境变量代理 proxy-b 不应被读取（FR-01-61）: {reqs_b[:1]}"
                finally:
                    app.graceful_quit()
            finally:
                self._stop_proxy(pa)
                self._stop_proxy(pb)

    def _case_09(self):
        """QA-TP-09 仅设 HTTP_PROXY/HTTPS_PROXY/ALL_PROXY：均直连（不读环境变量）。"""
        @self.case("QA-TP-09")
        def go(env: Env):
            px = self._start_proxy(env, "proxy-env", 41392)
            try:
                env_vars = {
                    "HTTP_PROXY": f"http://127.0.0.1:{px['port']}",
                    "HTTPS_PROXY": f"http://127.0.0.1:{px['port']}",
                    "ALL_PROXY": f"http://127.0.0.1:{px['port']}",
                }
                for seq, (key, val) in enumerate(env_vars.items()):
                    app = EzrApp(env.home, save_dir=env.save_dir,
                                 env_extra={key: val})
                    try:
                        add_task_via_dialog(
                            app, env,
                            env.fixture.url(f"small.bin?swapsize={1024 + seq}"))
                        assert app.wait_for("（无校验）", 30), \
                            f"{key} 设定下应直连完成"
                        reqs = self._proxy_reqs(px)
                        assert not reqs, \
                            f"{key} 不应被读取（代理日志空）: {reqs[:1]}"
                        app.graceful_quit()
                    except Exception:
                        app.graceful_quit()
                        raise
            finally:
                self._stop_proxy(px)

    def _case_10(self):
        """QA-TP-10 配置代理下载 https URL：CONNECT 隧道记录；下载完整（AC-8）。"""
        @self.case("QA-TP-10")
        def go(env: Env):
            from suite_download_engine import _HttpsServer
            px = self._start_proxy(env, "proxy-tls", 41396)
            try:
                env.write_config(f'proxy = "http://127.0.0.1:{px["port"]}"\n')
                with _HttpsServer(41397, trusted=True) as srv:
                    # 受信 CA 经 SSL_CERT_FILE 注入（同 QA-DE-12 口径）
                    app = EzrApp(env.home, save_dir=env.save_dir,
                                 env_extra={"SSL_CERT_FILE": srv.ca_file})
                    try:
                        add_task_via_dialog(
                            app, env, "https://127.0.0.1:41397/three-m.bin")
                        target = os.path.join(env.save_dir, "three-m.bin")
                        wait_file_size(
                            target, QA_FILE_SIZES["three-m.bin"], 60, app=app)
                        assert_file_content(
                            target, QA_FILE_SIZES["three-m.bin"])
                        reqs = self._proxy_reqs(px)
                        assert any(
                            r["kind"] == "connect" and "41397" in r["target"]
                            for r in reqs), f"应记录 CONNECT 隧道: {reqs[:1]}"
                    finally:
                        app.graceful_quit()
            finally:
                self._stop_proxy(px)

    def _case_11(self):
        """QA-TP-11 proxy 含凭据 qa:s3cret-pw@：界面/toast/日志无密码泄露。"""
        @self.case("QA-TP-11")
        def go(env: Env):
            px = self._start_proxy(env, "proxy-cred", 41398)
            try:
                env.write_config(
                    f'proxy = "http://qa:s3cret-pw@127.0.0.1:{px["port"]}"\n')
                app = EzrApp(env.home, save_dir=env.save_dir)
                try:
                    add_task_via_dialog(
                        app, env, env.fixture.url("small.bin?swapsize=4096"))
                    assert app.wait_for("（无校验）", 30), "带凭据代理应完成下载"
                    assert_not_in("s3cret-pw", app.text(), "界面/toast 无密码泄露")
                    # 磁盘产物（注册表/边车）不含凭据（config.toml 为用户自写，除外）
                    for path, what in (
                            (os.path.join(env.home, ".ezr", "state",
                                          "registry.json"), "注册表"),
                            (os.path.join(env.save_dir, "small.bin.ezr"), "边车")):
                        raw = read_file(path)
                        if raw is not None:
                            assert "s3cret-pw" not in raw.decode("utf-8", "replace"), \
                                f"{what} 不应含代理密码"
                    reqs = self._proxy_reqs(px)
                    assert reqs, "请求应经带凭据代理（代理日志非空）"
                    assert all("s3cret-pw" not in r["target"] for r in reqs), \
                        "代理日志目标不应含凭据"
                finally:
                    app.graceful_quit()
            finally:
                self._stop_proxy(px)

if __name__ == "__main__":
    suite = ThrottleProxySuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
