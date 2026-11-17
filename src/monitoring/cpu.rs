use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use std::time::Instant;

use crate::models::metrics::MetricValue;
use crate::models::telemetry::{CpuTelemetry, CpuUtilizationScope, LogicalCpuTelemetry};
#[cfg(target_os = "android")]
use crate::platform::android::{android_cpu_temperature, android_soc_model, android_soc_vendor};
#[cfg(target_os = "linux")]
use crate::platform::linux::hwmon::{
    discover_cpu_temp_sensor, read_hwmon_millidegrees_celsius, CpuTempSensor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle_total: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessCpuSample {
    pub cpu_seconds: f64,
    pub wall_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStatSample {
    pub overall: CpuTimes,
    pub per_cpu: BTreeMap<usize, CpuTimes>,
}

#[derive(Debug, Clone)]
pub struct StaticCpuInfo {
    pub model_name: MetricValue<String>,
    pub vendor: MetricValue<String>,
    pub architecture: String,
    pub physical_cores: MetricValue<usize>,
    pub logical_cpus: MetricValue<usize>,
}

pub struct CpuMonitor {
    static_info: StaticCpuInfo,
    prev_stat: Option<ProcStatSample>,
    prev_proc_usage: Option<ProcessCpuSample>,
    #[cfg(target_os = "linux")]
    temp_sensor: Option<CpuTempSensor>,
    #[cfg(target_os = "linux")]
    temp_sensor_checked: bool,
}

impl Default for CpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuMonitor {
    #[cfg(target_os = "android")]
    pub fn resume_after_pause(&mut self) {
        self.prev_stat = None;
        self.prev_proc_usage = None;
    }
    pub fn new() -> Self {
        let static_info = read_static_cpu_info();
        // Capture initial /proc/stat and process CPU baselines so the first UI sample
        // has a real delta after the brief warmup or first poll tick.
        let prev_stat = fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|s| parse_proc_stat(&s).ok());
        let prev_proc_usage = read_process_cpu_sample();

        Self {
            static_info,
            prev_stat,
            prev_proc_usage,
            #[cfg(target_os = "linux")]
            temp_sensor: None,
            #[cfg(target_os = "linux")]
            temp_sensor_checked: false,
        }
    }

    pub fn collect(&mut self) -> CpuTelemetry {
        let stat_read = fs::read_to_string("/proc/stat");
        let stat_perm_denied =
            matches!(&stat_read, Err(e) if e.kind() == ErrorKind::PermissionDenied);
        let current_stat = stat_read
            .map_err(|e| e.to_string())
            .and_then(|s| parse_proc_stat(&s));

        let (overall_utilization, per_core_util) = match (&self.prev_stat, &current_stat) {
            (Some(prev), Ok(curr)) => {
                let overall = compute_utilization(prev.overall, curr.overall);
                let mut per_core = Vec::with_capacity(curr.per_cpu.len());
                for (&id, &curr_times) in &curr.per_cpu {
                    let util = prev
                        .per_cpu
                        .get(&id)
                        .map(|&p| compute_utilization(p, curr_times))
                        .unwrap_or_else(|| {
                            MetricValue::Unavailable("Core just came online".to_string())
                        });
                    per_core.push((id, util));
                }
                (overall, per_core)
            }
            (None, Ok(curr)) => {
                let msg = "Awaiting second CPU sample";
                let per_core = curr
                    .per_cpu
                    .keys()
                    .map(|&id| (id, MetricValue::Unavailable(msg.to_string())))
                    .collect();
                (MetricValue::Unavailable(msg.to_string()), per_core)
            }
            (_, Err(_)) if stat_perm_denied => {
                let reason = "Restricted by OS/SELinux policy";
                let count = self.static_info.logical_cpus.copied().unwrap_or(0);
                let per_core = (0..count)
                    .map(|id| (id, MetricValue::PermissionError(reason.to_string())))
                    .collect();
                (MetricValue::PermissionError(reason.to_string()), per_core)
            }
            (_, Err(err)) => (MetricValue::Unavailable(err.clone()), Vec::new()),
        };

        if let Ok(curr) = current_stat {
            self.prev_stat = Some(curr);
        }

        let logical_count_hint = self
            .static_info
            .logical_cpus
            .copied()
            .unwrap_or(per_core_util.len())
            .max(1);

        let curr_proc_usage = read_process_cpu_sample();
        let process_utilization = match (self.prev_proc_usage, curr_proc_usage) {
            (Some(prev), Some(curr)) => compute_process_utilization(prev, curr, logical_count_hint),
            (None, Some(_)) => {
                MetricValue::Unavailable("Awaiting second process CPU sample".into())
            }
            (_, None) => MetricValue::Unsupported("Process CPU rusage unsupported".into()),
        };
        if curr_proc_usage.is_some() {
            self.prev_proc_usage = curr_proc_usage;
        }

        let utilization_scope = if overall_utilization.is_available() {
            CpuUtilizationScope::SystemWide
        } else if process_utilization.is_available() {
            CpuUtilizationScope::ProcessOnly
        } else {
            CpuUtilizationScope::Unavailable
        };

        let per_core_freqs = read_sysfs_cpu_frequencies(
            self.static_info
                .logical_cpus
                .copied()
                .unwrap_or(per_core_util.len()),
        );

        let mut per_core = Vec::with_capacity(per_core_util.len());
        let mut freq_sum = 0.0_f64;
        let mut freq_count = 0usize;

        for (id, util) in per_core_util {
            let freq = per_core_freqs
                .get(&id)
                .cloned()
                .unwrap_or_else(|| MetricValue::Unsupported("cpufreq not exposed".to_string()));
            if let Some(mhz) = freq.copied() {
                freq_sum += mhz;
                freq_count += 1;
            }
            per_core.push(LogicalCpuTelemetry {
                id,
                utilization_percent: util,
                frequency_mhz: freq,
            });
        }

        let average_frequency_mhz = if freq_count > 0 {
            MetricValue::Available(freq_sum / freq_count as f64)
        } else if per_core
            .iter()
            .any(|c| matches!(c.frequency_mhz, MetricValue::PermissionError(_)))
        {
            MetricValue::PermissionError("cpufreq restricted by OS/SELinux policy".to_string())
        } else {
            read_cpuinfo_average_mhz()
        };

        let (temperature_celsius, temperature_source) = self.collect_cpu_temperature();

        let logical_cpus = if self.static_info.logical_cpus.is_available() {
            self.static_info.logical_cpus.clone()
        } else if !per_core.is_empty() {
            MetricValue::Available(per_core.len())
        } else {
            MetricValue::Unavailable("Logical CPU count unavailable".to_string())
        };

        CpuTelemetry {
            model_name: self.static_info.model_name.clone(),
            vendor: self.static_info.vendor.clone(),
            architecture: self.static_info.architecture.clone(),
            physical_cores: self.static_info.physical_cores.clone(),
            logical_cpus,
            overall_utilization,
            process_utilization,
            utilization_scope,
            average_frequency_mhz,
            temperature_celsius,
            temperature_source,
            per_core,
        }
    }

    fn collect_cpu_temperature(&mut self) -> (MetricValue<f64>, Option<String>) {
        #[cfg(target_os = "linux")]
        {
            if !self.temp_sensor_checked {
                self.temp_sensor_checked = true;
                self.temp_sensor = discover_cpu_temp_sensor();
            }
            match &self.temp_sensor {
                Some(sensor) => {
                    let val = read_hwmon_millidegrees_celsius(&sensor.input_path);
                    let src = match &sensor.label {
                        Some(lbl) => format!("{} ({lbl})", sensor.driver_name),
                        None => sensor.driver_name.clone(),
                    };
                    (val, Some(src))
                }
                None => (
                    MetricValue::Unsupported("No dedicated CPU thermal sensor found".to_string()),
                    None,
                ),
            }
        }
        #[cfg(target_os = "android")]
        {
            android_cpu_temperature()
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            (
                MetricValue::Unsupported("CPU thermal sensor unsupported".to_string()),
                None,
            )
        }
    }
}

#[cfg(unix)]
fn read_process_cpu_sample() -> Option<ProcessCpuSample> {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    if rc != 0 {
        return None;
    }
    let user_secs = (usage.ru_utime.tv_sec as f64) + (usage.ru_utime.tv_usec as f64) / 1_000_000.0;
    let sys_secs = (usage.ru_stime.tv_sec as f64) + (usage.ru_stime.tv_usec as f64) / 1_000_000.0;
    Some(ProcessCpuSample {
        cpu_seconds: (user_secs + sys_secs).max(0.0),
        wall_at: Instant::now(),
    })
}

#[cfg(not(unix))]
fn read_process_cpu_sample() -> Option<ProcessCpuSample> {
    None
}

pub fn compute_process_utilization(
    prev: ProcessCpuSample,
    curr: ProcessCpuSample,
    logical_cpus: usize,
) -> MetricValue<f64> {
    let wall_secs = curr
        .wall_at
        .saturating_duration_since(prev.wall_at)
        .as_secs_f64();
    if wall_secs <= 0.001 {
        return MetricValue::Unavailable("Insufficient elapsed wall time between samples".into());
    }
    let delta_cpu = (curr.cpu_seconds - prev.cpu_seconds).max(0.0);
    let capacity = wall_secs * (logical_cpus.max(1) as f64);
    let pct = (delta_cpu / capacity) * 100.0;
    MetricValue::Available(pct.clamp(0.0, 100.0))
}

pub fn compute_utilization(prev: CpuTimes, curr: CpuTimes) -> MetricValue<f64> {
    let delta_total = curr.total.saturating_sub(prev.total);
    if delta_total == 0 {
        return MetricValue::Unavailable("No elapsed CPU ticks between samples".to_string());
    }
    let delta_idle = curr.idle_total.saturating_sub(prev.idle_total);
    let busy = delta_total.saturating_sub(delta_idle);
    let pct = ((busy as f64) / (delta_total as f64)) * 100.0;
    MetricValue::Available(pct.clamp(0.0, 100.0))
}

pub fn parse_proc_stat(content: &str) -> Result<ProcStatSample, String> {
    let mut overall: Option<CpuTimes> = None;
    let mut per_cpu = BTreeMap::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let Some(label) = parts.next() else {
            continue;
        };

        if label == "cpu" {
            overall = Some(parse_cpu_times(parts)?);
        } else if let Some(idx_str) = label.strip_prefix("cpu") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                per_cpu.insert(idx, parse_cpu_times(parts)?);
            }
        }
    }

    overall
        .map(|o| ProcStatSample {
            overall: o,
            per_cpu,
        })
        .ok_or_else(|| "Missing aggregate 'cpu' line in /proc/stat".to_string())
}

