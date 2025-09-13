#!/usr/bin/env python3
"""
Simple UDP receiver for BMW-CAN firmware testing.

Listens on a host/port (default 0.0.0.0:45454) and prints lines
sent by the ESP32 in CSV format: ts_us,0xID,DLC,DATAHEX

Usage:
  python3 tools/udp_recv.py --port 45454
  python3 tools/udp_recv.py --host 0.0.0.0 --port 45454 --raw
"""

import argparse
import socket
import sys
import time


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description="UDP receiver for CAN frame CSV")
    ap.add_argument("--host", default="0.0.0.0", help="bind host (default: 0.0.0.0)")
    ap.add_argument("--port", type=int, default=45454, help="bind port (default: 45454)")
    ap.add_argument("--raw", action="store_true", help="print raw datagrams without parsing")
    ap.add_argument("--ts", action="store_true", help="prepend arrival time (local)")
    return ap.parse_args()


def print_parsed_line(s: str, ts: float | None):
    # Expect: ts_us,0xID,DLC,DATAHEX
    parts = s.strip().split(",")
    if len(parts) != 4:
        print(s)
        return
    ts_us, can_id, dlc, data_hex = parts
    prefix = time.strftime("%H:%M:%S", time.localtime(ts)) + " " if ts is not None else ""
    print(f"{prefix}ts={ts_us} id={can_id} dlc={dlc} data={data_hex}")


def main() -> int:
    args = parse_args()
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind((args.host, args.port))
    print(f"Listening on {args.host}:{args.port} (Ctrl+C to quit)")
    try:
        while True:
            data, addr = sock.recvfrom(4096)
            now = time.time() if args.ts else None
            try:
                text = data.decode("utf-8", errors="replace")
            except Exception:
                sys.stdout.buffer.write(data + b"\n")
                sys.stdout.flush()
                continue
            # Datagrams may contain multiple newline-separated frames
            for line in text.splitlines():
                if not line.strip():
                    continue
                if args.raw:
                    prefix = time.strftime("%H:%M:%S", time.localtime(now)) + " " if now else ""
                    print(prefix + line)
                else:
                    print_parsed_line(line, now)
    except KeyboardInterrupt:
        print("\nBye.")
    finally:
        sock.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

