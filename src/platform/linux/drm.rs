use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::models::metrics::MetricValue;
use crate::models::telemetry::{GpuTelemetry, GpuVendor};
use crate::platform::linux::hwmon::{
    read_hwmon_microwatts_to_watts, read_hwmon_millidegrees_celsius,
};

#[derive(Debug, Clone)]
struct DiscoveredDrmCard {
    card_name: String,
    device_dir: PathBuf,
    hwmon_dir: Option<PathBuf>,
    vendor: GpuVendor,
    vendor_name: String,
    display_name: String,
    driver_name: Option<String>,
    pci_slot: Option<String>,
}

#[derive(Default)]
pub struct LinuxDrmProvider {
    cards: Vec<DiscoveredDrmCard>,
    discovered: bool,
}

impl LinuxDrmProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_discovered(&mut self) {
        if self.discovered {
            return;
        }
        self.discovered = true;
        self.cards = discover_drm_cards(Path::new("/sys/class/drm"));
    }

    pub fn collect(&mut self, exclude_pci_slots: &HashSet<String>) -> Vec<GpuTelemetry> {
        self.ensure_discovered();

        self.cards
            .iter()
            .filter(|card| {
                if let Some(slot) = &card.pci_slot {
                    !exclude_pci_slots.contains(slot)
                } else {
                    true
                }
            })
            .map(read_card_telemetry)
            .collect()
    }
}

fn discover_drm_cards(drm_root: &Path) -> Vec<DiscoveredDrmCard> {
    let Ok(entries) = fs::read_dir(drm_root) else {
        return Vec::new();
    };

    let mut card_dirs: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            // Match card0, card1, ... and skip connector entries like card0-DP-1
            if let Some(suffix) = name.strip_prefix("card") {
                if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                    return Some((name, e.path()));
                }
            }
            None
        })
        .collect();

    card_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut cards = Vec::new();
    for (card_name, card_path) in card_dirs {
        let device_dir = card_path.join("device");
        if !device_dir.exists() {
            continue;
        }

        let vendor_id = read_trimmed(&device_dir.join("vendor")).unwrap_or_default();
        let device_id = read_trimmed(&device_dir.join("device")).unwrap_or_default();
        let uevent = fs::read_to_string(device_dir.join("uevent")).unwrap_or_default();
        let (driver_name, pci_slot) = parse_uevent(&uevent);

        let (vendor, vendor_name) = match vendor_id.to_ascii_lowercase().as_str() {
            "0x1002" => (GpuVendor::Amd, "AMD".to_string()),
            "0x8086" => (GpuVendor::Intel, "Intel".to_string()),
            "0x10de" => (GpuVendor::Nvidia, "NVIDIA".to_string()),
            _ => (GpuVendor::Other, "Unknown".to_string()),
        };

        // Resolve PCI device marketing name from system pci.ids once at discovery if available.
        let pci_name = lookup_pci_device_name(&vendor_id, &device_id);
        let display_name = match pci_name {
            Some(name) => format!("{vendor_name} {name}"),
            None if !vendor_id.is_empty() && !device_id.is_empty() => {
                format!(
                    "{vendor_name} GPU [{}:{}]",
                    vendor_id.trim_start_matches("0x"),
                    device_id.trim_start_matches("0x")
                )
            }
            None => format!("{vendor_name} GPU ({card_name})"),
        };

        let hwmon_dir = find_first_hwmon(&device_dir.join("hwmon"));

        cards.push(DiscoveredDrmCard {
            card_name,
            device_dir,
            hwmon_dir,
            vendor,
            vendor_name,
            display_name,
            driver_name,
            pci_slot,
        });
    }

    cards
}

