#!/usr/bin/env python3
"""suite_named_proxy.py — QA 套件 · 02-named-proxy 命名代理与任务级选择（期号 01）。

对应规程：project/qa/02-named-proxy-qa.md（18 用例，QA-NP-01..18 连续编号）。
端到端 UI 层验证：对话框交互 + ezr-proxy 日志逐请求核对 + 启动 toast + 注册表观察；
不使用项目内部 API。
受限项（沿规程环境前置与遗留受限项口径）：ezr-proxy 现仅支持 http 监听
（绑定 127.0.0.1），https（代理自身 TLS）/ socks5（含认证）/ IPv6 回环三类监听
依赖 ezr-proxy 扩展 —— NP-08 https+socks5 相位、NP-13 https 相位、NP-15、NP-18
下载相位按 S 登记（其余可观测相位先行断言）。
"""

from __future__ import annotations

import os
import time

from harness import (
    Env, EzrApp, SkipCase, Suite, TMP_ROOT, assert_file_content, assert_in,
    assert_not_in, proxy_reqs, start_proxy, stop_proxy, wait_file_size,
    QA_FILE_SIZES,
)

ONE_M = QA_FILE_SIZES["one-m.bin"]
HALF_M = QA_FILE_SIZES["half-m.bin"]
TWO_M = QA_FILE_SIZES["two-m.bin"]
FIVE_M = QA_FILE_SIZES["five-m.bin"]

SLOW = "?speed=300000"  # 门控速度（观察窗内 toast 不被完成提示覆盖）


def proxies_toml(entries: list[dict]) -> str:
    """[[proxies]] 条目转 TOML（缺键即省略行；port 可为 int 或 str）。"""
    out = ""
    for e in entries:
        out += "[[proxies]]\n"
        for k in ("name", "type", "ip", "port", "username", "password"):
            if k not in e or e[k] is None:
                continue
            v = e[k]
            out += f'{k} = "{v}"\n' if isinstance(v, str) else f"{k} = {v}\n"
    return out


