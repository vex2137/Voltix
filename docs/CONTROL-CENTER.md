# Unified Alpha 4 monitoring

The version remains `0.1.0-alpha.4`. Modern, Professional and Gaming use shared
collectors but different presentation: essentials, dense control center, and
performance/MangoHud workflows. Voltix does not read another game's FPS. Existing
CPU/GPU/memory/storage/system pages and MangoHud review/backup rules remain intact.

Desktop navigation is permanent; Settings uses category navigation with a compact
selector only at narrow sizes. Android always uses five fixed destinations:
Dashboard, CPU, GPU, Battery, More. More provides the remaining pages and About.
Theme, presentation, navigation, chart source/range and process sort persist in the
existing settings file. Missing fields retain safe defaults in older settings.

## Sources and overhead

- Linux network: `/proc/net/dev`, excluding loopback. Rates require two monotonic
  samples; resets and missing counters remain unavailable. Dashboard rates sum
  interfaces: tunnels and bridges may count the same traffic more than once.
- Disk activity: `/proc/diskstats` and physical `/sys/block/*/device` entries;
  sectors are 512 bytes per the kernel ABI. Capacity uses the original deduplicated
  mount collector. No privileged SMART query or UPS-specific protocol is invoked.
- Processes: read-only `/proc/<pid>/stat`, keyed by PID **and start time**. RSS uses
  the platform page size; CPU percentage is relative to one logical core and can
  exceed 100 for multithreaded processes. At most 2048 are scanned every 2 seconds,
  only while the page is visible; filtering precedes the 200-row display limit.
- Linux sensors: accessible hwmon temperature/fan/power/voltage and thermal-zone
  values, capped at 128. PSI reports `some avg10`. No permissions are modified.
- NVML and DRM reuse existing optional providers. Missing sensors are never zeroed.
- Detailed history is capped at 901 samples for each of ten sources, at most 1 Hz,
  with selectable 60/300/900-second windows and discontinuities preserved. CPU
  scope and GPU selection changes clear affected history to avoid mixing sources.

## Android bridge

`VoltixActivity` extends NativeActivity and provides JSON snapshots through a small
JNI bridge. BatteryManager and ACTION_BATTERY_CHANGED supply charge/status/source,
temperature, voltage, signed current, instantaneous estimated power, technology and
Android's enumerated health. Capacity-health percentages, cycle counts and lifetime
are never inferred. Unsupported extras/properties are null, not zero.

ActivityManager supplies RAM and the low-memory flag; Display supplies pixel/density/
refresh information; SensorManager supplies an inventory, **not fabricated live
sensor values**. TrafficStats supplies device-wide byte totals; the active-link
name comes from ConnectivityManager and is not a per-interface TrafficStats count.
Rates use the bridge's elapsed-realtime timestamp, not repeated Rust polling times.
The original Vulkan/EGL GPU identification (including driver-reported Mali names)
and access-restricted GPU status handling are preserved.

Only normal install-time ACCESS_NETWORK_STATE is added. No dangerous runtime
permission, location access, storage permission, root or background service is
required. The diagnostics page opens Android's application settings on request.
Foreground callbacks run no faster than 1 Hz, stop on pause, and stale snapshots
expire after 15 seconds. Native background sampling is reduced to at most 1/5 Hz.

## Export and limitations

Export writes the current snapshot with availability states and source version.
Linux creates a unique, owner-readable JSON file in the chosen directory; Android
uses ACTION_CREATE_DOCUMENT and writes on an export worker thread. Nothing is
uploaded by Voltix. Exports can contain host/device/interface/process names.

Test APK signing follows the existing configured release-key or debug fallback
workflow; Android requires the same signing certificate to update an installed
app. JSON settings remain backward compatible; physical-device upgrades must be
verified with the intended signing key. Physical Android, OEM current-sign quirks,
Wayland/NVIDIA and in-game MangoHud behavior require real-device validation.