fn read_card_telemetry(card: &DiscoveredDrmCard) -> GpuTelemetry {
    let driver_version = match &card.driver_name {
        Some(d) => MetricValue::Available(d.clone()),
        None => MetricValue::Unavailable("Kernel driver unknown".to_string()),
    };

    match card.vendor {
        GpuVendor::Amd => read_amd_telemetry(card, driver_version),
        GpuVendor::Intel => read_intel_telemetry(card, driver_version),
        _ => GpuTelemetry {
            id: format!("drm:{}", card.card_name),
            name: card.display_name.clone(),
            vendor: card.vendor,
            vendor_name: card.vendor_name.clone(),
            pci_slot: card.pci_slot.clone(),
            driver_version,
            utilization_percent: MetricValue::Unsupported(
                "Sysfs telemetry unavailable for this adapter".into(),
            ),
            vram_total_bytes: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            vram_used_bytes: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            vram_free_bytes: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            vram_utilization_percent: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            temperature_celsius: read_hwmon_temp(card.hwmon_dir.as_deref()),
            core_clock_mhz: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            memory_clock_mhz: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            power_draw_watts: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            power_limit_watts: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
            fan_speed_percent: MetricValue::Unsupported("Not exposed via DRM sysfs".into()),
        },
    }
}

fn read_amd_telemetry(
    card: &DiscoveredDrmCard,
    driver_version: MetricValue<String>,
) -> GpuTelemetry {
    let utilization_percent = read_u64_sysfs(&card.device_dir.join("gpu_busy_percent"))
        .map(|v| (*v as f64).clamp(0.0, 100.0));

    let vram_total_bytes = read_u64_sysfs(&card.device_dir.join("mem_info_vram_total"));
    let vram_used_bytes = read_u64_sysfs(&card.device_dir.join("mem_info_vram_used"));

    let (vram_free_bytes, vram_utilization_percent) = match (&vram_total_bytes, &vram_used_bytes) {
        (MetricValue::Available(total), MetricValue::Available(used)) if *total > 0 => (
            MetricValue::Available(total.saturating_sub(*used)),
            MetricValue::Available((*used as f64 / *total as f64) * 100.0),
        ),
        (MetricValue::Available(0), _) => (
            MetricValue::Unavailable("Reported total VRAM is zero".into()),
            MetricValue::Unavailable("Reported total VRAM is zero".into()),
        ),
        _ => (
            MetricValue::Unsupported("VRAM counters unavailable".into()),
            MetricValue::Unsupported("VRAM counters unavailable".into()),
        ),
    };

    let temperature_celsius = read_hwmon_temp(card.hwmon_dir.as_deref());

    let (power_draw_watts, power_limit_watts) = match &card.hwmon_dir {
        Some(hw) => {
            let draw = if hw.join("power1_average").exists() {
                read_hwmon_microwatts_to_watts(&hw.join("power1_average"))
            } else if hw.join("power1_input").exists() {
                read_hwmon_microwatts_to_watts(&hw.join("power1_input"))
            } else {
                MetricValue::Unsupported("Power telemetry not exposed".into())
            };
            let limit = if hw.join("power1_cap").exists() {
                read_hwmon_microwatts_to_watts(&hw.join("power1_cap"))
            } else {
                MetricValue::Unsupported("Power limit not exposed".into())
            };
            (draw, limit)
        }
        None => (
            MetricValue::Unsupported("No GPU hwmon interface".into()),
            MetricValue::Unsupported("No GPU hwmon interface".into()),
        ),
    };

    let core_clock_mhz = read_amd_active_dpm_clock(&card.device_dir.join("pp_dpm_sclk"))
        .or_else_freq(card.hwmon_dir.as_deref(), "freq1_input");
    let memory_clock_mhz = read_amd_active_dpm_clock(&card.device_dir.join("pp_dpm_mclk"))
        .or_else_freq(card.hwmon_dir.as_deref(), "freq2_input");

    let fan_speed_percent = read_amd_fan_percent(card.hwmon_dir.as_deref());

    GpuTelemetry {
        id: format!("drm:{}", card.card_name),
        name: card.display_name.clone(),
        vendor: GpuVendor::Amd,
        vendor_name: "AMD".to_string(),
        pci_slot: card.pci_slot.clone(),
        driver_version,
        utilization_percent,
        vram_total_bytes,
        vram_used_bytes,
        vram_free_bytes,
        vram_utilization_percent,
        temperature_celsius,
        core_clock_mhz,
        memory_clock_mhz,
        power_draw_watts,
        power_limit_watts,
        fan_speed_percent,
    }
}

