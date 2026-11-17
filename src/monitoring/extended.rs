//! Unprivileged, bounded collectors. Rates require two samples and never bridge resets.
pub use super::processes::{parse_process, Process};
use super::processes::{ProcessDetails, ProcessKey};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::fs;
use std::sync::Arc;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Battery {
    pub percent: Option<f64>,
    pub status: i32,
    pub source: i32,
    pub temperature_c: Option<f64>,
    pub voltage_v: Option<f64>,
    pub current_a: Option<f64>,
    pub power_w: Option<f64>,
    pub technology: String,
    pub health: i32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Platform {
    pub elapsed_ms: u64,
    pub battery: Option<Battery>,
    pub foreground: bool,
    pub received: Option<u64>,
    pub transmitted: Option<u64>,
    pub network: String,
    pub network_permission: bool,
    pub display: String,
    pub sensors: Vec<String>,
    pub memory_total: Option<u64>,
    pub memory_available: Option<u64>,
    pub memory_low: bool,
    pub export_status: String,
    pub action_status: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Network {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
    pub down_bps: Option<f64>,
    pub up_bps: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct DiskIo {
    pub name: String,
    pub read_bps: Option<f64>,
    pub write_bps: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Sensor {
    pub name: String,
    pub value: f64,
    pub unit: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Extended {
    pub network: Vec<Network>,
    pub disks: Vec<DiskIo>,
    pub processes: Arc<Vec<Process>>,
    pub process_count: Option<u64>,
    pub thread_count: Option<u64>,
    pub process_generation: u64,
    pub process_current_uid: Option<u32>,
    pub process_truncated: bool,
    // Selected-process arguments belong to the local details view, not the
    // existing telemetry export (which only promises basic process readings).
    #[serde(skip_serializing)]
    pub process_details: Option<ProcessDetails>,
    pub sensors: Vec<Sensor>,
    pub battery: Option<Battery>,
    pub memory_pressure: Option<f64>,
    pub platform: Platform,
    pub network_available: bool,
    pub disk_available: bool,
    pub processes_available: bool,
}
pub fn rate(previous: u64, current: u64, seconds: f64) -> Option<f64> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    current
        .checked_sub(previous)
        .map(|delta| delta as f64 / seconds)
}
pub fn parse_net_dev(text: &str) -> Vec<(String, u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let (name, data) = line.split_once(':')?;
            let values: Vec<_> = data.split_whitespace().collect();
            let name = name.trim();
            if name == "lo" {
                return None;
            }
            Some((
                name.into(),
                values.first()?.parse().ok()?,
                values.get(8)?.parse().ok()?,
            ))
        })
        .collect()
}
pub fn parse_diskstats(text: &str) -> Vec<(String, u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let v: Vec<_> = line.split_whitespace().collect();
            Some((
                v.get(2)?.to_string(),
                v.get(5)?.parse::<u64>().ok()?.checked_mul(512)?,
                v.get(9)?.parse::<u64>().ok()?.checked_mul(512)?,
            ))
        })
        .collect()
}
#[cfg(target_os = "linux")]
fn number(path: impl AsRef<std::path::Path>) -> Option<f64> {
    let v = fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()?;
    v.is_finite().then_some(v)
}
#[cfg(target_os = "linux")]
fn text(path: impl AsRef<std::path::Path>) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub struct ExtendedMonitor {
    previous: Instant,
    #[cfg(target_os = "android")]
    platform_time: Option<u64>,
    expensive_at: Option<Instant>,
    net: HashMap<String, (u64, u64)>,
    #[cfg(target_os = "linux")]
    disk: HashMap<String, (u64, u64)>,
    #[cfg(target_os = "linux")]
    process_monitor: super::processes::ProcessMonitor,
    cached: Extended,
}
impl Default for ExtendedMonitor {
    fn default() -> Self {
        Self {
            previous: Instant::now(),
            #[cfg(target_os = "android")]
            platform_time: None,
            expensive_at: None,
            net: HashMap::new(),
            #[cfg(target_os = "linux")]
            disk: HashMap::new(),
            #[cfg(target_os = "linux")]
            process_monitor: super::processes::ProcessMonitor::default(),
            cached: Extended::default(),
        }
    }
}
impl ExtendedMonitor {
    pub fn collect(&mut self, want_processes: bool, selected: Option<ProcessKey>) -> Extended {
        let now = Instant::now();
        let seconds = now.duration_since(self.previous).as_secs_f64();
        self.previous = now;
        #[cfg(target_os = "linux")]
        {
            let data = fs::read_to_string("/proc/net/dev");
            self.cached.network_available = data.is_ok();
            self.network(parse_net_dev(&data.unwrap_or_default()), seconds);
            let data = fs::read_to_string("/proc/diskstats");
            self.cached.disk_available = data.is_ok();
            let mut next = HashMap::new();
            self.cached.disks = parse_diskstats(&data.unwrap_or_default())
                .into_iter()
                .filter(|(name, _, _)| {
                    std::path::Path::new("/sys/block")
                        .join(name)
                        .join("device")
                        .exists()
                })
                .map(|(name, read, write)| {
                    let previous = self.disk.get(&name);
                    let row = DiskIo {
                        name: name.clone(),
                        read_bps: previous.and_then(|v| rate(v.0, read, seconds)),
                        write_bps: previous.and_then(|v| rate(v.1, write, seconds)),
                    };
                    next.insert(name, (read, write));
                    row
                })
                .collect();
            self.disk = next;
        }
        #[cfg(target_os = "android")]
        {
            let platform = crate::platform::android_bridge::snapshot();
            self.cached.network_available = platform.foreground
                && platform.received.is_some()
                && platform.transmitted.is_some();
            if platform.foreground {
                if self.platform_time != Some(platform.elapsed_ms) {
                    let dt = self
                        .platform_time
                        .and_then(|last| platform.elapsed_ms.checked_sub(last))
                        .map(|ms| ms as f64 / 1000.0)
                        .unwrap_or(0.0);
                    let counters = platform
                        .received
                        .zip(platform.transmitted)
                        .map(|(r, t)| {
                            vec![(
                                if platform.network.is_empty() {
                                    "TrafficStats".into()
                                } else {
                                    platform.network.clone()
                                },
                                r,
                                t,
                            )]
                        })
                        .unwrap_or_default();
                    self.network(counters, dt);
                    self.platform_time = Some(platform.elapsed_ms);
                }
                self.cached.battery = platform.battery.clone();
            } else {
                self.cached.battery = None;
                self.network(Vec::new(), seconds);
                self.platform_time = None;
            }
            self.cached.platform = platform;
        }
        if self
            .expensive_at
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(2))
        {
            self.expensive_at = Some(now);
            #[cfg(target_os = "linux")]
            {
                self.cached.sensors = linux_sensors();
                self.cached.battery = linux_battery();
                self.cached.memory_pressure = text("/proc/pressure/memory")
                    .lines()
                    .find(|line| line.starts_with("some "))
                    .and_then(|line| {
                        line.split_whitespace()
                            .find_map(|v| v.strip_prefix("avg10=").and_then(|n| n.parse().ok()))
                    });
                if want_processes {
                    let sample = self.process_monitor.sample(now, selected);
                    self.cached.processes = sample.rows;
                    self.cached.process_count = sample.count;
                    self.cached.thread_count = sample.threads;
                    self.cached.process_current_uid = Some(sample.current_uid);
                    self.cached.processes_available = sample.available;
                    self.cached.process_generation = sample.generation;
                    self.cached.process_truncated = sample.truncated;
                    self.cached.process_details = sample.details;
                } else {
                    self.process_monitor.reset();
                    self.cached.processes = Arc::default();
                    self.cached.processes_available = false;
                    self.cached.process_details = None;
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (want_processes, selected);
        self.cached.clone()
    }
    fn network(&mut self, values: Vec<(String, u64, u64)>, seconds: f64) {
        let mut next = HashMap::new();
        self.cached.network = values
            .into_iter()
            .map(|(name, r, t)| {
                let previous = self.net.get(&name);
                let row = Network {
                    name: name.clone(),
                    received: r,
                    transmitted: t,
                    down_bps: previous.and_then(|v| rate(v.0, r, seconds)),
                    up_bps: previous.and_then(|v| rate(v.1, t, seconds)),
                };
                next.insert(name, (r, t));
                row
            })
            .collect();
        self.net = next;
    }
}
#[cfg(target_os = "linux")]
fn linux_hwmon() -> Vec<Sensor> {
    let mut sensors = Vec::new();
    for entry in fs::read_dir("/sys/class/hwmon")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .take(64)
    {
        let root = entry.path();
        let driver = text(root.join("name"));
        for sensor in fs::read_dir(&root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let file = sensor.file_name().to_string_lossy().into_owned();
            if !file.ends_with("_input") {
                continue;
            }
            let (scale, unit) = if file.starts_with("temp") {
                (1000.0, "°C")
            } else if file.starts_with("fan") {
                (1.0, "RPM")
            } else if file.starts_with("power") {
                (1_000_000.0, "W")
            } else if file.starts_with("in") {
                (1000.0, "V")
            } else {
                continue;
            };
            if let Some(value) = number(sensor.path()) {
                let label = text(root.join(file.replace("_input", "_label")));
                sensors.push(Sensor {
                    name: format!("{driver} / {}", if label.is_empty() { file } else { label }),
                    value: value / scale,
                    unit: unit.into(),
                });
            }
            if sensors.len() >= 128 {
                return sensors;
            }
        }
    }
    sensors
}
#[cfg(target_os = "linux")]
fn linux_sensors() -> Vec<Sensor> {
    let mut sensors = linux_hwmon();
    for entry in fs::read_dir("/sys/class/thermal")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("thermal_zone"))
        .take(64)
    {
        if sensors.len() >= 128 {
            break;
        }
        if let Some(value) = number(entry.path().join("temp")) {
            sensors.push(Sensor {
                name: format!(
                    "{} / {}",
                    entry.file_name().to_string_lossy(),
                    text(entry.path().join("type"))
                ),
                value: value / 1000.0,
                unit: "°C".into(),
            });
        }
    }
    sensors
}
#[cfg(target_os = "linux")]
fn linux_charging_source() -> i32 {
    let mut seen = false;
    for entry in fs::read_dir("/sys/class/power_supply")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let kind = text(entry.path().join("type"));
        let code = match kind.as_str() {
            "Mains" => 1,
            "USB" | "USB_C" | "USB_PD" => 2,
            "Wireless" => 4,
            _ => continue,
        };
        if let Some(online) = number(entry.path().join("online")) {
            seen = true;
            if online > 0.0 {
                return code;
            }
        }
    }
    if seen {
        0
    } else {
        -1
    }
}
#[cfg(target_os = "linux")]
fn linux_battery() -> Option<Battery> {
    for entry in fs::read_dir("/sys/class/power_supply")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if text(p.join("type")) != "Battery" || text(p.join("present")) == "0" {
            continue;
        }
        let status = match text(p.join("status")).as_str() {
            "Charging" => 2,
            "Discharging" => 3,
            "Not charging" => 4,
            "Full" => 5,
            _ => 1,
        };
        let voltage = number(p.join("voltage_now")).map(|v| v / 1e6);
        let current = number(p.join("current_now")).map(|v| v / 1e6);
        return Some(Battery {
            percent: number(p.join("capacity")).filter(|v| (0.0..=100.0).contains(v)),
            status,
            source: linux_charging_source(),
            voltage_v: voltage,
            current_a: current,
            power_w: number(p.join("power_now"))
                .map(|v| v / 1e6)
                .or_else(|| voltage.zip(current).map(|(v, a)| v * a)),
            temperature_c: number(p.join("temp")).map(|v| v / 10.0),
            technology: text(p.join("technology")),
            health: 0,
        });
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rates_reject_resets_first_samples_and_invalid_time() {
        assert_eq!(rate(100, 300, 2.0), Some(100.0));
        assert_eq!(rate(300, 1, 2.0), None);
        assert_eq!(rate(1, 2, 0.0), None);
        assert_eq!(rate(1, 2, f64::NAN), None);
    }
    #[test]
    fn network_and_disk_counters_have_correct_units() {
        let n = parse_net_dev(" eth0: 100 0 0 0 0 0 0 0 200 0\n lo: 1 0 0 0 0 0 0 0 2");
        assert_eq!(n, vec![("eth0".into(), 100, 200)]);
        assert_eq!(
            parse_diskstats("8 0 sda 1 0 4 0 2 0 8 0"),
            vec![("sda".into(), 2048, 4096)]
        );
    }
    #[test]
    fn process_stat_handles_spaces_and_parentheses() {
        let fields = "R 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21";
        let (p, t, start) =
            parse_process(4, &format!("4 (name (with) spaces) {fields}"), 4096).unwrap();
        assert_eq!(p.name, "name (with) spaces");
        assert_eq!(t, 23);
        assert_eq!(start, 19);
        assert_eq!(p.memory, 21 * 4096);
    }
}
