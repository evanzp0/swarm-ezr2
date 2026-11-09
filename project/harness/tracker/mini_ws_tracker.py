#!/usr/bin/env python3
"""mini_ws_tracker：ws/wss 双监听的最小 WebSocket tracker（QA-BE-20 行为规格）。

行为规格（qa/09-bt-engine-qa.md QA-BE-20；20261109 对齐真实 WebTorrent 线上协议修订）：
- ws://127.0.0.1:17905/announce 与 wss://127.0.0.1:17906/announce 同时监听；
  wss 以自签证书提供 TLS（客户端按 harness 自签信任口径连接）。
- WebSocket 文本帧 JSON 消息：announce 请求含 action/info_hash/peer_id/port/
  uploaded/downloaded/left/event。id 编码兼容两种线上形态：20 字符二进制串
  （真实 WebTorrent，latin-1 字符）与 40 字符小写 hex（旧契约）。
- 响应含 action:'announce' 与 info_hash（真实 tracker 契约，20261109 与
  webtorrent-cli 6.0.1 实测补齐：官方客户端按 action 分派、对 info_hash 做
  bin2hex，缺任一字段即判 invalid/崩溃并销毁全部 offers）、interval/
  complete/incomplete/peers 列表（peer 含 ip/port/peer_id）。
- 注册多 peer 并互换 peers 下发（同 info_hash 的 swarm，排除发起者自身）。
- 接受 started/completed 事件并按 event 字段记录。
- 信令中继（WebRTC 数据传输用，09-bt-engine-19/20）：
  * offers 元素兼容真实格式 {offer_id, offer:{type,sdp}, to_peer_id?} 与
    旧扁平格式 {offer_id, sdp, to_peer_id}；带 to_peer_id 定向转发；缺省
    （盲 offer）随机转发给 swarm 内其他 peer（对齐真实服务器抽样转发语义）。
  * answer 兼容顶层 {answer:{type,sdp}, offer_id, to_peer_id}（真实协议）与
    旧扁平嵌套 {offer_id, sdp, to_peer_id}；转发为该 peer 响应中的 answers。
  * offer_id 重复投递去重。
- 客户端异常断开不影响服务。
"""
from __future__ import annotations

import argparse
import asyncio
import json
import random
import ssl
import sys
from pathlib import Path

from websockets.asyncio.server import serve

DEFAULT_INTERVAL = 2

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from cert_gen import ensure_self_signed_cert  # noqa: E402

LOCK = asyncio.Lock()
SWARMS: dict[str, dict[str, dict]] = {}
INBOX: dict[str, list[dict]] = {}
CONNS: dict[str, object] = {}  # peer_key(hex) → ws 连接（顶层推送用）


def bin2hex_id(s: str) -> str:
    """20 字符二进制串（真实 WebTorrent）→ 40 hex；40 hex 原样小写。"""
    if len(s) == 20:
        return "".join(f"{ord(c):02x}" for c in s)
    return s.lower()


async def push_signal(to_key: str | None, msg: dict, relay: dict) -> None:
    """按真实 WebTorrent 服务器语义：offer/answer 作为独立顶层消息即时推送给目标。
    （官方 bittorrent-tracker 客户端只认顶层 offer/answer 字段；捎带数组
    仅 ezr/harness 契约认。推送失败忽略——对端可能已断开。）"""
    if not to_key:
        return
    ws = CONNS.get(to_key)
    if ws is None:
        return
    payload: dict = {
        "action": "announce",
        "info_hash": msg.get("info_hash"),
        "peer_id": relay["from_raw"],
    }
    if relay["kind"] == "offer":
        payload["offer"] = {"type": "offer", "sdp": relay["sdp"]}
        payload["offer_id"] = relay["offer_id"]
    else:
        payload["answer"] = {"type": "answer", "sdp": relay["sdp"]}
        payload["offer_id"] = relay["offer_id"]
    try:
        await ws.send(json.dumps(payload))
    except Exception:
        pass


def sdp_of(entry: dict) -> str | None:
    """从 offer/answer 元素提取 SDP：兼容对象形态（offer:{type,sdp}）与扁平形态。"""
    for key in ("offer", "answer"):
        o = entry.get(key)
        if isinstance(o, dict) and isinstance(o.get("sdp"), str):
            return o["sdp"]
    sdp = entry.get("sdp")
    return sdp if isinstance(sdp, str) else None


