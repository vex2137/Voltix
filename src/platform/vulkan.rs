//! One-shot Vulkan discovery, executed by the monitoring worker. No logical
//! device, queues, profiling workload, root, or surface is created.
use crate::models::vulkan::{VulkanDevice, VulkanHeap, VulkanMemoryType, VulkanReport};
use ash::{vk, Entry};
use std::{
    ffi::CStr,
    time::{SystemTime, UNIX_EPOCH},
};

struct InstanceGuard(ash::Instance);
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            self.0.destroy_instance(None);
        }
    }
}
fn string(raw: &[std::ffi::c_char]) -> String {
    let bytes = raw
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| c.to_ne_bytes()[0])
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().into()
}
fn version(raw: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(raw),
        vk::api_version_minor(raw),
        vk::api_version_patch(raw)
    )
}

fn create_compatible_instance<T>(
    requested: u32,
    mut create: impl FnMut(u32) -> Result<T, vk::Result>,
) -> Result<(u32, T), vk::Result> {
    match create(requested) {
        Ok(value) => Ok((requested, value)),
        Err(vk::Result::ERROR_INCOMPATIBLE_DRIVER) if requested != vk::API_VERSION_1_0 => {
            create(vk::API_VERSION_1_0).map(|value| (vk::API_VERSION_1_0, value))
        }
        Err(error) => Err(error),
    }
}

pub fn probe() -> VulkanReport {
    #[cfg(target_os = "android")]
    let loader = "libvulkan.so";
    #[cfg(not(target_os = "android"))]
    let loader = "libvulkan.so.1";
    let result = unsafe { discover(loader) };
    match result {
        Ok(device) => VulkanReport {
            device: Some(device),
            error: None,
        },
        Err(error) => VulkanReport {
            device: None,
            error: Some(error),
        },
    }
}

