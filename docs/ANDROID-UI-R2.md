# Alpha 4 Android UI R2 investigation

## Confirmed evidence and root-cause limits

CI #38 ran workflow commit f93edc1fb2f79124fb7c5a976041b407384bf7a0, but its
checkout step explicitly pinned application source to
82e590c2ea871bcd4caca9972ca999eaf6301a95. The diff between those commits contains
only two workflow files. The run produced the artifact named
Voltix-Alpha4-ARM64-APK-82e590c.

There is one Android UI entry chain:
VoltixActivity (NativeActivity, lib_name=voltix) -> android_main in src/lib.rs ->
app::run -> generated AppWindow compiled from ui/app-window.slint by build.rs.
app::run sets android-platform=true on Android. The root instantiates MorePage
at page 12, SettingsPage at page 6, and BottomNav when android-platform is true.
The old DashboardPage component exists but is not the active dashboard;
ControlCenter is. No second Android root or Java/XML replacement screen was found.
The local Android code generated from source 82e590c embeds the mobile SVGs via
include_bytes; an unsupported-font glyph is not the new Dashboard/GPU icon source.

The user confirmed that installing CI #38 failed until the old app was uninstalled.
The workflow generated an ephemeral debug key per fresh runner, while versionCode
remained 5 and no source/build identifier appeared in the UI. This explains the
failed in-place update and the inability to identify an installed build. It does
NOT prove why the UI still looked unchanged after a successful clean install.
We cannot establish that final visual cause without the affected device/APK
installation state. Claiming an inactive replacement root would contradict the
source trace. The earlier implementation also lacked real root interaction tests
and its row-only restyling was insufficient to establish the requested overhaul.

The previous workflow's permanent 82e590c pin would also exclude all subsequent
fixes. R2 checks out and records the exact triggering workflow/application commit.

## R2 changes

- Actual MorePage now has a prominent title and two grouped surfaces, compact
  rows, colored icon tiles and chevrons, retaining all eight destinations.
- Actual SettingsPage has a six-category mobile home with descriptions. Entering
  Settings from More/dashboard explicitly opens home; each category retains its
  existing controls and back navigation. Linux settings remain available.
- Root header and About display Mobile UI R2 and the embedded full source commit.
- build.rs explicitly embeds referenced Slint image resources. The APK verification
  step checks the native library for all five SVGs and the exact source marker.
- Version code is 6; marketing version stays 0.1.0-alpha.4.
- A headless test renders the production AppWindow in Android mode and dispatches
  pointer events through all five bottom destinations, all eight More rows, and
  all six Settings categories/back. It checks icon pixels and narrow-screen
  navigation in English/dark and Polish/light themes. PNGs are uploaded in CI.
- The APK workflow builds the triggering commit, checks package/ELF/signature,
  verifies SHA256SUMS, and uploads APK, provenance and UI test evidence.

## Data and signing

The CI validation build uses -PvoltixPreview=true and package
com.voltix.monitor.alpha4test, labelled Voltix Alpha4 R2. It installs alongside the
existing com.voltix.monitor, so no uninstall or deletion of original data is needed.
Preview settings are separate; original settings are not automatically imported.
Normal/release application ID and storage remain unchanged. No settings/telemetry
schema or data cleanup was introduced.

The new test signing key is cached and builds are serialized to prevent concurrent
key generation. GitHub caches can expire or be evicted; a permanently configured
signing secret remains necessary for guaranteed long-term update compatibility.
The private signing key from CI #38 is not recoverable from its APK. Do not
uninstall the original application to test this preview if its data must be kept.

## Verification limits

Headless Slint rendering tests use the actual production root with Android mode,
but a Linux software renderer. They do not constitute visual testing of the
Android backend, Android APK installation, Mali rendering, safe-area behavior or
real touch/scroll behavior. There is no attached Android device or emulator/KVM
in this environment. On-device visual correctness remains unverified.

The agent cannot fetch CI #38's authenticated artifact download through the blocked
GitHub API. CI #38 source wiring was audited; the new workflow explicitly inspects
the actual APK it packages. Local old APK checks are not presented as inspection
of the user's downloaded CI #38 file.

## Local validation before publishing

72 tests passed, one existing native clipboard test ignored. Linux and Android
Clippy passed with warnings denied. Formatting, actionlint, workflow shell syntax,
and debug manifest XML/merger checks passed. The merged validation manifest has
package com.voltix.monitor.alpha4test, versionCode 6 and launcher label
Voltix Alpha4 R2. The production root was rendered and inspected headlessly at
390x844 (English/dark) and 320x640 (Polish/light); category controls and navigation
were visible in those renders. These are not Android-device screenshots.

## Exact files changed

- .github/workflows/android-alpha4-apk.yml
- .github/workflows/ci.yml
- android/app/build.gradle.kts
- android/app/src/debug/AndroidManifest.xml
- assets/icons/mobile/dashboard.svg
- assets/icons/mobile/gpu.svg
- build.rs
- docs/ANDROID-UI-R2.md
- src/app.rs
- src/build_info.rs
- src/lib.rs
- tests/android_navigation.rs
- ui/app-window.slint
- ui/components/bottom-nav.slint
- ui/components/navigation-row.slint
- ui/pages/more.slint
- ui/pages/settings.slint
