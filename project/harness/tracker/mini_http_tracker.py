#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import socket
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote_to_bytes

DEFAULT_INTERVAL = 30

LOCK = threading.Lock()
SWARMS: dict[bytes, dict[bytes, dict]] = {}


def bencode(obj) -> bytes:
    if isinstance(obj, bool):
        raise TypeError
    if isinstance(obj, int):
        return b"i" + str(obj).encode() + b"e"
    if isinstance(obj, bytes):
        return str(len(obj)).encode() + b":" + obj
    if isinstance(obj, str):
        raw = obj.encode()
        return str(len(raw)).encode() + b":" + raw
    if isinstance(obj, list):
        return b"l" + b"".join(bencode(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        parts = []
        for k in sorted(obj.keys(), key=lambda v: v if isinstance(v, bytes) else v.encode()):
            kb = k if isinstance(k, bytes) else k.encode()
            parts.append(bencode(kb) + bencode(obj[k]))
        return b"d" + b"".join(parts) + b"e"
    raise TypeError(f"unencodable: {type(obj)}")


def parse_raw_query(qs: str) -> dict[str, bytes]:
    out: dict[str, bytes] = {}
    for part in qs.split("&"):
        if not part:
            continue
        k, _, v = part.partition("=")
        out[unquote_to_bytes(k).decode("latin-1")] = unquote_to_bytes(v)
    return out


def compact_peers(peers: list[dict]) -> bytes:
    return b"".join(socket.inet_aton(p["ip"]) + p["port"].to_bytes(2, "big") for p in peers)


class AnnounceHandler(BaseHTTPRequestHandler):
    tracker_log = None
    interval = DEFAULT_INTERVAL

    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        path, _, qs = self.path.partition("?")
        if path != "/announce":
            self._respond(404, bencode({"failure reason": "unknown path"}))
            return
        params = parse_raw_query(qs)
        info_hash = params.get("info_hash")
        peer_id = params.get("peer_id")
        try:
            port = int(params.get("port", b""))
        except ValueError:
            port = 0
        try:
            compact = int(params.get("compact", b"1"))
        except ValueError:
            compact = 1
        try:
            numwant = int(params.get("numwant", b"50"))
        except ValueError:
            numwant = 50
        event = (params.get("event") or b"").decode("latin-1") or None
        left = int(params.get("left", b"0") or b"0")
        uploaded = int(params.get("uploaded", b"0") or b"0")
        downloaded = int(params.get("downloaded", b"0") or b"0")
        ip = self.client_address[0]

        if not info_hash or len(info_hash) != 20:
            self._respond(200, bencode({"failure reason": "missing or bad info_hash"}))
            return
        if not peer_id:
            self._respond(200, bencode({"failure reason": "missing peer_id"}))
            return
        if not port:
            self._respond(200, bencode({"failure reason": "missing port"}))
            return

        with LOCK:
            swarm = SWARMS.setdefault(info_hash, {})
            if event == "stopped":
                swarm.pop(peer_id, None)
            else:
                swarm[peer_id] = {
                    "ip": ip,
                    "port": port,
                    "left": left,
                    "uploaded": uploaded,
                    "downloaded": downloaded,
                    "event": event,
                    "ts": time.time(),
                }
            all_peers = list(swarm.values())
            peers_by_id = {pid: p for pid, p in swarm.items()}

        reply_peers = [p for p in all_peers if not (p["ip"] == ip and p["port"] == port)]
        if numwant >= 0:
            reply_peers = reply_peers[:numwant]
        complete = sum(1 for p in all_peers if p["left"] == 0 or p["event"] == "completed")
        incomplete = len(all_peers) - complete

        body_dict: dict = {
            b"interval": self.interval,
            b"min interval": max(5, self.interval // 2),
            b"complete": complete,
            b"incomplete": incomplete,
        }
        if compact:
            body_dict[b"peers"] = compact_peers(reply_peers)
        else:
            reply_ids = [pid for pid, p in peers_by_id.items() if p in reply_peers]
            body_dict[b"peers"] = [
                {b"peer id": pid, b"ip": peers_by_id[pid]["ip"].encode(), b"port": peers_by_id[pid]["port"]}
                for pid in reply_ids
            ]

        self._log_announce(info_hash, peer_id, port, event, left, uploaded, downloaded)
        self._respond(200, bencode(body_dict))

    def _respond(self, code: int, body: bytes):
        self.send_response(code)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _log_announce(self, info_hash, peer_id, port, event, left, uploaded, downloaded):
        if not self.tracker_log:
            return
        row = {
            "ts": time.strftime("%H:%M:%S"),
            "ip": self.client_address[0],
            "port": port,
            "info_hash": info_hash.hex(),
            "peer_id": peer_id.decode("latin-1"),
            "event": event or "periodic",
            "left": left,
            "uploaded": uploaded,
            "downloaded": downloaded,
        }
        with LOCK:
            with open(self.tracker_log, "a") as f:
                f.write(json.dumps(row) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description="minimal BEP3 HTTP bittorrent tracker (harness fixture)")
    ap.add_argument("--bind", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=17901)
    ap.add_argument("--interval", type=int, default=DEFAULT_INTERVAL)
    ap.add_argument("--log", default=None, help="JSONL announce log path")
    args = ap.parse_args()

    AnnounceHandler.interval = args.interval
    if args.log:
        Path(args.log).parent.mkdir(parents=True, exist_ok=True)
        Path(args.log).write_text("")
        AnnounceHandler.tracker_log = args.log

    srv = ThreadingHTTPServer((args.bind, args.port), AnnounceHandler)
    print(f"http-tracker listening on {args.bind}:{args.port} interval={args.interval}", flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
