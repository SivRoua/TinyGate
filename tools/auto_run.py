"""Run the Wi-Fi/Zigbee bench; preserve captures and hide credentials."""
import argparse
from collections import deque
import ipaddress
import json
import math
from pathlib import Path
import re
import sys
import time

BOUNDS = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2000, 5000]
ANSI = re.compile(r"\x1b\[[0-9;]*m")
HELP = "COEX: start RUN_ID SECONDS"


class BenchError(RuntimeError):
    pass


def parse_coex(line):
    found = re.search(r"\bCOEX (\{.*\})", ANSI.sub("", line))
    if not found:
        return None
    try:
        stats = json.loads(found[1])
    except json.JSONDecodeError:
        return None
    if (not isinstance(stats, dict) or type(stats.get("run")) is not int
            or type(stats.get("active")) is not bool
            or type(stats.get("elapsed_ms")) is not int
            or stats["elapsed_ms"] < 0):
        return None
    return {"source": "COEX", "pc_time_ns": time.time_ns(), "stats": stats}


def percentile_upper(histogram, fraction):
    total = sum(histogram)
    if not total:
        return None
    count = 0
    for i, n in enumerate(histogram):
        count += n
        if count >= total * fraction:
            return BOUNDS[i] if i < len(BOUNDS) else ">5000"


def summarize(jsonl_path, run_id=None):
    runs = {}
    for line in jsonl_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        item = json.loads(line)
        if item.get("source") == "COEX":
            stats = item["stats"]
            if run_id is None or stats["run"] == run_id:
                runs.setdefault(stats["run"], []).append(stats)
    if not runs:
        print("[*] No samples from the requested run; test was not completed.")
        return
    print("\nTEST SUMMARY (counters only; no automatic PASS)")
    for run, rows in runs.items():
        first, last = rows[0], rows[-1]
        result = dict(last)
        result["capture_has_stop_snapshot"] = not last["active"]
        result["queue_drops_during_capture"] = last["queue_drop_total"] - first["queue_drop_total"]
        for protocol in ("udp", "ctrl"):
            for percentile in (50, 95, 99):
                result[f"{protocol}_p{percentile}_upper_ms"] = percentile_upper(
                    last[f"{protocol}_hist"], percentile / 100)
        print(json.dumps(result, indent=2))


def wifi_failure(reason):
    messages = {
        15: "4WAY_HANDSHAKE_TIMEOUT (四次握手超时，检查密码、安全模式及信号)",
        200: "BEACON_TIMEOUT (持续未收到 AP 信标)",
        201: "NO_AP_FOUND (本次未找到目标 AP；检查 2.4GHz、SSID、信号/距离与热点状态，不能仅凭此码确定原因)",
        202: "AUTH_FAIL (鉴权失败，检查凭据与安全模式；不一定只是密码错误)",
        203: "ASSOC_FAIL (关联失败)",
        204: "HANDSHAKE_TIMEOUT (握手超时，检查密码、安全模式及信号)",
        210: "NO_AP_FOUND_W_COMPATIBLE_SECURITY (目标 AP 安全配置不兼容)",
        211: "NO_AP_FOUND_IN_AUTHMODE_THRESHOLD (AP 不满足鉴权模式门槛)",
        212: "NO_AP_FOUND_IN_RSSI_THRESHOLD (AP 不满足信号强度门槛)",
    }
    return f"reason={reason}: {messages.get(reason, '未映射的板端断线原因')}"