async def handle_announce(swarm: str, peer_id: str, msg: dict) -> dict:
    async with LOCK:
        peers = SWARMS.setdefault(swarm, {})
        entry = {
            "ip": msg.get("ip") or "127.0.0.1",
            "port": int(msg.get("port") or 0),
            "event": msg.get("event"),
            "left": msg.get("left"),
            "raw_id": str(msg.get("peer_id") or ""),
        }
        peers[peer_id] = entry
        others = [
            {"ip": p["ip"], "port": p["port"], "peer_id": pid}
            for pid, p in peers.items()
            if pid != peer_id
        ]
        # 真实 WebTorrent tracker（bittorrent-tracker server）响应契约：
        # action:'announce' 与 info_hash 必须回带——官方客户端 _onSocketData
        # 按 action 分派（缺省判 invalid action 并销毁全部 offers），
        # _onAnnounceResponse 对 info_hash 做 bin2hex（缺省直接 TypeError 崩溃）。
        # （20261109 与 webtorrent-cli 6.0.1 实测定位，见 notes/architecture.md）
        resp = {
            "action": "announce",
            "info_hash": msg.get("info_hash"),
            "interval": DEFAULT_INTERVAL,
            "complete": sum(1 for p in peers.values() if p.get("left") == 0),
            "incomplete": sum(1 for p in peers.values() if p.get("left") != 0),
            "peers": others,
        }
        inbox = [m for m in INBOX.get(peer_id, []) if m.get("to") == peer_id]
        INBOX[peer_id] = [m for m in INBOX.get(peer_id, []) if m not in inbox]
        offers = [m for m in inbox if m.get("kind") == "offer"]
        answers = [m for m in inbox if m.get("kind") == "answer"]
        if offers:
            resp["offers_to_answer"] = [
                {"offer_id": m["offer_id"], "sdp": m["sdp"], "peer_id": m["from"]}
                for m in offers
            ]
        if answers:
            resp["answers"] = [
                {"offer_id": m["offer_id"], "sdp": m["sdp"], "peer_id": m["from"]}
                for m in answers
            ]
        other_ids = [pid for pid in peers if pid != peer_id]
        for o in msg.get("offers") or []:
            sdp = sdp_of(o)
            if sdp is None:
                continue
            to = o.get("to_peer_id")
            to = bin2hex_id(str(to)) if to else (random.choice(other_ids) if other_ids else None)
            relay = {
                "kind": "offer",
                "offer_id": o["offer_id"],
                "sdp": sdp,
                "from": peer_id,
                "from_raw": entry["raw_id"],
                "to": to,
            }
            bucket = INBOX.setdefault(relay["to"], [])
            if relay["to"] and all(m["offer_id"] != relay["offer_id"] for m in bucket):
                bucket.append(relay)
                await push_signal(relay["to"], msg, relay)
        a = msg.get("answer")
        if a:
            sdp = sdp_of(a)
            offer_id = msg.get("offer_id") or a.get("offer_id")
            to = msg.get("to_peer_id") or a.get("to_peer_id")
            if sdp is not None and offer_id is not None and to:
                relay = {
                    "kind": "answer",
                    "offer_id": offer_id,
                    "sdp": sdp,
                    "from": peer_id,
                    "from_raw": entry["raw_id"],
                    "to": bin2hex_id(str(to)),
                }
                bucket = INBOX.setdefault(relay["to"], [])
                if all(m["offer_id"] != relay["offer_id"] for m in bucket):
                    bucket.append(relay)
                    await push_signal(relay["to"], msg, relay)
        return resp


async def on_conn(ws) -> None:
    peer_id = ""
    swarm = ""
    try:
        async for raw in ws:
            try:
                msg = json.loads(raw)
            except (json.JSONDecodeError, TypeError):
                continue
            if msg.get("action") != "announce":
                continue
            peer_id = bin2hex_id(str(msg.get("peer_id") or ""))
            swarm = bin2hex_id(str(msg.get("info_hash") or ""))
            if not peer_id or not swarm:
                continue
            CONNS[peer_id] = ws
            resp = await handle_announce(swarm, peer_id, msg)
            await ws.send(json.dumps(resp))
    except Exception:
        pass
    finally:
        if peer_id and swarm:
            async with LOCK:
                SWARMS.get(swarm, {}).pop(peer_id, None)
            CONNS.pop(peer_id, None)


def make_wss_ssl(cert: Path, key: Path) -> ssl.SSLContext:
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert, key)
    return ctx


async def main() -> None:
    ap = argparse.ArgumentParser(description="mini ws/wss tracker")
    ap.add_argument("--ws-port", type=int, default=17905)
    ap.add_argument("--wss-port", type=int, default=17906)
    ap.add_argument("--cert-dir", default=str(Path(__file__).resolve().parent / "certs"))
    args = ap.parse_args()

    cert, key = ensure_self_signed_cert(Path(args.cert_dir))

    async with serve(on_conn, "127.0.0.1", args.ws_port) as ws_srv, serve(
        on_conn, "127.0.0.1", args.wss_port, ssl=make_wss_ssl(cert, key)
    ) as wss_srv:
        print(
            f"mini_ws_tracker ready ws://127.0.0.1:{ws_srv.sockets[0].getsockname()[1]}/announce "
            f"wss://127.0.0.1:{wss_srv.sockets[0].getsockname()[1]}/announce",
            flush=True,
        )
        await asyncio.Future()


if __name__ == "__main__":
    asyncio.run(main())