fn parse_cpu_times<'a>(mut fields: impl Iterator<Item = &'a str>) -> Result<CpuTimes, String> {
    let user: u64 = fields
        .next()
        .ok_or("Missing user field")?
        .parse()
        .map_err(|e| format!("Invalid user ticks: {e}"))?;
    let nice: u64 = fields
        .next()
        .ok_or("Missing nice field")?
        .parse()
        .map_err(|e| format!("Invalid nice ticks: {e}"))?;
    let system: u64 = fields
        .next()
        .ok_or("Missing system field")?
        .parse()
        .map_err(|e| format!("Invalid system ticks: {e}"))?;
    let idle: u64 = fields
        .next()
        .ok_or("Missing idle field")?
        .parse()
        .map_err(|e| format!("Invalid idle ticks: {e}"))?;
    let iowait: u64 = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let irq: u64 = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let softirq: u64 = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let steal: u64 = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    let idle_total = idle.saturating_add(iowait);
    let non_idle = user
        .saturating_add(nice)
        .saturating_add(system)
        .saturating_add(irq)
        .saturating_add(softirq)
        .saturating_add(steal);
    let total = idle_total.saturating_add(non_idle);

    Ok(CpuTimes { idle_total, total })
}

pub fn parse_proc_cpuinfo(content: &str) -> StaticCpuInfo {
    let mut model_name: Option<String> = None;
    let mut vendor: Option<String> = None;
    let mut logical_count = 0usize;
    let mut cpu_cores_fallback: Option<usize> = None;

    let mut current_phys_id: Option<u32> = None;
    let mut current_core_id: Option<u32> = None;
    let mut physical_pairs: HashSet<(u32, u32)> = HashSet::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let (Some(pid), Some(cid)) = (current_phys_id, current_core_id) {
                physical_pairs.insert((pid, cid));
            }
            current_phys_id = None;
            current_core_id = None;
            continue;
        }

        let Some((k, v)) = trimmed.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();

        match key {
            "processor" => logical_count += 1,
            "model name" | "Hardware" | "Processor" | "cpu model" if model_name.is_none() => {
                if !val.is_empty() {
                    model_name = Some(val.to_string());
                }
            }
            "vendor_id" | "CPU implementer" if vendor.is_none() => {
                if !val.is_empty() {
                    vendor = Some(normalize_cpu_vendor(val));
                }
            }
            "physical id" => current_phys_id = val.parse().ok(),
            "core id" => current_core_id = val.parse().ok(),
            "cpu cores" if cpu_cores_fallback.is_none() => {
                cpu_cores_fallback = val.parse().ok();
            }
            _ => {}
        }
    }

    if let (Some(pid), Some(cid)) = (current_phys_id, current_core_id) {
        physical_pairs.insert((pid, cid));
    }

    let physical_cores = if !physical_pairs.is_empty() {
        MetricValue::Available(physical_pairs.len())
    } else if let Some(cores) = cpu_cores_fallback {
        MetricValue::Available(cores)
    } else {
        MetricValue::Unavailable("Physical core topology not exposed".to_string())
    };

    StaticCpuInfo {
        model_name: model_name
            .map(MetricValue::Available)
            .unwrap_or_else(|| MetricValue::Unavailable("Model name not found".to_string())),
        vendor: vendor
            .map(MetricValue::Available)
            .unwrap_or_else(|| MetricValue::Unavailable("Vendor not found".to_string())),
        architecture: std::env::consts::ARCH.to_string(),
        physical_cores,
        logical_cpus: if logical_count > 0 {
            MetricValue::Available(logical_count)
        } else {
            MetricValue::Unavailable("Processor count not found".to_string())
        },
    }
}

