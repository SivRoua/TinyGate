"""Exercise serial ordering, stale Wi-Fi state, and incomplete captures without a board."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("auto_run", ROOT / "tools/auto_run.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def stats(**overrides):
    value = dict(run=0, active=False, elapsed_ms=0, wifi_has_ip=False, wifi_reason=0,
                 wifi_rssi=0, queue_drop_total=0, zb_unique=0, zb_missing=0,
                 ctrl_ok=0, ctrl_sent=0, heap=100000, udp_hist=[0]*12,
                 ctrl_hist=[0]*12, ble_tested=False)
    value.update(overrides)
    return value


def log(value):
    return "COEX " + json.dumps(value) + "\n"


class Clock:
    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now


class Board:
    """Small model of the existing ordered CLI and its asynchronous snapshots."""
    def __init__(self, wifi_reason=0, failure=0, reject=None, wrong_mode=False):
        self.clock = Clock()
        self.rx = bytearray(log(stats(wifi_reason=wifi_reason)).encode())
        self.state = stats(wifi_reason=wifi_reason)
        self.commands = []
        self.wifi_due = None
        self.failure = failure
        self.reject = reject
        self.wrong_mode = wrong_mode
        self.started = None
        self.duration = None
        self.next_stats = 0
        self.closed = False

    @property
    def in_waiting(self):
        return len(self.rx)

    def emit(self, text):
        self.rx.extend(text.encode())

    def write(self, data):
        for command in data.decode().splitlines():
            self.commands.append(command)
            if command == self.reject:
                self.emit("COEX command rejected: mocked rejection\n")
            elif command == "status":
                self.emit(log(self.state))
            elif command == "help":
                self.emit(("Commands: wifi scan/connect" if self.wrong_mode else runner.HELP) + "\n")
            elif command == "wifi off":
                self.state.update(wifi_has_ip=False, wifi_reason=0)
                self.wifi_due = None
            elif command.startswith("wifi connect "):
                self.state.update(wifi_has_ip=False, wifi_reason=0)
                self.wifi_due = self.clock() + 0.5
            elif command.startswith("start "):
                _, run, seconds = command.split()
                self.state.update(run=int(run), active=True, elapsed_ms=0)
                self.started = self.clock()
                self.duration = int(seconds)
                self.next_stats = self.clock() + 0.5
            elif command == "stop":
                self.state["active"] = False
        return len(data)

    def read(self, size):
        self.clock.now += 0.1
        if self.wifi_due is not None and self.clock() >= self.wifi_due:
            self.wifi_due = None
            self.state.update(wifi_has_ip=not self.failure, wifi_reason=self.failure, wifi_rssi=-50)
            self.emit(log(self.state))
        if self.state["active"] and self.clock() >= self.next_stats:
            elapsed = int((self.clock() - self.started)*1000)
            self.state.update(elapsed_ms=elapsed, active=elapsed < self.duration*1000)
            self.next_stats = self.clock() + 0.5
            self.emit(log(self.state))
        result = bytes(self.rx[:size])
        del self.rx[:size]
        return result

    def flush(self):
        pass

    def open(self):
        pass

    def close(self):
        self.closed = True

    def reset_input_buffer(self):
        self.rx.clear()


class RunnerTest(unittest.TestCase):
    def setUp(self):
        self.quiet = contextlib.redirect_stdout(io.StringIO())
        self.quiet.__enter__()

    def tearDown(self):
        self.quiet.__exit__(None, None, None)

    def args(self, *extra):
        return runner.arguments(["--run", "102", "--seconds", "1", "--control", "0", *extra])

    def execute(self, board, *extra):
        output = io.StringIO()
        runner.run_bench(self.args(*extra), runner.Console(board, clock=board.clock), output)
        return [json.loads(line) for line in output.getvalue().splitlines()]

    def test_stale_no_ap_is_ignored_and_only_requested_run_is_saved(self):
        board = Board(wifi_reason=201)
        records = self.execute(board, "--wifi", "Test Network|test_password")
        self.assertTrue(all(row["stats"]["run"] == 102 for row in records))
        self.assertTrue(records[0]["stats"]["wifi_has_ip"])
        self.assertFalse(records[-1]["stats"]["active"])
        self.assertLess(board.clock(), 4)  # No extra five-second capture after completion.
        self.assertLess(board.commands.index("wifi off"), board.commands.index("wifi connect Test Network|test_password"))

    def test_current_no_ap_is_a_failure_and_no_test_is_started(self):
        board = Board(failure=201)
        with self.assertRaisesRegex(runner.BenchError, "reason=201.*NO_AP_FOUND"):
            self.execute(board, "--wifi", "Network|test_password")
        self.assertFalse(any(c.startswith("start ") for c in board.commands))

    def test_ip_timeout_does_not_start_the_run(self):
        board = Board()
        original_write = board.write

        def write(data):
            count = original_write(data)
            board.wifi_due = None  # Association/DHCP never produces an IP or failure.
            return count

        board.write = write
        with self.assertRaisesRegex(runner.BenchError, "Wi-Fi did not obtain an IP within 1s"):
            self.execute(board, "--wifi", "Network|test_password", "--wifi-timeout", "1")
        self.assertFalse(any(c.startswith("start ") for c in board.commands))

    def test_no_snapshot_amid_non_coex_noise_is_incomplete(self):
        board = Board()
        board.rx.clear()
        original_read = board.read

        def read(size):
            board.emit("unrelated SDK log\n")
            return original_read(size)

        board.read = read
        console = runner.Console(board, clock=board.clock)
        with self.assertRaises(runner.BenchError):
            runner.capture(console, io.StringIO(), 102, 180,
                           dict(source="COEX", stats=stats(run=102, active=True)))
        self.assertLess(board.clock(), 11)

    def test_configuration_rejection_prevents_start(self):
        board = Board(reject="control 0")
        with self.assertRaisesRegex(runner.BenchError, "rejected"):
            self.execute(board)
        self.assertFalse(any(c.startswith("start ") for c in board.commands))

    def test_start_rejection_cannot_be_mistaken_for_an_old_snapshot(self):
        with self.assertRaisesRegex(runner.BenchError, "rejected"):
            self.execute(Board(reject="start 102 1"))

    def test_wrong_firmware_is_reported_before_configuration(self):
        board = Board(wrong_mode=True)
        with self.assertRaisesRegex(runner.BenchError, "Wrong firmware"):
            self.execute(board)
        self.assertNotIn("wifi off", board.commands)

    def test_active_run_is_not_reconfigured_or_stopped(self):
        board = Board()
        board.state.update(run=999, active=True)
        board.next_stats = 100
        with self.assertRaisesRegex(runner.BenchError, "Another run"):
            self.execute(board)
        self.assertEqual(board.commands, ["status", "status", "help"])

    def test_settling_control_prevents_wifi_reconfiguration(self):
        board = Board()
        board.state["ctrl_inflight"] = True
        with self.assertRaisesRegex(runner.BenchError, "still settling"):
            self.execute(board, "--wifi", "Network|test_password")
        self.assertEqual(board.commands, ["status", "status", "help"])

    def test_early_stop_and_foreign_run_are_incomplete(self):
        for value in (stats(run=102, active=False, elapsed_ms=500),
                      stats(run=999, active=True, elapsed_ms=1000)):
            board = Board()
            board.rx = bytearray(log(value).encode())
            console = runner.Console(board, clock=board.clock)
            with self.assertRaises(runner.BenchError):
                runner.capture(console, io.StringIO(), 102, 1,
                               dict(source="COEX", stats=stats(run=102, active=True)))

    def test_reboot_aborts_capture(self):
        board = Board()
        board.rx = bytearray(b"ESP-ROM:esp32c6\n")
        console = runner.Console(board, clock=board.clock)
        console.synchronized = True
        with self.assertRaisesRegex(runner.BenchError, "reboot"):
            console.line(2)
        self.assertTrue(console.reset_seen)

    def test_existing_output_is_preserved_without_opening_serial(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/"existing.jsonl"
            path.write_text("keep", encoding="utf-8")
            with patch("serial.Serial") as serial_class, contextlib.redirect_stderr(io.StringIO()):
                code = runner.main(["--output", str(path)])
            self.assertEqual(code, 1)
            serial_class.assert_not_called()
            self.assertEqual(path.read_text(), "keep")

    def test_main_closes_serial_and_does_not_expose_credentials(self):
        board = Board(failure=201)
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as temp, patch("serial.Serial", return_value=board):
            path = Path(temp)/"failed.jsonl"
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
                code = runner.main(["--output", str(path), "--wifi", "Network|private_test_password"])
            self.assertEqual(code, 1)
            self.assertNotIn("private_test_password", path.read_text())
        self.assertNotIn("private_test_password", output.getvalue())
        self.assertTrue(board.closed)

    def test_invalid_arguments_cannot_inject_serial_commands(self):
        for extra in (("--seconds", "0"), ("--run", "-1"), ("--wifi-timeout", "nan"),
                      ("--wifi", "SSID|password\nstart 999 10"),
                      ("--wifi", "界"*11 + "|password"),
                      ("--udp", "192.168.1.1 9000 10")):
            with self.subTest(extra=extra), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.args(*extra)

    def test_fragmented_serial_line_and_oversized_line(self):
        board = Board()
        board.rx = bytearray(b"COEX {\"run\":102,")
        original_read = board.read
        called = [False]

        def read(size):
            data = original_read(size)
            if not called[0]:
                called[0] = True
                board.emit('"active":true,"elapsed_ms":0}\n')
            return data

        board.read = read
        console = runner.Console(board, clock=board.clock)
        self.assertEqual(runner.parse_coex(console.line(2))["stats"]["run"], 102)
        board.rx = bytearray(b"x"*9000)
        with self.assertRaisesRegex(runner.BenchError, "8192"):
            console.line(5)


if __name__ == "__main__":
    unittest.main()
