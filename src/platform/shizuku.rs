//! Pure validation of optional shell-UID GPU reports. No I/O or Android dependency.
use crate::{app::ControlRow, localization::Localizer, models::metrics::MetricValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Probe {
    pub kind: String,
    pub path: String,
    pub label: String,
    pub status: String,
    pub text: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub enabled: bool,
    pub installed: bool,
    pub service_running: bool,
    pub authorization: String,
    pub state: String,
    pub uid: i32,
    pub sampled_elapsed_ms: u64,
    pub records: Vec<Probe>,
}
#[derive(Clone, Debug)]
pub struct Reading {
    pub metric: MetricValue<f64>,
    pub source: String,
}
impl Reading {
    fn missing(key: &str) -> Self {
        Self {
            metric: MetricValue::Unavailable(key.into()),
            source: String::new(),
        }
    }
    fn consider(&mut self, metric: MetricValue<f64>, source: &str) {
        let rank = |m: &MetricValue<f64>| match m {
            MetricValue::Available(_) => 5,
            MetricValue::PermissionError(_) => 4,
            MetricValue::Unavailable(_) => 3,
            MetricValue::Unsupported(_) => 1,
        };
        if !self.metric.is_available() && rank(&metric) >= rank(&self.metric) {
            self.metric = metric;
            self.source = source.into();
        }
    }
}
pub struct Validated {
    pub utilization: Reading,
    pub frequency: Reading,
    pub temperature: Reading,
    pub source_rows: Vec<ControlRow>,
    pub state: String,
    pub range: Option<(f64, f64)>,
    pub operating: Option<String>,
}
fn mali_base(base: &str) -> bool {
    if base == "/sys/class/misc/mali0/device/devfreq/mali0" {
        return true;
    }
    let Some(name) = base.strip_prefix("/sys/class/devfreq/") else {
        return false;
    };
    !name.is_empty()
        && name.len() <= 80
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && !name.contains("..")
        && name.contains("mali")
        && !["mem", "bus", "cpu"].iter().any(|s| name.contains(s))
}
fn thermal_path(path: &str, suffix: &str) -> bool {
    path.strip_prefix("/sys/class/thermal/thermal_zone")
        .and_then(|s| s.strip_suffix(suffix))
        .is_some_and(|s| !s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_digit()))
}
fn gpu_label(label: &str) -> bool {
    ["gpu", "gpuss", "mali"].iter().any(|prefix| {
        let Some(tail) = label.strip_prefix(prefix) else {
            return false;
        };
        if tail.is_empty() {
            return true;
        }
        let tail = tail.strip_prefix(['-', '_']).unwrap_or(tail);
        tail == "thermal" || (!tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()))
    })
}
fn allowed(p: &Probe) -> bool {
    match p.kind.as_str() {
        "cur_freq_hz" => p.path.strip_suffix("/cur_freq").is_some_and(mali_base),
        "min_freq_hz" => p.path.strip_suffix("/min_freq").is_some_and(mali_base),
        "max_freq_hz" => p.path.strip_suffix("/max_freq").is_some_and(mali_base),
        "freqs_hz" => p
            .path
            .strip_suffix("/available_frequencies")
            .is_some_and(mali_base),
        "gpu_temp_mc" => thermal_path(&p.path, "/temp") && gpu_label(&p.label),
        "ged_load_v1" => p.path == "/sys/kernel/ged/hal/gpu_utilization",
        "ged_freq_unknown" => p.path == "/sys/kernel/ged/hal/current_freqency",
        "mali_util_unknown" => {
            [
                "/sys/class/misc/mali0/device/utilization",
                "/sys/class/misc/mali0/device/utilisation",
            ]
            .contains(&p.path.as_str())
                || p.path.strip_suffix("/load").is_some_and(mali_base)
        }
        "runtime_state" => {
            p.path == "/sys/class/misc/mali0/device/power/runtime_status"
                || p.path
                    .strip_suffix("/device/power/runtime_status")
                    .is_some_and(mali_base)
        }
        "identity" => p.path == "/proc/self/attr/current",
        "thermal_candidate" => thermal_path(&p.path, "/type"),
        "discovery" => {
            ["/sys/class/devfreq", "/sys/class/thermal"].contains(&p.path.as_str())
                || thermal_path(&p.path, "/type")
        }
        _ => false,
    }
}
fn unsigned(text: &str) -> Option<u64> {
    let text = text.trim();
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}
fn hz(text: &str) -> Option<f64> {
    unsigned(text)
        .filter(|n| (10_000_000..=10_000_000_000).contains(n))
        .map(|v| v as f64 / 1_000_000.)
}
fn read_failure(status: &str) -> Option<MetricValue<f64>> {
    Some(match status {
        "readable" => return None,
        "read_denied" => MetricValue::PermissionError("shz_read_denied".into()),
        "unsupported" => MetricValue::Unsupported("shz_source_missing".into()),
        "temporary" => MetricValue::Unavailable("shz_temporary".into()),
        "too_large" => MetricValue::Unavailable("shz_too_large".into()),
        _ => MetricValue::Unavailable("shz_read_failed".into()),
    })
}
pub fn parse_probe(p: &Probe) -> MetricValue<f64> {
    if p.text.len() > 1024 || p.path.len() > 240 || !allowed(p) {
        return MetricValue::Unavailable("shz_invalid_payload".into());
    }
    if let Some(failure) = read_failure(&p.status) {
        return failure;
    }
    if ["cur_freq_hz", "min_freq_hz", "max_freq_hz"].contains(&p.kind.as_str())
        && unsigned(&p.text) == Some(0)
    {
        return MetricValue::Unavailable("shz_zero_frequency".into());
    }
    let value = match p.kind.as_str() {
        "cur_freq_hz" | "min_freq_hz" | "max_freq_hz" => hz(&p.text),
        "gpu_temp_mc" => p
            .text
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|n| (-40_000..=150_000).contains(n))
            .map(|n| n as f64 / 1000.),
        "ged_load_v1" => {
            let words: Vec<_> = p.text.split_whitespace().collect();
            if words.len() != 3 {
                None
            } else {
                let values: Option<Vec<_>> = words
                    .iter()
                    .map(|s| unsigned(s).filter(|v| *v <= 100))
                    .collect();
                values
                    .filter(|v| (99..=100).contains(&(v[0] + v[2])))
                    .map(|v| v[0] as f64)
            }
        }
        _ => return MetricValue::Unsupported("shz_unverified".into()),
    };
    value
        .map(MetricValue::Available)
        .unwrap_or_else(|| MetricValue::Unavailable("shz_invalid_reading".into()))
}
fn frequencies(p: &Probe) -> Option<Vec<f64>> {
    if !allowed(p) || p.status != "readable" {
        return None;
    }
    let words: Vec<_> = p.text.split_whitespace().collect();
    if words.is_empty() || words.len() > 64 {
        return None;
    }
    words.iter().map(|s| hz(s)).collect()
}
fn runtime(p: &Probe) -> bool {
    allowed(p)
        && p.status == "readable"
        && [
            "active",
            "suspended",
            "suspending",
            "resuming",
            "unsupported",
            "error",
        ]
        .contains(&p.text.as_str())
}
fn session_reason(s: &Snapshot, foreground: bool, now: u64) -> Option<&'static str> {
    if !s.enabled {
        return Some("shz_disabled");
    }
    if !foreground {
        return Some("shz_paused");
    }
    if !s.installed {
        return Some("shz_not_installed");
    }
    if !s.service_running {
        return Some(if s.state == "disconnected" {
            "shz_disconnected"
        } else {
            "shz_not_running"
        });
    }
    if s.state == "shell_required" {
        return Some("shz_shell_required");
    }
    if s.state == "old_service" {
        return Some("shz_old_service");
    }
    if s.authorization != "granted" {
        return Some(if s.authorization == "denied" {
            "shz_permission_denied"
        } else {
            "shz_permission_required"
        });
    }
    if s.state != "connected" {
        return Some(match s.state.as_str() {
            "connecting" => "shz_connecting",
            "read_timeout" => "shz_read_timeout",
            "shell_required" => "shz_shell_required",
            "old_service" => "shz_old_service",
            _ => "shz_disconnected",
        });
    }
    if s.uid != 2000 {
        return Some("shz_shell_required");
    }
    if s.sampled_elapsed_ms == 0 {
        return Some("shz_sampling");
    }
    if now
        .checked_sub(s.sampled_elapsed_ms)
        .is_none_or(|age| age > 8000)
    {
        return Some("shz_stale");
    }
    if s.records.len() > 48
        || s.records.iter().any(|p| {
            p.text.len() > 1024
                || p.path.len() > 240
                || p.label.len() > 80
                || p.kind.len() > 32
                || p.status.len() > 32
        })
    {
        return Some("shz_invalid_payload");
    }
    None
}
fn title(p: &Probe) -> &'static str {
    match p.kind.as_str() {
        "cur_freq_hz" => "gpu_core_clock",
        "min_freq_hz" | "max_freq_hz" | "freqs_hz" => "shz_range",
        "gpu_temp_mc" => "cc_gpu_temp",
        "ged_load_v1" | "mali_util_unknown" => "gpu_utilization",
        "runtime_state" => "shz_operating",
        "identity" => "shz_identity",
        _ => "shz_sources",
    }
}
pub fn validate(s: &Snapshot, foreground: bool, now: u64, loc: &Localizer) -> Validated {
    validate_inner(s, foreground, now, Some(loc))
}
pub fn measurements(s: &Snapshot, foreground: bool, now: u64) -> Validated {
    validate_inner(s, foreground, now, None)
}
fn validate_inner(s: &Snapshot, foreground: bool, now: u64, loc: Option<&Localizer>) -> Validated {
    let reason = session_reason(s, foreground, now);
    let mut out = Validated {
        utilization: Reading::missing(reason.unwrap_or("shz_source_missing")),
        frequency: Reading::missing(reason.unwrap_or("shz_source_missing")),
        temperature: Reading::missing(reason.unwrap_or("shz_source_missing")),
        source_rows: Vec::new(),
        state: reason.unwrap_or("shz_connected").into(),
        range: None,
        operating: None,
    };
    if reason.is_some() {
        return out;
    }
    for p in &s.records {
        let metric = parse_probe(p);
        match p.kind.as_str() {
            "cur_freq_hz" => out.frequency.consider(metric.clone(), &p.path),
            "ged_load_v1" => out.utilization.consider(metric.clone(), &p.path),
            "gpu_temp_mc" => out.temperature.consider(metric.clone(), &p.path),
            "freqs_hz" => {
                if let Some(values) = frequencies(p) {
                    if out.range.is_none() {
                        out.range = Some((
                            values.iter().copied().fold(f64::INFINITY, f64::min),
                            values.iter().copied().fold(0., f64::max),
                        ));
                    }
                }
            }
            "runtime_state" if runtime(p) && out.operating.is_none() => {
                out.operating = Some(format!("shz_runtime_{}", p.text))
            }
            "mali_util_unknown"
                if allowed(p)
                    && !out.utilization.metric.is_available()
                    && p.status == "readable"
                    && out.utilization.metric.reason() == Some("shz_source_missing") =>
            {
                out.utilization = Reading {
                    metric: MetricValue::Unsupported("shz_unverified".into()),
                    source: p.path.clone(),
                };
            }
            "discovery" if allowed(p) && p.status == "read_denied" => {
                let target = if p.path.starts_with("/sys/class/thermal") {
                    Some(&mut out.temperature)
                } else if p.path == "/sys/class/devfreq" {
                    Some(&mut out.frequency)
                } else {
                    None
                };
                if let Some(target) = target {
                    target.consider(
                        MetricValue::PermissionError("shz_read_denied".into()),
                        &p.path,
                    );
                }
            }
            _ => {}
        }
        let status = if !allowed(p) {
            "shz_invalid_payload"
        } else if p.status == "readable"
            && ["mali_util_unknown", "ged_freq_unknown", "thermal_candidate"]
                .contains(&p.kind.as_str())
        {
            "shz_unverified"
        } else if (p.kind == "identity" && p.status == "readable" && !p.text.is_empty())
            || metric.is_available()
            || (p.kind == "runtime_state" && runtime(p))
            || (p.kind == "freqs_hz" && frequencies(p).is_some())
        {
            "gpu_readable"
        } else if p.status == "readable" && ["runtime_state", "freqs_hz"].contains(&p.kind.as_str())
        {
            "shz_invalid_reading"
        } else {
            metric.reason().unwrap_or("shz_read_failed")
        };
        if let Some(loc) = loc {
            out.source_rows.push(ControlRow {
                label: loc.tr(title(p)).into(),
                value: loc.tr(status).into(),
                detail: format!(
                    "{}{}\n{}",
                    p.path,
                    if p.label.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", p.label)
                    },
                    p.text
                )
                .into(),
            });
        }
    }
    // min/max are configured driver constraints, not a substitute for a supported OPP list.
    out
}
pub fn rows(
    s: &Snapshot,
    foreground: bool,
    now: u64,
    loc: &Localizer,
) -> (Validated, Vec<ControlRow>) {
    let v = validate(s, foreground, now, loc);
    let row = |label: &str, value: String, detail: String| ControlRow {
        label: loc.tr(label).into(),
        value: value.into(),
        detail: detail.into(),
    };
    let mut rows = vec![
        row(
            "shz_connection",
            loc.tr(if !s.enabled {
                "shz_disabled"
            } else if !s.installed {
                "shz_not_installed"
            } else if !s.service_running {
                "shz_not_running"
            } else {
                &v.state
            })
            .into(),
            String::new(),
        ),
        row(
            "shz_authorization",
            loc.tr(if !s.service_running {
                "status_unavailable"
            } else {
                match s.authorization.as_str() {
                    "granted" => "cc_granted",
                    "denied" => "cc_denied",
                    "requesting" => "shz_requesting",
                    _ => "shz_not_requested",
                }
            })
            .into(),
            String::new(),
        ),
    ];
    if s.enabled {
        for (title, r, unit) in [
            ("gpu_utilization", &v.utilization, "%"),
            ("gpu_core_clock", &v.frequency, " MHz"),
            ("cc_gpu_temp", &v.temperature, " °C"),
        ] {
            rows.push(row(
                title,
                r.metric
                    .copied()
                    .map(|n| format!("{n:.1}{unit}"))
                    .unwrap_or_else(|| loc.tr("status_unavailable").into()),
                if r.metric.is_available() {
                    r.source.clone()
                } else {
                    loc.tr(r.metric.reason().unwrap_or("shz_read_failed"))
                        .into()
                },
            ));
        }
        rows.push(row(
            "shz_operating",
            v.operating
                .as_ref()
                .map(|k| loc.tr(k).into())
                .unwrap_or_else(|| loc.tr("status_unavailable").into()),
            String::new(),
        ));
        rows.push(row(
            "shz_range",
            v.range
                .map(|(a, b)| format!("{a:.0}–{b:.0} MHz"))
                .unwrap_or_else(|| loc.tr("status_unavailable").into()),
            loc.tr("shz_range_note").into(),
        ));
    }
    (v, rows)
}

