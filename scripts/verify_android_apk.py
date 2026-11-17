#!/usr/bin/env python3
"""Audit the installable APK and Gradle graph for removed privileged integration."""
import argparse
import json
import re
import zipfile
from pathlib import Path

FORBIDDEN = [b"shizuku", b"nativegpucommand", b"readonlygpuservice",
             b"ireadonlygpuservice", b"com/voltix/monitor/gpusession",
             b"com.voltix.monitor.gpusession", b"advanced gpu monitoring"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk")
    parser.add_argument("--manifest-dump", required=True)
    parser.add_argument("--dependencies", required=True)
    args = parser.parse_args()
    manifest = Path(args.manifest_dump).read_text().lower()
    dependencies = Path(args.dependencies).read_text().lower()
    assert "shizuku" not in dependencies, "Removed dependency remains on runtime classpath"
    for marker in FORBIDDEN + [b"interact_across_users_full"]:
        assert marker.decode() not in manifest, f"Removed manifest declaration: {marker!r}"
    checked = []
    with zipfile.ZipFile(args.apk) as apk:
        for entry in apk.namelist():
            assert b"shizuku" not in entry.lower().encode(), entry
            if re.fullmatch(r"classes[0-9]*\.dex", entry) or entry.endswith(".so"):
                data = apk.read(entry).lower()
                for marker in FORBIDDEN:
                    assert marker not in data, f"Removed integration remains in {entry}: {marker!r}"
                checked.append(entry)
        assert "lib/arm64-v8a/libvoltix.so" in checked
        assert any(name.endswith(".dex") for name in checked)
    print(json.dumps({"removed_integration_absent": True, "manifest_verified": True,
                      "runtime_dependencies_verified": True, "checked_payloads": checked}, indent=2))


if __name__ == "__main__":
    main()
