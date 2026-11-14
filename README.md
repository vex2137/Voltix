# Voltix

**Version:** `0.1.0-alpha.1`  
**License:** MIT  
**Repository:** `vex2137/Voltix`

Voltix is a native hardware monitoring and diagnostics application written in **Rust** with a **Slint** graphical interface. It is designed for low overhead, unprivileged system telemetry on Linux, with a modular architecture prepared for future Android support.

## Implemented Features (0.1.0 Alpha 1)

- **Real-Time Monitoring Engine (Background Thread)**
  - Dedicated background polling thread with monotonic `Instant` timing and configurable intervals (`250 ms` – `5000 ms`, default `1000 ms`).
  - Explicit metric availability states (`Available`, `Unavailable`, `Unsupported`, `PermissionError`) — missing or unsupported sensors are never masked as `0`.
- **CPU Monitoring**
  - Processor model, vendor, architecture, physical core and logical CPU counts (`/proc/cpuinfo`).
  - Overall and per-logical-CPU utilization calculated from consecutive `/proc/stat` deltas.
  - Per-core and average clock frequencies via `/sys/devices/system/cpu/*/cpufreq/scaling_cur_freq` (with `/proc/cpuinfo` fallback).
  - Package/die temperature from dedicated Linux CPU `hwmon` drivers (`coretemp`, `k10temp`, `zenpower`, `cpu_thermal`, `soc_thermal`).
- **Memory Monitoring**
  - Physical RAM total, available (`MemAvailable`), used, and utilization percentage from `/proc/meminfo`.
  - Swap total, used, and utilization percentage.
- **GPU Monitoring**
  - Optional dynamic NVIDIA monitoring via **NVML** (`nvml-wrapper`), without requiring NVIDIA drivers on non-NVIDIA systems.
  - Multi-GPU selection support and Linux DRM/sysfs discovery (`/sys/class/drm/card*`) for AMD (`amdgpu`) and Intel (`i915` / `xe`) adapters, reporting unavailable/unsupported metrics explicitly.
- **Storage Monitoring**
  - Mounted filesystem capacity, used space, available space, and utilization via `/proc/mounts` and `statvfs`, with backing-device deduplication (e.g., Btrfs subvolumes on CachyOS/Arch Linux).
- **System Information**
  - OS name (`/etc/os-release`), kernel version (`/proc/sys/kernel/osrelease`), architecture, hostname, uptime (`/proc/uptime`), and detected hardware inventory.
- **Native Slint UI & Real-Time Charts**
  - 7 working views: **Dashboard**, **CPU**, **GPU**, **Memory**, **Storage**, **System**, **Settings**.
  - Bounded 60-second history charts rendered natively with Slint `Path` elements (splitting segments cleanly across missing samples).
  - **Dark** (default), **Light**, and **System** theme modes.
  - **English (`en`)** and **Polish (`pl`)** localization with OS locale auto-detection and English fallback.
  - Persistent user preferences in `$XDG_CONFIG_HOME/voltix/settings.json`.

## Technology Stack

- **Language:** Rust (2021 edition)
- **GUI Framework:** Slint (`1.9`)
- **Build System:** Cargo + `slint-build`
- **Telemetry Interfaces:** Linux `/proc`, `/sys` (`hwmon`, `cpufreq`, `drm`), `libc::statvfs`, optional dynamic `libnvidia-ml.so.1` via `nvml-wrapper`

## Linux Requirements & Dependencies

Tested target design: **CachyOS / Arch Linux** (Wayland & X11).

Install the Rust toolchain and native Slint/windowing build dependencies on Arch / CachyOS:

```bash
sudo pacman -S --needed rust cargo base-devel pkgconf fontconfig freetype2 libxkbcommon wayland libx11 libxcursor libxrandr libxi mesa
```

Optional runtime dependency for NVIDIA GPU telemetry:
- `nvidia-utils` (provides `libnvidia-ml.so.1` dynamically loaded at runtime)

Optional runtime data for AMD/Intel PCI device names:
- `hwdata` (provides `/usr/share/hwdata/pci.ids`)

## Build & Verification Instructions

Format check, lint, unit/integration tests, and release build:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Run in development or release mode:

```bash
cargo run
# or
./target/release/voltix
```

## Continuous Integration

GitHub Actions (`.github/workflows/ci.yml`) runs automated Ubuntu Linux checks (`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`) and uploads the compiled `voltix` binary as a workflow artifact.

## Known Limitations (Alpha 1)

- Detailed disk I/O throughput, network monitoring, fan curve control, and benchmarking suites are out of scope for Alpha 1.
- Intel integrated GPU utilization and power counters are marked unsupported via unprivileged sysfs when not exposed by the kernel driver.
- AppImage, distribution packaging, and Android UI targets are planned for future releases and are not included in Alpha 1.

## License

Licensed under the [MIT License](LICENSE).