unsafe fn discover(loader: &str) -> Result<VulkanDevice, String> {
    let entry = Entry::load_from(loader);
    let entry = entry.map_err(|_| "vk_loader_unavailable".to_owned())?;
    let loader = entry
        .try_enumerate_instance_version()
        .map_err(|e| format!("vk_loader_error:{}", e.as_raw()))?
        .unwrap_or(vk::API_VERSION_1_0);
    let (api, raw_instance) = create_compatible_instance(loader.min(vk::API_VERSION_1_3), |api| {
        let app = vk::ApplicationInfo::default()
            .application_name(c"Voltix diagnostics")
            .api_version(api);
        entry.create_instance(
            &vk::InstanceCreateInfo::default().application_info(&app),
            None,
        )
    })
    .map_err(|e| format!("vk_instance_error:{}", e.as_raw()))?;
    let instance = InstanceGuard(raw_instance);
    let devices = instance
        .0
        .enumerate_physical_devices()
        .map_err(|e| format!("vk_device_error:{}", e.as_raw()))?;
    // Prefer a real GPU over a CPU/software implementation, retaining driver order.
    let mut choices = devices
        .into_iter()
        .take(8)
        .map(|d| (d, instance.0.get_physical_device_properties(d)))
        .collect::<Vec<_>>();
    choices.sort_by_key(|(_, p)| p.device_type == vk::PhysicalDeviceType::CPU);
    let (physical, props) = choices.into_iter().next().ok_or("vk_no_device")?;
    let device_name = string(&props.device_name);
    if device_name.is_empty() {
        return Err("vk_no_device".into());
    }
    let extensions = instance
        .0
        .enumerate_device_extension_properties(physical)
        .ok()
        .map(|items| {
            let mut names = items
                .iter()
                .map(|p| string(&p.extension_name))
                .collect::<Vec<_>>();
            names.sort();
            names.dedup();
            names
        });
    let has = |name: &CStr| {
        extensions
            .as_ref()
            .is_some_and(|list| list.iter().any(|v| v.as_bytes() == name.to_bytes()))
    };
    let mut driver = vk::PhysicalDeviceDriverProperties::default();
    let has_properties2 = api >= vk::API_VERSION_1_1
        && entry
            .get_instance_proc_addr(
                instance.0.handle(),
                c"vkGetPhysicalDeviceProperties2".as_ptr(),
            )
            .is_some();
    let has_features2 = api >= vk::API_VERSION_1_1
        && entry
            .get_instance_proc_addr(
                instance.0.handle(),
                c"vkGetPhysicalDeviceFeatures2".as_ptr(),
            )
            .is_some();
    let has_memory2 = api >= vk::API_VERSION_1_1
        && entry
            .get_instance_proc_addr(
                instance.0.handle(),
                c"vkGetPhysicalDeviceMemoryProperties2".as_ptr(),
            )
            .is_some();
    let driver_supported = has_properties2
        && (props.api_version >= vk::API_VERSION_1_2 || has(ash::khr::driver_properties::NAME));
    if driver_supported {
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut driver);
        instance
            .0
            .get_physical_device_properties2(physical, &mut properties);
    }
    let base = instance.0.get_physical_device_features(physical);
    let mut features = Vec::new();
    macro_rules! feature { ($group:expr; $($field:ident),+ $(,)?) => { $(features.push((stringify!($field).to_owned(),$group.$field==vk::TRUE));)+ }; }
    feature!(base; robust_buffer_access,full_draw_index_uint32,image_cube_array,independent_blend,geometry_shader,tessellation_shader,sample_rate_shading,dual_src_blend,logic_op,multi_draw_indirect,draw_indirect_first_instance,depth_clamp,depth_bias_clamp,fill_mode_non_solid,depth_bounds,wide_lines,large_points,alpha_to_one,multi_viewport,sampler_anisotropy,texture_compression_etc2,texture_compression_astc_ldr,texture_compression_bc,occlusion_query_precise,pipeline_statistics_query,vertex_pipeline_stores_and_atomics,fragment_stores_and_atomics,shader_tessellation_and_geometry_point_size,shader_image_gather_extended,shader_storage_image_extended_formats,shader_storage_image_multisample,shader_storage_image_read_without_format,shader_storage_image_write_without_format,shader_uniform_buffer_array_dynamic_indexing,shader_sampled_image_array_dynamic_indexing,shader_storage_buffer_array_dynamic_indexing,shader_storage_image_array_dynamic_indexing,shader_clip_distance,shader_cull_distance,shader_float64,shader_int64,shader_int16,shader_resource_residency,shader_resource_min_lod,sparse_binding,sparse_residency_buffer,sparse_residency_image2_d,sparse_residency_image3_d,sparse_residency2_samples,sparse_residency4_samples,sparse_residency8_samples,sparse_residency16_samples,sparse_residency_aliased,variable_multisample_rate,inherited_queries);
    if has_features2 && api >= vk::API_VERSION_1_2 && props.api_version >= vk::API_VERSION_1_2 {
        let mut f11 = vk::PhysicalDeviceVulkan11Features::default();
        let mut f12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut f13 = vk::PhysicalDeviceVulkan13Features::default();
        let mut query = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut f11)
            .push_next(&mut f12);
        if api >= vk::API_VERSION_1_3 && props.api_version >= vk::API_VERSION_1_3 {
            query = query.push_next(&mut f13);
        }
        instance
            .0
            .get_physical_device_features2(physical, &mut query);
        feature!(f11; storage_buffer16_bit_access,multiview,variable_pointers,sampler_ycbcr_conversion,shader_draw_parameters);
        feature!(f12; shader_float16,shader_int8,descriptor_indexing,scalar_block_layout,timeline_semaphore,buffer_device_address,vulkan_memory_model);
        if api >= vk::API_VERSION_1_3 && props.api_version >= vk::API_VERSION_1_3 {
            feature!(f13; dynamic_rendering,synchronization2,maintenance4,subgroup_size_control,shader_demote_to_helper_invocation);
        }
    }
    let mut memory = instance.0.get_physical_device_memory_properties(physical);
    let mut budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
    let budget_supported = has_memory2 && has(ash::ext::memory_budget::NAME);
    if budget_supported {
        let mut query = vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
        instance
            .0
            .get_physical_device_memory_properties2(physical, &mut query);
        memory = query.memory_properties;
    }
    let counts_valid = memory.memory_heap_count as usize <= memory.memory_heaps.len()
        && memory.memory_type_count as usize <= memory.memory_types.len()
        && memory.memory_types[..memory.memory_type_count as usize]
            .iter()
            .all(|t| t.heap_index < memory.memory_heap_count);
    let heaps = counts_valid.then(|| {
        memory.memory_heaps[..memory.memory_heap_count as usize]
            .iter()
            .enumerate()
            .map(|(i, h)| VulkanHeap {
                size_bytes: h.size,
                flags: h.flags.as_raw(),
                budget_bytes_at_discovery: budget_supported.then_some(budget.heap_budget[i]),
                process_usage_bytes_at_discovery: budget_supported.then_some(budget.heap_usage[i]),
            })
            .collect()
    });
    let memory_types = counts_valid.then(|| {
        memory.memory_types[..memory.memory_type_count as usize]
            .iter()
            .map(|t| VulkanMemoryType {
                heap_index: t.heap_index,
                property_flags: t.property_flags.as_raw(),
            })
            .collect()
    });
    let l = props.limits;
    let mut limits = Vec::new();
    macro_rules! limit { ($($field:ident),+ $(,)?) => { $(limits.push((stringify!($field).to_owned(),format!("{}",l.$field)));)+ }; }
    limit!(
        max_image_dimension1_d,
        max_image_dimension2_d,
        max_image_dimension3_d,
        max_image_dimension_cube,
        max_image_array_layers,
        max_bound_descriptor_sets,
        max_uniform_buffer_range,
        max_storage_buffer_range,
        max_memory_allocation_count,
        max_sampler_allocation_count,
        max_push_constants_size,
        max_compute_shared_memory_size,
        max_compute_work_group_invocations,
        timestamp_period
    );
    limits.push((
        "max_compute_work_group_count".into(),
        format!("{:?}", l.max_compute_work_group_count),
    ));
    limits.push((
        "max_compute_work_group_size".into(),
        format!("{:?}", l.max_compute_work_group_size),
    ));
    limits.push((
        "timestamp_compute_and_graphics".into(),
        (l.timestamp_compute_and_graphics == vk::TRUE).to_string(),
    ));
    Ok(VulkanDevice {
        name: device_name,
        api_version: version(props.api_version),
        vendor_id: props.vendor_id,
        device_id: props.device_id,
        driver_version_raw: props.driver_version,
        driver_name: driver_supported.then(|| string(&driver.driver_name)),
        driver_info: driver_supported.then(|| string(&driver.driver_info)),
        driver_id: driver_supported.then(|| driver.driver_id.as_raw()),
        extensions,
        limits,
        features,
        heaps,
        memory_types,
        sampled_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn api_version_is_decoded_only_as_an_api_version() {
        assert_eq!(version(vk::make_api_version(0, 1, 3, 281)), "1.3.281");
    }
    #[test]
    fn older_drivers_retry_core_1_0_but_allocation_failures_do_not_retry() {
        let mut calls = Vec::new();
        let result = create_compatible_instance(vk::API_VERSION_1_3, |api| {
            calls.push(api);
            if api == vk::API_VERSION_1_0 {
                Ok(())
            } else {
                Err(vk::Result::ERROR_INCOMPATIBLE_DRIVER)
            }
        })
        .unwrap();
        assert_eq!(result.0, vk::API_VERSION_1_0);
        assert_eq!(calls, vec![vk::API_VERSION_1_3, vk::API_VERSION_1_0]);
        let mut calls = 0;
        let result = create_compatible_instance::<()>(vk::API_VERSION_1_3, |_| {
            calls += 1;
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY)
        });
        assert_eq!(result.unwrap_err(), vk::Result::ERROR_OUT_OF_HOST_MEMORY);
        assert_eq!(calls, 1);
    }
    #[test]
    fn missing_loader_is_a_reported_failure() {
        let result = unsafe { discover("/nonexistent-voltix-vulkan-loader.so") };
        assert_eq!(result.unwrap_err(), "vk_loader_unavailable");
    }
    #[test]
    #[ignore = "requires an installed Vulkan loader and driver; never required on CPU-only CI"]
    fn discover_real_host_driver_without_a_surface_or_logical_device() {
        let report = probe();
        let device = report
            .device
            .expect("Vulkan host loader/driver unavailable");
        assert!(!device.name.is_empty());
        assert!(device.features.len() >= 55);
        assert!(!device.limits.is_empty());
        let encoded = serde_json::to_string(&device).unwrap();
        assert!(encoded.contains("driver_version_raw"));
    }
}
