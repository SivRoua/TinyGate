import io
import json
from pathlib import Path
import sys
import threading
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import dual_run
sys.path.pop(0)


class DualCaptureTest(unittest.TestCase):
    def test_journal_excludes_credentials_and_keeps_control_evidence(self):
        output = io.StringIO()
        journal = dual_run.Journal(output)
        journal.record("COM8", "wifi connect ssid|private_password")
        journal.record("COM8", "COEX: help wifi connect SSID|PASS")
        journal.record("COM8", "CTRL_RESULT run=1 outcome=timeout")
        journal.record("COM9", 'NODE {"joined":true}')
        records = [json.loads(s) for s in output.getvalue().splitlines()]
        self.assertEqual([r["port"] for r in records], ["COM8", "COM9"])
        self.assertNotIn("private_password", output.getvalue())

    def test_failed_port_open_still_closes_handle(self):
        port = Mock()
        port.open.side_effect = OSError("access denied")
        with patch("serial.Serial", return_value=port):
            with self.assertRaises(OSError):
                dual_run.open_port("COM9")
        port.close.assert_called_once()

    def test_fragmented_node_snapshot_confirms_readiness(self):
        stop = threading.Event()
        ready = threading.Event()
        chunks = [b'NODE {"joined":', b'true}\n']

        class Node:
            port = "COM9"
            in_waiting = 1

            def read(self, size):
                if chunks:
                    return chunks.pop(0)
                stop.set()
                return b""

        errors = []
        output = io.StringIO()
        dual_run.read_node(Node(), dual_run.Journal(output), stop, ready, errors)
        self.assertTrue(ready.is_set())
        self.assertFalse(errors)
        self.assertEqual(len(output.getvalue().splitlines()), 1)


if __name__ == "__main__":
    unittest.main()
