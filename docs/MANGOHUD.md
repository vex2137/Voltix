# MangoHud Studio

Settings → MangoHud Studio manages **file profiles**, separately from the existing
quick-launch environment overrides. `MANGOHUD_CONFIG` overrides files: do not combine
the two workflows unintentionally.

- Select a profile and Load. Edit the metric checkboxes or appearance values (press
  Enter to commit a field). Colors are six hexadecimal digits; fonts are absolute
  TTF/OTF paths accessible to the game. Validate and use editor commits raw edits.
- Save creates a named JSON profile in `$XDG_CONFIG_HOME/voltix/mangohud-profiles`
  (fallback `~/.config`). Duplicate saves the current draft under a new name.
  Review profile replacement → tick confirmation → Confirm replaces an existing
  profile with a backup. Delete also requires confirmation and retains a backup.
- Profiles retain the game command and selected launch-file path. For per-game
  discovery use `MangoHud/<native-executable>.conf` or
  `MangoHud/wine-<case-sensitive-Windows-executable-without-.exe>.conf`.
- Enter an absolute file path. Import reads a `.conf` into the draft only. Export
  and Review application both stage a write. Review the destination and editor,
  tick confirmation, then Confirm. An externally changed destination invalidates
  the operation. Original bytes are retained in `.voltix-backup-N` siblings;
  backups are never overwritten. Symlink/non-regular destinations are rejected.
  To restore a supported `.conf` backup, import it and review application to the
  original path; arbitrary legacy backups can be restored manually. Unknown
  options are rejected during import rather than silently discarded.
- Use as launch file binds the chosen path to the profile. Copy file-based launch
  command generates safely quoted `MANGOHUD_CONFIGFILE=… mangohud …`; keep
  `%command%` for Steam/Proton, or enter a standalone command and arguments. Voltix
  never executes this command. Vulkan and OpenGL use the upstream wrapper;
  OpenGL dlsym is enabled by default in MangoHud 0.8.4.

## Reload and availability

MangoHud 0.8.4 watches its active configuration with inotify and supports atomic
file replacement. Confirming an application writes in that compatible manner;
no unconfirmed auto-save or synthetic key injection occurs. In a game already
using that file, supported appearance/metric changes can reload automatically.
Shift+F4 is the configured manual fallback. New library/API injection, launch-file
selection, Proton/runtime changes, and some font changes need a restart. Voltix
cannot confirm that another process reloaded or displayed particular sensors.

Launcher and common Vulkan/OpenGL installation paths are checked independently;
custom, Flatpak and 32-bit installations require game-side verification. GPU
voltage is AMD-only. Clocks, fan, power and temperatures depend on drivers and
permissions. The preview shows selected labels, orientation, position, scale,
text/background colors and opacity; it is illustrative, uses sample FPS/frametime,
and does not emulate MangoHud's font renderer, columns or every sensor color.

## Authoritative option baseline

Curated controls were checked against MangoHud **v0.8.4**:

- https://github.com/flightlessmango/MangoHud/blob/v0.8.4/data/MangoHud.conf
- https://github.com/flightlessmango/MangoHud/blob/v0.8.4/README.md#hud-configuration
- https://github.com/flightlessmango/MangoHud/blob/v0.8.4/src/overlay_params.h
- https://github.com/flightlessmango/MangoHud/blob/v0.8.4/src/overlay_params.cpp
- https://github.com/flightlessmango/MangoHud/blob/v0.8.4/src/notify.cpp

Advanced options outside the curated list (including `exec`) are deliberately
rejected; no unsupported option is silently claimed to work.
