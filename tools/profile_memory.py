#!/usr/bin/env python3
"""Sample a Voltix child on Linux; run inside the display session being measured.

Example: python3 tools/profile_memory.py --output /tmp/voltix-before.json
The default schedule samples after stabilization, five minutes, and fifteen minutes.
Interact with the app while it runs to measure navigation/mode changes; annotate
those actions separately. Window discovery is deliberately not called first frame.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def sample(pid):
    values = {}
    for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
        fields = line.split()
        if fields and fields[0].rstrip(":") in {"Rss", "Pss", "Private_Clean", "Private_Dirty", "Swap"}:
            values[fields[0].rstrip(":") + "_KiB"] = int(fields[1])
    parts = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    values["cpu_ticks"] = int(parts[11]) + int(parts[12])
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/release/voltix")
    parser.add_argument("--output", required=True)
    parser.add_argument("--times", default="3,300,900", help="seconds after launch, comma-separated")
    args = parser.parse_args()
    times = [float(value) for value in args.times.split(",")]
    if not times or any(t < 0 for t in times) or times != sorted(times):
        parser.error("times must be non-negative and ordered")
    result = {"binary": str(Path(args.binary).resolve()), "display_type": os.environ.get("XDG_SESSION_TYPE"),
              "slint_backend": os.environ.get("SLINT_BACKEND"), "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"), "samples": []}
    start = time.monotonic()
    process = subprocess.Popen([args.binary])
    try:
        for scheduled in times:
            time.sleep(max(0, start + scheduled - time.monotonic()))
            if process.poll() is not None:
                raise RuntimeError(f"Voltix exited with status {process.returncode}")
            result["samples"].append({"elapsed_seconds": time.monotonic() - start, **sample(process.pid)})
            Path(args.output).write_text(json.dumps(result, indent=2) + "\n")
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    main()
