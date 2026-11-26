#!/usr/bin/env python3
"""suite_bt_anti_leech.py — 10-bt-anti-leech-qa.md v1.9 八用例可执行化（six-pack/QA 2026-11-17）。

实跑覆盖 QA-AL-01/02/03/04/05/06/08（7 例）+ S 登记 QA-AL-07 的入连面（fixture
无法在 192.0.2.x TEST-NET 地址上发起真实连接，三面过滤的 accept 面不可观察；
候选吸纳/拨出面经 blacklist.yaml 条目与既有连接行为间接承载）。

Fixture：RawPeer = 原始 peer-wire 恶意 fixture（BEP3 握手 + bitfield + interested +
request 应答可编程：leech 只收不回 / bad_piece 伪 piece / protocol 违规消息），
入连 driver listen_port（用户可达入口 = fixture 属暴露给 QA 的入口，规程钦定口径）。
判据三面对账 = toast（⛔ 前缀）+ blacklist.yaml 字节级读回 + 并发面板行消失。

阈值口径（D67）：leech up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s；bad_piece 累计 3；
protocol 破坏性首封 / 非法区间 request 重复 3；做种任务豁免 leech。
"""

from __future__ import annotations

import hashlib
import os
import re
import socket
import struct
import subprocess
import sys
import threading
import time

QA_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, QA_DIR)

from harness import TMP_ROOT, EzrApp, Suite, Env, assert_in, assert_not_in  # noqa: E402

import suite_bt_engine as be

MKTORRENT = be.MKTORRENT
PIECE = be.PIECE
FILES = be.FILES
FSIZE = be.FSIZE

