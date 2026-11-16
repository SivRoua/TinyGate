"""Capture COEX/NODE summaries, never serial commands or credentials. Needs pyserial."""
import argparse
import json
import re
import time
from pathlib import Path


def parse(line):
    line = re.sub(r"\x1b\[[0-9;]*m", "", line)
    found = re.search(r"\b(COEX|NODE) (\{.*\})", line)
    if not found:
        return None
    try:
        obj = json.loads(found[2])
    except json.JSONDecodeError:
        return None
    return {"source": found[1], "pc_time_ns": time.time_ns(), "stats": obj}


def main():
    import serial
    p = argparse.ArgumentParser()
    p.add_argument("port")
    p.add_argument("--seconds", type=int, default=3600)
    p.add_argument("--output", default="test-results/coexist.jsonl")
    args = p.parse_args()
    if args.seconds <= 0:
        p.error("seconds must be positive")
    path = Path(args.output)
    path.parent.mkdir(parents=True, exist_ok=True)
    port = serial.Serial(port=None, baudrate=115200, timeout=0.2)
    port.dtr = port.rts = False
    port.port = args.port
    try:
        port.open()
        deadline = time.monotonic() + args.seconds
        # Exclusive serial ownership: send commands in another session BEFORE capturing.
        with path.open("x", encoding="utf-8") as output:
            pending = bytearray()
            while time.monotonic() < deadline:
                pending.extend(port.read(max(1, min(port.in_waiting, 4096))))
                while b"\n" in pending:
                    line, _, pending = pending.partition(b"\n")
                    record = parse(line.decode("utf-8", errors="replace"))
                    if record:
                        output.write(json.dumps(record) + "\n")
                        output.flush()
                        print(json.dumps(record), flush=True)
                if len(pending) > 8192:
                    pending.clear()
    finally:
        port.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
