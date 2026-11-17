# Linux rendering and Processes

Linux desktop startup prefers Slint's `winit-software` backend, explicitly enabled
in the build. GPU renderer support remains compiled in. Android initialization
and its renderer are unchanged.

An existing `SLINT_BACKEND` is never set, replaced or removed by Voltix, even if
its value is empty. Slint handles that explicit value, including any of Slint's
own fallback behavior. Examples:

```sh
# Preferred Linux default (no override)
env -u SLINT_BACKEND ./Voltix-Alpha4-test-x86_64.AppImage
# Explicit OpenGL renderer, using the installed graphics driver
SLINT_BACKEND=winit-femtovg ./Voltix-Alpha4-test-x86_64.AppImage
# Explicit software renderer
SLINT_BACKEND=winit-software ./Voltix-Alpha4-test-x86_64.AppImage
```

If preferred backend selection fails, Voltix logs the failure and tries Slint's
automatic selection. The pinned Slint Winit backend also tries other compiled
renderers if creating its preferred window renderer fails. A missing graphical
session can still prevent all backends from starting; both selection errors are
reported. A later window/surface initialization error also gets one fresh-process
retry with automatic selection, because Winit cannot recreate an event loop in
the same process. Arguments and environment are inherited; only the private
`VOLTIX_RENDERER_RETRY` marker is added to prevent retry loops. Explicit
`SLINT_BACKEND` values disable this Voltix retry. Voltix does not initialize a GPU
to probe a software-rendered UI.
NVIDIA NVML may still be loaded for actual hardware telemetry.

Set `VOLTIX_PROFILE_STARTUP=1` to report the selection path and existing startup
milestones. These milestones are not time to first presented frame.

## Performance tradeoff

The supplied CachyOS/KDE Wayland benchmark (i5-14400F, RTX 4060) measured software
versus default GPU rendering at 117.3 versus 207.0 ms average window detection,
69.3 versus 140.3 MiB combined application/launcher PSS, and 3.30 versus 2.73%
average CPU. These are user-provided reference measurements, not new Voltix
measurements or guarantees. Software rendering can trade more CPU for lower
startup and driver memory costs. GPU rendering remains useful for different
hardware, scaling and graphics workloads. Verify the tradeoff on the target
machine; window detection is not first-frame presentation.

## Process data and refresh

- `/proc/<pid>/stat` supplies name, state, parent PID, thread count, start ticks,
  CPU time and resident pages. CPU uses clock-tick and monotonic time deltas,
  expressed per logical core; multithreaded processes can exceed 100%.
- Effective UID comes from `Uid:` in `/proc/<pid>/status`, not directory
  ownership. `/etc/passwd` names are cached for 60 seconds; unresolved users
  appear as numeric UIDs. No network/NSS lookup runs on the UI thread.
- Total process count enumerates visible numeric proc directories, including
  unreadable entries. The thread summary uses the kernel task count in
  `/proc/loadavg`; in containers this can include host tasks.
- Up to 2048 process stat/status pairs are read every two seconds while the
  page is visible (a slower configured polling interval also slows collection).
  Top consumers, filters and table rows refer to that sample. A limit notice
  appears when the visible process count exceeds the cap.
- Snapshots share immutable process vectors. A generation cache avoids sorting
  and formatting unchanged samples. The virtualized ListView renders visible
  rows; its model is retained and updates only changed records, without resets.
- Names/PIDs match case-insensitively. CPU, resident MiB, PID and name sort in
  either direction; missing CPU samples sort last. Quick filters mean CPU
  >=10% of one core and RSS >=256 MiB. RAM bars show a fraction of system RAM;
  CPU bars clamp at one core while numeric CPU remains uncapped.
- Search and selection remain in memory through refreshes. Sorting, direction,
  quick filter and own-user filter are persisted. Selection uses PID plus start
  ticks, independent of row position and filters. A reused PID is not selected.
- Only the selected process incurs executable/command-line reads, on the
  sampler thread. Start ticks are checked again afterward to reject PID reuse.
  Command lines are quoted for readability and bounded at 16 KiB, with an
  explicit truncation notice. Selected executable/argument details stay local and
  are excluded from the existing telemetry export. Unreadable fields stay unavailable. Linux kernel
  process names can be truncated; details also show the executable basename.

Details use a side panel on wide windows and a dismissible drawer on narrower
ones. Optional columns disappear first; all available fields remain in details.
There are no process termination or permission-changing controls.

## Cloud validation (2026-11-15)

Measured build: `87b746a`. One 30-sample run per renderer, following a 5-second warmup, using the local
release AppImage in extract-and-run mode on Xvfb/X11. The Processes page was
visible with a 1-second polling interval. CPU is for the actual Voltix executable,
expressed per logical core. PSS includes the launcher and UI process tree.

| Renderer | Mean PSS | Mean CPU | Peak sampled CPU |
| --- | ---: | ---: | ---: |
| Default `winit-software` | 28.4 MiB | 1.89% | 3.92% |
| Explicit `winit-femtovg`, Mesa CPU rendering | 118.9 MiB | 15.02% | 34.99% |

The software process mapped no GL/EGL/LLVM libraries. The OpenGL process mapped
GL/GLX and LLVM. This validates distinct renderer paths and the explicit override
in this environment. Mesa CPU rendering is **not hardware GPU acceleration**;
these results do not predict performance on the supplied NVIDIA system. Startup
window detection, physical KDE Wayland behavior and target-machine PSS/CPU remain
to be measured there.

Virtual UI checks covered 1280x800, 740x620 and 740x480, a narrow details drawer,
Polish/light and English/dark themes, real process details, retained selection,
keyboard arrows/activation, scrolling, filtering and persisted sort direction /
own-user filtering. Strict Linux and Android Clippy, regression tests, native X11
clipboard and release AppImage startup checks are part of validation. A release
helper injected a failure after software-backend selection, then verified that
the exec retry selected automatic rendering and rendered the real application.
An unavailable-display test exited cleanly after exactly one fresh-process retry;
an empty `SLINT_BACKEND` retained its explicit override path. Physical
Android behavior and sustained memory profiling remain manual checks.

## Implementation map

- `src/platform/renderer.rs`, `src/main.rs`, `src/platform/mod.rs`, `Cargo.toml`:
  Linux selection policy and explicit build support; existing GPU support retained.
- `src/monitoring/processes.rs`, `src/monitoring/extended.rs`,
  `src/monitoring/mod.rs`: bounded worker sampling and selected details.
- `src/process_ui.rs`, `src/app.rs`, `src/control_center.rs`, `src/lib.rs`,
  `src/settings/mod.rs`: filtering, cache, lifetime selection, incremental models,
  preferences and application wiring.
- `ui/pages/processes.slint`, `ui/components/process-row.slint`,
  `ui/components/process-details.slint`, `ui/app-window.slint`, `ui/models.slint`:
  responsive virtualized table and details.
- `locales/en.json`, `locales/pl.json`, `ui/i18n/strings.slint`: translated controls.
- `tests/process_monitor.rs`, `.github/workflows/ci.yml`, `README.md`, this document:
  regressions, packaged OpenGL smoke check and usage/validation documentation.
