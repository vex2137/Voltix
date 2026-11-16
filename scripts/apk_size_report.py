#!/usr/bin/env python3
"""Compare real APK payloads without unpacking/repacking or external dependencies."""
import argparse
import hashlib
import json
import struct
import zipfile
from pathlib import Path


def inspect(path):
    path = Path(path)
    with zipfile.ZipFile(path) as apk:
        entries = sorted(apk.infolist(), key=lambda entry: entry.file_size, reverse=True)
        libraries = [entry for entry in entries if entry.filename.startswith("lib/") and entry.filename.endswith(".so")]
        result = {
            "file": path.name, "apk_bytes": path.stat().st_size,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "abis": sorted({entry.filename.split("/")[1] for entry in libraries}),
            "native_bytes": sum(entry.file_size for entry in libraries),
            "largest_entries": [{"path": entry.filename, "bytes": entry.file_size,
                                 "compressed_bytes": entry.compress_size, "zip_method": entry.compress_type}
                                for entry in entries[:8]],
        }
        native = apk.read("lib/arm64-v8a/libvoltix.so")
        if native[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", native, 18)[0] != 183:
            raise ValueError("Expected little-endian AArch64 ELF64")
        program_offset = struct.unpack_from("<Q", native, 32)[0]
        program_size, program_count = struct.unpack_from("<HH", native, 54)
        programs = [struct.unpack_from("<IIQQQQQQ", native, program_offset + i * program_size)
                    for i in range(program_count)]
        result["elf_load_alignments"] = [header[7] for header in programs if header[0] == 1]
        offset = struct.unpack_from("<Q", native, 40)[0]
        entry_size, count, names_index = struct.unpack_from("<HHH", native, 58)
        headers = [struct.unpack_from("<IIQQQQIIQQ", native, offset + i * entry_size) for i in range(count)]
        names_header = headers[names_index]
        names = native[names_header[4]:names_header[4] + names_header[5]]
        sections = {names[h[0]:].split(b"\0", 1)[0].decode("ascii"): h[5] for h in headers}
        result["elf_sections"] = {name: size for name, size in sections.items()
                                  if name in (".text", ".rodata", ".symtab", ".strtab", ".dynsym", ".dynstr", ".eh_frame")
                                  or name.startswith(".debug")}
        result["ordinary_symbol_bytes"] = sections.get(".symtab", 0) + sections.get(".strtab", 0)
        result["debug_section_bytes"] = sum(size for name, size in sections.items() if name.startswith(".debug"))
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before")
    parser.add_argument("after", nargs="?")
    parser.add_argument("--output")
    args = parser.parse_args()
    result = {"before": inspect(args.before)}
    if args.after:
        result["after"] = inspect(args.after)
        result["saved_bytes"] = result["before"]["apk_bytes"] - result["after"]["apk_bytes"]
        result["saved_percent"] = round(100 * result["saved_bytes"] / result["before"]["apk_bytes"], 2)
    raw = json.dumps(result, indent=2) + "\n"
    if args.output:
        Path(args.output).write_text(raw)
    print(raw, end="")


if __name__ == "__main__":
    main()
