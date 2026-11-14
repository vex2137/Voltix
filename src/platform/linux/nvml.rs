use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Nvml;

use crate::models::metrics::MetricValue;
use crate::models::telemetry::{GpuTelemetry, GpuVendor};

#[derive(Default)]
pub struct NvmlProvider {
    nvml: Option<Nvml>,
    init_attempted: bool,
}

impl NvmlProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_initialized(&mut self) -> Option<&Nvml> {
        if !self.init_attempted {
            self.init_attempted = true;
            match Nvml::init() {
                Ok(instance) => {
                    log::info!("NVML initialized successfully");
                    self.nvml = Some(instance);
                }
                Err(err) => {
                    log::info!("NVML unavailable (non-NVIDIA system or driver absent): {err}");
                    self.nvml = None;
                }
            }
        }
        self.nvml.as_ref()
    }

    pub fn collect(&mut self) -> Vec<GpuTelemetry> {
        let Some(nvml) = self.ensure_initialized() else {
            return Vec::new();
        };

        let driver_version = match nvml.sys_driver_version() {
            Ok(v) => MetricValue::Available(v),
            Err(err) => map_nvml_error(&err),
        };

        let count = match nvml.device_count() {
            Ok(c) => c,
            Err(err) => {
                log::warn!("NVML device_count failed: {err}");
                return Vec::new();
            }
        };

        let mut gpus = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = match nvml.device_by_index(index) {
                Ok(d) => d,
                Err(err) => {
                    log::warn!("NVML device_by_index({index}) failed: {err}");
                    continue;
                }
            };

            let name = device
                .name()
                .unwrap_or_else(|_| format!("NVIDIA GPU #{index}"));

            let pci_slot = device
                .pci_info()
                .ok()
                .map(|p| normalize_nvml_pci_bus_id(&p.bus_id));

            let utilization_percent = match device.utilization_rates() {
                Ok(u) => MetricValue::Available(u.gpu as f64),
                Err(err) => map_nvml_error(&err),
            };

            let (vram_total_bytes, vram_used_bytes, vram_free_bytes, vram_utilization_percent) =
                match device.memory_info() {
                    Ok(mem) => {
                        let pct = if mem.total > 0 {
                            MetricValue::Available((mem.used as f64 / mem.total as f64) * 100.0)
                        } else {
                            MetricValue::Unavailable("Reported total VRAM is zero".to_string())
                        };
                        (
                            MetricValue::Available(mem.total),
                            MetricValue::Available(mem.used),
                            MetricValue::Available(mem.free),
                            pct,
                        )
                    }
                    Err(err) => (
                        map_nvml_error(&err),
                        map_nvml_error(&err),
                        map_nvml_error(&err),
                        map_nvml_error(&err),
                    ),
                };

            let temperature_celsius = match device.temperature(TemperatureSensor::Gpu) {
                Ok(t) => MetricValue::Available(t as f64),
                Err(err) => map_nvml_error(&err),
            };

            let core_clock_mhz = match device.clock_info(Clock::Graphics) {
                Ok(mhz) => MetricValue::Available(mhz),
                Err(err) => map_nvml_error(&err),
            };

            let memory_clock_mhz = match device.clock_info(Clock::Memory) {
                Ok(mhz) => MetricValue::Available(mhz),
                Err(err) => map_nvml_error(&err),
            };

            let power_draw_watts = match device.power_usage() {
                Ok(mw) => MetricValue::Available(mw as f64 / 1000.0),
                Err(err) => map_nvml_error(&err),
            };

            let power_limit_watts = match device.enforced_power_limit() {
                Ok(mw) => MetricValue::Available(mw as f64 / 1000.0),
                Err(NvmlError::NotSupported) => match device.power_management_limit() {
                    Ok(mw) => MetricValue::Available(mw as f64 / 1000.0),
                    Err(err) => map_nvml_error(&err),
                },
                Err(err) => map_nvml_error(&err),
            };

            let fan_speed_percent = match device.fan_speed(0) {
                Ok(pct) => MetricValue::Available(pct),
                Err(err) => map_nvml_error(&err),
            };

            gpus.push(GpuTelemetry {
                id: format!("nvml:{index}"),
                name,
                vendor: GpuVendor::Nvidia,
                vendor_name: "NVIDIA".to_string(),
                pci_slot,
                driver_version: driver_version.clone(),
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
            });
        }

        gpus
    }
}

fn map_nvml_error<T>(err: &NvmlError) -> MetricValue<T> {
    match err {
        NvmlError::NotSupported => MetricValue::Unsupported("Not supported by GPU/driver".into()),
        NvmlError::NoPermission => MetricValue::PermissionError("NVML permission denied".into()),
        _ => MetricValue::Unavailable(err.to_string()),
    }
}

/// NVML often pads PCI domain to 8 hex digits (e.g., "00000000:01:00.0"),
/// whereas Linux sysfs uses 4 hex digits ("0000:01:00.0").
fn normalize_nvml_pci_bus_id(bus_id: &str) -> String {
    let trimmed = bus_id.trim().to_ascii_lowercase();
    if let Some((domain, rest)) = trimmed.split_once(':') {
        if domain.len() > 4 {
            let short_domain = &domain[domain.len() - 4..];
            return format!("{short_domain}:{rest}");
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::normalize_nvml_pci_bus_id;

    #[test]
    fn normalizes_nvml_eight_digit_pci_domain() {
        assert_eq!(normalize_nvml_pci_bus_id("00000000:01:00.0"), "0000:01:00.0");
        assert_eq!(normalize_nvml_pci_bus_id("0000:2D:00.0"), "0000:2d:00.0");
    }
}
