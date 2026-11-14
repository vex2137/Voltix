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
leak. Long-duration and before/after measurements remain to be completed.

The UI previously called `VecModel::set_vec()` each sample, resetting all
repeated components. Models now retain the same vector and only update changed
rows, append new rows, and remove surplus rows. This avoids full reset events; it
does not eliminate construction of the incoming telemetry model vectors and no
specific RSS reduction has been established. Histories already use a 240-entry
`VecDeque`; only one monitoring engine serves all interface modes.

## Remaining work

Allocation/renderer profiling, 5/15-minute comparisons and interaction workloads,
first-frame timing, the complete Theme Studio (including presets, custom themes,
contrast validation and import/export), deeper mode differentiation, MangoHud
integration, hardware/device checks and final Alpha 4 version synchronization are
still outstanding. No FPS data or overlay behavior is simulated. An Android update
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
MangoHud is absent in this environment, so an actual game/overlay launch and
configuration compatibility were not verified. Voltix does not yet expose overlay
controls, generate a config or ingest game FPS. No global configuration was changed.
