# Alpha 4 Linux desktop polish

Desktop-only changes on `feat/alpha4-performance-ui`:

- Aligned process headers/rows, alternating surfaces, hover/selection feedback,
  left-to-right CPU/RAM tracks, compact summaries and an explicit All filter.
  The ListView, 40 px row geometry, keyboard scrolling, process model, selection
  keys, enumeration, calculations, sorting and filter callbacks are retained.
- Desktop Sensors and Dashboard use responsive grouped cards for existing CPU,
  GPU and unit-classified sensor readings. No collectors, inferred measurements,
  thresholds or sampling algorithms were added. Empty inventory groups are omitted;
  every inventory reading remains accessible on Sensors. Nested row models are
  reused across refreshes. Temperatures/clocks/power use one decimal, RPM uses whole
  numbers, and other readings retain two decimals; only display precision changes.
- Desktop sidebar/settings navigation uses existing bundled icons and accent
  indicators. Shared theme definitions and Android navigation/layouts are retained.
- MangoHud Studio groups profile controls and related actions, aligns fields,
  distinguishes save/delete, and keeps the existing reviewed-write/delete backend
  checks and confirmation reset. Confirm buttons require the confirmation checkbox.

`tests/desktop_ui.rs` exercises the production AppWindow at 740×480, 1024×768 and
1280×800, English/Polish, dark/light and Professional/Gaming modes. It checks
viewport constraints, real process scrolling through refresh, selection, sensor
model reuse/units, and renders the Studio sections. Snapshot fixtures are simulated
presentation data, not substituted production telemetry. Set
`VOLTIX_DESKTOP_SNAPSHOTS=dist/desktop-snapshots` to save PPM captures.

Visual review uses these software-rendered desktop captures. It does not establish
physical Wayland, NVIDIA or in-game MangoHud acceptance. Existing backend/packaging
commands remain unchanged. A `[linux-ui]` push runs the existing Linux CI job and
skips Android APK packaging; the uploaded AppImage includes SHA256SUMS and
BUILD-INFO.txt identifying the exact source commit.