class Console:
    """The ordered CLI has no success ACK: use command/status/help as a barrier."""
    def __init__(self, port, clock=time.monotonic, command_timeout=10):
        self.port = port
        self.clock = clock
        self.command_timeout = command_timeout
        self.pending = bytearray()
        self.lines = deque()
        self.synchronized = False
        self.reset_seen = False

    def line(self, deadline):
        while self.clock() < deadline:
            if self.lines:
                text = ANSI.sub("", self.lines.popleft())
                if self.synchronized and re.search(
                        r"ESP-ROM:|rst:0x|COEX ready:|Guru Meditation|abort\(\) was called", text):
                    self.reset_seen = True
                    raise BenchError("Board reboot/panic detected; capture is incomplete.")
                return text
            data = self.port.read(max(1, min(self.port.in_waiting, 4096)))
            self.pending.extend(data)
            while b"\n" in self.pending:
                line, _, self.pending = self.pending.partition(b"\n")
                self.lines.append(line.decode("utf-8", errors="replace"))
            if len(self.pending) > 8192:
                raise BenchError("Serial line exceeded 8192 bytes; check firmware/console.")
        raise BenchError("Timed out waiting for the gateway console/COEX snapshot.")

    def command(self, text, timeout=None):
        # Never echo text: it may contain Wi-Fi credentials.
        if "\n" in text or "\r" in text or len(text.encode("utf-8")) > 256:
            raise BenchError("Invalid serial command length or newline.")
        deadline = self.clock() + (timeout if timeout is not None else self.command_timeout)
        payload = f"{text}\nstatus\nhelp\n".encode("utf-8")
        if self.port.write(payload) != len(payload):
            raise BenchError("Incomplete serial command write.")
        self.port.flush()
        record = None
        rejected = False
        while True:
            line = self.line(deadline)
            if "COEX command rejected:" in line:
                rejected = True
            if "Commands: wifi scan/connect" in line:
                raise BenchError("Wrong firmware: connectivity-demo; flash coexist-test first.")
            current = parse_coex(line)
            if current:
                record = current
            if HELP in line:
                if rejected:
                    raise BenchError("Gateway rejected a configuration command; run was not started/confirmed.")
                if record is None:
                    raise BenchError("COEX status missing before command barrier; check firmware.")
                self.synchronized = True
                return record


def wait_wifi(console, timeout):
    deadline = console.clock() + timeout
    while console.clock() < deadline:
        try:
            line = console.line(deadline)
        except BenchError:
            if console.clock() >= deadline:
                raise BenchError(f"Wi-Fi did not obtain an IP within {timeout:g}s (no fresh IP confirmation).") from None
            raise
        record = parse_coex(line)
        if record is None:
            continue
        stats = record["stats"]
        if stats.get("wifi_has_ip") is True:
            return record
        if stats.get("wifi_reason", 0):
            raise BenchError("Wi-Fi connection failed: " + wifi_failure(stats["wifi_reason"]))
    raise BenchError(f"Wi-Fi did not obtain an IP within {timeout:g}s.")


def save(output, record):
    output.write(json.dumps(record) + "\n")
    output.flush()


def capture(console, output, run_id, seconds, first):
    last = first["stats"]
    save(output, first)
    deadline = console.clock() + seconds + 5
    last_snapshot = console.clock()
    last_progress = 0
    while console.clock() < deadline:
        record = parse_coex(console.line(min(deadline, last_snapshot + 10)))
        if record is None:
            continue
        stats = record["stats"]
        if stats["run"] != run_id or stats["elapsed_ms"] < last["elapsed_ms"]:
            raise BenchError("Run ID changed or elapsed time went backwards; capture is incomplete.")
        save(output, record)
        last = stats
        now = console.clock()
        last_snapshot = now
        if now - last_progress >= 2:
            last_progress = now
            print(f"[*] Run {run_id}: {stats['elapsed_ms'] // 1000}/{seconds}s; "
                  f"ZB={stats.get('zb_unique', 0)}, missing={stats.get('zb_missing', 0)}, "
                  f"control={stats.get('ctrl_ok', 0)}/{stats.get('ctrl_sent', 0)}, "
                  f"Wi-Fi IP={stats.get('wifi_has_ip', False)}", flush=True)
        if not stats["active"]:
            if stats["elapsed_ms"] < seconds * 1000:
                raise BenchError("Run stopped before the requested duration; capture is incomplete.")
            return last
    raise BenchError("No final stop snapshot received; capture is incomplete.")


def run_bench(args, console, output):
    first = console.command("status", timeout=30)
    if first["stats"]["active"]:
        raise BenchError("Another run is active; stop it explicitly before configuring a new run.")
    if first["stats"].get("ctrl_inflight"):
        raise BenchError("Previous control still settling in SDK; wait before starting or reconfiguring Wi-Fi.")
    # No inherited UDP traffic, Wi-Fi connection or historical failure reason.
    for command in ("udp off", "wifi off", f"control {args.control}", f"ui {args.ui}"):
        print(f"[*] Sending: {command}", flush=True)
        console.command(command)
    if args.wifi is not None:
        print("[*] Connecting to Wi-Fi (credentials hidden)...", flush=True)
        fresh = console.command(f"wifi connect {args.wifi}")
        if fresh["stats"].get("wifi_has_ip") is not True:
            if fresh["stats"].get("wifi_reason", 0):
                raise BenchError("Wi-Fi connection failed: " + wifi_failure(fresh["stats"]["wifi_reason"]))
            fresh = wait_wifi(console, args.wifi_timeout)
        print(f"[+] Wi-Fi has IP; RSSI={fresh['stats'].get('wifi_rssi', 0)} dBm", flush=True)
    if args.udp:
        print(f"[*] Sending: udp {args.udp}", flush=True)
        console.command(f"udp {args.udp}")
    owned = False
    completed = False
    try:
        print(f"[*] Sending: start {args.run} {args.seconds}", flush=True)
        first = console.command(f"start {args.run} {args.seconds}")
        if first["stats"]["run"] != args.run or not first["stats"]["active"]:
            raise BenchError("Gateway did not confirm the requested active run.")
        owned = True
        capture(console, output, args.run, args.seconds, first)
        completed = True
    finally:
        if owned and not completed and not console.reset_seen:
            try:
                current = console.command("status")
                if current["stats"]["run"] == args.run and current["stats"]["active"]:
                    stopped = console.command("stop")
                    save(output, stopped)
                    print("[*] Stopped this runner's incomplete run.", flush=True)
            except (BenchError, OSError):
                print("[!] Could not confirm stop; the board's finite-duration run may still be active.", file=sys.stderr)


