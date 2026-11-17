# Alpha 4: optional Shizuku GPU access

The integration is disabled by default. It uses the official API/provider
`dev.rikka.shizuku:{api,provider}:13.1.5` and a non-daemon AIDL UserService.
Enable it on GPU → Advanced GPU Monitoring, start Shizuku in its non-root wireless
debugging / ADB mode, and explicitly authorize Voltix. An external computer or
ADB command is not required by Voltix. Existing CPU/RAM/GPU identity, ordinary
telemetry, settings and the Linux desktop remain available without Shizuku.

## Privilege and lifecycle

The helper accepts **no command, path or setting argument**. Its sole telemetry
operation is `readSnapshot()`, plus the official lifecycle `destroy()` transaction.
It opens fixed files with `O_RDONLY | O_CLOEXEC | O_NONBLOCK`, reads at most 1 KiB
per file and returns at most 48 records. There are no subprocesses, device ioctls,
clock/governor/voltage/thermal writes, hidden-API bypass or root requests. Sui
initialization is disabled. Both the client and helper reject identities other
than shell UID 2000. `/proc/self/attr/current` reports the helper's actual SELinux
context in technical details; UID alone is not proof of a particular SELinux domain.

Reads run on a single worker, at most once per two seconds (also limited in the
helper), only while enabled and foregrounded. There is one in-flight request,
a six-second read deadline and ten-second connection deadline. Pause, disable,
denial and disconnection discard cached readings, invalidate in-flight results
and unbind/remove the helper. Missing/denied paths are retried with the 30-second discovery cache, avoiding
repeated denied syscalls every sample. It has no autonomous sampling loop. An already
started read can complete during a lifecycle transition, but its result is
rejected. Android's ordinary monitoring worker also sleeps while backgrounded;
CPU and rate baselines restart on resume. SharedPreferences stores only the
opt-in flag; Shizuku manages its own authorization separately.

## Sources and interpretation

| Source | Treatment |
| --- | --- |
| `/sys/class/devfreq/13000000.mali` and bounded discovery of explicitly named Mali devfreq cores | `cur_freq` in the standard devfreq Hz ABI; no unit guessing |
| `/sys/class/misc/mali0/device/devfreq/mali0` | Same devfreq ABI fallback |
| `available_frequencies` | Validated driver OPP list, displayed as its reported range; configured min/max files are separately diagnosed, not substituted for hardware support |
| GPU device `power/runtime_status` | Kernel runtime power state, never an inferred utilization measurement |
| `/sys/class/thermal/thermal_zone*/{type,temp}` | Only explicitly GPU/Mali/GPUSS-labelled zones, in milli-Celsius; CPU/SoC/battery zones are excluded |
| `/sys/kernel/ged/hal/gpu_utilization` | Only the documented three-integer loading/block/idle percentage ABI; rejects out-of-range, inconsistent and uninitialized `0 0 0` samples |
| Mali `load`, `utilization`, `utilisation`; GED `current_freqency` | Raw diagnostic reads only. A scalar or unknown-unit counter never becomes a measurement |

Clock validation accepts 10–10,000 MHz, temperatures −40–150 °C. GED loading and
idle must sum to 99–100 (integer rounding); all three fields must be 0–100. These
are conservative format checks, not a claim that the phone exposes this GED ABI.
The displayed GED value is kernel-reported loading from the driver's sample;
Voltix does not estimate utilization from clock speed or application GPU timing.
Missing, denied, temporarily unavailable, oversized and malformed/unverified
sources remain distinct. EACCES does not identify DAC versus SELinux by itself.
There is no dedicated-VRAM inference or CPU-temperature substitution.

The GPU page localizes connection/authorization, measurements and probe statuses
in English/Polish. Paths, errno and raw technical values remain canonical. Existing
GPU cards use verified Shizuku readings only when the ordinary source is unavailable.
JSON export includes the bounded report so physical-device results can be reviewed
without shell commands. Expired, future, root, denied and disconnected replies
cannot populate measurements.

## Evidence and verification limits

- [Official API guide](https://github.com/RikkaApps/Shizuku-API/blob/master/README.md):
  provider protection, requestPermission, UserService, UID 2000 versus root, lifecycle.
- [Published MediaTek GED HAL at a pinned kernel commit](https://github.com/MotorolaMobilityLLC/kernel-mtk/blob/4cbe43fe20dc19b9c5218553468efb3b868567c5/drivers/gpu/mediatek/ged/src/ged_hal.c#L172):
  `gpu_utilization_show` reads loading/block/idle and emits three integers. This is
  ABI reference evidence, **not evidence of readability or the same ABI on POCO**.
- Xiaomi's public `bsp-rodin-v-oss` branch was found at
  `2cd69ee4193920840ee3c837384716a66b1faecd`; it does not establish the running
  HyperOS build's vendor counter contract. The POCO's actual paths and SELinux
  policy must be verified using the installed APK's technical details/export.

Rust tests cover parsing, freshness, privilege, permission denial/disconnection,
source restrictions and localization. JVM tests cover the production lifecycle
gate. The production mobile root is exercised at 320/390 px widths with simulated
reports. APK verification checks **defined** DEX classes (official API/provider,
client, helper, AIDL Stub), the merged protected provider/official metadata, native
Advanced UI, signature, ARM64 metadata and embedded source identity.

No physical POCO/Shizuku service is attached to this environment. Actual permission
dialogs, UserService startup on HyperOS, shell/SELinux node access, GED behaviour,
GPU measurements and on-device rendering remain unverified. Shizuku does not
bypass SELinux and may enable no additional counters on this device.
