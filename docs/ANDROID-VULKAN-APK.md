# Android Vulkan diagnostics and APK optimization (Alpha 4)

Base: feat/alpha4-performance-ui, source 1241e72dce2b01fa527b0e7ff659f445c2dd19ed.
Development branch: feat/android-vulkan-apk-optimization. No Alpha 5 work.

## GPU discovery and presentation

Android GPU identity still tries Vulkan, then the existing EGL probe, then system
properties. Vulkan now uses typed ash 0.38 bindings instead of a handwritten,
overallocated physical-device property structure. ash was already in Cargo.lock;
its loaded/std features are used without its optional debug formatting. No linked
Vulkan library or new Android permission is required.

The existing monitoring worker creates a Vulkan instance once, queries a physical
GPU and immediately destroys the instance/unloads its loader reference. It creates
no logical device, queue, surface or profiling workload. An incompatible newer
instance request retries Vulkan 1.0 once; other creation failures are reported.
Optional properties2/features2/memory2 calls require negotiated core support and
actual function pointers. Failed discovery is cached too and preserves EGL/property
identity fallback. No Vulkan native call is made by UI presentation code.

The immutable report is shared through Arc in telemetry snapshots, including JSON
exports. Linux GPU records omit this optional field and never invoke this probe.
The existing expandable Diagnostics UI adds a Vulkan section showing:

- Device Vulkan API version, vendor ID and device ID.
- Raw vendor-defined driver version, optional driver name/info/driverID.
- Reported device extensions, selected hardware limits, all core 1.0 features and
  selected core 1.1–1.3 features when legally queryable. Absent extension feature
  bits are not guessed from extension names.
- Memory heap sizes/flags and memory types with heap indices/property flags.
- Optional VK_EXT_memory_budget driver estimates captured at discovery, explicitly
  labelled as a non-live snapshot and this process's estimated usage.

The UI formatter runs once per locale, using the cached report. Technical Vulkan
identifiers remain canonical; explanations/statuses are in English and Polish.
All GPU names/IDs/versions/capabilities come from the driver, not a POCO/SoC table.

## Non-root sensor limits

The test device is POCO X7 Pro / Dimensity 8400-Ultra / Mali-G720 MC7 running
Android 16 / HyperOS 3, but no such device is attached to this environment.

Vulkan capability data, timestamp precision and application GPU timings do not
measure global GPU utilization. ADPF/PerformanceHint APIs do not expose global
load/frequency/temperature. No root, Shizuku, adb, privileged profiler or fabricated
measurements are used. The existing Mali utilization scalar remains unverified
and therefore unavailable. GPU-specific thermal nodes are the only thermal source;
CPU/SoC temperature is never substituted. Protected/missing nodes remain restricted
or unsupported in diagnostics and show Unavailable as Android GPU measurements.

Clock discovery additionally recognizes explicitly named Mali devfreq core devices
(e.g. /sys/class/devfreq/13000000.mali/cur_freq). Discovery is bounded/cached and
excludes CPU, DDR, GPU bus and Mali memory nodes. The standard cur_freq ABI is Hz;
only readable valid values produce MHz. SELinux may still deny these paths on the
test phone. No MediaTek vendor counters of unknown units are interpreted.

A DEVICE_LOCAL Vulkan heap can be shared system RAM; it is neither dedicated VRAM
nor free RAM. Heap sizes/budgets never populate the existing VRAM cards. Budgets
are driver estimates, not guaranteed accurate global memory measurements, and
are not refreshed continuously. Restarting the process produces a new discovery
snapshot. Unsupported budget queries show Unavailable, not zero.

Android collectors pause while backgrounded. A lightweight one-second lifecycle/
shutdown check remains; it does not sample CPU, RAM, GPU or sysfs. Resume clears
CPU/rate baselines to avoid averaging across a background gap. Vulkan discovery
remains cached across resumes.

## APK inspection and measured candidate

The release baseline was built from the exact base commit in a separate worktree,
using the original Rust release settings and the same ARM64 debug-signed Gradle
preview variant used by the test workflow. It is not the old 95 MB debug-Rust APK.

- Baseline APK: 35,719,918 bytes; native library: 35,695,408 bytes.
- Candidate APK, including the added diagnostics: 17,008,526 bytes (52.38% smaller).
- Stripping the baseline library alone reduced it to 23,766,232 bytes.
- Java/DEX/launcher resources are only tens of KB. One ABI (arm64-v8a), one native
  library, no packaged font files or duplicated ABI copies. SVG assets remain
  embedded in the native library and all five navigation icons are verified.
- Ordinary symbol/string tables account for approximately 12 MB of the baseline;
  there were no .debug sections. The candidate has neither ordinary symbol tables
  nor debug sections.

The candidate was a measurement build from the working tree. Final builds also
include compatibility/error-handling checks and explicit 16 KiB ELF alignment;
final artifact APK-SIZE.json/checksums are authoritative for exact bytes/source.
No native library ZIP compression trick is used: it remains directly mmap-able.
APK size does not establish startup/RSS improvements; no phone performance
benchmark is claimed.

Android Slint default features previously pulled in desktop Winit, FemtoVG and
software-renderer stacks although its Android backend uses Skia. Android now
explicitly enables std, compatibility and Android activity support while retaining
the same Skia renderer, SVG support and native accessibility integration. Linux's
Slint feature set and default software-renderer selection are unchanged.

Only the dedicated Android profile uses the evaluated combination:

```toml
[profile.android-apk]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = "symbols"
```

Linux `cargo build --release` keeps its existing defaults. Android linking adds
16 KiB max-page-size only to the final cdylib. CI verifies ELF load alignment and
APK ZIP alignment, exported JNI/activity symbols, version/ABI/signature, source
identity, icon resources and SHA-256. Panic abort affects Android fatal panics;
normal loader/driver/read errors use Results/Unavailable and do not panic.

Reproduce:

```sh
cargo ndk -t arm64-v8a --platform 26 -o android/app/src/main/jniLibs build --locked --lib --profile android-apk
gradle -p android assembleDebug -PvoltixPreview=true --no-daemon
python3 scripts/apk_size_report.py before.apk after.apk --output comparison.json
```

The existing preview package/signing-key cache is retained; versionCode is 8.
No settings/data schema, application ID or storage path is changed. Production
release signing still requires the existing production key; test APKs are clearly
debug-signed. No app data reset is needed for an update with the cached test key.

## Validation and remaining limits

Core and presentation tests cover API version decoding, failed loader discovery,
older-driver fallback, no retry on allocation failure, heap/budget semantics,
localization and rejection of unrelated devfreq sources. The typed query path was
also exercised with a workspace-local Mesa Lavapipe ICD without a surface/logical
device. The initial environment had a loader but no ICD; that absence was correctly
reported as a failure rather than inventing capabilities.

Existing CPU/RAM, navigation/icon, settings, process and MangoHud tests are retained.
No actual POCO/HyperOS device or Android emulator is attached. Non-root readability,
Android rendering, app installation and on-device startup/memory performance remain
unverified. Software Vulkan testing is not evidence of Mali counter access.

Local validation also built the Linux release binary and AppImage. Both default
software rendering and explicit SLINT_BACKEND=winit-femtovg stayed running through
10-second Xvfb launch checks (timeout exit 124), with only the environment's missing
desktop D-Bus settings warning. This checks launch compatibility, not visual
acceptance or a performance benchmark. Android-target Clippy and workflow
actionlint/bash syntax checks passed. APK comparison data is generated by
scripts/apk_size_report.py; the Actions artifact includes signature, architecture,
alignment, exported-entry-point, source and SHA-256 verification reports.
