# Android APK size (Alpha 4)

The earlier ~17 MB optimization is commit `06e8febe234512781782b088a30099c683cb4dba`
on `feat/android-vulkan-apk-optimization`, not an ancestor of this branch.
Its size settings were never incorporated here. The target branch continued to
use Cargo's generic release defaults (no LTO or symbol stripping) and Slint's
Android default features. Dependency inspection confirms an unused software
renderer was included alongside the actual Android Skia renderer.

A fresh ARM64 baseline from `e3737cb0eefc6ebfb82c2e93627848fb5a140115`
was inspected before these changes: 36,915,166 APK bytes, including a 36,894,000-byte
`libvoltix.so`. Ordinary `.symtab`/`.strtab` tables occupy 12,463,795 bytes; there
are no `.debug` sections. DEX and launcher resources total only tens of KB.
There is one ABI and one native library; no duplicate packaged font/image files
or extra native libraries were found. Full measurements and SHA-256 are in
[ANDROID-APK-BASELINE.json](ANDROID-APK-BASELINE.json). The baseline was built
locally for measurement and is not a distributable update.

Only Android opts into `--profile android-apk`: `opt-level="z"`, LTO,
one codegen unit, abort on fatal panic, symbol stripping. Its Slint features
retain std, compatibility, Android NativeActivity, Skia and SVG support while
excluding the unused software-renderer dependency. Linux's Slint features,
release profile and packaging are unchanged. No application/UI/telemetry logic
or dependency versions change. Native errors continue to use existing error
handling; the abort setting applies to otherwise fatal Rust panics.

Both Android workflows and the README build command select this profile.
The finished APK remains directly mmap-able (uncompressed native library), with
16 KiB ELF/ZIP alignment. CI verifies native entry points, source identity,
all five navigation SVGs, the absence of the removed privileged integration,
ARM64 ABI, signature and metadata. A 17 MiB artifact budget guards size regressions.
`APK-SIZE.json` and `APK-COMPARISON.json` in the Actions artifact report the exact
final bytes, largest components and reduction against the measured baseline.
Size alone does not establish functional correctness or startup performance.

Version code 11 retains version name `0.1.0-alpha.4`, the preview package ID,
signing-key cache and settings/data locations. No uninstall or data reset is
required for an update signed with the existing cached key. CI checks signing
and loading compatibility; physical-device installation, rendering and
performance remain unverified without an attached Android device.
