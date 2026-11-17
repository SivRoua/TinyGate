"""Run auto_run with a second serial reader and a credential-free event journal."""
import argparse
import json
from pathlib import Path
import re
import sys
import threading
import time

import auto_run


class Journal:
    def __init__(self, output):
        self.output = output
        self.lock = threading.Lock()

    def record(self, port, line):
        line = auto_run.ANSI.sub("", line)
        # Exclude arbitrary echoes and commands, including wifi connect credentials.
        if not re.search(r"\b(?:CTRL_\w+|COEX|NODE|ACTUATOR|SENSOR|WIFI_STATUS|WIFI_SCAN)\b|"
                         r"Report EP|feedback deferred|ESP-ROM:|Guru Meditation|HOST_ERROR", line):
            return
        # COEX help contains placeholders rather than samples; omit it too.
        if "COEX:" in line or "wifi connect " in line:
            return
        with self.lock:
            self.output.write(json.dumps({"port": port, "pc_time_ns": time.time_ns(), "line": line}) + "\n")
            self.output.flush()


class Console(auto_run.Console):
    def __init__(self, port, journal):
        super().__init__(port)
        self.journal = journal

    def line(self, deadline):
        line = super().line(deadline)
        self.journal.record(self.port.port, line)
        return line


def read_node(port, journal, stop, ready, errors):
    pending = bytearray()
    try:
        while not stop.is_set():
            pending.extend(port.read(max(1, min(port.in_waiting, 4096))))
            while b"\n" in pending:
                line, _, pending = pending.partition(b"\n")
                text = auto_run.ANSI.sub("", line.decode("utf-8", errors="replace"))
                journal.record(port.port, text)
                match = re.search(r"\bNODE (\{.*\})", text)
                if match:
                    try:
                        status = json.loads(match[1])
                        if status.get("joined") is True:
                            ready.set()
                    except json.JSONDecodeError:
                        pass
            if len(pending) > 8192:
                raise auto_run.BenchError("Node serial line exceeds 8192 bytes")
    except (OSError, auto_run.BenchError) as error:
        errors.append(str(error))
        journal.record("HOST", "HOST_ERROR node serial capture failed")


def open_port(name):
    import serial
    port = serial.Serial(port=None, baudrate=115200, timeout=0.2, write_timeout=2)
    port.dtr = port.rts = False
    port.port = name
    try:
        port.open()
        port.reset_input_buffer()
    except BaseException:
        port.close()
        raise
    return port


def main(argv=None):
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--node-port", default="COM9")
    parser.add_argument("--journal")
    parser.add_argument("--drain-seconds", type=float, default=5)
    extra, remainder = parser.parse_known_args(argv)
    args = auto_run.arguments(remainder)
    if not 0 <= extra.drain_seconds <= 30:
        raise auto_run.BenchError("drain-seconds must be 0..30")
    if args.port.casefold() == extra.node_port.casefold():
        raise auto_run.BenchError("Gateway and node need distinct serial ports")
    path = Path(args.output or f"test-results/run{args.run}.jsonl")
    journal_path = Path(extra.journal) if extra.journal else path.with_name(path.stem + "-dual.jsonl")
    ports = []
    stop = threading.Event()
    ready = threading.Event()
    errors = []
    thread = None
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        journal_path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("x", encoding="utf-8") as output, journal_path.open("x", encoding="utf-8") as audit:
            journal = Journal(audit)
            for name in (args.port, extra.node_port):
                ports.append(open_port(name))
            thread = threading.Thread(target=read_node, args=(ports[1], journal, stop, ready, errors))
            thread.start()
            try:
                if not ready.wait(10):
                    raise auto_run.BenchError("No joined NODE snapshot on the node port within 10s")
                print(f"[*] Dual capture: gateway={args.port}, node={extra.node_port}, journal={journal_path}", flush=True)
                console = Console(ports[0], journal)
                auto_run.run_bench(args, console, output)
                # Intentional diagnostic tail: retain late SDK callbacks after stop.
                deadline = time.monotonic() + extra.drain_seconds
                while time.monotonic() < deadline:
                    try:
                        line = console.line(deadline)
                    except auto_run.BenchError:
                        if time.monotonic() >= deadline:
                            break
                        raise
                    record = auto_run.parse_coex(line)
                    if record and record["stats"]["run"] == args.run and not record["stats"]["active"]:
                        auto_run.save(output, record)
                if errors:
                    raise auto_run.BenchError("Node capture incomplete: " + errors[0])
            finally:
                stop.set()
                thread.join(timeout=2)
        auto_run.summarize(path, args.run)
        print(f"[*] Dual run completed. Journal: {journal_path}")
        return 0
    except (OSError, auto_run.BenchError) as error:
        print(f"[-] {error}; partial files were preserved.", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("[-] Interrupted; partial files were preserved.", file=sys.stderr)
        return 130
    finally:
        stop.set()
        for port in ports:
            port.close()
        if thread is not None:
            thread.join(timeout=2)


if __name__ == "__main__":
    sys.exit(main())