fn read_intel_telemetry(
    card: &DiscoveredDrmCard,
    driver_version: MetricValue<String>,
) -> GpuTelemetry {
    // Intel i915 exposes gt_cur_freq_mhz on cardN/, while xe exposes tile0/gt0/freq0/cur_freq
    let card_dir = card.device_dir.parent().unwrap_or(&card.device_dir);
    let core_clock_mhz = if card_dir.join("gt_cur_freq_mhz").exists() {
        read_u64_sysfs(&card_dir.join("gt_cur_freq_mhz")).map(|v| *v as u32)
    } else if card.device_dir.join("tile0/gt0/freq0/cur_freq").exists() {
        read_u64_sysfs(&card.device_dir.join("tile0/gt0/freq0/cur_freq")).map(|v| *v as u32)
    } else {
        MetricValue::Unsupported("Intel GPU clock sysfs not exposed".into())
    };

    let temperature_celsius = read_hwmon_temp(card.hwmon_dir.as_deref());

    GpuTelemetry {
        id: format!("drm:{}", card.card_name),
        name: card.display_name.clone(),
        vendor: GpuVendor::Intel,
        vendor_name: "Intel".to_string(),
        pci_slot: card.pci_slot.clone(),
        driver_version,
        utilization_percent: MetricValue::Unsupported(
            "Unprivileged GPU busy % requires PMU/xe telemetry".into(),
        ),
        vram_total_bytes: MetricValue::Unsupported("Shared system memory / not exposed".into()),
        vram_used_bytes: MetricValue::Unsupported("Shared system memory / not exposed".into()),
        vram_free_bytes: MetricValue::Unsupported("Shared system memory / not exposed".into()),
        vram_utilization_percent: MetricValue::Unsupported(
            "Shared system memory / not exposed".into(),
        ),
        temperature_celsius,
        core_clock_mhz,
        memory_clock_mhz: MetricValue::Unsupported("Not exposed by Intel DRM sysfs".into()),
        power_draw_watts: MetricValue::Unsupported("Not exposed via unprivileged sysfs".into()),
        power_limit_watts: MetricValue::Unsupported("Not exposed via unprivileged sysfs".into()),
        fan_speed_percent: MetricValue::Unsupported("Not applicable / not exposed".into()),
    }
}

trait ClockFallbackExt {
    fn or_else_freq(self, hwmon_dir: Option<&Path>, file: &str) -> MetricValue<u32>;
}

impl ClockFallbackExt for MetricValue<u32> {
    fn or_else_freq(self, hwmon_dir: Option<&Path>, file: &str) -> MetricValue<u32> {
        if self.is_available() {
            return self;
        }
        if let Some(hw) = hwmon_dir {
            let p = hw.join(file);
            if p.exists() {
                return read_u64_sysfs(&p).map(|hz| (*hz / 1_000_000) as u32);
            }
        }
        self
    }
}

fn read_amd_active_dpm_clock(path: &Path) -> MetricValue<u32> {
    match fs::read_to_string(path) {
        Ok(content) => parse_active_dpm_mhz(&content)
            .map(MetricValue::Available)
            .unwrap_or_else(|| MetricValue::Unavailable("No active DPM clock state".into())),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            MetricValue::PermissionError(format!("Permission denied: {}", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            MetricValue::Unsupported("DPM clock table not exposed".into())
        }
        Err(err) => MetricValue::Unavailable(err.to_string()),
    }
}

pub fn parse_active_dpm_mhz(content: &str) -> Option<u32> {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.ends_with('*') {
            continue;
        }
        for token in trimmed.split_whitespace() {
            if let Some(mhz_str) = token
                .strip_suffix("Mhz")
                .or_else(|| token.strip_suffix("MHz"))
            {
                if let Ok(mhz) = mhz_str.parse::<u32>() {
                    return Some(mhz);
                }
            }
        }
    }
    None
}

