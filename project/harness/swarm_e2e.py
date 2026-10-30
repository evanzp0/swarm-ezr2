#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pty
import random
import select
import shutil
import struct
import subprocess
import sys
import termios
import threading
import time
import fcntl
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
HTTP_PORT = 17901
UDP_PORT = 17902
LT_SEED_PORT = 17903
LT_LEECH_PORT = 17904

PROXY_ENV_KEYS = ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY")


def log(msg: str) -> None:
    print(f"[swarm-e2e {time.strftime('%H:%M:%S')}] {msg}", flush=True)


def sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def make_payload(path: Path, size_mb: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rng = random.Random(0xE27)
    with open(path, "wb") as f:
        remaining = size_mb * (1 << 20)
        while remaining > 0:
            n = min(remaining, 1 << 20)
            f.write(rng.randbytes(n))
            remaining -= n


def make_torrent(payload: Path, torrent: Path, piece_size: int = 262144) -> dict:
    import torf

    t = torf.Torrent(
        path=payload,
        trackers=[
            f"http://127.0.0.1:{HTTP_PORT}/announce",
            f"udp://127.0.0.1:{UDP_PORT}/announce",
        ],
        piece_size=piece_size,
    )
    t.generate()
    t.write(torrent)
    return {"infohash": t.infohash, "valid": True, "piece_size": t.piece_size}


def wait_port_udp(port: int, timeout: float = 5.0) -> bool:
    import socket

    deadline = time.time() + timeout
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.settimeout(1.0)
    while time.time() < deadline:
        try:
            sock.sendto(struct.pack("!QIIH", 0x41727101980, 0, 1, 0x4172), ("127.0.0.1", port))
            sock.recvfrom(64)
            sock.close()
            return True
        except socket.timeout:
            continue
        except OSError:
            time.sleep(0.2)
    sock.close()
    return False


def wait_port_tcp(port: int, timeout: float = 5.0) -> bool:
    import socket

    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.8):
                return True
        except OSError:
            time.sleep(0.2)
    return False


class LtPeer:
    def __init__(self, name: str, port: int, torrent: Path, save_dir: Path):
        import libtorrent as lt

        self.name = name
        self.lt = lt
        settings = {
            "listen_interfaces": f"0.0.0.0:{port}",
            "enable_dht": False,
            "enable_lsd": False,
            "enable_upnp": False,
            "enable_natpmp": False,
            "announce_to_all_trackers": True,
            "announce_to_all_tiers": True,
            "connections_limit": 200,
            "active_downloads": -1,
            "active_seeds": -1,
            "active_limit": -1,
            "alert_mask": lt.alert.category_t.error_notification
            | lt.alert.category_t.status_notification
            | lt.alert.category_t.tracker_notification,
        }
        self.session = lt.session(settings)
        info = lt.torrent_info(str(torrent))
        self.handle = self.session.add_torrent({"ti": info, "save_path": str(save_dir)})
        self.alerts: list[str] = []
        self.stop_flag = threading.Event()
        self.thread = threading.Thread(target=self._pump_alerts, daemon=True)
        self.thread.start()

    def _pump_alerts(self) -> None:
        lt = self.lt
        while not self.stop_flag.is_set():
            for a in self.session.pop_alerts():
                self.alerts.append(a.message())
                if a.__class__.__name__ == "torrent_finished_alert":
                    log(f"{self.name}: torrent finished")
            self.stop_flag.wait(0.2)

    def wait_seeding(self, timeout: float = 60.0) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            st = self.handle.status()
            if st.is_seeding:
                return True
            time.sleep(0.3)
        return False

    def shutdown(self) -> None:
        self.stop_flag.set()
        self.thread.join(timeout=2)
        self.session.pause()


