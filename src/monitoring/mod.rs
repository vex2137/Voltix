pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod memory;
pub mod system;

use std::time::Instant;

use crate::models::telemetry::TelemetrySnapshot;
use crate::monitoring::cpu::CpuMonitor;
use crate::monitoring::disk::DiskMonitor;
use crate::monitoring::gpu::GpuMonitor;
use crate::monitoring::memory::MemoryMonitor;
use crate::monitoring::system::SystemMonitor;

pub struct MonitoringEngine {
    system: SystemMonitor,
    cpu: CpuMonitor,
    memory: MemoryMonitor,
    gpu: GpuMonitor,
    disk: DiskMonitor,
}

impl Default for MonitoringEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitoringEngine {
    pub fn new() -> Self {
        Self {
            system: SystemMonitor::new(),
            cpu: CpuMonitor::new(),
            memory: MemoryMonitor::new(),
            gpu: GpuMonitor::new(),
            disk: DiskMonitor::new(),
        }
    }

    pub fn collect_snapshot(&mut self) -> TelemetrySnapshot {
        let sampled_at = Instant::now();
        TelemetrySnapshot {
            sampled_at,
            system: self.system.collect(),
            cpu: self.cpu.collect(),
            memory: self.memory.collect(),
            gpus: self.gpu.collect(),
            storage: self.disk.collect(),
        }
    }
}
