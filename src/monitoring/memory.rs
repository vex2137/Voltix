use std::fs;
use std::io::ErrorKind;

use crate::models::metrics::{MetricStatus, MetricValue};
use crate::models::telemetry::MemoryTelemetry;

#[derive(Default)]
pub struct MemoryMonitor;

impl MemoryMonitor {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&self) -> MemoryTelemetry {
        match fs::read_to_string("/proc/meminfo") {
            Ok(content) => parse_proc_meminfo(&content),
            Err(err) => {
                let status = if err.kind() == ErrorKind::PermissionDenied {
                    MetricStatus::PermissionError
                } else {
                    MetricStatus::Unavailable
                };
                #[cfg(target_os = "android")]
                {
                    let mut mem = unavailable_memory(
                        status,
                        &format!("Unable to read /proc/meminfo: {err}"),
                    );
                    let mut info: libc::sysinfo = unsafe { std::mem::zeroed() };
                    if unsafe { libc::sysinfo(&mut info) } == 0 && info.totalram > 0 {
                        let unit = (info.mem_unit as u64).max(1);
                        let total = (info.totalram as u64).saturating_mul(unit);
                        mem.total_bytes = MetricValue::Available(total);
                    }
                    mem
                }
                #[cfg(not(target_os = "android"))]
                {
                    unavailable_memory(status, &format!("Unable to read /proc/meminfo: {err}"))
                }
            }
        }
    }
}

fn map_metric_error<T>(status: MetricStatus, reason: &str) -> MetricValue<T> {
    match status {
        MetricStatus::PermissionError => MetricValue::PermissionError(reason.to_string()),
        MetricStatus::Unsupported => MetricValue::Unsupported(reason.to_string()),
        _ => MetricValue::Unavailable(reason.to_string()),
    }
}

fn unavailable_memory(status: MetricStatus, reason: &str) -> MemoryTelemetry {
    MemoryTelemetry {
        total_bytes: map_metric_error(status, reason),
        available_bytes: map_metric_error(status, reason),
        used_bytes: map_metric_error(status, reason),
        utilization_percent: map_metric_error(status, reason),
        swap_total_bytes: map_metric_error(status, reason),
        swap_used_bytes: map_metric_error(status, reason),
        swap_utilization_percent: map_metric_error(status, reason),
    }
}

pub fn parse_proc_meminfo(content: &str) -> MemoryTelemetry {
    let mut mem_total_kib: Option<u64> = None;
    let mut mem_available_kib: Option<u64> = None;
    let mut swap_total_kib: Option<u64> = None;
    let mut swap_free_kib: Option<u64> = None;

    for line in content.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value_kib = rest
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u64>().ok());

        match key.trim() {
            "MemTotal" => mem_total_kib = value_kib,
            "MemAvailable" => mem_available_kib = value_kib,
            "SwapTotal" => swap_total_kib = value_kib,
            "SwapFree" => swap_free_kib = value_kib,
            _ => {}
        }
    }

    let total_bytes = mem_total_kib
        .map(|k| MetricValue::Available(k.saturating_mul(1024)))
        .unwrap_or_else(|| MetricValue::Unavailable("MemTotal missing in /proc/meminfo".into()));

    let available_bytes = mem_available_kib
        .map(|k| MetricValue::Available(k.saturating_mul(1024)))
        .unwrap_or_else(|| {
            MetricValue::Unavailable("MemAvailable missing in /proc/meminfo".into())
        });

    let (used_bytes, utilization_percent) = match (&total_bytes, &available_bytes) {
        (MetricValue::Available(total), MetricValue::Available(avail)) if *total > 0 => {
            let used = total.saturating_sub(*avail);
            let pct = ((used as f64) / (*total as f64)) * 100.0;
            (MetricValue::Available(used), MetricValue::Available(pct.clamp(0.0, 100.0)))
        }
        (MetricValue::Available(0), _) => (
            MetricValue::Unavailable("MemTotal reported as zero".into()),
            MetricValue::Unavailable("MemTotal reported as zero".into()),
        ),
        _ => (
            MetricValue::Unavailable("Missing MemTotal or MemAvailable".into()),
            MetricValue::Unavailable("Cannot calculate RAM utilization".into()),
        ),
    };

    let swap_total_bytes = swap_total_kib
        .map(|k| MetricValue::Available(k.saturating_mul(1024)))
        .unwrap_or_else(|| MetricValue::Unavailable("SwapTotal missing".into()));

    let (swap_used_bytes, swap_utilization_percent) = match (swap_total_kib, swap_free_kib) {
        (Some(total_k), Some(free_k)) => {
            let total_b = total_k.saturating_mul(1024);
            let free_b = free_k.saturating_mul(1024);
            let used_b = total_b.saturating_sub(free_b);
            let pct = if total_b > 0 {
                MetricValue::Available(((used_b as f64) / (total_b as f64)) * 100.0)
            } else {
                MetricValue::Unsupported("No swap space configured".into())
            };
            (MetricValue::Available(used_b), pct)
        }
        _ => (
            MetricValue::Unavailable("SwapFree missing".into()),
            MetricValue::Unavailable("SwapFree missing".into()),
        ),
    };

    MemoryTelemetry {
        total_bytes,
        available_bytes,
        used_bytes,
        utilization_percent,
        swap_total_bytes,
        swap_used_bytes,
        swap_utilization_percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo_using_memavailable() {
        let sample = "\
MemTotal:       32768000 kB
MemFree:         2048000 kB
MemAvailable:   16384000 kB
Buffers:          500000 kB
Cached:         13000000 kB
SwapTotal:       8388608 kB
SwapFree:        6291456 kB
";
        let mem = parse_proc_meminfo(sample);
        assert_eq!(mem.total_bytes, MetricValue::Available(32_768_000 * 1024));
        assert_eq!(mem.available_bytes, MetricValue::Available(16_384_000 * 1024));
        assert_eq!(mem.used_bytes, MetricValue::Available(16_384_000 * 1024));
        assert_eq!(mem.utilization_percent, MetricValue::Available(50.0));
        assert_eq!(mem.swap_total_bytes, MetricValue::Available(8_388_608 * 1024));
        assert_eq!(mem.swap_used_bytes, MetricValue::Available(2_097_152 * 1024));
        assert_eq!(mem.swap_utilization_percent, MetricValue::Available(25.0));
    }

    #[test]
    fn missing_memavailable_is_reported_unavailable_not_memfree() {
        let sample = "\
MemTotal:       16384000 kB
MemFree:         1000000 kB
";
        let mem = parse_proc_meminfo(sample);
        assert!(matches!(mem.available_bytes, MetricValue::Unavailable(_)));
        assert!(matches!(mem.used_bytes, MetricValue::Unavailable(_)));
    }
}