class EzrPty:
    def __init__(self, home: Path, argv: list[str], cols: int = 120, rows: int = 44, env_extra: dict[str, str] | None = None):
        import pyte

        self.home = home
        env = {
            "HOME": str(home),
            "TERM": "xterm-256color",
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "LANG": "C.UTF-8",
        }
        for k in PROXY_ENV_KEYS:
            env[k] = ""
        env["NO_PROXY"] = "*"
        if env_extra:
            env.update(env_extra)
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            try:
                os.chdir(str(home))
                os.environ.clear()
                os.environ.update(env)
                stderr_log = env_extra.get("EZR_STDERR_FILE") if env_extra else None
                if stderr_log:
                    fd2 = os.open(stderr_log, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
                    os.dup2(fd2, 2)
                os.execvp(argv[0], argv)
            except Exception:
                os._exit(127)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        self.screen = pyte.Screen(cols, rows)
        self.stream = pyte.ByteStream(self.screen)
        self.exit_code: int | None = None
        self.pump(0.4)

    def pump(self, dur: float = 0.1) -> None:
        deadline = time.time() + dur
        while time.time() < deadline:
            ready, _, _ = select.select([self.fd], [], [], min(0.05, max(0.0, deadline - time.time())))
            if self.fd not in ready:
                continue
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                data = b""
            if not data:
                self._reap()
                return
            self.stream.feed(data)

    def _reap(self) -> None:
        if self.exit_code is None:
            try:
                wpid, status = os.waitpid(self.pid, os.WNOHANG)
                if wpid == self.pid:
                    self.exit_code = os.waitstatus_to_exitcode(status) if status else 0
            except ChildProcessError:
                pass

    def send_key(self, key: str, gap: float = 0.12) -> None:
        mapping = {"Enter": b"\r", "Tab": b"\t", "Esc": b"\x1b", "Space": b" "}
        raw = mapping.get(key, key.encode())
        os.write(self.fd, raw)
        self.pump(gap)

    def type_text(self, text: str) -> None:
        for ch in text:
            os.write(self.fd, ch.encode())
            self.pump(0.015)

    def text(self) -> str:
        return "\n".join(row.rstrip() for row in self.screen.display)

    def wait_for(self, needle: str, timeout: float = 60.0) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if needle in self.text():
                return True
            self.pump(0.4)
            self._reap()
            if self.exit_code is not None:
                return needle in self.text()
        return needle in self.text()

    def quit_and_wait(self, timeout: float = 8.0) -> int | None:
        for key in ("q", "Q"):
            if self.exit_code is None:
                self.send_key(key, gap=0.3)
        deadline = time.time() + timeout
        while time.time() < deadline and self.exit_code is None:
            self.pump(0.2)
        if self.exit_code is None:
            try:
                os.kill(self.pid, 9)
            except OSError:
                pass
            self.pump(0.5)
        self._reap()
        return self.exit_code

    def close(self) -> None:
        if self.exit_code is None:
            try:
                os.kill(self.pid, 9)
            except OSError:
                pass
        try:
            os.close(self.fd)
        except OSError:
            pass


def find_downloaded_file(root: Path) -> Path | None:
    candidates = [p for p in root.rglob("*") if p.is_file() and p.name != "registry.json"]
    if not candidates:
        return None
    return max(candidates, key=lambda p: p.stat().st_size)


def main() -> int:
    ap = argparse.ArgumentParser(description="BT interop e2e: ezr + third-party trackers + libtorrent control peers")
    ap.add_argument("--ezr-bin", required=True)
    ap.add_argument("--workdir", default=None)
    ap.add_argument("--payload-mb", type=int, default=4)
    ap.add_argument("--timeout", type=float, default=150.0, help="overall download window seconds")
    ap.add_argument("--keep", action="store_true", help="keep workdir for inspection")
    ap.add_argument("--piece-kib", type=int, default=256, help="torrent piece size KiB")
    args = ap.parse_args()
    args.ezr_bin = str(Path(args.ezr_bin).resolve())

    repo_root = HARNESS.parent.parent
    workdir = Path(args.workdir) if args.workdir else repo_root / ".work" / "tmp" / "swarm-e2e"
    if workdir.exists() and not args.keep:
        shutil.rmtree(workdir)
    workdir.mkdir(parents=True, exist_ok=True)

    payload = workdir / "payload" / "swarm-payload.bin"
    payload.parent.mkdir(parents=True, exist_ok=True)
    log(f"generating {args.payload_mb} MiB payload")
    make_payload(payload, args.payload_mb)
    want_hash = sha256_file(payload)

    torrent = workdir / "swarm-payload.torrent"
    meta = make_torrent(payload, torrent, piece_size=args.piece_kib * 1024)
    log(f"torrent written: infohash={meta['infohash']} piece={meta['piece_size']}B")

    procs: list[subprocess.Popen] = []
    results: dict = {
        "infohash": meta["infohash"],
        "payload_sha256": want_hash,
        "trackers": {"http": None, "udp": None},
        "clients": {},
    }
    http_log = workdir / "http_announces.jsonl"
    udp_log = workdir / "udp_tracker.log"

    try:
        http_proc = subprocess.Popen(
            [
                sys.executable,
                str(HARNESS / "tracker" / "mini_http_tracker.py"),
                "--port",
                str(HTTP_PORT),
                "--interval",
                "30",
                "--log",
                str(http_log),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        procs.append(http_proc)
        udp_proc = subprocess.Popen(
            [
                sys.executable,
                str(HARNESS / "tracker" / "pybtracker_server.py"),
                "--port",
                str(UDP_PORT),
                "--interval",
                "20",
            ],
            stdout=open(udp_log, "w"),
            stderr=subprocess.STDOUT,
        )
        procs.append(udp_proc)

        results["trackers"]["http"] = wait_port_tcp(HTTP_PORT)
        results["trackers"]["udp"] = wait_port_udp(UDP_PORT)
        log(f"trackers up: http={results['trackers']['http']} udp={results['trackers']['udp']}")
        if not (results["trackers"]["http"] and results["trackers"]["udp"]):
            raise RuntimeError("tracker startup failed")

        seeder = LtPeer("lt-seeder", LT_SEED_PORT, torrent, payload.parent)
        log("libtorrent seeder added; waiting for seeding state")
        if not seeder.wait_seeding(60):
            raise RuntimeError("libtorrent seeder never reached seeding state")
        log("seeder is seeding")

        ezr_home = workdir / "ezr_home"
        ezr_out = workdir / "ezr_out"
        (ezr_home / ".ezr").mkdir(parents=True, exist_ok=True)
        ezr_out.mkdir(parents=True, exist_ok=True)
        (ezr_home / ".ezr" / "config.toml").write_text(
            f'download_dir = "{ezr_out}"\n'
            f'auto_retry = false\n'
            f'max_retries = 3\n'
        )

        log("starting ezr TUI (pty)")
        ezr = EzrPty(ezr_home, [args.ezr_bin])
        if not ezr.wait_for("命令窗口", timeout=10):
            Path(workdir / "ezr_screen_not_ready.txt").write_text(ezr.text())
            raise RuntimeError(
                f"TUI not ready after 10s (exit_code={ezr.exit_code}); binary={args.ezr_bin}"
            )
        ezr.send_key("A")
        ezr.type_text(str(torrent))
        ezr.send_key("Enter")
        if not ezr.wait_for("添加任务：BT", timeout=10):
            Path(workdir / "ezr_screen_no_dialog.txt").write_text(ezr.text())
            raise RuntimeError("BT dialog did not open after cmd-window route")
        log("BT dialog opened; confirming")
        ezr.send_key("Enter")

        log("starting libtorrent leecher simultaneously")
        leecher = LtPeer("lt-leecher", LT_LEECH_PORT, torrent, workdir / "lt_out")

        ezr_done = {"ok": False, "elapsed": None}
        t0 = time.time()
        while time.time() - t0 < args.timeout:
            ezr.pump(0.4)
            screen = ezr.text()
            if "做种中" in screen or "✓ 下载完成" in screen or "已完成 (1)" in screen:
                ezr_done = {"ok": True, "elapsed": round(time.time() - t0, 1)}
                break
            if ezr.exit_code is not None:
                break
            time.sleep(0.2)
        Path(workdir / "ezr_screen_final.txt").write_text(ezr.text())
        lt_done = leecher.wait_seeding(max(5.0, args.timeout - (time.time() - t0)))

        ezr.quit_and_wait()
        ezr.close()
        leecher.shutdown()
        seeder.shutdown()

        ezr_file = find_downloaded_file(ezr_out)
        lt_file = find_downloaded_file(workdir / "lt_out")
        results["clients"]["ezr"] = {
            "reached_seeding": ezr_done["ok"],
            "elapsed_s": ezr_done["elapsed"],
            "file": str(ezr_file) if ezr_file else None,
            "sha256": sha256_file(ezr_file) if ezr_file else None,
        }
        results["clients"]["libtorrent-leecher"] = {
            "reached_seeding": bool(lt_done),
            "file": str(lt_file) if lt_file else None,
            "sha256": sha256_file(lt_file) if lt_file else None,
        }
        if http_log.exists():
            results["http_announce_rows"] = len(http_log.read_text().splitlines())

        all_ok = (
            results["trackers"]["http"]
            and results["trackers"]["udp"]
            and ezr_done["ok"]
            and results["clients"]["ezr"]["sha256"] == want_hash
            and results["clients"]["libtorrent-leecher"]["sha256"] == want_hash
        )
        (workdir / "result.json").write_text(json.dumps(results, indent=2, ensure_ascii=False))
        log("== RESULT ==")
        print(json.dumps(results, indent=2, ensure_ascii=False))
        log("PASS" if all_ok else "FAIL")
        return 0 if all_ok else 1
    finally:
        for p in procs:
            if p.poll() is None:
                p.terminate()
        for p in procs:
            try:
                p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                p.kill()
        if not args.keep and workdir.exists():
            keepers = {
                "result.json",
                "http_announces.jsonl",
                "udp_tracker.log",
                "ezr_screen_final.txt",
            }
            for f in workdir.iterdir():
                if f.name in keepers:
                    continue
                if f.is_dir():
                    shutil.rmtree(f, ignore_errors=True)
                else:
                    f.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
