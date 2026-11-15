# Voltix

**Version:** `0.1.0-alpha.3`
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

## Packaging & Cross-Platform Targets

### Linux x86_64 AppImage

- Built on an **Ubuntu 22.04** (`GLIBC 2.35`) baseline for broad compatibility across Linux distributions (including CachyOS / Arch Linux, Fedora, Debian 12+, and Ubuntu 22.04+), supporting both Wayland and X11.
- Desktop metadata and icon live in `packaging/linux/voltix.desktop` and `assets/icons/voltix.svg`.

### Android ARM64 (`com.voltix.monitor`)

- **Target ABI:** `arm64-v8a` (`aarch64-linux-android`)
- **Minimum SDK:** API 26 (Android 8.0)
- **Slint Backend:** `backend-android-activity-06` via `android.app.NativeActivity` (`src/lib.rs`)
- **Responsive UI:** Automatically switches from the desktop sidebar to a touch-friendly bottom navigation bar and stacked card grids on narrow screens.
- **Unprivileged Telemetry:** Uses Bionic system properties (`__system_property_get`), `libc::uname`, `libc::sysinfo`, `statvfs`, and readable thermal zones without requiring root access. On Android 8+, SELinux restricts `/proc/stat`, `/proc/meminfo` (on some devices), and vendor GPU counters; restricted values are explicitly shown as `Permission Denied` or `Unsupported` rather than fabricated.

Build the Android native library and debug APK locally (requires Android SDK 35, NDK r27, `cargo-ndk`, and Gradle 8.9):

```bash
rustup target add aarch64-linux-android
cargo ndk -t arm64-v8a -p 26 -o android/app/src/main/jniLibs build --lib --release
gradle -p android assembleDebug --no-daemon
```

## Continuous Integration

GitHub Actions (`.github/workflows/ci.yml`) runs two automated jobs:

1. **Linux x86_64 (`ubuntu-22.04`):** `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`, `desktop-file-validate`, `linuxdeploy` AppImage packaging (`Voltix-0.1.0-alpha.3-x86_64.AppImage`), headless `xvfb-run` launch verification, and artifact uploads (`voltix-linux-x86_64`, `voltix-linux-x86_64-appimage`).
2. **Android ARM64-v8a:** `cargo ndk` Clippy check and `--lib --release` build, Gradle `assembleDebug` (`Voltix-0.1.0-alpha.3-arm64.apk`), APK native `.so` verification, and artifact upload (`voltix-android-arm64-apk`).

## Known Limitations (Alpha 1)

- Detailed disk I/O throughput, network monitoring, fan curve control, and benchmarking suites are out of scope for Alpha 1.
- Intel integrated GPU utilization and power counters are marked unsupported via unprivileged sysfs when not exposed by the kernel driver.
- On Android 8+ (API 26+), SELinux policies restrict direct access to `/proc/stat`, `cpufreq`, and proprietary GPU counters for non-privileged apps, so Voltix reports those metrics honestly as `Permission Denied` or `Unsupported`.

## License

Licensed under the [MIT License](LICENSE).

## Android Alpha 2 updates

The package ID remains `com.voltix.monitor`; Alpha 3 development uses version code 4.
Release builds require the same private signing keystore used for the installed
release. Configure all four CI secrets: `VOLTIX_KEYSTORE_BASE64`,
`VOLTIX_KEYSTORE_PASSWORD`, `VOLTIX_KEY_ALIAS`, and `VOLTIX_KEY_PASSWORD`.
Local release builds use `VOLTIX_KEYSTORE_PATH` instead of the Base64 secret.
Keep this key backed up securely and reuse it for every release.

CI verifies the APK signature, package ID, version code and ARM64 library.
Without signing secrets, CI produces an explicitly named `-debug.apk` for testing;
fresh runners generate different debug certificates, so these APKs cannot be
relied on for in-place updates. Partial signing configuration fails the build.
An installed APK signed with another key cannot be updated while preserving its
data unless the original key (or a supported signing-key rotation) is available.
A higher version code alone cannot resolve a certificate mismatch.

## Alpha 3 development — interface milestone

Interface Mode in Appearance switches immediately between Modern (balanced cards),
Professional (dense current readings, GPU clocks/power and CPU core rows) and Gaming
(performance summary and larger CPU/GPU history charts). All modes share the same
monitoring engine. Four accent choices and interface mode persist in settings;
older settings files migrate to Modern. Desktop content is capped at 1200 logical
pixels and key/value groups keep readings near their labels. Existing mobile
navigation, scrolling and safe-area handling remain in place.

Gaming FPS/frametime explicitly show unavailable: no game telemetry provider is
connected. The MangoHud backend, dedicated overlay editor, game launch integration,
metric ordering, min/max tables and reduced-animation preference are **not yet
implemented** in this milestone. No overlay controls pretend to enable injection.
These remain work within Alpha 3. Existing UI uses no graph interpolation animation.

For opt-in development timing, run `VOLTIX_PROFILE_STARTUP=1 cargo run --release`.
Logs distinguish process entry, Slint component creation, monitoring-engine creation,
event-loop entry and first telemetry applied to the UI. These are initialization
milestones, **not first-frame presentation** or an actual usable-interface benchmark.
No first-frame or sub-500ms claim is made. Normal runs do not emit timing logs.
NVIDIA power draw uses NVML `power_usage` (mW converted to W), separately from the
enforced/management power limit. Unsupported measurements remain unavailable;
validation on real NVIDIA hardware is still required.

### MangoHud clipboard export (Linux)

AppImages bundle clipboard helpers. For development builds, install `wl-clipboard` for Wayland or `xclip` for X11 (`sudo pacman -S wl-clipboard xclip` on CachyOS).
In Settings, “Include MangoHud in launch options” enables MangoHud and copies the command.
After changing metrics or position, use “Copy command” again. Keep `%command%` for Steam;
for other launchers enter the game command with its arguments and shell quoting.
Voltix copies text only and never executes it or writes MangoHud configuration files.
Clipboard failures are shown in Settings rather than reported as success.

The AppImage uses the `voltix` desktop/application ID and the official bundled icon.
On Plasma Wayland, install/integrate the bundled `voltix.desktop` and icon through your
AppImage manager so Plasma can resolve that ID to its desktop entry.