/// Bounded UI intents: latest enable/disable plus deduplicated authorization/manager.
/// Enabling is consumed before authorization, so fast consecutive taps cannot lose it.
#[derive(Default)]
pub struct ActionQueue(std::sync::atomic::AtomicU8);
impl ActionQueue {
    pub const fn new() -> Self {
        Self(std::sync::atomic::AtomicU8::new(0))
    }
    pub fn request(&self, action: i32) {
        use std::sync::atomic::Ordering;
        let mut bits = self.0.load(Ordering::Acquire);
        loop {
            let updated = match action {
                1 => (bits & !3) | 1,
                2 => (bits & !7) | 2,
                3 => bits | 4,
                4 => bits | 8,
                _ => return,
            };
            match self
                .0
                .compare_exchange_weak(bits, updated, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(current) => bits = current,
            }
        }
    }
    pub fn take(&self) -> i32 {
        use std::sync::atomic::Ordering;
        let mut bits = self.0.load(Ordering::Acquire);
        loop {
            let (mask, action) = if bits & 2 != 0 {
                (2, 2)
            } else if bits & 1 != 0 {
                (1, 1)
            } else if bits & 4 != 0 {
                (4, 3)
            } else if bits & 8 != 0 {
                (8, 4)
            } else {
                return 0;
            };
            match self.0.compare_exchange_weak(
                bits,
                bits & !mask,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return action,
                Err(updated) => bits = updated,
            }
        }
    }
}
