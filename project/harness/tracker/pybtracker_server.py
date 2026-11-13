#!/usr/bin/env python3
from __future__ import annotations

import argparse
import asyncio
import logging
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "pybtracker"))

from pybtracker import TrackerServer


async def serve(host: str, port: int, interval: int) -> None:
    tracker = TrackerServer(local_addr=(host, port), interval=interval)
    await tracker.start()
    await asyncio.Event().wait()


def main() -> int:
    ap = argparse.ArgumentParser(description="pybtracker launcher (BEP15 UDP tracker, third-party elektito/pybtracker)")
    ap.add_argument("--bind", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=17902)
    ap.add_argument("--interval", type=int, default=20)
    ap.add_argument("--log-level", default="info", choices=["debug", "info", "warning", "error"])
    args = ap.parse_args()

    logging.basicConfig(
        level=getattr(logging, args.log_level.upper()),
        format="%(asctime)s %(levelname)s %(name)s %(message)s",
        stream=sys.stdout,
    )
    try:
        asyncio.run(serve(args.bind, args.port, args.interval))
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
