//! Only verified counter formats may produce measurements.
use crate::models::metrics::MetricValue;
use std::{io, path::Path};

/// KGSL documents gpubusy as busy/total microseconds. Zero total is a
/// temporary lack of a sample; malformed or inconsistent counters are failures.
pub fn kgsl_busy(raw: &str) -> MetricValue<f64> {
    let values: Vec<_> = raw.split_whitespace().collect();
    if values.len() == 2 {
        if let (Ok(busy), Ok(total)) = (values[0].parse::<u64>(), values[1].parse::<u64>()) {
            if total == 0 {
                return MetricValue::Unavailable(
                    "Temporarily unavailable: counter interval is zero".into(),
                );
            }
            if busy <= total {
                return MetricValue::Available(busy as f64 / total as f64 * 100.);
            }
        }
    }
    MetricValue::Unavailable("Sensor read failure: invalid KGSL busy/total counters".into())
}

pub fn read_metric<T>(path: &str, parse: impl FnOnce(&str) -> MetricValue<T>) -> MetricValue<T> {
    match std::fs::read_to_string(Path::new(path)) {
        Ok(raw) => parse(&raw),
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => MetricValue::Unsupported(format!("Device/driver does not expose {path}")),
            io::ErrorKind::PermissionDenied => MetricValue::PermissionError(format!("Permission denied: {path}. Android sandbox or SELinux policy may restrict this node; the cause cannot be identified from EACCES alone.")),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => MetricValue::Unavailable(format!("Temporarily unavailable: {path}")),
            _ => MetricValue::Unavailable(format!("Sensor read failure: {path}: {err}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_counters_never_fabricate_or_clamp() {
        assert_eq!(kgsl_busy("25 100"), MetricValue::Available(25.));
        assert_eq!(kgsl_busy("0 100"), MetricValue::Available(0.));
        for raw in ["101 100", "-1 100", "NaN", "10", "10 100 200"] {
            assert!(!kgsl_busy(raw).is_available(), "{raw}");
        }
        assert!(kgsl_busy("0 0").reason().unwrap().contains("Temporarily"));
    }
    #[test]
    fn absent_node_is_not_permission_denied() {
        let value = read_metric("/nonexistent-voltix-gpu-counter", kgsl_busy);
        assert!(matches!(value, MetricValue::Unsupported(_)));
    }
}