def arguments(argv=None):
    parser = argparse.ArgumentParser(description="Automate the TinyGate Wi-Fi/Zigbee bench (BLE disabled)")
    parser.add_argument("--port", default="COM8")
    parser.add_argument("--run", type=int, default=101)
    parser.add_argument("--seconds", type=int, default=180)
    parser.add_argument("--control", type=int, default=10, help="Seconds between controls; 0 disables")
    parser.add_argument("--ui", choices=["on", "off"], default="off")
    parser.add_argument("--wifi", help="SSID|PASSWORD; never stored in capture")
    parser.add_argument("--wifi-timeout", type=float, default=30, help="IP wait timeout, seconds")
    parser.add_argument("--udp", help="IPv4 PORT HZ; requires --wifi")
    parser.add_argument("--output", help="New JSONL path; existing files are never overwritten")
    args = parser.parse_args(argv)
    if not 1 <= args.run <= 0xffffffff or not 1 <= args.seconds <= 86400 or not 0 <= args.control <= 3600:
        parser.error("run must be 1..4294967295, seconds 1..86400, control 0..3600")
    if not math.isfinite(args.wifi_timeout) or not 1 <= args.wifi_timeout <= 300:
        parser.error("wifi-timeout must be 1..300 seconds")
    if args.wifi is not None:
        ssid, separator, password = args.wifi.partition("|")
        if (not separator or not 1 <= len(ssid.encode("utf-8")) <= 32
                or (password and not 8 <= len(password.encode("utf-8")) <= 63)
                or any(c in args.wifi for c in "\r\n\x00")):
            parser.error("Invalid Wi-Fi format/UTF-8 byte length; expected SSID|PASSWORD")
    if args.udp:
        try:
            ip, port, hz = args.udp.split()
            address = ipaddress.IPv4Address(ip)
            allowed = any(address in ipaddress.IPv4Network(net) for net in
                          ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8", "169.254.0.0/16"))
            if not allowed or not 1 <= int(port) <= 65535 or not 1 <= int(hz) <= 1000 or args.wifi is None:
                raise ValueError
            args.udp = f"{address} {int(port)} {int(hz)}"
        except ValueError:
            parser.error("udp needs --wifi and 'LAN_IP PORT(1..65535) HZ(1..1000)'")
    return args


def main(argv=None):
    args = arguments(argv)
    path = Path(args.output or f"test-results/run{args.run}.jsonl")
    port = None
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        # Reserve before touching the board. Failed setup may leave an empty file.
        with path.open("x", encoding="utf-8") as output:
            import serial
            port = serial.Serial(port=None, baudrate=115200, timeout=0.2, write_timeout=2)
            port.dtr = port.rts = False
            port.port = args.port
            print(f"[*] Opening {args.port} (115200 baud)...", flush=True)
            port.open()
            port.reset_input_buffer()
            run_bench(args, Console(port), output)
        print(f"[*] Run completed with final stop snapshot: {path}")
        summarize(path, args.run)
        return 0
    except FileExistsError:
        print("[-] Output already exists; choose a new --output. Existing data was preserved.", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print(f"\n[-] Interrupted; capture is incomplete: {path}", file=sys.stderr)
        return 130
    except (BenchError, OSError, ImportError) as exc:
        print(f"[-] {exc}\n[*] Test not completed. Partial/empty capture: {path}", file=sys.stderr)
        return 1
    finally:
        if port is not None:
            port.close()


if __name__ == "__main__":
    sys.exit(main())
