# Alpha 4 R2.1 density and diagnostics hotfix

Version remains 0.1.0-alpha.4; Android version code is 7.

## Scope and causes

Battery used the generic telemetry grid: minimum 84px cells and stretchable text
layouts made simple label/value readings unnecessarily tall. It now uses compact,
natural-height horizontal rows in three groups (status, measurements, battery
information). Signed current/power, units, precision, Android health status and all
nine existing metrics come from the same unchanged presentation adapter/collectors.
The chart and history selection remain accessible by scrolling.

The Android Permissions & Diagnostics page previously reused one long card with
all technical explanations expanded. It now has distinct Permissions, GPU
telemetry, Platform diagnostics and Build information sections. Each status row
can reveal/collapse its explanation without hiding the status itself. The normal
network-state permission is separate from hardware availability, and first-sample
state is not incorrectly shown as a denied permission. The page explicitly states
that no dangerous runtime permissions are needed and ordinary permissions cannot
unlock protected Mali/SELinux telemetry. The existing app-settings shortcut is
retained; no permission requests or new permissions were added.

All authored Android diagnostic explanations have English/Polish catalog entries,
including dynamic probe templates. Sysfs paths, device identifiers and raw OS error
payloads remain intact as technical data. No fabricated availability or sensor
values are introduced. Linux keeps its existing diagnostics page.

Dashboard Disks/Network use compact rows instead of the generic grid, including
Professional mode. Generic telemetry rows/grids no longer stretch their text to
consume spare height. The compact CPU/GPU/RAM/VRAM cards remain, with two columns
also at 320px Android widths. Navigation icons/assets are unchanged.

The header shows only R2.1 plus seven commit characters. Full source hashes appear
in About or the expandable Build information row, not the dashboard/Settings home.

## Regression checks

The production-root headless test now uses populated, explicitly simulated battery,
diagnostics, disk and network fixtures. It exercises 320px layouts in English and
Polish, limits blank vertical runs in the battery readings, checks diagnostic
expansion, and preserves all previous tab/category/icon interaction checks. CI
uploads those renders as PNGs. Catalog tests verify translated probe text retains
its paths and runtime permissions remain separate from GPU support.

These use the real Slint root with a Linux software renderer. They do not replace
Android-device visual testing. No Android device/emulator is attached here.

## Data and installation

This continues updating the existing R2 preview package
com.voltix.monitor.alpha4test. It uses the previously cached R2 test signing key;
the workflow refuses to generate a replacement key if restoration fails. Existing
R2 preview settings can therefore remain in place during a successful update.
The original com.voltix.monitor app and its data are untouched. Do not uninstall
either app as part of testing this hotfix. No telemetry collectors, settings schema,
battery formulas or storage paths were changed.

The Actions artifact includes the ARM64 APK, SHA256SUMS, source/build information,
signature/metadata verification, resource verification, and headless test PNGs.

## Exact files changed

- .github/workflows/android-alpha4-apk.yml
- .github/workflows/ci.yml
- android/app/build.gradle.kts
- docs/ANDROID-R2-DENSITY-HOTFIX.md
- locales/en.json
- locales/pl.json
- src/app.rs
- src/build_info.rs
- src/control_center.rs
- tests/android_diagnostics.rs
- tests/android_navigation.rs
- ui/app-window.slint
- ui/components/compact-metric-rows.slint
- ui/components/telemetry-grid.slint
- ui/components/telemetry-rows.slint
- ui/i18n/strings.slint
- ui/pages/control-center.slint
- ui/pages/details.slint
- ui/pages/diagnostics.slint
- ui/pages/settings.slint

## Local results

74 tests passed; one existing native clipboard test skipped. Final host Clippy,
formatting and actionlint checks passed. The populated 320px English/Polish
production-root renders were inspected for battery density, folded/expanded
Diagnostics, the two-column dashboard and short header identifier. Unit checks
cover permission/sampling semantics and translated GPU probe templates. Android
cross-target Clippy also passed during implementation; the Actions build performs
the final cross-target build and APK/resource/signature verification.
