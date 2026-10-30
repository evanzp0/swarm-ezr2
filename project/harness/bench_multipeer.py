#!/usr/bin/env python3
"""Multi-peer speed benchmark: ezr leecher vs N libtorrent seeders + M concurrent leechers.

Reuses swarm_e2e.py fixtures (payload/torrent/trackers/LtPeer/EzrPty).
Progress sampled from ezr's BT sidecar (<download>/<name>.ezr, JSON done_pieces,
saved on the engine's 1s tick).
"""
from __future__ import annotations

import argparse
import re as _re
import hashlib
import json
import shutil
import sys
import time
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS))

from swarm_e2e import (  # noqa: E402
    EzrPty,
    LtPeer,
    make_payload,
    make_torrent,
    log,
    sha256_file,
    wait_port_tcp,
    wait_port_udp,
)


def sidecar_done(path: Path) -> int | None:
    try:
        sc = json.loads(path.read_text())
        return len(sc.get("done_pieces", []))
    except Exception:
        return None


class LoggingLtPeer(LtPeer):
    """LtPeer + peer connect/disconnect alert log (writes peer_alerts.log on shutdown)."""

    def __init__(self, name: str, port: int, torrent: Path, save_dir: Path, listen_ip: str = "0.0.0.0"):
        self._alert_log: list[str] = []
        self._orig_name = name
        super().__init__(name, port, torrent, save_dir)
        self.lt_settings_ip = listen_ip
        self.session.apply_settings({"listen_interfaces": f"{listen_ip}:{port}"})

    def _pump_alerts(self) -> None:
        lt = self.lt
        while not self.stop_flag.is_set():
            for a in self.session.pop_alerts():
                msg = a.message()
                cls = a.__class__.__name__
                if cls in ("peer_connect_alert", "peer_disconnected_alert", "torrent_finished_alert"):
                    self._alert_log.append(f"{time.strftime('%H:%M:%S')} {cls}: {msg}")
                    if cls == "torrent_finished_alert":
                        log(f"{self.name}: torrent finished")
            self.stop_flag.wait(0.2)

    def shutdown(self) -> None:
        super().shutdown()
        try:
            Path(f"{self._orig_name}_alerts.log").write_text("\n".join(self._alert_log) or "(none)")
        except Exception:
            pass


def find_downloaded_file(root: Path) -> Path | None:
    candidates = [p for p in root.rglob("*") if p.is_file() and p.name != "registry.json"]
    if not candidates:
        return None
    return max(candidates, key=lambda p: p.stat().st_size)


