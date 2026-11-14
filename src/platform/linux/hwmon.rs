use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::models::metrics::MetricValue;

#[derive(Debug, Clone)]
pub struct CpuTempSensor {
    pub driver_name: String,
    pub label: Option<String>,
    pub input_path: PathBuf,
}

/// CPU-specific hwmon driver names on Linux.
/// We explicitly restrict CPU temperature discovery to known CPU thermal drivers
/// rather than guessing from generic motherboard/ACPI sensors.
const CPU_HWMON_DRIVERS: &[&str] = &[
    "coretemp",
    "k10temp",
    "zenpower",
    "cpu_thermal",
    "soc_thermal",
];

pub fn read_hwmon_millidegrees_celsius(path: &Path) -> MetricValue<f64> {
    match fs::read_to_string(path) {
        Ok(raw) => match raw.trim().parse::<i64>() {
            Ok(milli) => MetricValue::Available(milli as f64 / 1000.0),
            Err(err) => MetricValue::Unavailable(format!("Malformed sensor value: {err}")),
        },
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            MetricValue::PermissionError(format!("Permission denied: {}", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            MetricValue::Unavailable(format!("Sensor vanished: {}", path.display()))
        }
        Err(err) => MetricValue::Unavailable(format!("Sensor read error: {err}")),
    }
}

pub fn read_hwmon_microwatts_to_watts(path: &Path) -> MetricValue<f64> {
    match fs::read_to_string(path) {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(uw) => MetricValue::Available(uw as f64 / 1_000_000.0),
            Err(err) => MetricValue::Unavailable(format!("Malformed power value: {err}")),
        },
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            MetricValue::PermissionError(format!("Permission denied: {}", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            MetricValue::Unsupported("Power sensor not exposed".to_string())
        }
        Err(err) => MetricValue::Unavailable(format!("Power read error: {err}")),
    }
}

pub fn discover_cpu_temp_sensor() -> Option<CpuTempSensor> {
    discover_cpu_temp_sensor_in(Path::new("/sys/class/hwmon"))
}

pub fn discover_cpu_temp_sensor_in(hwmon_root: &Path) -> Option<CpuTempSensor> {
    let entries = fs::read_dir(hwmon_root).ok()?;
    let mut candidates: Vec<(u8, CpuTempSensor)> = Vec::new();

    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(name_raw) = fs::read_to_string(dir.join("name")) else {
            continue;
        };
        let driver = name_raw.trim().to_string();
        if !CPU_HWMON_DRIVERS.contains(&driver.as_str()) {
            continue;
        }

        let Ok(files) = fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let fname = file.file_name();
            let fname_str = fname.to_string_lossy();
            if !(fname_str.starts_with("temp") && fname_str.ends_with("_input")) {
                continue;
            }
            let prefix = fname_str.trim_end_matches("_input");
            let label = fs::read_to_string(dir.join(format!("{prefix}_label")))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let priority = match (driver.as_str(), label.as_deref()) {
                // AMD Ryzen: Tdie is actual die temp; Tctl may carry an offset on older chips
                ("k10temp" | "zenpower", Some("Tdie")) => 0,
                ("k10temp" | "zenpower", Some("Tctl")) => 1,
                // Intel: Package id 0 is package temperature
                ("coretemp", Some(l)) if l.starts_with("Package id 0") => 0,
                ("coretemp", Some(l)) if l.starts_with("Package") => 1,
                // temp1_input on CPU thermal drivers
                (_, _) if prefix == "temp1" => 2,
                _ => 3,
            };

            candidates.push((
                priority,
                CpuTempSensor {
                    driver_name: driver.clone(),
                    label,
                    input_path: file.path(),
                },
            ));
        }
    }

    candidates.sort_by_key(|(prio, _)| *prio);
    candidates.into_iter().next().map(|(_, sensor)| sensor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_non_cpu_hwmon_drivers() {
        let base = std::env::temp_dir().join("voltix_test_hwmon_ignore");
        let _ = fs::remove_dir_all(&base);
        let h0 = base.join("hwmon0");
        fs::create_dir_all(&h0).expect("mkdir");
        fs::write(h0.join("name"), "nct6775\n").expect("write name");
        fs::write(h0.join("temp1_input"), "45000\n").expect("write temp");

        assert!(discover_cpu_temp_sensor_in(&base).is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn prefers_tdie_over_tctl_for_k10temp() {
        let base = std::env::temp_dir().join("voltix_test_hwmon_k10temp");
        let _ = fs::remove_dir_all(&base);
        let h0 = base.join("hwmon0");
        fs::create_dir_all(&h0).expect("mkdir");
        fs::write(h0.join("name"), "k10temp\n").expect("write name");
        fs::write(h0.join("temp1_label"), "Tctl\n").expect("write label1");
        fs::write(h0.join("temp1_input"), "62000\n").expect("write temp1");
        fs::write(h0.join("temp2_label"), "Tdie\n").expect("write label2");
        fs::write(h0.join("temp2_input"), "52000\n").expect("write temp2");

        let selected = discover_cpu_temp_sensor_in(&base).expect("should find sensor");
        assert_eq!(selected.label.as_deref(), Some("Tdie"));
        assert_eq!(
            read_hwmon_millidegrees_celsius(&selected.input_path),
            MetricValue::Available(52.0)
        );
        let _ = fs::remove_dir_all(&base);
    }
}
