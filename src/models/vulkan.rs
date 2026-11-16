//! Immutable, discovery-time Vulkan data. Heap sizes are not dedicated VRAM.
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct VulkanReport {
    pub device: Option<VulkanDevice>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct VulkanDevice {
    pub name: String,
    pub api_version: String,
    pub vendor_id: u32,
    pub device_id: u32,
    /// Vendor-defined encoding; deliberately not decoded as a Vulkan API version.
    pub driver_version_raw: u32,
    pub driver_name: Option<String>,
    pub driver_info: Option<String>,
    pub driver_id: Option<i32>,
    pub extensions: Option<Vec<String>>,
    pub limits: Vec<(String, String)>,
    /// Core 1.0 and selected core 1.1–1.3 features, not every extension feature.
    pub features: Vec<(String, bool)>,
    pub heaps: Option<Vec<VulkanHeap>>,
    pub memory_types: Option<Vec<VulkanMemoryType>>,
    pub sampled_at_unix_seconds: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct VulkanHeap {
    pub size_bytes: u64,
    pub flags: u32,
    /// EXT_memory_budget estimates at discovery; never live global/VRAM metrics.
    pub budget_bytes_at_discovery: Option<u64>,
    pub process_usage_bytes_at_discovery: Option<u64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct VulkanMemoryType {
    pub heap_index: u32,
    pub property_flags: u32,
}