fn normalize_cpu_vendor(raw: &str) -> String {
    match raw {
        "AuthenticAMD" => "AMD".to_string(),
        "GenuineIntel" => "Intel".to_string(),
        "0x41" => "ARM".to_string(),
        "0x48" => "HiSilicon".to_string(),
        "0x4e" => "NVIDIA".to_string(),
        "0x51" => "Qualcomm".to_string(),
        "0x53" => "Samsung".to_string(),
        "0x61" => "Apple".to_string(),
        "0xc0" => "Ampere".to_string(),
        other => other.to_string(),
    }
}

fn read_sysfs_physical_cores(logical_count: usize) -> Option<usize> {
    let cpu_root = Path::new("/sys/devices/system/cpu");
    let mut pairs: HashSet<(String, String)> = HashSet::new();
    for id in 0..logical_count.max(1) {
        let topo = cpu_root.join(format!("cpu{id}")).join("topology");
        let Ok(core_id) = fs::read_to_string(topo.join("core_id")) else {
            continue;
        };
        let pkg_id = fs::read_to_string(topo.join("physical_package_id"))
            .unwrap_or_else(|_| "0".to_string());
        pairs.insert((pkg_id.trim().to_string(), core_id.trim().to_string()));
    }
    if pairs.is_empty() {
        None
    } else {
        Some(pairs.len())
    }
}

