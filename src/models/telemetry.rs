use std::time::Instant;

use crate::models::metrics::MetricValue;

#[derive(Debug, Clone)]
pub struct LogicalCpuTelemetry {
    pub id: usize,
    pub utilization_percent: MetricValue<f64>,
    pub frequency_mhz: MetricValue<f64>,
}

#[derive(Debug, Clone)]
pub struct CpuTelemetry {
    pub model_name: MetricValue<String>,
    pub vendor: MetricValue<String>,
    pub architecture: String,
    pub physical_cores: MetricValue<usize>,
    pub logical_cpus: MetricValue<usize>,
    pub overall_utilization: MetricValue<f64>,
    pub average_frequency_mhz: MetricValue<f64>,
    pub temperature_celsius: MetricValue<f64>,
    pub temperature_source: Option<String>,
    pub per_core: Vec<LogicalCpuTelemetry>,
}

#[derive(Debug, Clone)]
pub struct MemoryTelemetry {
    pub total_bytes: MetricValue<u64>,
    pub available_bytes: MetricValue<u64>,
    pub used_bytes: MetricValue<u64>,
    pub utilization_percent: MetricValue<f64>,
    pub swap_total_bytes: MetricValue<u64>,
    pub swap_used_bytes: MetricValue<u64>,
    pub swap_utilization_percent: MetricValue<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

#[derive(Debug, Clone)]
pub struct GpuTelemetry {
    pub id: String,
    pub name: String,
    pub vendor: GpuVendor,
    pub vendor_name: String,
    pub pci_slot: Option<String>,
    pub driver_version: MetricValue<String>,
    pub utilization_percent: MetricValue<f64>,
    pub vram_total_bytes: MetricValue<u64>,
    pub vram_used_bytes: MetricValue<u64>,
    pub vram_free_bytes: MetricValue<u64>,
    pub vram_utilization_percent: MetricValue<f64>,
    pub temperature_celsius: MetricValue<f64>,
    pub core_clock_mhz: MetricValue<u32>,
    pub memory_clock_mhz: MetricValue<u32>,
    pub power_draw_watts: MetricValue<f64>,
    pub power_limit_watts: MetricValue<f64>,
    pub fan_speed_percent: MetricValue<u32>,
}

#[derive(Debug, Clone)]
pub struct StorageDeviceTelemetry {
    pub device: String,
    pub mount_point: String,
    pub filesystem: String,
    pub total_bytes: MetricValue<u64>,
    pub used_bytes: MetricValue<u64>,
    pub available_bytes: MetricValue<u64>,
    pub utilization_percent: MetricValue<f64>,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: MetricValue<String>,
    pub kernel_version: MetricValue<String>,
    pub architecture: String,
    pub hostname: MetricValue<String>,
    pub uptime_seconds: MetricValue<u64>,
}

#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    pub sampled_at: Instant,
    pub system: SystemInfo,
    pub cpu: CpuTelemetry,
    pub memory: MemoryTelemetry,
    pub gpus: Vec<GpuTelemetry>,
    pub storage: Vec<StorageDeviceTelemetry>,
}