def main() -> int:
    ap = argparse.ArgumentParser(description="ezr multi-peer download speed benchmark")
    ap.add_argument("--ezr-bin", required=True)
    ap.add_argument("--seeders", type=int, default=1)
    ap.add_argument("--leechers", type=int, default=0)
    ap.add_argument("--payload-mb", type=int, default=128)
    ap.add_argument("--piece-kib", type=int, default=256)
    ap.add_argument("--timeout", type=float, default=300.0)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--seed-port-base", type=int, default=17903)
    ap.add_argument("--leech-port-base", type=int, default=17920)
    ap.add_argument("--seeder-upload-kbps", type=int, default=0, help="per-seeder upload rate limit KiB/s (0=unlimited)")
    ap.add_argument("--late-seeders", type=int, default=0, help="extra seeders started N seconds after ezr begins downloading")
    ap.add_argument("--late-seeder-delay", type=float, default=15.0)
    args = ap.parse_args()
    args.ezr_bin = str(Path(args.ezr_bin).resolve())

    workdir = Path(args.workdir)
    if workdir.exists():
        shutil.rmtree(workdir)
    workdir.mkdir(parents=True)

    payload = workdir / "payload" / "bench-payload.bin"
    payload.parent.mkdir(parents=True, exist_ok=True)
    log(f"generating {args.payload_mb} MiB payload")
    make_payload(payload, args.payload_mb)
    want_hash = sha256_file(payload)
    total_bytes = args.payload_mb * (1 << 20)

    torrent = workdir / "bench-payload.torrent"
    meta = make_torrent(payload, torrent, piece_size=args.piece_kib * 1024)
    piece_bytes = meta["piece_size"]
    num_pieces = (total_bytes + piece_bytes - 1) // piece_bytes
    log(f"torrent: infohash={meta['infohash']} piece={piece_bytes}B pieces~{num_pieces}")

    import subprocess

    procs: list = []
    peers: list = []
    curve: list[list] = []
    result: dict = {
        "label": f"s{args.seeders}l{args.leechers}",
        "seeders": args.seeders,
        "leechers": args.leechers,
        "payload_mb": args.payload_mb,
        "piece_bytes": piece_bytes,
        "seeder_upload_kbps": args.seeder_upload_kbps,
    }
    try:
        http_proc = subprocess.Popen(
            [sys.executable, str(HARNESS / "tracker" / "mini_http_tracker.py"),
             "--port", "17901", "--interval", "30",
             "--log", str(workdir / "http_announces.jsonl")],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        procs.append(http_proc)
        udp_proc = subprocess.Popen(
            [sys.executable, str(HARNESS / "tracker" / "pybtracker_server.py"),
             "--port", "17902", "--interval", "20"],
            stdout=open(workdir / "udp_tracker.log", "w"), stderr=subprocess.STDOUT,
        )
        procs.append(udp_proc)
        assert wait_port_tcp(17901) and wait_port_udp(17902), "tracker startup failed"

        for i in range(args.seeders):
            seeder = LoggingLtPeer(f"lt-seeder-{i}", args.seed_port_base + i, torrent, payload.parent,
                                   listen_ip="127.0.0.1")
            if args.seeder_upload_kbps > 0:
                seeder.handle.set_upload_limit(args.seeder_upload_kbps * 1024)
                seeder.session.apply_settings({"upload_rate_limit": args.seeder_upload_kbps * 1024})
            if not seeder.wait_seeding(60):
                raise RuntimeError(f"seeder {i} never reached seeding state")
            peers.append(seeder)
        log(f"{args.seeders} seeders seeding (upload_limit={args.seeder_upload_kbps}KiB/s each)")
        late_markers: list[int] = []

        ezr_home = workdir / "ezr_home"
        ezr_out = workdir / "ezr_out"
        (ezr_home / ".ezr").mkdir(parents=True, exist_ok=True)
        ezr_out.mkdir(parents=True, exist_ok=True)
        (ezr_home / ".ezr" / "config.toml").write_text(
            f'download_dir = "{ezr_out}"\nauto_retry = false\nmax_retries = 3\n'
        )

        ezr = EzrPty(ezr_home, [args.ezr_bin])
        if not ezr.wait_for("命令窗口", timeout=10):
            Path(workdir / "ezr_screen_not_ready.txt").write_text(ezr.text())
            raise RuntimeError("TUI not ready")
        ezr.send_key("A")
        ezr.type_text(str(torrent))
        ezr.send_key("Enter")
        if not ezr.wait_for("添加任务：BT", timeout=10):
            Path(workdir / "ezr_screen_no_dialog.txt").write_text(ezr.text())
            raise RuntimeError("BT dialog did not open")
        ezr.send_key("Enter")

        leechers = []
        for i in range(args.leechers):
            leechers.append(LoggingLtPeer(f"lt-leecher-{i}", args.leech_port_base + i, torrent, workdir / f"lt_out_{i}"))
        if leechers:
            log(f"{args.leechers} concurrent libtorrent leechers started")
        late_joined = 0
        next_late_port = args.seed_port_base + args.seeders

        sidecar_glob = ezr_out / "*.ezr"
        screens_log = workdir / "screens.log"
        lt_status_log = workdir / "lt_status.log"
        last_screen_dump = 0.0
        t0 = time.time()
        last_done = 0
        saw_sidecar = False
        peak_window = 0.0
        window_bytes = 0
        window_t0 = t0
        finished = False
        while time.time() - t0 < args.timeout:
            ezr.pump(0.3)
            if ezr.exit_code is not None:
                break
            sidecars = sorted(ezr_out.glob("*.ezr"))
            done = sidecar_done(sidecars[0]) if sidecars else None
            now = time.time()
            if sidecars:
                saw_sidecar = True
            if done is None and saw_sidecar:
                done = num_pieces
            if done is not None and done != last_done:
                curve.append([round(now - t0, 2), done])
                if done > last_done:
                    window_bytes += (done - last_done) * piece_bytes
                last_done = done
                if now - window_t0 >= 2.0:
                    peak_window = max(peak_window, window_bytes / (now - window_t0))
                    window_t0 = now
                    window_bytes = 0
            screen = ezr.text()
            if time.time() - last_screen_dump >= 5.0:
                last_screen_dump = time.time()
                with open(screens_log, "a") as sf:
                    sf.write(f"\n===== t={time.time() - t0:.1f}s =====\n")
                    sf.write(screen)
                if leechers:
                    with open(lt_status_log, "a") as lf:
                        st = leechers[0].handle.status()
                        lf.write(
                            f"t={time.time() - t0:.1f}s dl={st.download_payload_rate / 1e6:.2f}MB/s "
                            f"up={st.upload_payload_rate / 1e6:.2f}MB/s "
                            f"prog={st.progress * 100:.1f}% peers={st.num_peers}\n"
                        )
            m = _re.search(r"并发 (\d+)", screen)
            if m:
                curve.append([round(time.time() - t0, 2), -100 - int(m.group(1))])
            if (late_joined < args.late_seeders and time.time() - t0 >= args.late_seeder_delay):
                late_ip = f"127.0.0.{2 + late_joined}"
                late = LoggingLtPeer(f"lt-late-seeder-{late_joined}", next_late_port, torrent, payload.parent,
                                     listen_ip=late_ip)
                late.handle.set_upload_limit(args.seeder_upload_kbps * 1024)
                late.session.apply_settings({"upload_rate_limit": args.seeder_upload_kbps * 1024})
                next_late_port += 1
                peers.append(late)
                late_joined += 1
                curve.append([round(time.time() - t0, 2), -1 - late_joined])
                log(f"late seeder {late_joined} joined at t={time.time() - t0:.1f}s")
            if "做种中" in screen or "✓ 下载完成" in screen or "已完成 (1)" in screen:
                finished = True
                break
            if done is not None and done >= num_pieces:
                finished = True
                break

        elapsed = time.time() - t0
        Path(workdir / "ezr_screen_final.txt").write_text(ezr.text())
        Path(workdir / "speed_curve.json").write_text(json.dumps(curve))

        ezr.quit_and_wait()
        ezr.close()
        for leecher in leechers:
            leecher.shutdown()
        for seeder in peers:
            seeder.shutdown()

        ezr_file = find_downloaded_file(ezr_out)
        got_hash = sha256_file(ezr_file) if ezr_file else None
        result.update({
            "finished": finished,
            "elapsed_s": round(elapsed, 1),
            "avg_speed_Bps": round(total_bytes / elapsed, 1) if finished else None,
            "peak_2s_speed_Bps": round(peak_window, 1),
            "final_done_pieces": last_done,
            "num_pieces": num_pieces,
            "sha256_match": got_hash == want_hash,
        })
        (workdir / "result.json").write_text(json.dumps(result, indent=2, ensure_ascii=False))
        log("== RESULT ==")
        print(json.dumps(result, indent=2, ensure_ascii=False))
        return 0 if (finished and result["sha256_match"]) else 1
    finally:
        for p in procs:
            if p.poll() is None:
                p.terminate()
        for p in procs:
            try:
                p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                p.kill()


if __name__ == "__main__":
    sys.exit(main())
