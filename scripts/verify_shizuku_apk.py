#!/usr/bin/env python3
"""Verify defined DEX classes and the merged provider, not just source references."""
import argparse
import json
import re
import struct
import zipfile
from pathlib import Path


def defined_classes(data):
    if not data.startswith(b"dex\n") or len(data) < 112:
        raise ValueError("Expected standard DEX")
    if struct.unpack_from("<I", data, 40)[0] != 0x12345678:
        raise ValueError("Expected little-endian DEX")
    strings_count, strings_offset = struct.unpack_from("<II", data, 56)
    types_count, types_offset = struct.unpack_from("<II", data, 64)
    classes_count, classes_offset = struct.unpack_from("<II", data, 96)
    def descriptor(index):
        if index >= types_count:
            raise ValueError("Invalid DEX class type")
        string_index = struct.unpack_from("<I", data, types_offset + index * 4)[0]
        if string_index >= strings_count:
            raise ValueError("Invalid DEX descriptor string")
        offset = struct.unpack_from("<I", data, strings_offset + string_index * 4)[0]
        for _ in range(5):
            byte = data[offset]
            offset += 1
            if byte < 128:
                return data[offset:data.index(b"\0", offset)].decode("utf-8")
        raise ValueError("Invalid DEX string length")
    return {descriptor(struct.unpack_from("<I", data, classes_offset + i * 32)[0])
            for i in range(classes_count)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk")
    parser.add_argument("--manifest-dump", required=True, help="aapt dump xmltree output")
    args = parser.parse_args()
    required = ["rikka/shizuku/Shizuku", "rikka/shizuku/Shizuku$UserServiceArgs",
                "rikka/shizuku/ShizukuProvider", "com/voltix/monitor/VoltixShizukuProvider",
                "com/voltix/monitor/ShizukuGpuBridge", "com/voltix/monitor/ReadOnlyGpuService",
                "com/voltix/monitor/GpuSession", "com/voltix/monitor/IReadOnlyGpuService$Stub"]
    classes = set()
    with zipfile.ZipFile(args.apk) as apk:
        for name in apk.namelist():
            if re.fullmatch(r"classes(?:[0-9]+)?\.dex", name):
                classes.update(defined_classes(apk.read(name)))
        native = apk.read("lib/arm64-v8a/libvoltix.so")
        if b"Advanced GPU Monitoring" not in native:
            raise AssertionError("Advanced GPU UI/localization is missing from native library")
    missing = [name for name in required if "L" + name + ";" not in classes]
    if missing:
        raise AssertionError("Missing DEX class definitions: " + ", ".join(missing))
    manifest = Path(args.manifest_dump).read_text()
    provider = re.search(r"E: provider\b(.*?)(?=\n\s*E:|\Z)", manifest, re.S)
    if not provider:
        raise AssertionError("No provider in merged APK manifest")
    for marker in ["com.voltix.monitor.VoltixShizukuProvider", ".shizuku",
                   "android.permission.INTERACT_ACROSS_USERS_FULL"]:
        if marker not in provider.group():
            raise AssertionError("Provider missing " + marker)
    if not re.search(r"android:exported[^\n]*0xffffffff", provider.group()):
        raise AssertionError("Provider must be exported, protected by framework permission")
    for marker in ["moe.shizuku.client.V3_SUPPORT", "moe.shizuku.manager.permission.API_V23"]:
        if marker not in manifest:
            raise AssertionError("Official provider manifest merge missing " + marker)
    print(json.dumps({"defined_shizuku_classes": required, "provider_verified": True,
                      "advanced_native_ui_verified": True,
                      "limitation": "Binary-content audit; physical Shizuku, POCO sensor access and Android visuals are unverified."}, indent=2))


if __name__ == "__main__":
    main()
