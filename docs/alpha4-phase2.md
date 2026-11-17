# Alpha 4 phase 2: work in progress

The application remains labelled Alpha 3 until the requested Alpha 4 requirements
are complete. This branch is not a release candidate.

## Memory investigation

`tools/profile_memory.py` launches a child process and samples Linux
`/proc/<pid>/smaps_rollup` (RSS, PSS, private clean/dirty and swap) and CPU ticks.
From a native Wayland desktop, build a release binary and run:

```sh
cargo build --locked --release
python3 tools/profile_memory.py --label baseline-commit --output /tmp/voltix-before.json
# Repeat with the changed build, same renderer, window size, polling and machine:
python3 tools/profile_memory.py --label changed-commit --output /tmp/voltix-after.json
```

The default samples are at 3, 300 and 900 seconds. Output records the binary SHA-256, kernel and supplied label; record the actual renderer separately when it is auto-selected. Keep the same desktop session,
renderer, window size and telemetry settings. The sampler terminates only the
process it started. For CPU percentage over an interval, subtract the CPU ticks,
divide by `clock_ticks_per_second`, then by elapsed wall seconds, and multiply by
100 for percentage of one core. Record navigation/mode changes separately; these
change the workload and invalidate an idle comparison. Use a different output
file for each run. The tool does not measure first-frame presentation, instrument
allocations, or prove absence of leaks. It requires Linux `/proc` permissions.

A short isolated Xvfb/winit-software check sampled RSS 26,068–26,076 KiB and PSS
21,713–21,721 KiB at 3–6 seconds before the model change. This is not comparable
to the user's KDE Wayland measurements and is too short to infer stability or a
leak. Two concurrent 15-minute Xvfb/winit-software samples used the pre-model-update
binary (baseline) and the incremental-row binary (changed), with 1000 ms polling:

| Time | Baseline RSS/PSS/private KiB | Changed RSS/PSS/private KiB |
| --- | --- | --- |
| 5 min | 26016 / 21145 / 18204 | 25992 / 21125 / 18180 |
| 15 min | 25076 / 20226 / 17288 | 25688 / 21278 / 17916 |

Private is clean plus dirty; swap was zero. From 5–15 minutes process CPU was
approximately 1.61% and 1.49% of one core respectively. The instances ran
concurrently on separate Xvfb displays and builds/other checks ran during the
measurement, so shared mappings, scheduling and PSS are affected. This does not
establish a meaningful reduction, idle CPU target, lack of leaks, hardware GPU
behavior or KDE Wayland performance. Page/mode cycling is not included. Raw
samples are `/workspace/voltix-p2-baseline.json` and `/workspace/voltix-p2-after.json`.


The UI previously called `VecModel::set_vec()` each sample, resetting all
repeated components. Models now retain the same vector and only update changed
rows, append new rows, and remove surplus rows. This avoids full reset events; it
does not eliminate construction of the incoming telemetry model vectors and no
specific RSS reduction has been established. Histories use a 240-entry
`VecDeque`; disconnected GPU histories are pruned once older than the chart window
plus a five-second grace period. Hidden chart paths are not rebuilt; changing page
or interface mode refreshes the newly visible paths immediately. Only one
monitoring engine serves all interface modes.

## Remaining work

Controlled native Wayland comparisons and interaction workloads,
first-presented-frame timing, accessible contrast for the exact Minimal
palette, deeper mode differentiation, overlay color/layout controls, hardware/device
checks and final Alpha 4 version synchronization are still outstanding. No FPS data or overlay behavior is simulated. An Android update
still requires the installed app's original signing certificate and has not been
verified on a device. Do not merge, publish or call this branch a completed Alpha 4.

## Renderer investigation (development machine)

Two sequential six-second runs used the same release binary SHA-256
`be6445f67b0797a20955ed129b83866abfe772408a9f7e1f9af54c024ec19b5b`, the
same Xvfb 1280×800 display and settings. Other profiling processes were active.
At six seconds, `winit-software` reported RSS 25,908 KiB / PSS 20,814 KiB;
`winit-femtovg` with `LIBGL_ALWAYS_SOFTWARE=1` reported RSS 123,784 KiB / PSS
103,709 KiB. These are X11 software rendering and Mesa llvmpipe observations,
not a physical GPU or KDE Wayland comparison. They justify further renderer and
driver profiling, not changing the default renderer without functional and
hardware validation. Swap was zero in both samples.

