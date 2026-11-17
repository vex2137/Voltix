# Alpha 4 mobile UI and telemetry follow-up

Version remains 0.1.0-alpha.4. No Alpha 5 work.

## Changes

Android Settings uses a six-category home and a category title/back control.
The existing controls, callbacks and persistence remain in use. Category -1
represents the mobile home and is persisted. Linux retains its sidebar/dropdown.
About from More opens the About category directly; Diagnostics remains available
from More. MangoHud's Linux-only explanation is immediately visible on Android.

All five bottom destinations use locally bundled, tintable SVG assets with the
same 24px canvas and 1.8px stroke. No system font provides these icons.
More groups all existing destinations under Hardware and Application.
Dashboard summaries use two columns at ordinary phone widths, falling back to
one below 360px. Battery/details grids use two columns when content width permits;
current, voltage and estimated instantaneous power have two decimal places.
The Android battery collector, including the platform-reported current sign,
is unchanged.

The target branch already contained the Linux Processes redesign and software
renderer preference/fallback. Those implementations are retained, including
incremental VecModel updates, PID+start-ticks identity, read-only details,
search/sorting, and explicit SLINT_BACKEND handling. See RENDERER-PROCESSES.md.

## Non-root Android GPU investigation

GPU identity uses the existing Vulkan/EGL probes. Identity support does not imply
access to utilization, power or thermal counters.

- Android BatteryManager measures battery current/voltage, not GPU power.
- Public Thermal APIs expose thermal status/headroom, not a portable per-GPU
  temperature sensor. They must not be converted into a GPU temperature.
- ADPF PerformanceHintManager accepts workload hints; it does not provide
  device-wide GPU utilization. Profiling tools/counters (AGI, Perfetto, Mali
  Streamline/HW counters) may need a debuggable application, adb permissions,
  driver support or privileged access. They are not a production-app workaround.
- KGSL gpubusy has a known busy/total microsecond format. Invalid ratios are
  failures, not clamped percentages; zero intervals are temporarily unavailable.
- Mali utilization sysfs is vendor-dependent. The existing bare-number parser
  was removed: the units/range and sampling semantics have not been verified for
  Mali-G720 MC7. An accessible node alone does not validate a percentage.
- Readable GPU-specific thermal zones and documented devfreq cur_freq (Hz)
  can supply temperature/frequency. CPU/SoC zones are never used for GPU readings.
- No dedicated VRAM is reported for the integrated shared-memory GPU. System RAM
  is not relabeled as VRAM.
- No verified unprivileged GPU power interface is available in this implementation.

Missing nodes now mean unsupported device/driver interface. EACCES means permission
denied, potentially due to the Android sandbox or SELinux; EACCES alone cannot
prove SELinux caused the denial. Invalid values/I/O errors mean sensor read
failure. Zero counter intervals/idle clocks mean temporarily unavailable.
The Diagnostics page includes each GPU metric's status and underlying reason;
cards retain concise unavailable labels.

No Mali device was attached. On-device readability, Android policy and vendor
counter semantics remain unverified. No new Mali utilization or power reading
is claimed.

## Manual device acceptance still required

Install the APK on Mali-G720 MC7 hardware; inspect all five icons in the APK,
all Settings controls in English/Polish, light/dark themes, category back
navigation, persistence after restart, More destinations, battery signs and
history controls. Inspect small phones, display cutouts, large system fonts,
and system Back behavior. Linux runtime smoke tests alone cannot establish
Android visual correctness or absence of refresh flicker.

API references for reproducing the investigation:

- https://developer.android.com/reference/android/os/PowerManager#getThermalHeadroom(int)
- https://developer.android.com/reference/android/os/PerformanceHintManager
- https://developer.android.com/agi/supported-devices
- https://developer.android.com/reference/android/os/BatteryManager#BATTERY_PROPERTY_CURRENT_NOW

## Build environment

The initial environment had no Rust toolchain, Android SDK/NDK, Gradle or javac,
and lacked Linux development packages. These were installed/extracted inside
/workspace for local builds; host configuration was not changed. Test AppImages
use the debug Rust profile with symbols stripped at packaging. They are test
artifacts, not release/performance benchmark builds. Android test signing is
the Gradle debug key; production signing credentials are not present.

## Validation results

- cargo test: 71 passed, 0 failed; one pre-existing native clipboard test ignored
  (requires native clipboard helpers and a live session).
- cargo fmt --all -- --check and git diff --check: passed.
- Linux cargo clippy --all-targets -- -D warnings: passed.
- Android cargo clippy --lib --target aarch64-linux-android -- -D warnings: passed.
- Linux test AppImage built. Default software and explicit winit-femtovg launches
  under Xvfb both stayed alive for the 8-second smoke-test interval and applied
  telemetry. Explicit femtovg logged a rendered frame. These are smoke checks,
  not visual acceptance or performance benchmarks.
- ARM64 Android native library built against API 26 using NDK r27c.
- Gradle assembleDebug: passed (35 tasks). APK signature verified with apksigner.
  aapt confirms version 0.1.0-alpha.4, min SDK 26, target SDK 35; APK contains
  lib/arm64-v8a/libvoltix.so.
- No Android device/emulator was attached. APK icons, touch navigation, settings
  layout, actual battery readings and Mali sysfs access were NOT visually or
  physically verified.
- Process search/sorting/identity and MangoHud manager tests passed. A real game
  overlay and process-table refresh flicker were not visually tested.
- English/Polish catalog completeness tests passed. New section labels are
  localized. Low-level GPU probe reasons include English technical details and
  sysfs paths; per-metric status labels use the selected language.

Artifacts (relative to repository):
- dist/Voltix-0.1.0-alpha.4-test-x86_64.AppImage
- dist/Voltix-0.1.0-alpha.4-test-arm64.apk
- dist/validation/ (logs, checksums and changed-files.txt)

Both artifacts use the unoptimized Rust debug profile and are intended for
acceptance testing. The APK is debug-signed; no production release signature
or on-device release-build visual verification is claimed. Gradle could not
automatically strip the library; native debug information was stripped manually
before packaging. Headless Linux logs include an unavailable desktop D-Bus
session warning, which did not prevent the smoke tests from running.

## Changed files

- .gitignore
- assets/icons/mobile/about.svg
- assets/icons/mobile/back.svg
- assets/icons/mobile/battery.svg
- assets/icons/mobile/chevron.svg
- assets/icons/mobile/cpu.svg
- assets/icons/mobile/dashboard.svg
- assets/icons/mobile/disk.svg
- assets/icons/mobile/export.svg
- assets/icons/mobile/gpu.svg
- assets/icons/mobile/memory.svg
- assets/icons/mobile/more.svg
- assets/icons/mobile/network.svg
- assets/icons/mobile/sensor.svg
- assets/icons/mobile/settings.svg
- assets/icons/mobile/system.svg
- docs/ALPHA4-UI-VALIDATION.md
- locales/en.json
- locales/pl.json
- src/app.rs
- src/control_center.rs
- src/platform/android/mod.rs
- src/platform/gpu_probe.rs
- src/platform/mod.rs
- src/settings/mod.rs
- ui/components/bottom-nav.slint
- ui/components/navigation-row.slint
- ui/components/telemetry-grid.slint
- ui/i18n/strings.slint
- ui/pages/control-center.slint
- ui/pages/details.slint
- ui/pages/more.slint
- ui/pages/settings.slint
