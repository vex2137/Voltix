#[cfg(not(target_os = "android"))]
use std::fs;

use crate::models::metrics::MetricValue;
use crate::models::telemetry::SystemInfo;
#[cfg(target_os = "android")]
use crate::platform::android::{
    android_device_name, android_kernel_version, android_os_name, android_uptime_seconds,
};

pub struct SystemMonitor {
    os_name: MetricValue<String>,
    kernel_version: MetricValue<String>,
    architecture: String,
    hostname: MetricValue<String>,
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self {
            os_name: read_os_name(),
            kernel_version: read_kernel_version(),
            architecture: std::env::consts::ARCH.to_string(),
            hostname: read_hostname(),
        }
    }

    pub fn collect(&self) -> SystemInfo {
        SystemInfo {
            os_name: self.os_name.clone(),
            kernel_version: self.kernel_version.clone(),
            architecture: self.architecture.clone(),
            hostname: self.hostname.clone(),
            uptime_seconds: read_uptime_seconds(),
        }
    }
}

pub fn parse_os_release(content: &str) -> Option<String> {
    let mut pretty = None;
    let mut name = None;
    let mut version = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        let val = v.trim().trim_matches('"').to_string();
        match k.trim() {
            "PRETTY_NAME" if !val.is_empty() => pretty = Some(val),
            "NAME" if !val.is_empty() => name = Some(val),
            "VERSION_ID" if !val.is_empty() => version = Some(val),
            _ => {}
        }
    }

    pretty.or_else(|| match (name, version) {
        (Some(n), Some(v)) => Some(format!("{n} {v}")),
        (Some(n), None) => Some(n),
        _ => None,
    })
}

fn read_os_name() -> MetricValue<String> {
    #[cfg(target_os = "android")]
    {
        android_os_name()
    }
    #[cfg(not(target_os = "android"))]
    {
        match fs::read_to_string("/etc/os-release") {
            Ok(content) => parse_os_release(&content)
                .map(MetricValue::Available)
                .unwrap_or_else(|| {
                    MetricValue::Unavailable("OS name not found in os-release".into())
                }),
            Err(err) => MetricValue::Unavailable(err.to_string()),
        }
    }
}

fn read_kernel_version() -> MetricValue<String> {
    #[cfg(target_os = "android")]
    {
        android_kernel_version()
    }
    #[cfg(not(target_os = "android"))]
    {
        match fs::read_to_string("/proc/sys/kernel/osrelease") {
            Ok(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    MetricValue::Unavailable("Empty kernel release".into())
                } else {
                    MetricValue::Available(trimmed.to_string())
                }
            }
            Err(err) => MetricValue::Unavailable(err.to_string()),
        }
    }
}

fn read_hostname() -> MetricValue<String> {
    #[cfg(target_os = "android")]
    {
        android_device_name()
    }
    #[cfg(not(target_os = "android"))]
    {
        match fs::read_to_string("/proc/sys/kernel/hostname") {
            Ok(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    MetricValue::Unavailable("Empty hostname".into())
                } else {
                    MetricValue::Available(trimmed.to_string())
                }
            }
            Err(err) => MetricValue::Unavailable(err.to_string()),
        }
    }
}

pub fn parse_proc_uptime(content: &str) -> Option<u64> {
    content
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| v as u64)
}

fn read_uptime_seconds() -> MetricValue<u64> {
    #[cfg(target_os = "android")]
    {
        android_uptime_seconds()
    }
    #[cfg(not(target_os = "android"))]
    {
        match fs::read_to_string("/proc/uptime") {
            Ok(content) => parse_proc_uptime(&content)
                .map(MetricValue::Available)
                .unwrap_or_else(|| MetricValue::Unavailable("Malformed /proc/uptime".into())),
            Err(err) => MetricValue::Unavailable(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cachyos_os_release() {
        let sample = "NAME=\"CachyOS Linux\"\nPRETTY_NAME=\"CachyOS\"\nID=cachyos\n";
        assert_eq!(parse_os_release(sample).as_deref(), Some("CachyOS"));
    }

    #[test]
    fn parses_proc_uptime_seconds() {
        assert_eq!(parse_proc_uptime("12345.67 98765.43\n"), Some(12345));
        assert_eq!(parse_proc_uptime("bad"), None);
    }
}