class NamedProxySuite(Suite):
    def __init__(self):
        super().__init__("02-named-proxy", port_base=42100)
        self._cur_app: "EzrApp | None" = None

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

    def _new_app(self, env: Env, save_dir: str | None = None) -> EzrApp:
        app = EzrApp(env.home, save_dir=save_dir or env.save_dir)
        self._cur_app = app
        return app

    @staticmethod
    def _dropdown_options(app: EzrApp) -> str:
        """展开添加对话框代理下拉并返回全画面文本（选项行内嵌其中）。"""
        app.send("A")
        assert app.wait_for("添加下载任务", 3), "添加对话框未打开"
        app.send(*(["tab"] * 5))  # URL→目录→并发→校验→校验码→代理
        app.send("enter")         # 展开代理下拉
        assert app.wait_for("选择代理", 3), "代理下拉未展开"
        text = app.text()
        app.send("esc")           # 收起
        app.send("esc")           # 关闭对话框
        app.pump(0.3)
        return text

    @staticmethod
    def _add_via_proxy(app: EzrApp, env: Env, url: str, option_index: int,
                       save_dir: str | None = None) -> None:
        """添加任务并选中代理下拉第 option_index 项（0 = 直连）。"""
        app.send("A")
        assert app.wait_for("添加下载任务", 3), "添加对话框未打开"
        app.type_text(url)
        app.send("tab")                                  # URL(0)→目录(1)
        app.type_text(env.save_dir if save_dir is None else save_dir)
        app.send(*(["tab"] * 3))                         # 目录→并发→校验→校验码
        app.send("tab")                                  # 校验码(4)→代理(5)
        app.send("enter")                                # 展开代理下拉
        assert app.wait_for("选择代理", 3), "代理下拉未展开"
        app.send("home")
        for _ in range(option_index):
            app.send("down")
        app.send("enter")                                # 选中
        app.pump(0.2)
        app.send("tab")                                  # 代理(5)→确认(6)
        app.send("enter")
        app.pump(0.3)

    # -- 作废条目共性（NP-09/10/16/17 共用：合法条目指向存活 ezr-proxy 实例）--

    def _invalid_entry_case(self, case_id: str, warn_needle: str, entries: dict,
                            legal: dict, phases: list[str]) -> None:
        """各相位：条目不在下拉 + 启动警告一次 + 合法条目在位且下载成功。"""
        @self.case(case_id)
        def go(env: Env):
            px = start_proxy(env, f"{case_id.split('-')[1].lower()}-ok",
                             self._next_port())
            legal_live = {**legal, "port": px["port"]}
            try:
                for phase in phases:
                    env.write_config(proxies_toml([entries[phase], legal_live]))
                    app = self._new_app(env)
                    try:
                        assert app.wait_for(warn_needle, 5), \
                            f"[{phase}] 启动警告一次"
                        assert app.wait_for("EZR Downloader", 4), \
                            f"[{phase}] 启动不受阻"
                        text = self._dropdown_options(app)
                        e = entries[phase]
                        if "type" in e:
                            assert_not_in(f"{e['name']}（{e['type']}）", text,
                                          f"[{phase}] 作废条目不在下拉")
                        else:
                            assert_not_in(f"{e['name']}（", text,
                                          f"[{phase}] 作废条目不在下拉")
                        assert_in(f"{legal['name']}（{legal['type']}）", text,
                                  f"[{phase}] 合法条目在位")
                        # 合法条目下载成功（任务完成 ⇒ 引擎未受作废条目干扰）；
                        # 每相位独立子目录（同 URL 同目录被重复守卫拦截，QA-01-16）
                        sub = os.path.join(env.save_dir, f"ph-{phase}")
                        self._add_via_proxy(
                            app, env, env.fixture.url(f"one-m.bin{SLOW}"), 1,
                            save_dir=sub)
                        assert app.wait_for("（无校验）", 45), \
                            f"[{phase}] 合法条目下载成功"
                    finally:
                        app.graceful_quit()
            finally:
                stop_proxy(px)

    # -- 用例 -----------------------------------------------------------------

    def run(self) -> list:
        for i in range(1, 19):
            getattr(self, f"_case_{i:02d}")()
        return self.results

    def _case_01(self):
        """QA-NP-01 三个命名代理（其一含认证）下拉：直连+3 项，名（type）标注，无凭证。"""
        @self.case("QA-NP-01")
        def go(env: Env):
            env.write_config(proxies_toml([
                {"name": "corp-http", "type": "http", "ip": "10.0.0.1", "port": 8080},
                {"name": "corp-tls", "type": "https", "ip": "10.0.0.2", "port": 8443},
                {"name": "corp-socks", "type": "socks5", "ip": "10.0.0.3",
                 "port": 1080, "username": "alice", "password": "s3cret"},
            ]))
            app = self._new_app(env)
            try:
                assert app.wait_for("EZR Downloader", 6)
                text = self._dropdown_options(app)
                for label in ("直连", "corp-http（http）", "corp-tls（https）",
                              "corp-socks（socks5）"):
                    assert_in(label, text, f"下拉选项 {label}")
                assert_not_in("alice", text, "选项文案不含用户名")
                assert_not_in("s3cret", text, "选项文案不含密码")
            finally:
                app.graceful_quit()

    def _case_02(self):
        """QA-NP-02 A→proxy-a、B→proxy-b、C→直连并行：日志互不串线、均完成。"""
        @self.case("QA-NP-02")
        def go(env: Env):
            pa = start_proxy(env, "proxy-a", self._next_port())
            pb = start_proxy(env, "proxy-b", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "proxy-a", "type": "http",
                     "ip": "127.0.0.1", "port": pa["port"]},
                    {"name": "proxy-b", "type": "http",
                     "ip": "127.0.0.1", "port": pb["port"]},
                ]))
                app = self._new_app(env)
                try:
                    self._add_via_proxy(app, env, env.fixture.url("one-m.bin"), 1,
                                        save_dir=os.path.join(env.save_dir, "a"))
                    self._add_via_proxy(app, env, env.fixture.url("two-m.bin"), 2,
                                        save_dir=os.path.join(env.save_dir, "b"))
                    self._add_via_proxy(app, env, env.fixture.url("half-m.bin"), 0,
                                        save_dir=os.path.join(env.save_dir, "c"))
                    wait_file_size(os.path.join(env.save_dir, "a", "one-m.bin"),
                                   ONE_M, 30, app=app)
                    wait_file_size(os.path.join(env.save_dir, "b", "two-m.bin"),
                                   TWO_M, 30, app=app)
                    wait_file_size(os.path.join(env.save_dir, "c", "half-m.bin"),
                                   HALF_M, 30, app=app)
                finally:
                    app.graceful_quit()
                assert_file_content(os.path.join(env.save_dir, "a", "one-m.bin"), ONE_M)
                assert_file_content(os.path.join(env.save_dir, "b", "two-m.bin"), TWO_M)
                assert_file_content(os.path.join(env.save_dir, "c", "half-m.bin"), HALF_M)
                ra = [r["target"] for r in proxy_reqs(pa)]
                rb = [r["target"] for r in proxy_reqs(pb)]
                assert any("one-m" in t for t in ra), "proxy-a 应记录 A 的请求"
                assert not any("two-m" in t or "half-m" in t for t in ra), \
                    "proxy-a 不应串线 B/C"
                assert any("two-m" in t for t in rb), "proxy-b 应记录 B 的请求"
                assert not any("one-m" in t or "half-m" in t for t in rb), \
                    "proxy-b 不应串线 A/C"
            finally:
                stop_proxy(pa)
                stop_proxy(pb)

    def _case_03(self):
        """QA-NP-03 重名/空名条目 + 合法条目：警告一次、重复副本不入下拉、下载不受阻。"""
        @self.case("QA-NP-03")
        def go(env: Env):
            # 重名去重保留首个：首个 dup 合法在位，重复副本与空名条目作废
            px = start_proxy(env, "np03-ok", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "dup", "type": "http", "ip": "127.0.0.1", "port": 1},
                    {"name": "dup", "type": "http", "ip": "127.0.0.2", "port": 2},
                    {"name": "", "type": "http", "ip": "127.0.0.3", "port": 3},
                    {"name": "ok1", "type": "http", "ip": "127.0.0.1",
                     "port": px["port"]},
                ]))
                app = self._new_app(env)
                try:
                    assert app.wait_for("已忽略重复代理条目", 5), "重名警告"
                    assert app.wait_for("name 为空", 3), "空名警告"
                    text = self._dropdown_options(app)
                    assert_in("ok1（http）", text, "合法条目应在下拉")
                    assert_in("dup（http）", text, "首个重名条目合法保留")
                    assert text.count("dup（http）") == 1, "重复副本不应再次出现"
                    # 选项序：直连(0) + dup(1) + ok1(2) —— 经 ok1 下载
                    self._add_via_proxy(
                        app, env, env.fixture.url(f"one-m.bin{SLOW}"), 2)
                    assert app.wait_for("（无校验）", 45), "合法条目下载完成"
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(px)

    def _case_04(self):
        """QA-NP-04 http 认证代理下载：成功 + 全界面/日志无凭证明文。"""
        @self.case("QA-NP-04")
        def go(env: Env):
            pa = start_proxy(env, "proxy-auth", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "auth-http", "type": "http", "ip": "127.0.0.1",
                     "port": pa["port"], "username": "bob", "password": "pw-secret"},
                ]))
                app = self._new_app(env)
                try:
                    self._add_via_proxy(app, env, env.fixture.url("one-m.bin"), 1)
                    assert app.wait_for("（无校验）", 30), "经认证配置代理下载成功"
                    text = app.text()
                    assert_not_in("bob", text, "界面无用户名明文")
                    assert_not_in("pw-secret", text, "界面无密码明文")
                finally:
                    app.graceful_quit()
                for r in proxy_reqs(pa):
                    blob = str(r)
                    assert "bob" not in blob and "pw-secret" not in blob, \
                        f"代理日志无凭证明文: {blob[:120]}"
                # ezr-proxy 无认证要求模式（遗留扩展受限项）：「代理侧强制认证
                # 子判据」（407 质询 → 认证握手成功在案）不可测，按 S 登记
                raise SkipCase(
                    "已验：经认证配置代理下载成功 + 全界面/代理日志无凭证明文；"
                    "代理侧强制认证子判据依赖 ezr-proxy 认证要求模式（遗留扩展受限项）")
            finally:
                stop_proxy(pa)

    def _case_05(self):
        """QA-NP-05 引用 gone-proxy 存盘 → 下会话删除条目续传：直连完成 + 提醒一次。"""
        @self.case("QA-NP-05")
        def go(env: Env):
            # 会话 1：gone-proxy 指向存活实例（任务经它下载、暂停播种引用）
            px = start_proxy(env, "gone-live", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "gone-proxy", "type": "http",
                     "ip": "127.0.0.1", "port": px["port"]},
                ]))
                app = self._new_app(env)
                try:
                    self._add_via_proxy(
                        app, env, env.fixture.url(f"five-m.bin{SLOW}"), 1)
                    assert app.wait_for("下载中", 8)
                    app.send("space")  # 暂停（稳定中间态，播种引用）
                    assert app.wait_for("已暂停", 5)
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(px)  # 代理消亡
            # 会话 2：配置删除该条目
            env.write_config("")
            app = self._new_app(env)
            try:
                assert app.wait_for("已暂停", 8), "暂停任务应还原"
                app.send("space")  # 恢复 → 引擎解析引用失败 → 提醒一次 + 直连
                assert app.wait_for("不存在，已按直连", 8), "引用不存在提醒一次"
                assert app.wait_for("下载中", 8), "直连续传"
                wait_file_size(os.path.join(env.save_dir, "five-m.bin"),
                               FIVE_M, 60, app=app)
                assert_file_content(os.path.join(env.save_dir, "five-m.bin"), FIVE_M)
            finally:
                app.graceful_quit()

    def _case_06(self):
        """QA-NP-06 旧形态注册表（proxy:"global"）启动续传：直连完成不崩溃。"""
        @self.case("QA-NP-06")
        def go(env: Env):
            app = self._new_app(env)
            try:
                self._add_via_proxy(app, env, env.fixture.url(f"five-m.bin{SLOW}"), 0)
                assert app.wait_for("下载中", 8)
                app.send("space")
                assert app.wait_for("已暂停", 5)
            finally:
                app.graceful_quit()
            # 注册表旧形态改写（v1.5 "global" 值；FR-01-90 兼容可读，映射直连）
            reg = os.path.join(env.home, ".ezr", "state", "registry.json")
            with open(reg, encoding="utf-8") as f:
                raw = f.read()
            needle = '"proxy": "direct"'
            assert needle in raw, \
                f"前置：注册表应含 direct 值: {raw[:200]}"
            with open(reg, "w", encoding="utf-8") as f:
                f.write(raw.replace(needle, '"proxy": "global"'))
            app = self._new_app(env)
            try:
                assert app.wait_for("已暂停", 8), "任务应还原（不崩溃）"
                assert_not_in("不存在", app.text(), "global 映射直连无需提醒")
                app.send("space")
                assert app.wait_for("下载中", 8)
                wait_file_size(os.path.join(env.save_dir, "five-m.bin"),
                               FIVE_M, 60, app=app)
            finally:
                app.graceful_quit()

    def _case_07(self):
        """QA-NP-07 ip 无法构成合法代理地址：toast 一次 + 直连完成不崩溃。"""
        @self.case("QA-NP-07")
        def go(env: Env):
            env.write_config(proxies_toml([
                {"name": "bad-ip", "type": "http", "ip": "bad host", "port": 9999},
            ]))
            app = self._new_app(env)
            try:
                # 慢速任务：无效端点 toast 不被完成提示覆盖（toast 槽位单一）
                self._add_via_proxy(
                    app, env, env.fixture.url(f"five-m.bin{SLOW}"), 1)
                assert app.wait_for("代理地址无效", 8), "使用时应 toast 地址无效"
                assert app.wait_for("已按直连", 3), "按直连回退提示"
                assert app.wait_for("（无校验）", 45), "直连完成"
            finally:
                app.graceful_quit()

    def _case_08(self):
        """QA-NP-08 type=http/https/socks5：下拉标注 + http 相位连接形态在案。"""
        @self.case("QA-NP-08")
        def go(env: Env):
            pa = start_proxy(env, "p-http", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "px-http", "type": "http",
                     "ip": "127.0.0.1", "port": pa["port"]},
                    {"name": "px-https", "type": "https",
                     "ip": "127.0.0.1", "port": self._next_port()},
                    {"name": "px-socks5", "type": "socks5",
                     "ip": "127.0.0.1", "port": self._next_port(),
                     "username": "u", "password": "p"},
                ]))
                app = self._new_app(env)
                try:
                    text = self._dropdown_options(app)
                    for label in ("px-http（http）", "px-https（https）",
                                  "px-socks5（socks5）"):
                        assert_in(label, text, f"下拉标注 {label}")
                    # http 相位：连接以 http 代理形态抵达对应端口
                    self._add_via_proxy(app, env, env.fixture.url("one-m.bin"), 1)
                    assert app.wait_for("（无校验）", 30), "http 相位下载成功"
                    assert any("one-m" in r["target"] for r in proxy_reqs(pa)), \
                        "http 相位：代理日志在案"
                finally:
                    app.graceful_quit()
                raise SkipCase(
                    "已验：下拉三值标注 + http 相位下载成功且日志在案；"
                    "https（代理自身 TLS）与 socks5 监听形态依赖 ezr-proxy 扩展"
                    "（遗留受限项，规程环境前置口径）")
            finally:
                stop_proxy(pa)

    def _case_09(self):
        """QA-NP-09 type 缺失（ip/port 在位）：作废 + 警告；合法条目可用。"""
        self._invalid_entry_case(
            "QA-NP-09", "type 缺失",
            {"missing": {"name": "no-type", "ip": "127.0.0.1", "port": 1}},
            {"name": "ok9", "type": "http", "ip": "127.0.0.1", "port": 1},
            ["missing"])

    def _case_10(self):
        """QA-NP-10 type 未知值（ftp）：作废 + 警告；合法条目可用。"""
        self._invalid_entry_case(
            "QA-NP-10", "应为 http / https / socks5 三值之一",
            {"ftp": {"name": "bad-type", "type": "ftp",
                     "ip": "127.0.0.1", "port": 1}},
            {"name": "ok10", "type": "http", "ip": "127.0.0.1", "port": 1},
            ["ftp"])

    def _case_11(self):
        """QA-NP-11 socks5 缺 username：作废 + 警告（文案不含凭证值）；合法可用。"""
        @self.case("QA-NP-11")
        def go(env: Env):
            px = start_proxy(env, "np11-ok", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "no-user", "type": "socks5", "ip": "127.0.0.1",
                     "port": 1, "password": "hidden-pw"},
                    {"name": "ok11", "type": "http",
                     "ip": "127.0.0.1", "port": px["port"]},
                ]))
                app = self._new_app(env)
                try:
                    assert app.wait_for("username 与 password 必填", 5), "凭证缺失警告"
                    text = self._dropdown_options(app)
                    assert_not_in("no-user（socks5）", text, "作废条目不在下拉")
                    assert_in("ok11（http）", text, "合法条目在位")
                    assert_not_in("hidden-pw", text, "警告文案不含凭证值")
                    self._add_via_proxy(app, env, env.fixture.url(f"one-m.bin{SLOW}"), 1)
                    assert app.wait_for("（无校验）", 45), "合法条目下载成功"
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(px)

    def _case_12(self):
        """QA-NP-12 socks5 缺 password：同 NP-11 判据。"""
        @self.case("QA-NP-12")
        def go(env: Env):
            px = start_proxy(env, "np12-ok", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "no-pass", "type": "socks5", "ip": "127.0.0.1",
                     "port": 1, "username": "some-user"},
                    {"name": "ok12", "type": "http",
                     "ip": "127.0.0.1", "port": px["port"]},
                ]))
                app = self._new_app(env)
                try:
                    assert app.wait_for("username 与 password 必填", 5), "凭证缺失警告"
                    text = self._dropdown_options(app)
                    assert_not_in("no-pass（socks5）", text, "作废条目不在下拉")
                    assert_in("ok12（http）", text, "合法条目在位")
                    self._add_via_proxy(app, env, env.fixture.url(f"one-m.bin{SLOW}"), 1)
                    assert app.wait_for("（无校验）", 45), "合法条目下载成功"
                finally:
                    app.graceful_quit()
            finally:
                stop_proxy(px)

    def _case_13(self):
        """QA-NP-13 http/https 匿名条目：无凭证警告；请求经代理；代理侧无认证记录。"""
        @self.case("QA-NP-13")
        def go(env: Env):
            pa = start_proxy(env, "anon-http", self._next_port())
            try:
                env.write_config(proxies_toml([
                    {"name": "anon", "type": "http",
                     "ip": "127.0.0.1", "port": pa["port"]},
                ]))
                app = self._new_app(env)
                try:
                    assert_not_in("凭证", app.text(), "http 匿名无凭证警告")
                    self._add_via_proxy(app, env, env.fixture.url("one-m.bin"), 1)
                    assert app.wait_for("（无校验）", 30), "匿名 http 代理下载成功"
                    for r in proxy_reqs(pa):
                        assert set(r.keys()) <= {"ts", "proxy", "kind", "target"}, \
                            f"代理日志无认证记录字段: {r}"
                finally:
                    app.graceful_quit()
                raise SkipCase(
                    "已验：http 匿名相位（无凭证警告 + 经代理 + 日志无认证记录）；"
                    "https 匿名相位依赖 ezr-proxy https 监听扩展（遗留受限项）")
            finally:
                stop_proxy(pa)

    def _case_14(self):
        """QA-NP-14 http/https 半填凭证（4 相位）：作废 + 成对警告；合法条目可用。"""
        @self.case("QA-NP-14")
        def go(env: Env):
            px = start_proxy(env, "np14-ok", self._next_port())
            legal = {"name": "ok14", "type": "http", "ip": "127.0.0.1",
                     "port": px["port"]}
            phases = [
                {"name": "half-hu", "type": "http", "ip": "127.0.0.1",
                 "port": 1, "username": "only-user"},
                {"name": "half-hp", "type": "http", "ip": "127.0.0.1",
                 "port": 1, "password": "only-pw"},
                {"name": "half-su", "type": "https", "ip": "127.0.0.1",
                 "port": 1, "username": "only-user"},
                {"name": "half-sp", "type": "https", "ip": "127.0.0.1",
                 "port": 1, "password": "only-pw"},
            ]
            try:
                for e in phases:
                    env.write_config(proxies_toml([e, legal]))
                    app = self._new_app(env)
                    try:
                        assert app.wait_for("需成对配置", 5), f"[{e['name']}] 成对警告"
                        text = self._dropdown_options(app)
                        assert_not_in(f"{e['name']}（{e['type']}）", text,
                                      f"[{e['name']}] 作废条目不在下拉")
                        assert_in("ok14（http）", text, "合法条目在位")
                        sub = os.path.join(env.save_dir, f"ph-{e['name']}")
                        self._add_via_proxy(
                            app, env, env.fixture.url(f"one-m.bin{SLOW}"), 1,
                            save_dir=sub)
                        assert app.wait_for("（无校验）", 45), \
                            f"[{e['name']}] 合法条目下载成功"
                    finally:
                        app.graceful_quit()
            finally:
                stop_proxy(px)

    def _case_15(self):
        """QA-NP-15 socks5/https 带正确凭证下载：认证按类型生效（监听未就绪 → S）。"""
        @self.case("QA-NP-15")
        def go(env: Env):
            env.write_config(proxies_toml([
                {"name": "socks-auth", "type": "socks5", "ip": "127.0.0.1",
                 "port": 1080, "username": "u15", "password": "p15"},
                {"name": "tls-auth", "type": "https", "ip": "127.0.0.1",
                 "port": 8443, "username": "u15", "password": "p15"},
            ]))
            app = self._new_app(env)
            try:
                assert app.wait_for("EZR Downloader", 6)
                text = self._dropdown_options(app)
                assert_in("socks-auth（socks5）", text, "凭证齐备条目在下拉")
                assert_in("tls-auth（https）", text, "https 条目在下拉")
                assert_not_in("u15", text, "选项文案不含凭证")
            finally:
                app.graceful_quit()
            raise SkipCase(
                "已验：两类型条目正常加载（下拉标注一致，文案不含凭证）；"
                "认证生效判定依赖 socks5+认证与 https（TLS 会话内 basic auth）监听"
                "——ezr-proxy 扩展未就绪（规程环境前置登记口径）")

    def _case_16(self):
        """QA-NP-16 ip 缺失/空串/纯空白（3 相位）：作废 + 警告；合法条目可用。"""
        self._invalid_entry_case(
            "QA-NP-16", "ip 缺失或空白",
            {
                "no-ip": {"name": "bad16a", "type": "http", "port": 1},
                "empty-ip": {"name": "bad16b", "type": "http", "ip": "", "port": 1},
                "blank-ip": {"name": "bad16c", "type": "http", "ip": "   ", "port": 1},
            },
            {"name": "ok16", "type": "http", "ip": "127.0.0.1", "port": 1},
            ["no-ip", "empty-ip", "blank-ip"])

    def _case_17(self):
        """QA-NP-17 port 缺失/非整数/0/65536/-1（5 相位）：作废 + 警告；合法可用。"""
        self._invalid_entry_case(
            "QA-NP-17", "port 须为 1..=65535 内的整数",
            {
                "no-port": {"name": "bad17a", "type": "http", "ip": "127.0.0.1"},
                "str-port": {"name": "bad17b", "type": "http", "ip": "127.0.0.1",
                             "port": "not-a-number"},
                "zero-port": {"name": "bad17c", "type": "http", "ip": "127.0.0.1",
                              "port": 0},
                "big-port": {"name": "bad17d", "type": "http", "ip": "127.0.0.1",
                             "port": 65536},
                "neg-port": {"name": "bad17e", "type": "http", "ip": "127.0.0.1",
                             "port": -1},
            },
            {"name": "ok17", "type": "http", "ip": "127.0.0.1", "port": 1},
            ["no-port", "str-port", "zero-port", "big-port", "neg-port"])

    def _case_18(self):
        """QA-NP-18 ip="::1" IPv6 字面量：条目正常加载；下载经 IPv6 代理（监听受限 → S）。"""
        @self.case("QA-NP-18")
        def go(env: Env):
            env.write_config(proxies_toml([
                {"name": "s6-proxy", "type": "http", "ip": "::1", "port": 1},
            ]))
            app = self._new_app(env)
            try:
                assert app.wait_for("EZR Downloader", 6)
                assert_not_in("已忽略", app.text(), "IPv6 字面量条目合法加载")
                text = self._dropdown_options(app)
                assert_in("s6-proxy（http）", text, "条目正常加载（下拉标注）")
                assert_not_in("异常", text, "无因 ip 含冒号的异常文案")
            finally:
                app.graceful_quit()
            raise SkipCase(
                "已验：IPv6 字面量条目正常加载（下拉标注一致、无异常文案）；"
                "经 ::1 代理下载判定依赖 IPv6 回环监听——ezr-proxy 绑定 127.0.0.1"
                "（遗留扩展受限项，规程登记口径）")


if __name__ == "__main__":
    suite = NamedProxySuite()
    results = suite.run()
    p, f, s = suite.summary()
    print(f"== {suite.name}: P={p} F={f} S={s} ==")
    for r in results:
        if r.status != "P":
            print(f"  {r.case_id}: {r.status} {r.note}")
    raise SystemExit(1 if f else 0)
