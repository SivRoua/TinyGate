import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ToolsTest(unittest.TestCase):
    def test_echo_rejects_foreign_truncated_and_oversized_packets(self):
        echo = load("coexist_echo")
        packet = b"TGW1" + bytes(16) + b"\xa5" * 236
        self.assertTrue(echo.valid(packet))
        for data in (packet[:-1], packet + b"x", b"junk" + packet[4:]):
            self.assertFalse(echo.valid(data))

    def test_capture_ignores_credentials_and_malformed_logs(self):
        capture = load("capture_coexist")
        self.assertIsNone(capture.parse("wifi connect ssid|secret"))
        self.assertIsNone(capture.parse("COEX {broken}"))
        result = capture.parse('\x1b[0mI (1) test: COEX {"run":1,"ble_tested":false}')
        self.assertEqual(result["stats"]["run"], 1)
        self.assertFalse(result["stats"]["ble_tested"])


if __name__ == "__main__":
    unittest.main()
