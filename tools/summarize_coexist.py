"""Summarize captured counters; never labels a run PASS without its test requirements."""
import argparse
import json
from pathlib import Path
BOUNDS = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2000, 5000]


def percentile_upper(histogram, fraction):
    total = sum(histogram)
    if not total:
        return None
    count = 0
    for i, n in enumerate(histogram):
        count += n
        if count >= total * fraction:
            return BOUNDS[i] if i < len(BOUNDS) else ">5000"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("capture", type=Path)
    args = p.parse_args()
    runs = {}
    for line in args.capture.read_text(encoding="utf-8").splitlines():
        item = json.loads(line)
        if item["source"] != "COEX":
            continue
        stats = item["stats"]
        runs.setdefault(stats["run"], []).append(stats)
    for run, rows in runs.items():
        first, last = rows[0], rows[-1]
        result = dict(last)
        result["capture_has_stop_snapshot"] = not last["active"]
        result["queue_drops_during_capture"] = last["queue_drop_total"] - first["queue_drop_total"]
        for protocol in ("udp", "ctrl"):
            for percentile in (50, 95, 99):
                result[f"{protocol}_p{percentile}_upper_ms"] = percentile_upper(last[f"{protocol}_hist"], percentile / 100)
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