fn read_static_cpu_info() -> StaticCpuInfo {
    let mut info = match fs::read_to_string("/proc/cpuinfo") {
        Ok(content) => parse_proc_cpuinfo(&content),
        Err(err) => StaticCpuInfo {
            model_name: MetricValue::Unavailable(err.to_string()),
            vendor: MetricValue::Unavailable(err.to_string()),
            architecture: std::env::consts::ARCH.to_string(),
            physical_cores: MetricValue::Unavailable(err.to_string()),
            logical_cpus: MetricValue::Unavailable(err.to_string()),
        },
    };

    if !info.logical_cpus.is_available() {
        if let Ok(n) = std::thread::available_parallelism() {
            info.logical_cpus = MetricValue::Available(n.get());
        }
    }

    if !info.physical_cores.is_available() {
        if let Some(logical) = info.logical_cpus.copied() {
            if let Some(cores) = read_sysfs_physical_cores(logical) {
                info.physical_cores = MetricValue::Available(cores);
            }
        }
    }

    #[cfg(target_os = "android")]
    {
        let used_soc_model = if !info.model_name.is_available() {
            if let Some(soc) = android_soc_model() {
                info.model_name = MetricValue::Available(soc);
                true
            } else {
                false
            }
        } else {
            false
        };
        if !info.vendor.is_available() || used_soc_model {
            if let Some(v) = android_soc_vendor() {
                info.vendor = MetricValue::Available(v);
            }
        }
    }

    info
}

