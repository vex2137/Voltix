//! Localized presentation of an immutable discovery snapshot. No native calls.
use crate::{
    app::ControlRow, localization::Localizer, models::vulkan::VulkanReport,
    utils::format::format_bytes_binary,
};
fn row(loc: &Localizer, key: &str, value: String, detail: String) -> ControlRow {
    ControlRow {
        label: loc.tr(key).into(),
        value: value.into(),
        detail: detail.into(),
    }
}
pub fn rows(report: &VulkanReport, loc: &Localizer) -> Vec<ControlRow> {
    let Some(d) = &report.device else {
        let raw = report.error.as_deref().unwrap_or("vk_no_device");
        let (key, code) = raw
            .split_once(':')
            .map(|(k, v)| (k, Some(v)))
            .unwrap_or((raw, None));
        let detail = format!(
            "{}{}",
            loc.tr(key),
            code.map(|v| format!(" (VkResult={v})")).unwrap_or_default()
        );
        return vec![row(
            loc,
            "vk_title",
            loc.tr("status_unavailable").into(),
            detail,
        )];
    };
    let unavailable = || loc.tr("status_unavailable").to_owned();
    let mut out = vec![
        row(
            loc,
            "vk_api",
            d.api_version.clone(),
            loc.tr("vk_static_note").into(),
        ),
        row(
            loc,
            "vk_ids",
            format!("0x{:04x} / 0x{:04x}", d.vendor_id, d.device_id),
            d.name.clone(),
        ),
        row(
            loc,
            "vk_driver",
            format!("0x{:08x}", d.driver_version_raw),
            format!(
                "{}\n{}\n{}{}",
                loc.tr("vk_driver_note"),
                d.driver_name.as_deref().unwrap_or(""),
                d.driver_info.as_deref().unwrap_or(""),
                d.driver_id
                    .map(|id| format!("\ndriverID={id}"))
                    .unwrap_or_default()
            ),
        ),
        row(
            loc,
            "vk_extensions",
            d.extensions
                .as_ref()
                .map(|v| v.len().to_string())
                .unwrap_or_else(unavailable),
            d.extensions
                .as_ref()
                .map(|v| v.join("\n"))
                .unwrap_or_else(unavailable),
        ),
        row(
            loc,
            "vk_limits",
            d.limits.len().to_string(),
            d.limits
                .iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        row(
            loc,
            "vk_features",
            format!(
                "{} / {}",
                d.features.iter().filter(|(_, v)| *v).count(),
                d.features.len()
            ),
            format!(
                "{}\n{}",
                loc.tr("vk_feature_scope"),
                d.features
                    .iter()
                    .map(|(k, v)| format!(
                        "{k}: {}",
                        loc.tr(if *v { "vk_supported" } else { "vk_unsupported" })
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        ),
    ];
    let heaps = d.heaps.as_ref().map(|heaps| {
        heaps
            .iter()
            .enumerate()
            .map(|(i, h)| {
                format!(
                    "{} {i}: {} · flags=0x{:x}{}",
                    loc.tr("vk_heap"),
                    format_bytes_binary(h.size_bytes),
                    h.flags,
                    if h.flags & 1 != 0 {
                        " DEVICE_LOCAL"
                    } else {
                        ""
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    out.push(row(
        loc,
        "vk_heaps",
        d.heaps
            .as_ref()
            .map(|v| v.len().to_string())
            .unwrap_or_else(unavailable),
        format!(
            "{}\n{}",
            loc.tr("vk_heap_note"),
            heaps.unwrap_or_else(unavailable)
        ),
    ));
    let types = d.memory_types.as_ref().map(|types| {
        types
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let names = [
                    (1, "DEVICE_LOCAL"),
                    (2, "HOST_VISIBLE"),
                    (4, "HOST_COHERENT"),
                    (8, "HOST_CACHED"),
                    (16, "LAZILY_ALLOCATED"),
                    (32, "PROTECTED"),
                ]
                .into_iter()
                .filter_map(|(bit, name)| (t.property_flags & bit != 0).then_some(name))
                .collect::<Vec<_>>()
                .join(" | ");
                format!(
                    "{} {i} → {} {}: flags=0x{:x} {names}",
                    loc.tr("vk_type"),
                    loc.tr("vk_heap"),
                    t.heap_index,
                    t.property_flags
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    out.push(row(
        loc,
        "vk_memory_types",
        d.memory_types
            .as_ref()
            .map(|v| v.len().to_string())
            .unwrap_or_else(unavailable),
        types.unwrap_or_else(unavailable),
    ));
    let budgets = d
        .heaps
        .as_ref()
        .map(|heaps| {
            heaps
                .iter()
                .enumerate()
                .filter_map(|(i, h)| {
                    h.budget_bytes_at_discovery.map(|budget| {
                        format!(
                            "{} {i}: {} {} · {} {}",
                            loc.tr("vk_heap"),
                            loc.tr("vk_budget"),
                            format_bytes_binary(budget),
                            loc.tr("vk_process_usage"),
                            h.process_usage_bytes_at_discovery
                                .map(format_bytes_binary)
                                .unwrap_or_else(unavailable)
                        )
                    })
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    out.push(row(
        loc,
        "vk_budget",
        if budgets.is_empty() {
            unavailable()
        } else {
            loc.tr("vk_snapshot").into()
        },
        format!(
            "{}\n{}\nUnix: {}",
            loc.tr("vk_budget_note"),
            if budgets.is_empty() {
                loc.tr("vk_budget_unavailable")
            } else {
                &budgets
            },
            d.sampled_at_unix_seconds
        ),
    ));
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_report_never_becomes_zero_or_vram() {
        for code in [
            crate::localization::LocaleCode::En,
            crate::localization::LocaleCode::Pl,
        ] {
            let loc = Localizer::new(code);
            let report = VulkanReport {
                device: None,
                error: Some("vk_instance_error:-9".into()),
            };
            let out = rows(&report, &loc);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].value.as_str(), loc.tr("status_unavailable"));
            assert!(out[0].detail.contains("VkResult=-9"));
        }
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::models::vulkan::{VulkanDevice, VulkanHeap, VulkanMemoryType};
    #[test]
    fn shared_heaps_and_process_budgets_are_not_presented_as_live_vram() {
        let report = VulkanReport {
            error: None,
            device: Some(VulkanDevice {
                name: "Fixture Mali".into(),
                api_version: "1.3.0".into(),
                vendor_id: 0x13b5,
                device_id: 1,
                driver_version_raw: 0x80000000,
                driver_name: None,
                driver_info: None,
                driver_id: None,
                extensions: Some(vec!["VK_EXT_memory_budget".into()]),
                limits: vec![],
                features: vec![("geometry_shader".into(), false)],
                heaps: Some(vec![VulkanHeap {
                    size_bytes: 8 << 30,
                    flags: 1,
                    budget_bytes_at_discovery: Some(6 << 30),
                    process_usage_bytes_at_discovery: Some(64 << 20),
                }]),
                memory_types: Some(vec![VulkanMemoryType {
                    heap_index: 0,
                    property_flags: 7,
                }]),
                sampled_at_unix_seconds: 42,
            }),
        };
        for code in [
            crate::localization::LocaleCode::En,
            crate::localization::LocaleCode::Pl,
        ] {
            let loc = Localizer::new(code);
            let out = rows(&report, &loc);
            assert_eq!(out[2].value.as_str(), "0x80000000");
            assert!(out[6].detail.starts_with(loc.tr("vk_heap_note")));
            assert_eq!(out[8].value.as_str(), loc.tr("vk_snapshot"));
            assert!(out[8].detail.starts_with(loc.tr("vk_budget_note")));
            assert!(out[8].detail.contains("Unix: 42"));
            assert!(out[7].detail.contains("HOST_VISIBLE"));
            assert!(!out.iter().any(|r| r.value.as_str() == "8.0 GiB"));
        }
    }
}