fn read_amd_fan_percent(hwmon_dir: Option<&Path>) -> MetricValue<u32> {
    let Some(hw) = hwmon_dir else {
        return MetricValue::Unsupported("No GPU hwmon interface".into());
    };
    let pwm_path = hw.join("pwm1");
    if pwm_path.exists() {
        return read_u64_sysfs(&pwm_path)
            .map(|pwm| (((*pwm as f64) / 255.0) * 100.0).round().clamp(0.0, 100.0) as u32);
    }
    MetricValue::Unsupported("Fan sensor not exposed".into())
}

fn read_hwmon_temp(hwmon_dir: Option<&Path>) -> MetricValue<f64> {
    let Some(hw) = hwmon_dir else {
        return MetricValue::Unsupported("No GPU hwmon interface".into());
    };
    let temp_path = hw.join("temp1_input");
    if temp_path.exists() {
        read_hwmon_millidegrees_celsius(&temp_path)
    } else {
        MetricValue::Unsupported("Temperature sensor not exposed".into())
    }
}

fn find_first_hwmon(hwmon_parent: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(hwmon_parent).ok()?;
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter().next()
}

fn read_u64_sysfs(path: &Path) -> MetricValue<u64> {
    match fs::read_to_string(path) {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(v) => MetricValue::Available(v),
            Err(err) => MetricValue::Unavailable(format!("Malformed sysfs integer: {err}")),
        },
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            MetricValue::PermissionError(format!("Permission denied: {}", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            MetricValue::Unsupported(format!("Not exposed: {}", path.display()))
        }
        Err(err) => MetricValue::Unavailable(err.to_string()),
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn parse_uevent(content: &str) -> (Option<String>, Option<String>) {
    let mut driver = None;
    let mut slot = None;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("DRIVER=") {
            driver = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("PCI_SLOT_NAME=") {
            slot = Some(val.trim().to_ascii_lowercase());
        }
    }
    (driver, slot)
}

fn lookup_pci_device_name(vendor_hex: &str, device_hex: &str) -> Option<String> {
    let v_target = vendor_hex
        .trim()
        .trim_start_matches("0x")
        .to_ascii_lowercase();
    let d_target = device_hex
        .trim()
        .trim_start_matches("0x")
        .to_ascii_lowercase();
    if v_target.is_empty() || d_target.is_empty() {
        return None;
    }

    let pci_ids_paths = [
        "/usr/share/hwdata/pci.ids",
        "/usr/share/misc/pci.ids",
        "/usr/share/pci.ids",
    ];

    let content = pci_ids_paths
        .iter()
        .find_map(|p| fs::read_to_string(p).ok())?;

    let mut in_vendor = false;
    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            in_vendor = line.starts_with(&v_target)
                && line
                    .as_bytes()
                    .get(v_target.len())
                    .is_some_and(|b| *b == b' ');
        } else if in_vendor && !line.starts_with("\t\t") {
            let rest = &line[1..];
            if let Some(after) = rest.strip_prefix(&d_target) {
                if let Some(name) = after.strip_prefix("  ") {
                    return Some(name.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_amd_active_dpm_clock() {
        let sample = "\
0: 300Mhz
1: 1450Mhz *
2: 2400Mhz
";
        assert_eq!(parse_active_dpm_mhz(sample), Some(1450));
    }

    #[test]
    fn parses_uevent_driver_and_pci_slot() {
        let sample = "DRIVER=amdgpu\nPCI_CLASS=30000\nPCI_SLOT_NAME=0000:03:00.0\n";
        let (driver, slot) = parse_uevent(sample);
        assert_eq!(driver.as_deref(), Some("amdgpu"));
        assert_eq!(slot.as_deref(), Some("0000:03:00.0"));
    }
}
