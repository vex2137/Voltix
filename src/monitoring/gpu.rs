#[cfg(target_os = "linux")]
use std::collections::HashSet;

use crate::models::telemetry::GpuTelemetry;
#[cfg(target_os = "linux")]
use crate::platform::linux::{drm::LinuxDrmProvider, nvml::NvmlProvider};

pub struct GpuMonitor {
    #[cfg(target_os = "linux")]
    nvml: NvmlProvider,
    #[cfg(target_os = "linux")]
    drm: LinuxDrmProvider,
}

impl Default for GpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuMonitor {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            nvml: NvmlProvider::new(),
            #[cfg(target_os = "linux")]
            drm: LinuxDrmProvider::new(),
        }
    }

    pub fn collect(&mut self) -> Vec<GpuTelemetry> {
        #[cfg(target_os = "linux")]
        {
            let mut gpus = self.nvml.collect();
            let nvml_slots: HashSet<String> =
                gpus.iter().filter_map(|g| g.pci_slot.clone()).collect();
            let drm_gpus = self.drm.collect(&nvml_slots);
            gpus.extend(drm_gpus);
            gpus
        }
        #[cfg(target_os = "android")]
        {
            crate::platform::android::collect_android_gpus()
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Vec::new()
        }
    }
}