fn read_sysfs_cpu_frequencies(expected_cpus: usize) -> BTreeMap<usize, MetricValue<f64>> {
    let mut result = BTreeMap::new();
    let cpu_root = Path::new("/sys/devices/system/cpu");
    let limit = expected_cpus.max(1);

    for id in 0..limit {
        let cpufreq_dir = cpu_root.join(format!("cpu{id}")).join("cpufreq");
        let mut perm_denied = false;
        let mut found = None;

        for file in ["scaling_cur_freq", "cpuinfo_cur_freq"] {
            match fs::read_to_string(cpufreq_dir.join(file)) {
                Ok(raw) => {
                    if let Ok(khz) = raw.trim().parse::<u64>() {
                        found = Some(MetricValue::Available(khz as f64 / 1000.0));
                        break;
                    }
                }
                Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                    perm_denied = true;
                }
                Err(_) => {}
            }
        }

        if let Some(val) = found {
            result.insert(id, val);
        } else if perm_denied {
            result.insert(
                id,
                MetricValue::PermissionError("cpufreq restricted".to_string()),
            );
        }
    }
    result
}

fn read_cpuinfo_average_mhz() -> MetricValue<f64> {
    let Ok(content) = fs::read_to_string("/proc/cpuinfo") else {
        return MetricValue::Unavailable("Unable to read /proc/cpuinfo".to_string());
    };
    let mut sum = 0.0_f64;
    let mut count = 0usize;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim() == "cpu MHz" {
                if let Ok(mhz) = v.trim().parse::<f64>() {
                    sum += mhz;
                    count += 1;
                }
            }
        }
    }
    if count > 0 {
        MetricValue::Available(sum / count as f64)
    } else {
        MetricValue::Unsupported("CPU frequency not exposed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parses_proc_stat_and_computes_delta_utilization() {
        let s1 = "\
cpu  1000 0 200 8800 0 0 0 0 0 0
cpu0 500 0 100 4400 0 0 0 0 0 0
cpu1 500 0 100 4400 0 0 0 0 0 0
intr 12345
";
        let s2 = "\
cpu  1250 0 250 9300 0 0 0 0 0 0
cpu0 700 0 150 4550 0 0 0 0 0 0
cpu1 550 0 100 4750 0 0 0 0 0 0
";
        let p1 = parse_proc_stat(s1).expect("parse s1");
        let p2 = parse_proc_stat(s2).expect("parse s2");

        // Overall: delta_busy = (250 + 50) = 300, delta_idle = 500, total = 800 -> 37.5%
        let overall = compute_utilization(p1.overall, p2.overall);
        assert_eq!(overall, MetricValue::Available(37.5));

        // cpu0: busy = 250, idle = 150, total = 400 -> 62.5%
        let c0 = compute_utilization(p1.per_cpu[&0], p2.per_cpu[&0]);
        assert_eq!(c0, MetricValue::Available(62.5));

        // Zero delta returns Unavailable instead of 0.0
        let zero = compute_utilization(p2.overall, p2.overall);
        assert!(matches!(zero, MetricValue::Unavailable(_)));
    }

    #[test]
    fn computes_process_utilization_normalized_by_logical_cpus() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_secs(2);
        let prev = ProcessCpuSample {
            cpu_seconds: 1.0,
            wall_at: t0,
        };
        let curr = ProcessCpuSample {
            cpu_seconds: 3.0,
            wall_at: t1,
        };
        // 2.0s of CPU time across 2s wall * 4 CPUs = 25.0%
        let pct = compute_process_utilization(prev, curr, 4);
        assert_eq!(pct, MetricValue::Available(25.0));

        // Zero elapsed wall time must return Unavailable, not 0.0
        let zero_wall = compute_process_utilization(prev, prev, 4);
        assert!(matches!(zero_wall, MetricValue::Unavailable(_)));
    }

    #[test]
    fn parses_proc_cpuinfo_topology_and_vendor() {
        let info = "\
processor\t: 0
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 7 7800X3D 8-Core Processor
physical id\t: 0
core id\t\t: 0
cpu cores\t: 8

processor\t: 1
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 7 7800X3D 8-Core Processor
physical id\t: 0
core id\t\t: 0
cpu cores\t: 8

processor\t: 2
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 7 7800X3D 8-Core Processor
physical id\t: 0
core id\t\t: 1
cpu cores\t: 8
";
        let parsed = parse_proc_cpuinfo(info);
        assert_eq!(
            parsed.model_name,
            MetricValue::Available("AMD Ryzen 7 7800X3D 8-Core Processor".to_string())
        );
        assert_eq!(parsed.vendor, MetricValue::Available("AMD".to_string()));
        assert_eq!(parsed.physical_cores, MetricValue::Available(2));
        assert_eq!(parsed.logical_cpus, MetricValue::Available(3));
    }
}