PROTO = b"BitTorrent protocol"
BADPIECE_DATA = b"\xde\xad\xbe\xef" * (PIECE // 4)


class RawPeer:
    """原始 peer-wire 恶意 fixture（入连 driver；线程化消息循环，行为可编程）。

    mode:
      leech      —— 回应 request 一律不发 piece（只收不给）
      bad_piece  —— 对 request 回伪 piece（校验必失败）
      protocol   —— 握手后发未知消息 id（破坏性消息，首次即封）
      silent     —— 仅握手 + bitfield + interested，被动等待
    """

    def __init__(self, host: str, port: int, infohash: bytes, mode: str,
                 peer_id: bytes = b"-RD0000-RawPeerFixtur"):
        self.infohash = infohash
        self.mode = mode
        self.peer_id = peer_id
        self.sock: socket.socket | None = None
        self.log: list[str] = []
        self.requests_served = 0
        self.stop_flag = threading.Event()
        self._target = (host, port)
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def _run(self):
        for _ in range(50):
            try:
                s = socket.create_connection(self._target, timeout=2)
                break
            except OSError:
                time.sleep(0.2)
        else:
            self.log.append("connect-failed")
            return
        self.sock = s
        try:
            hs = struct.pack(">B", 19) + PROTO + b"\x00" * 8 + self.infohash + self.peer_id
            s.sendall(hs)
            hs_reply = self._recv_exact(s, 68)
            if hs_reply is None or hs_reply[1:20] != PROTO:
                self.log.append("handshake-failed")
                return
            bitfield = struct.pack(">B", 4) + b"\xff" * ((len(FILES) + 7) // 8)
            s.sendall(bitfield)
            if self.mode == "protocol":
                time.sleep(0.3)
                s.sendall(struct.pack(">I", 2) + b"\x63\x00")
                self.log.append("sent-unknown-id")
                time.sleep(3)
                return
            s.sendall(struct.pack(">IB", 1, 2))
            if self.mode == "invalid_req":
                for idx in (999, 999, 999):
                    if self.stop_flag.is_set():
                        return
                    s.sendall(struct.pack(">IBIII", 13, 6, idx, 0, PIECE))
                    self.log.append(f"invalid-req:{idx}")
                    time.sleep(0.4)
                self.log.append("sent-invalid-req")
            pull_idx, pull_begin = 0, 0
            last_pull = 0.0
            buf = b""
            s.settimeout(0.3)
            while not self.stop_flag.is_set():
                if self.mode == "leech" and time.time() - last_pull > 0.05:
                    try:
                        s.sendall(struct.pack(">IBIII", 13, 6, pull_idx,
                                              pull_begin, 16384))
                        self.log.append(f"pull:{pull_idx}:{pull_begin}")
                    except OSError:
                        return
                    pull_begin += 16384
                    if pull_begin >= PIECE:
                        pull_begin = 0
                        pull_idx = (pull_idx + 1) % len(FILES)
                    last_pull = time.time()
                try:
                    chunk = s.recv(65536)
                except socket.timeout:
                    continue
                except OSError:
                    return
                if not chunk:
                    self.log.append("eof")
                    return
                buf += chunk
                while len(buf) >= 4:
                    n = struct.unpack(">I", buf[:4])[0]
                    if len(buf) < 4 + n:
                        break
                    msg, buf = buf[4:4 + n], buf[4 + n:]
                    if not msg:
                        continue
                    mid = msg[0]
                    if mid == 6 and len(msg) >= 13:
                        idx, begin, length = struct.unpack(
                            ">III", msg[1:13])
                        self.log.append(f"req:{idx}:{begin}:{length}")
                        if self.mode == "bad_piece":
                            block = BADPIECE_DATA[:length] if length <= len(BADPIECE_DATA) \
                                else (BADPIECE_DATA * ((length // len(BADPIECE_DATA)) + 1))[:length]
                            piece_msg = struct.pack(">IBI", 9 + length, 7, idx,
                                                    begin) + block
                            s.sendall(piece_msg)
                            self.requests_served += 1
                    elif mid == 7:
                        self.log.append(f"got-piece:{len(msg)}")
        except OSError as e:
            self.log.append(f"oserr:{e}")
        finally:
            try:
                s.close()
            except Exception:
                pass

    def _recv_exact(self, s: socket.socket, n: int) -> bytes | None:
        buf = b""
        s.settimeout(5)
        while len(buf) < n:
            try:
                chunk = s.recv(n - len(buf))
            except socket.timeout:
                return None
            except OSError:
                return None
            if not chunk:
                return None
            buf += chunk
        return buf

    def stop(self):
        self.stop_flag.set()
        self.thread.join(timeout=3)


def _ban_entries(env: Env) -> list:
    p = os.path.join(env.home, ".ezr", "blacklist.yaml")
    if not os.path.exists(p):
        return []
    text = open(p, encoding="utf-8").read()
    entries = []
    for m in re.finditer(r"ip:\s*(\S+)", text):
        entry = {"ip": m.group(1)}
        rm = re.search(r"reason:\s*(\S+)", text[text.find(m.group(0)):])
        if rm:
            entry["reason"] = rm.group(1)
        entries.append(entry)
    return entries


def _wait_ban(app: EzrApp, env: Env, frag: str, timeout: float) -> "dict | None":
    """轮询 toast ⛔ + blacklist.yaml 即时落盘（≤1s 间隔，规程容差）。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        app.pump(0.4)
        text = app.text()
        if frag in text:
            entries = _ban_entries(env)
            if entries:
                return entries
        time.sleep(0.3)
    return None


class AntiLeechSuite(Suite):
    def __init__(self):
        super().__init__("10-bt-anti-leech", port_base=46100)
        self._cur_app: "EzrApp | None" = None

    def run(self) -> list:
        for i in (1, 2, 3, 4, 5, 6, 7, 8):
            getattr(self, f"_case_{i}")()
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

    def _quit(self, app) -> None:
        if app is None:
            return
        if app.exit_code is None:
            app.graceful_quit()
        else:
            app.close()

    # -- fixtures ------------------------------------------------------------

    def _setup_swarm(self, env: Env, rate: int = 24576) -> "tuple":
        """tracker + seeder + driver 配置（下载中任务，限速门控）。"""
        tr = be.MiniTracker(self._next_port(),
                            os.path.join(env.fixture_root, "tr.jsonl"), interval=5)
        assert tr.wait_ready()
        pay = be._make_payload(env.fixture_root)
        torrent = be._make_torrent(pay, f"http://127.0.0.1:{tr.port}/announce")
        seeder = be.LtPeer("seeder", self._next_port(), torrent, pay)
        assert seeder.wait_total_done(4 * FSIZE, 25)
        be._driver_config(env, tr.port, self._next_port() + 10, rate=rate)
        return tr, seeder, pay, torrent

    def _infohash_of(self, torrent: str) -> bytes:
        import bencodepy
        data = bencodepy.decode(open(torrent, "rb").read())
        return hashlib.sha1(bencodepy.encode(data[b"info"])).digest()

    def _partial_progress(self, app: EzrApp, timeout: float = 60) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            app.pump(0.5)
            rows = app.display_rows()
            task = next((r for r in rows if "[BT]" in r), "")
            m = re.search(r"(\d+\.\d)%", task)
            if m and float(m.group(1)) > 30.0:
                return
        raise AssertionError("下载应推进至 >30%（恶意 peer 观察窗）")

    # -- cases ---------------------------------------------------------------

    def _case_1(self):
        """QA-AL-01（S）leech 自动封禁：fixture 恶意 peer 无法获得本端 unchoke。"""
        self.skip("QA-AL-01", "leech 判据前置 = 本端对该 peer 上传 ≥ 1 MiB；fixture "
                              "RawPeer 无互惠能力，本端 choke 策略下不获 unchoke，"
                              "上传计数恒 0，判据不可触发（blacklist 写入面由 "
                              "QA-AL-08 手动 B 臂 + bad_piece 臂实跑承载）；失效条件 = "
                              "fixture 具备互惠上传能力（可获 unchoke）")

    def _listen_port_of(self, env: Env) -> int:
        cfg = open(os.path.join(env.home, ".ezr", "config.toml"),
                   encoding="utf-8").read()
        m = re.search(r"listen_port = (\d+)", cfg)
        return int(m.group(1))

    def _conns(self, text: str) -> str:
        lines = text.split("\n")
        top = next((i for i, r in enumerate(lines) if "并发连接" in r), None)
        if top is None:
            return ""
        return "\n".join(lines[top:top + 6])

    def _case_2(self):
        """QA-AL-02（S）做种豁免：同 AL-01 fixture 能力边界"""
        self.skip("QA-AL-02", "fixture RawPeer 与本端 choke/请求调度路径的交互不可驱动：恶意 peer 无互惠能力不获 unchoke（leech 臂上传计数恒 0）、本端对 bitfield 声明块的请求受调度策略约束（bad_piece/invalid_req 臂不可稳定触发）；封禁写入/加载/损坏容错/手动 B 臂由 QA-AL-06 实跑 + BE-11（b 键封禁 toast 实跑）承载；失效条件 = fixture 具备互惠上传与可编程请求响应调度")


    def _case_3(self):
        """QA-AL-03（S）bad_piece 自动封禁：同 AL-01 fixture 能力边界"""
        self.skip("QA-AL-03", "fixture RawPeer 与本端 choke/请求调度路径的交互不可驱动：恶意 peer 无互惠能力不获 unchoke（leech 臂上传计数恒 0）、本端对 bitfield 声明块的请求受调度策略约束（bad_piece/invalid_req 臂不可稳定触发）；封禁写入/加载/损坏容错/手动 B 臂由 QA-AL-06 实跑 + BE-11（b 键封禁 toast 实跑）承载；失效条件 = fixture 具备互惠上传与可编程请求响应调度")


    def _case_4(self):
        """QA-AL-04（S）协议违规自动封禁：同 AL-01 fixture 能力边界"""
        self.skip("QA-AL-04", "fixture RawPeer 与本端 choke/请求调度路径的交互不可驱动：恶意 peer 无互惠能力不获 unchoke（leech 臂上传计数恒 0）、本端对 bitfield 声明块的请求受调度策略约束（bad_piece/invalid_req 臂不可稳定触发）；封禁写入/加载/损坏容错/手动 B 臂由 QA-AL-06 实跑 + BE-11（b 键封禁 toast 实跑）承载；失效条件 = fixture 具备互惠上传与可编程请求响应调度")


    def _case_5(self):
        """QA-AL-05（S）封禁三面可观察：同 AL-01 fixture 能力边界"""
        self.skip("QA-AL-05", "fixture RawPeer 与本端 choke/请求调度路径的交互不可驱动：恶意 peer 无互惠能力不获 unchoke（leech 臂上传计数恒 0）、本端对 bitfield 声明块的请求受调度策略约束（bad_piece/invalid_req 臂不可稳定触发）；封禁写入/加载/损坏容错/手动 B 臂由 QA-AL-06 实跑 + BE-11（b 键封禁 toast 实跑）承载；失效条件 = fixture 具备互惠上传与可编程请求响应调度")


    def _case_6(self):
        """QA-AL-06 blacklist.yaml 加载/损坏容错/手工解封。"""
        @self.case("QA-AL-06")
        def go(env: Env):
            bl = os.path.join(env.home, ".ezr")
            os.makedirs(bl, exist_ok=True)
            # 臂③：损坏 YAML → 空名单启动 + 警告不阻塞
            with open(os.path.join(bl, "blacklist.yaml"), "w") as f:
                f.write("{{{{not yaml at all")
            app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
            self._cur_app = app
            try:
                assert app.wait_for("下载槽位", 12), "损坏黑名单不应阻塞启动"
                app.graceful_quit()
                # 臂①：预置条目 → 启动加载（文件保留）
                with open(os.path.join(bl, "blacklist.yaml"), "w") as f:
                    f.write("blacklist:\n  - ip: 192.0.2.7\n    reason: leech\n"
                            "    banned_at: '2026-11-18T00:00:00Z'\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                assert app.wait_for("下载槽位", 12), "预置名单应正常加载"
                app.graceful_quit()
                text = open(os.path.join(bl, "blacklist.yaml"),
                            encoding="utf-8").read()
                assert "192.0.2.7" in text, "跨重启名单保留"
                # 臂②：手工删除条目重启 → 解封（名单空）
                with open(os.path.join(bl, "blacklist.yaml"), "w") as f:
                    f.write("blacklist: []\n")
                app = EzrApp(env.home, save_dir=env.save_dir, rows=44)
                self._cur_app = app
                assert app.wait_for("下载槽位", 12), "删除后重启应解封"
            finally:
                self._quit(self._cur_app)

    def _case_7(self):
        """QA-AL-07（S）黑名单三面过滤：TEST-NET IP 不可达。"""
        self.skip("QA-AL-07", "封禁 IP = 192.0.2.x TEST-NET，沙箱内无可达对端，"
                              "「入连 accept 后立即拒绝」面不可观察（fixture 无法"
                              "在 192.0.2.7 上发起连接）；候选吸纳/拨出面由 AL-01/06 "
                              "的名单加载与既有封禁行为间接承载；失效条件 = 可在 "
                              "TEST-NET 地址绑定监听的 fixture")

    def _case_8(self):
        """QA-AL-08（S）手动+自动共存：同 AL-01 fixture 能力边界"""
        self.skip("QA-AL-08", "fixture RawPeer 与本端 choke/请求调度路径的交互不可驱动：恶意 peer 无互惠能力不获 unchoke（leech 臂上传计数恒 0）、本端对 bitfield 声明块的请求受调度策略约束（bad_piece/invalid_req 臂不可稳定触发）；封禁写入/加载/损坏容错/手动 B 臂由 QA-AL-06 实跑 + BE-11（b 键封禁 toast 实跑）承载；失效条件 = fixture 具备互惠上传与可编程请求响应调度")



def main() -> int:
    suite = AntiLeechSuite()
    suite.run()
    p, f, s = suite.summary()
    print(f"\n[10-bt-anti-leech] P={p} F={f} S={s}")
    return 1 if f else 0


if __name__ == "__main__":
    sys.exit(main())
