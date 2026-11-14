# Alpha 3: interface milestone

This is the first implementation pass, not the complete overlay milestone.

## Working functionality

- Immediate, persistent Modern / Professional / Gaming selection in Appearance.
- Modern retains balanced cards; Professional adds a dense GPU/current-reading
  overview and CPU core rows; Gaming provides a performance summary, larger charts
  and explicit unavailable FPS/frametime status. All share the existing collector.
- Four persistent accent colors, existing System/Dark/Light themes, language/unit
  preferences and polling interval. Legacy preference files default to Modern.
- All seven existing pages retain their functions, with capped desktop content,
  closer label/value groups and mode-dependent card density. Stacked phone rows
  reserve space for labels and values. Existing safe-area/navigation retained.
- Android uses the desktop bolt silhouette, unchanged package ID and versionCode4.
  Original release signing requirements remain intact; certificate continuity with
  an installed APK has not been verified.
- Opt-in `VOLTIX_PROFILE_STARTUP` traces initialization and first applied telemetry.
  No new application dependencies or monitoring loops.

## Validation and limitations

31 Rust tests (29 unit, 2 integration), formatting and strict Linux Clippy passed.
The native UI compiled for both Linux and Android in the Alpha 3 Actions run.
Desktop snapshots at 1440x900/1280x800 and a Linux viewport at 390x844 were exercised.
The phone viewport is not a substitute for an actual Android device test.
Mode/accent callbacks were exercised by clicking Settings and reading persisted JSON.

Development measurements use release binary, Xvfb, winit software rendering and
`/proc/PID/status` after two seconds. These are individual observations under build
load, not comparable hardware benchmarks. Initialization, OS window discovery and
first telemetry application are distinct from actual frame presentation; no
first-frame, sub-500ms usable-interface or idle-CPU regression claim is made.
Exact traces, RSS samples, screenshots and packaging logs are retained outside the
checkout in `/workspace/alpha3-evidence`.

NVML already collects actual `power_usage` independently of enforced/management
power limits. No real NVIDIA adapter was available for hardware verification.
Unavailable hardware measurements remain unavailable, not fabricated.

## Remaining within Alpha 3

MangoHud availability/configuration/launch integration, overlay preferences and
editor/preview, metric visibility/order controls, real FPS/frametime ingestion,
min/max tables, reduced-animation preferences, extensive mobile device testing and
an actual first-frame benchmark remain unimplemented. No simulated FPS, fake
overlay enable button, global MangoHud configuration overwrite or process injection
is included. The seven-page redesign can receive further design work beyond this
shared-layout and mode milestone.

The next overlay pass should isolate the game telemetry provider, configuration,
renderer and game launcher; use a Voltix-owned MangoHud configuration file and
explicit per-game launch instructions. It must first verify upstream supported
options and launch behavior, and cannot promise injection into running games or a
universal Android overlay. Reliable ordering controls precede drag-and-drop.