## MangoHud investigation

Upstream documents Steam launch option `mangohud %command%` and the environment
variable `MANGOHUD_CONFIGFILE` for an explicit config file. Options in upstream's
`data/MangoHud.conf` include `fps`, `frametime`, `cpu_stats`, `gpu_stats`,
`cpu_temp`, `gpu_temp`, `ram`, `vram`, `gpu_power`, `font_size`, `position` and
`background_alpha`. Upstream also documents that `MANGOHUD_CONFIG` takes priority
over the config file. Sources: <https://github.com/flightlessmango/MangoHud> and
<https://github.com/flightlessmango/MangoHud/blob/master/data/MangoHud.conf>.
A Debian MangoHud package was extracted into a user-owned directory for a test.
An actual OpenGL application (Voltix itself) displayed MangoHud frame readings on
Xvfb/Mesa llvmpipe when the documented `--dlsym` preload was used; Vulkan, Proton,
KDE Wayland and an actual game have not been tested. Unavailable sensor errors were
observed in this container. Voltix generates selectable per-game launch options;
it does not inject into running processes, change global files or ingest game FPS.
On Android it explains the platform limitation. Executable detection checks PATH
only and does not certify that a graphics layer works.

## Built-in palettes

Four persisted palettes are selectable in Appearance: Voltix Default, Cyberpunk,
Emerald and Minimal. Selecting the default restores the previous system/dark/light
preference; choosing a theme or one of the existing accent swatches returns to the
default palette. Palette colors propagate to CPU, GPU, RAM and VRAM indicators.
Custom themes are edited or imported as JSON via copy/paste in Appearance; applying
the JSON updates the application immediately. It supports named themes persisted in
settings (up to 32), loading saved themes, and copying the JSON to export. Import
validates a bounded payload, known fields, name, `#RRGGBB` colors and a minimum
4.5:1 text/metric contrast against both background and surface. A native file
picker and graphical color picker are not implemented. Minimal retains the requested
#777777 secondary text on #101010 (approximately 4.25:1); this falls below WCAG AA
4.5:1 for small text and still needs a design decision.


## Overlay launch-option editor

Gaming → Overlay settings opens the settings section. On Linux it detects the
MangoHud executable, offers a manual recheck, and stores position, text size,
opacity, OpenGL `--dlsym` compatibility and visibility of FPS, frame timing,
CPU/GPU, temperatures, RAM/VRAM and GPU power. The read-only command preview can
be copied into Steam launch options. Including/disabling MangoHud changes only
that preview until the user applies it to a game; it cannot change running games.
The environment setting overrides any MangoHud configuration for that game.
No global configuration file is modified and no overlay background process or
polling is started. Current controls do not configure overlay colors or provide a
visual HUD layout editor. Game FPS remains unavailable inside Voltix itself.

The optional startup trace now reports the first `AfterRendering` callback when
the renderer supports it. Slint documents that this happens before buffer
presentation; it is not a first-visible-frame measurement. Unsupported backends
report the unavailable callback rather than a fabricated time.

## Heap allocation trace

A Debian-verified Heaptrack package was extracted for local profiling. A graceful
12.86-second Xvfb/software-renderer trace of the pre-optimization 2071d87 build
reported 219,743 allocation calls, 36,062 temporary allocations and 2.02 MB peak
heap. A graceful 12.71-second trace of the later build reported 216,454 allocation
calls, 35,832 temporary allocations and 2.02 MB peak heap. Fontconfig/font discovery
accounted for many startup allocations. These short traces cannot establish an
improvement or a leak; heap figures exclude other RSS and GPU memory. Raw traces
are machine-specific and stored outside the checkout under
`/workspace/voltix-heap-profile`.

A separate Xvfb/software-renderer interaction run navigated pages and modes and
resized the window. It reported RSS/PSS 26,432/21,936 KiB at 3 seconds and
27,700/23,200 KiB at 25 seconds. This brief workload is not a long-term or native
Wayland measurement.
