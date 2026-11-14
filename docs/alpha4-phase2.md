# Alpha 4 phase 2: work in progress

The application remains labelled Alpha 3 until the requested Alpha 4 requirements
are complete. This branch is not a release candidate.

## Memory investigation

`tools/profile_memory.py` launches a child process and samples Linux
`/proc/<pid>/smaps_rollup` (RSS, PSS, private clean/dirty and swap) and CPU ticks.
From a native Wayland desktop, build a release binary and run:

```sh
cargo build --locked --release
python3 tools/profile_memory.py --output /tmp/voltix-before.json
# Repeat with the changed build, same renderer, window size, polling and machine:
python3 tools/profile_memory.py --output /tmp/voltix-after.json
```

The default samples are at 3, 300 and 900 seconds. Keep the same desktop session,
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
