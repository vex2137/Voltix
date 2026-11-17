//! Presentation adapter for shared real telemetry. No sampling or disk I/O on the UI path.
use crate::models::history::BoundedHistory;
use crate::models::telemetry::CpuUtilizationScope;
use crate::{
    app::{AppWindow, ControlRow, ControlSummary, I18n},
    localization::Localizer,
    models::telemetry::TelemetrySnapshot,
    monitoring::extended::{Battery, Process},
    utils::format::format_bytes_binary,
};
use slint::{ComponentHandle, ModelRc};
use std::time::Duration;
pub fn localize(window: &AppWindow, loc: &Localizer) {
    crate::process_ui::localize(window, loc);
    let i18n = window.global::<I18n>();
    i18n.set_hotfix_diagnostics(loc.tr("hotfix_diagnostics").into());
    i18n.set_hotfix_permissions(loc.tr("hotfix_permissions").into());
    i18n.set_hotfix_gpu(loc.tr("hotfix_gpu").into());
    i18n.set_hotfix_platform(loc.tr("hotfix_platform").into());
    i18n.set_hotfix_build(loc.tr("hotfix_build").into());
    i18n.set_hotfix_source(loc.tr("hotfix_source").into());
    i18n.set_hotfix_technical(loc.tr("hotfix_technical").into());
    i18n.set_hotfix_battery_status(loc.tr("hotfix_battery_status").into());
    i18n.set_hotfix_battery_measurements(loc.tr("hotfix_battery_measurements").into());
    i18n.set_hotfix_battery_condition(loc.tr("hotfix_battery_condition").into());
    i18n.set_hotfix_runtime(loc.tr("hotfix_runtime").into());
    i18n.set_hotfix_not_required(loc.tr("hotfix_not_required").into());
    i18n.set_hotfix_network_permission(loc.tr("hotfix_network_permission").into());
    i18n.set_hotfix_network_normal(loc.tr("hotfix_network_normal").into());
    i18n.set_hotfix_gpu_policy(loc.tr("hotfix_gpu_policy").into());
    i18n.set_hotfix_gpu_policy_value(loc.tr("hotfix_gpu_policy_value").into());
    i18n.set_hotfix_gpu_policy_detail(loc.tr("hotfix_gpu_policy_detail").into());
    i18n.set_hotfix_gpu_vram(loc.tr("hotfix_gpu_vram").into());
    i18n.set_hotfix_no_gpu(loc.tr("hotfix_no_gpu").into());
    i18n.set_cc_category_diagnostics(loc.tr("cc_category_diagnostics").into());
    i18n.set_mobile_hardware(loc.tr("mobile_hardware").into());
    i18n.set_mobile_application(loc.tr("mobile_application").into());
    i18n.set_cc_battery_short(loc.tr("cc_battery_short").into());
    i18n.set_cc_android_fps_note(loc.tr("cc_android_fps_note").into());

    i18n.set_cc_profiles(loc.tr("cc_profiles").into());
    i18n.set_cc_metrics(loc.tr("cc_metrics").into());
    i18n.set_cc_preview(loc.tr("cc_preview").into());
    i18n.set_cc_apply(loc.tr("cc_apply").into());
    i18n.set_cc_yes(loc.tr("cc_yes").into());
    i18n.set_cc_no(loc.tr("cc_no").into());

    if !window.get_cc_export_code().is_empty() {
        window.set_cc_export_status(
            format!(
                "{}\n{}",
                loc.tr(window.get_cc_export_code().as_str()),
                window.get_cc_export_detail()
            )
            .into(),
        );
    }
    i18n.set_cc_network(loc.tr("cc_network").into());
    i18n.set_cc_battery(loc.tr("cc_battery").into());
    i18n.set_cc_sensors(loc.tr("cc_sensors").into());
    i18n.set_cc_processes(loc.tr("cc_processes").into());
    i18n.set_cc_more(loc.tr("cc_more").into());
    i18n.set_cc_studio(loc.tr("cc_studio").into());
    i18n.set_cc_settings(loc.tr("cc_settings").into());
    i18n.set_cc_system(loc.tr("cc_system").into());
    i18n.set_cc_memory(loc.tr("cc_memory").into());
    i18n.set_cc_storage(loc.tr("cc_storage").into());
    i18n.set_cc_about(loc.tr("cc_about").into());
    i18n.set_cc_general(loc.tr("cc_general").into());
    i18n.set_cc_appearance(loc.tr("cc_appearance").into());
    i18n.set_cc_monitoring(loc.tr("cc_monitoring").into());
    i18n.set_cc_data_export(loc.tr("cc_data_export").into());
    i18n.set_cc_permissions(loc.tr("cc_permissions").into());
    i18n.set_cc_control_center(loc.tr("cc_control_center").into());
    i18n.set_cc_modern(loc.tr("cc_modern").into());
    i18n.set_cc_professional(loc.tr("cc_professional").into());
    i18n.set_cc_gaming(loc.tr("cc_gaming").into());
    i18n.set_cc_download(loc.tr("cc_download").into());
    i18n.set_cc_upload(loc.tr("cc_upload").into());
    i18n.set_cc_received(loc.tr("cc_received").into());
    i18n.set_cc_transmitted(loc.tr("cc_transmitted").into());
    i18n.set_cc_disk_read(loc.tr("cc_disk_read").into());
    i18n.set_cc_disk_write(loc.tr("cc_disk_write").into());
    i18n.set_cc_capacity(loc.tr("cc_capacity").into());
    i18n.set_cc_activity(loc.tr("cc_activity").into());
    i18n.set_cc_pressure(loc.tr("cc_pressure").into());
    i18n.set_cc_low_memory(loc.tr("cc_low_memory").into());
    i18n.set_cc_history(loc.tr("cc_history").into());
    i18n.set_cc_history_note(loc.tr("cc_history_note").into());
    i18n.set_cc_range(loc.tr("cc_range").into());
    i18n.set_cc_one_min(loc.tr("cc_one_min").into());
    i18n.set_cc_five_min(loc.tr("cc_five_min").into());
    i18n.set_cc_fifteen_min(loc.tr("cc_fifteen_min").into());
    i18n.set_cc_battery_percent(loc.tr("cc_battery_percent").into());
    i18n.set_cc_battery_power(loc.tr("cc_battery_power").into());
    i18n.set_cc_battery_temp(loc.tr("cc_battery_temp").into());
    i18n.set_cc_status(loc.tr("cc_status").into());
    i18n.set_cc_source(loc.tr("cc_source").into());
    i18n.set_cc_voltage(loc.tr("cc_voltage").into());
    i18n.set_cc_current(loc.tr("cc_current").into());
    i18n.set_cc_technology(loc.tr("cc_technology").into());
    i18n.set_cc_health(loc.tr("cc_health").into());
    i18n.set_cc_health_note(loc.tr("cc_health_note").into());
    i18n.set_cc_charging(loc.tr("cc_charging").into());
    i18n.set_cc_discharging(loc.tr("cc_discharging").into());
    i18n.set_cc_not_charging(loc.tr("cc_not_charging").into());
    i18n.set_cc_full(loc.tr("cc_full").into());
    i18n.set_cc_unknown(loc.tr("cc_unknown").into());
    i18n.set_cc_ac(loc.tr("cc_ac").into());
    i18n.set_cc_usb(loc.tr("cc_usb").into());
    i18n.set_cc_wireless(loc.tr("cc_wireless").into());
    i18n.set_cc_dock(loc.tr("cc_dock").into());
    i18n.set_cc_unplugged(loc.tr("cc_unplugged").into());
    i18n.set_cc_good(loc.tr("cc_good").into());
    i18n.set_cc_overheat(loc.tr("cc_overheat").into());
    i18n.set_cc_dead(loc.tr("cc_dead").into());
    i18n.set_cc_overvoltage(loc.tr("cc_overvoltage").into());
    i18n.set_cc_failure(loc.tr("cc_failure").into());
    i18n.set_cc_cold(loc.tr("cc_cold").into());
    i18n.set_cc_display(loc.tr("cc_display").into());
    i18n.set_cc_no_battery(loc.tr("cc_no_battery").into());
    i18n.set_cc_no_network(loc.tr("cc_no_network").into());
    i18n.set_cc_no_disk(loc.tr("cc_no_disk").into());
    i18n.set_cc_no_sensors(loc.tr("cc_no_sensors").into());
    i18n.set_cc_process_note(loc.tr("cc_process_note").into());
    i18n.set_cc_process_unavailable(loc.tr("cc_process_unavailable").into());
    i18n.set_cc_filter(loc.tr("cc_filter").into());
    i18n.set_cc_sort_cpu(loc.tr("cc_sort_cpu").into());
    i18n.set_cc_sort_memory(loc.tr("cc_sort_memory").into());
    i18n.set_cc_sort_pid(loc.tr("cc_sort_pid").into());
    i18n.set_cc_cpu_temp(loc.tr("cc_cpu_temp").into());
    i18n.set_cc_gpu_temp(loc.tr("cc_gpu_temp").into());
    i18n.set_cc_cpu_freq(loc.tr("cc_cpu_freq").into());
    i18n.set_cc_gpu_freq(loc.tr("cc_gpu_freq").into());
    i18n.set_cc_gpu_power(loc.tr("cc_gpu_power").into());
    i18n.set_cc_gpu_fan(loc.tr("cc_gpu_fan").into());
    i18n.set_cc_fps_note(loc.tr("cc_fps_note").into());
    i18n.set_cc_open_studio(loc.tr("cc_open_studio").into());
    i18n.set_cc_open_settings(loc.tr("cc_open_settings").into());
    i18n.set_cc_open_diagnostics(loc.tr("cc_open_diagnostics").into());
    i18n.set_cc_sensors_note(loc.tr("cc_sensors_note").into());
    i18n.set_cc_android_limits(loc.tr("cc_android_limits").into());
    i18n.set_cc_network_permission(loc.tr("cc_network_permission").into());
    i18n.set_cc_granted(loc.tr("cc_granted").into());
    i18n.set_cc_denied(loc.tr("cc_denied").into());
    i18n.set_cc_permission_note(loc.tr("cc_permission_note").into());
    i18n.set_cc_app_settings(loc.tr("cc_app_settings").into());
    i18n.set_cc_linux_diagnostics(loc.tr("cc_linux_diagnostics").into());
    i18n.set_cc_nvml_ok(loc.tr("cc_nvml_ok").into());
    i18n.set_cc_nvml_missing(loc.tr("cc_nvml_missing").into());
    i18n.set_cc_hwmon(loc.tr("cc_hwmon").into());
    i18n.set_cc_smart_note(loc.tr("cc_smart_note").into());
    i18n.set_cc_export_note(loc.tr("cc_export_note").into());
    i18n.set_cc_export(loc.tr("cc_export").into());
    i18n.set_cc_export_saved(loc.tr("cc_export_saved").into());
    i18n.set_cc_export_cancelled(loc.tr("cc_export_cancelled").into());
    i18n.set_cc_export_failed(loc.tr("cc_export_failed").into());
    i18n.set_cc_export_choose(loc.tr("cc_export_choose").into());
    i18n.set_cc_export_path(loc.tr("cc_export_path").into());
    i18n.set_cc_no_snapshot(loc.tr("cc_no_snapshot").into());
    i18n.set_cc_sensor_available(loc.tr("cc_sensor_available").into());
    i18n.set_cc_mode(loc.tr("cc_mode").into());
    i18n.set_cc_process_count(loc.tr("cc_process_count").into());
    i18n.set_cc_sample_wait(loc.tr("cc_sample_wait").into());
    i18n.set_cc_history_current(loc.tr("cc_history_current").into());
    i18n.set_cc_system_info(loc.tr("cc_system_info").into());
    i18n.set_cc_export_running(loc.tr("cc_export_running").into());
}
fn value(v: Option<f64>, suffix: &str, loc: &Localizer) -> String {
    v.filter(|n| n.is_finite())
        .map(|n| format!("{n:.1}{suffix}"))
        .unwrap_or_else(|| loc.tr("status_unavailable").into())
}
fn bytes(v: Option<u64>, loc: &Localizer) -> String {
    v.map(format_bytes_binary)
        .unwrap_or_else(|| loc.tr("status_unavailable").into())
}
fn speed(v: Option<f64>, loc: &Localizer) -> String {
    v.filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| format!("{}/s", format_bytes_binary(v as u64)))
        .unwrap_or_else(|| loc.tr("status_unavailable").into())
}
fn temperature(v: Option<f64>, unit: i32, loc: &Localizer) -> String {
    v.map(|v| {
        crate::utils::format::format_temperature(
            v,
            if unit == 1 {
                crate::settings::TemperatureUnit::Fahrenheit
            } else {
                crate::settings::TemperatureUnit::Celsius
            },
        )
    })
    .unwrap_or_else(|| loc.tr("status_unavailable").into())
}
fn row(key: &str, v: String, detail: String, loc: &Localizer) -> ControlRow {
    ControlRow {
        label: loc.tr(key).into(),
        value: v.into(),
        detail: detail.into(),
    }
}
fn raw_row(label: String, v: String, detail: String) -> ControlRow {
    ControlRow {
        label: label.into(),
        value: v.into(),
        detail: detail.into(),
    }
}
fn rows(model: ModelRc<ControlRow>, next: Vec<ControlRow>) {
    crate::app::update_vec_model(model, next);
}
fn summary(label: &str, v: String, detail: String, p: Option<f64>) -> ControlSummary {
    ControlSummary {
        label: label.into(),
        value: v.into(),
        detail: detail.into(),
        percent: p.unwrap_or(0.0) as f32,
        status: if p.is_some() { 0 } else { 1 },
    }
}
pub fn battery_rows(b: &Battery, loc: &Localizer, unit: i32) -> Vec<ControlRow> {
    let status = match b.status {
        2 => "cc_charging",
        3 => "cc_discharging",
        4 => "cc_not_charging",
        5 => "cc_full",
        _ => "cc_unknown",
    };
    let source = match b.source {
        1 => "cc_ac",
        2 => "cc_usb",
        4 => "cc_wireless",
        8 => "cc_dock",
        0 => "cc_unplugged",
        _ => "cc_unknown",
    };
    let health = match b.health {
        2 => "cc_good",
        3 => "cc_overheat",
        4 => "cc_dead",
        5 => "cc_overvoltage",
        6 => "cc_failure",
        7 => "cc_cold",
        _ => "cc_unknown",
    };
    vec![
        row(
            "cc_battery_percent",
            value(b.percent, "%", loc),
            String::new(),
            loc,
        ),
        row("cc_status", loc.tr(status).into(), String::new(), loc),
        row(
            "cc_source",
            loc.tr(if b.source == 0 && b.status == 2 {
                "cc_unknown"
            } else {
                source
            })
            .into(),
            String::new(),
            loc,
        ),
        row(
            "cc_battery_temp",
            temperature(b.temperature_c, unit, loc),
            String::new(),
            loc,
        ),
        row(
            "cc_voltage",
            b.voltage_v
                .filter(|v| v.is_finite())
                .map(|v| format!("{v:.2} V"))
                .unwrap_or_else(|| loc.tr("status_unavailable").into()),
            String::new(),
            loc,
        ),
        row(
            "cc_current",
            b.current_a
                .filter(|v| v.is_finite())
                .map(|v| format!("{v:.2} A"))
                .unwrap_or_else(|| loc.tr("status_unavailable").into()),
            String::new(),
            loc,
        ),
        row(
            "cc_battery_power",
            b.power_w
                .filter(|v| v.is_finite())
                .map(|v| format!("{v:.2} W"))
                .unwrap_or_else(|| loc.tr("status_unavailable").into()),
            String::new(),
            loc,
        ),
        row(
            "cc_technology",
            if b.technology.is_empty() {
                loc.tr("cc_unknown").into()
            } else {
                b.technology.clone()
            },
            String::new(),
            loc,
        ),
        row(
            "cc_health",
            loc.tr(health).into(),
            loc.tr("cc_health_note").into(),
            loc,
        ),
    ]
}
pub fn filter_processes<'a>(processes: &'a [Process], query: &str, sort: i32) -> Vec<&'a Process> {
    crate::process_ui::filtered(
        processes,
        &crate::process_ui::Filter {
            query: query.into(),
            sort,
            descending: sort != 2,
            ..Default::default()
        },
    )
}
pub fn chart_samples(
    snap: &TelemetrySnapshot,
    cpu: Option<f64>,
    gpu_index: usize,
) -> [Option<f64>; 10] {
    let e = &snap.extended;
    let sum = |v: Vec<Option<f64>>| {
        if v.is_empty() || v.iter().any(Option::is_none) {
            None
        } else {
            Some(v.into_iter().flatten().sum())
        }
    };
    [
        cpu,
        snap.gpus
            .get(gpu_index)
            .and_then(|g| g.utilization_percent.copied()),
        snap.memory.utilization_percent.copied(),
        sum(e.network.iter().map(|n| n.down_bps).collect()),
        sum(e.network.iter().map(|n| n.up_bps).collect()),
        sum(e.disks.iter().map(|d| d.read_bps).collect()),
        sum(e.disks.iter().map(|d| d.write_bps).collect()),
        e.battery.as_ref().and_then(|b| b.percent),
        e.battery.as_ref().and_then(|b| b.power_w),
        e.battery.as_ref().and_then(|b| b.temperature_c),
    ]
}
pub fn android_permission_rows(
    platform: &crate::monitoring::extended::Platform,
    loc: &Localizer,
) -> Vec<ControlRow> {
    vec![
        row(
            "hotfix_runtime",
            loc.tr("hotfix_not_required").into(),
            loc.tr("cc_permission_note").into(),
            loc,
        ),
        row(
            "hotfix_network_permission",
            loc.tr(if platform.elapsed_ms == 0 {
                "status_sampling"
            } else if platform.network_permission {
                "cc_granted"
            } else {
                "cc_denied"
            })
            .into(),
            loc.tr("hotfix_network_normal").into(),
            loc,
        ),
    ]
}

pub fn gpu_diagnostic_rows(
    gpu: Option<&crate::models::telemetry::GpuTelemetry>,
    loc: &Localizer,
) -> Vec<ControlRow> {
    use crate::models::metrics::MetricStatus;
    let Some(gpu) = gpu else {
        return vec![row(
            "nav_gpu",
            loc.tr("status_unavailable").into(),
            loc.tr("hotfix_no_gpu").into(),
            loc,
        )];
    };
    let mut out = Vec::new();
    for (label, status, reason) in [
        (
            "gpu_utilization",
            gpu.utilization_percent.status(),
            gpu.utilization_percent.reason(),
        ),
        (
            "cc_gpu_temp",
            gpu.temperature_celsius.status(),
            gpu.temperature_celsius.reason(),
        ),
        (
            "cc_gpu_freq",
            gpu.core_clock_mhz.status(),
            gpu.core_clock_mhz.reason(),
        ),
        (
            "hotfix_gpu_vram",
            gpu.vram_total_bytes.status(),
            gpu.vram_total_bytes.reason(),
        ),
        (
            "cc_gpu_power",
            gpu.power_draw_watts.status(),
            gpu.power_draw_watts.reason(),
        ),
    ] {
        let key = match status {
            MetricStatus::Available => "gpu_readable",
            MetricStatus::Unsupported => "status_unsupported",
            MetricStatus::PermissionError => "status_permission_denied",
            MetricStatus::Unavailable => "status_unavailable",
        };
        out.push(row(
            label,
            loc.tr(key).into(),
            loc.diagnostic(reason.unwrap_or("")),
            loc,
        ));
    }
    out
}

pub fn render(window: &AppWindow, snap: &TelemetrySnapshot, loc: &Localizer, gpu_index: usize) {
    let e = &snap.extended;
    let gpu = snap.gpus.get(gpu_index);
    let live = chart_samples(
        snap,
        match snap.cpu.utilization_scope {
            CpuUtilizationScope::SystemWide => snap.cpu.overall_utilization.copied(),
            CpuUtilizationScope::ProcessOnly => snap.cpu.process_utilization.copied(),
            CpuUtilizationScope::Unavailable => None,
        },
        gpu_index,
    );
    let disk_total: u64 = snap
        .storage
        .iter()
        .filter_map(|d| d.total_bytes.copied())
        .sum();
    let disk_used: u64 = snap
        .storage
        .iter()
        .filter_map(|d| d.used_bytes.copied())
        .sum();
    let capacity = if disk_total > 0 {
        Some(disk_used as f64 * 100.0 / disk_total as f64)
    } else {
        None
    };
    let mut summaries = vec![
        summary(
            if snap.cpu.utilization_scope == CpuUtilizationScope::ProcessOnly {
                loc.tr("cpu_process_util")
            } else {
                "CPU"
            },
            value(live[0], "%", loc),
            value(snap.cpu.average_frequency_mhz.copied(), " MHz", loc),
            live[0],
        ),
        summary(
            "GPU",
            value(live[1], "%", loc),
            gpu.map(|g| g.name.clone())
                .unwrap_or_else(|| loc.tr("status_unavailable").into()),
            live[1],
        ),
        summary(
            "RAM",
            bytes(snap.memory.used_bytes.copied(), loc),
            bytes(snap.memory.total_bytes.copied(), loc),
            live[2],
        ),
        summary(
            "VRAM",
            bytes(gpu.and_then(|g| g.vram_used_bytes.copied()), loc),
            bytes(gpu.and_then(|g| g.vram_total_bytes.copied()), loc),
            gpu.and_then(|g| g.vram_utilization_percent.copied()),
        ),
        summary(
            loc.tr("cc_storage"),
            bytes((disk_total > 0).then_some(disk_used), loc),
            bytes((disk_total > 0).then_some(disk_total), loc),
            capacity,
        ),
        summary(
            loc.tr("cc_network"),
            speed(live[3], loc),
            format!("{}: {}", loc.tr("cc_upload"), speed(live[4], loc)),
            live[3],
        ),
    ];
    if window.get_android_platform() {
        for index in [1, 3] {
            if summaries[index].status != 0 {
                summaries[index].value = "N/A".into();
            }
        }
        if summaries[3].detail == loc.tr("status_unavailable") {
            summaries[3].detail = "N/A".into();
        }
    }
    // Network has no meaningful percent; its bar is disabled by the Slint card.
    summaries[5].percent = 0.0;
    crate::app::update_vec_model(window.get_cc_summaries(), summaries);
    window.set_cc_has_battery(e.battery.is_some());
    let mut performance = vec![
        row(
            "cc_cpu_temp",
            temperature(
                snap.cpu.temperature_celsius.copied(),
                window.get_temp_unit(),
                loc,
            ),
            snap.cpu.temperature_source.clone().unwrap_or_default(),
            loc,
        ),
        row(
            "cc_cpu_freq",
            value(snap.cpu.average_frequency_mhz.copied(), " MHz", loc),
            String::new(),
            loc,
        ),
        row(
            "cc_gpu_temp",
            value(gpu.and_then(|g| g.temperature_celsius.copied()), " °C", loc),
            String::new(),
            loc,
        ),
        row(
            "cc_gpu_freq",
            format!(
                "{} / {}",
                value(
                    gpu.and_then(|g| g.core_clock_mhz.copied()).map(f64::from),
                    " MHz",
                    loc
                ),
                value(
                    gpu.and_then(|g| g.memory_clock_mhz.copied()).map(f64::from),
                    " MHz",
                    loc
                )
            ),
            String::new(),
            loc,
        ),
        row(
            "cc_gpu_power",
            value(gpu.and_then(|g| g.power_draw_watts.copied()), " W", loc),
            String::new(),
            loc,
        ),
        row(
            "cc_gpu_fan",
            value(
                gpu.and_then(|g| g.fan_speed_percent.copied())
                    .map(f64::from),
                "%",
                loc,
            ),
            String::new(),
            loc,
        ),
    ];
    if window.get_android_platform() {
        for item in performance.iter_mut().skip(2) {
            let text = item.value.replace(loc.tr("status_unavailable"), "N/A");
            item.value = if text == "N/A / N/A" {
                "N/A".into()
            } else {
                text.into()
            };
        }
    }
    if !window.get_android_platform() {
        crate::desktop_sensors::render(window, &performance, &e.sensors, loc);
    }
    rows(window.get_cc_performance(), performance);
    let network: Vec<_> = e
        .network
        .iter()
        .map(|n| {
            raw_row(
                n.name.clone(),
                format!(
                    "{}: {}  ·  {}: {}",
                    loc.tr("cc_download"),
                    speed(n.down_bps, loc),
                    loc.tr("cc_upload"),
                    speed(n.up_bps, loc)
                ),
                format!(
                    "{}: {} · {}: {}",
                    loc.tr("cc_received"),
                    format_bytes_binary(n.received),
                    loc.tr("cc_transmitted"),
                    format_bytes_binary(n.transmitted)
                ),
            )
        })
        .collect();
    let disk: Vec<_> = e
        .disks
        .iter()
        .map(|d| {
            raw_row(
                d.name.clone(),
                format!(
                    "{}: {} · {}: {}",
                    loc.tr("cc_disk_read"),
                    speed(d.read_bps, loc),
                    loc.tr("cc_disk_write"),
                    speed(d.write_bps, loc)
                ),
                String::new(),
            )
        })
        .collect();
    rows(window.get_cc_network(), network.clone());
    rows(window.get_cc_disks(), disk.clone());
    let mut sensors: Vec<_> = e
        .sensors
        .iter()
        .map(|s| {
            raw_row(
                s.name.clone(),
                format!("{:.2} {}", s.value, s.unit),
                String::new(),
            )
        })
        .collect();
    sensors.extend(e.platform.sensors.iter().map(|name| {
        raw_row(
            name.clone(),
            loc.tr("cc_sensor_available").into(),
            String::new(),
        )
    }));
    // A professional overview is deliberately bounded; the Sensors page exposes the whole inventory.
    rows(
        window.get_cc_sensors(),
        sensors.iter().take(8).cloned().collect(),
    );
    let mut diagnostics = vec![row(
        "cc_sensors",
        if window.get_android_platform() && e.platform.elapsed_ms == 0 {
            loc.tr("status_sampling").into()
        } else {
            (e.sensors.len() + e.platform.sensors.len()).to_string()
        },
        loc.tr("cc_sensors_note").into(),
        loc,
    )];
    #[cfg(target_os = "linux")]
    {
        diagnostics.push(raw_row(
            "NVIDIA / NVML".into(),
            loc.tr(if snap.gpus.iter().any(|g| g.id.starts_with("nvml:")) {
                "cc_nvml_ok"
            } else {
                "cc_nvml_missing"
            })
            .into(),
            String::new(),
        ));
        diagnostics.push(row(
            "cc_storage",
            loc.tr("cc_smart_note").into(),
            String::new(),
            loc,
        ));
    }
    if !e.platform.action_status.is_empty() {
        diagnostics.push(row(
            "cc_permissions",
            loc.tr(&e.platform.action_status).into(),
            String::new(),
            loc,
        ));
    }
    diagnostics.push(row(
        "cc_pressure",
        value(e.memory_pressure, "%", loc),
        String::new(),
        loc,
    ));
    let mut gpu_diagnostics = gpu_diagnostic_rows(gpu, loc);
    if window.get_android_platform() {
        gpu_diagnostics.push(row(
            "hotfix_gpu_policy",
            loc.tr("hotfix_gpu_policy_value").into(),
            loc.tr("hotfix_gpu_policy_detail").into(),
            loc,
        ));
    }
    let permissions = android_permission_rows(&e.platform, loc);
    rows(window.get_cc_permission_rows(), permissions.clone());
    rows(window.get_cc_gpu_diagnostic_rows(), gpu_diagnostics.clone());
    if window.get_android_platform() {
        diagnostics.push(row(
            "cc_display",
            if e.platform.display.is_empty() {
                loc.tr("status_sampling").into()
            } else {
                e.platform.display.clone()
            },
            String::new(),
            loc,
        ));
    }
    rows(
        window.get_cc_platform_diagnostic_rows(),
        diagnostics.clone(),
    );
    diagnostics.extend(gpu_diagnostics);
    if window.get_android_platform() {
        diagnostics.extend(permissions);
    }
    rows(window.get_cc_diagnostics(), diagnostics.clone());
    let system = vec![
        row(
            "cc_system_info",
            format!(
                "{} · {}",
                snap.system
                    .os_name
                    .value()
                    .cloned()
                    .unwrap_or_else(|| loc.tr("status_unavailable").into()),
                snap.system
                    .kernel_version
                    .value()
                    .cloned()
                    .unwrap_or_default()
            ),
            snap.cpu.model_name.value().cloned().unwrap_or_default(),
            loc,
        ),
        row(
            "cc_memory",
            bytes(snap.memory.total_bytes.copied(), loc),
            format!(
                "{}: {}",
                loc.tr("cc_pressure"),
                value(e.memory_pressure, "%", loc)
            ),
            loc,
        ),
    ];
    rows(window.get_cc_system(), system);
    let page = window.get_current_page();
    let (title, note, detail) = match page {
        7 => (
            "cc_network",
            if cfg!(target_os = "android") {
                "cc_android_network_note"
            } else {
                "cc_network_aggregate"
            },
            network,
        ),
        8 => (
            "cc_battery",
            if e.battery.is_some() {
                "cc_health_note"
            } else {
                "cc_no_battery"
            },
            e.battery
                .as_ref()
                .map(|b| battery_rows(b, loc, window.get_temp_unit()))
                .unwrap_or_default(),
        ),
        9 => ("cc_sensors", "cc_sensors_note", sensors),
        13 => (
            "cc_permissions",
            if cfg!(target_os = "android") {
                "cc_permission_note"
            } else {
                "cc_linux_diagnostics"
            },
            diagnostics,
        ),
        _ => ("cc_system", "", Vec::new()),
    };
    if [7, 8, 9, 13].contains(&page) {
        window.set_cc_detail_title(loc.tr(title).into());
        window.set_cc_detail_note(if page == 7 && !detail.is_empty() {
            "".into()
        } else {
            loc.tr(note).into()
        });
        rows(window.get_cc_detail_rows(), detail);
    }
    if !e.platform.export_status.is_empty() {
        window.set_cc_export_code(e.platform.export_status.clone().into());
        window.set_cc_export_detail("".into());
        window.set_cc_export_status(loc.tr(&e.platform.export_status).into());
    }
    let pressure = if cfg!(target_os = "android") {
        if e.platform.memory_total.is_some() {
            loc.tr(if e.platform.memory_low {
                "cc_yes"
            } else {
                "cc_no"
            })
            .to_string()
        } else {
            loc.tr("status_unavailable").into()
        }
    } else {
        value(e.memory_pressure, "%", loc)
    };
    window.set_cc_memory_pressure(pressure.into());
}
pub fn render_history(
    window: &AppWindow,
    history: &BoundedHistory,
    now: std::time::Instant,
    polling_ms: u64,
    loc: &Localizer,
) {
    let source = window.get_cc_chart_source();
    let converted = (source == 9 && window.get_temp_unit() == 1).then(|| {
        let mut out = BoundedHistory::new(901);
        for sample in history.samples() {
            out.push(
                sample.timestamp,
                sample
                    .value
                    .map(crate::utils::format::celsius_to_fahrenheit),
            );
        }
        out
    });
    let history = converted.as_ref().unwrap_or(history);
    let seconds = match window.get_cc_chart_range() {
        1 => 300,
        2 => 900,
        _ => 60,
    };
    let cutoff = now.checked_sub(Duration::from_secs(seconds));
    let mut min = 0.0_f64;
    let mut max: f64 = if [0, 1, 2, 7].contains(&source) {
        100.0
    } else {
        1.0
    };
    for sample in history
        .samples()
        .iter()
        .filter(|s| cutoff.is_none_or(|c| s.timestamp >= c))
    {
        if let Some(v) = sample.value.filter(|v| v.is_finite()) {
            min = min.min(v);
            max = max.max(v);
        }
    }
    let chart = history.build_chart_paths(
        now,
        Duration::from_secs(seconds),
        600.0,
        140.0,
        min,
        max,
        Duration::from_millis(polling_ms.max(1000) * 3),
    );
    let format = |v| {
        if (3..=6).contains(&source) {
            speed(v, loc)
        } else {
            value(
                v,
                if [0, 1, 2, 7].contains(&source) {
                    "%"
                } else if source == 8 {
                    " W"
                } else {
                    if window.get_temp_unit() == 1 {
                        " °F"
                    } else {
                        " °C"
                    }
                },
                loc,
            )
        }
    };
    window.set_cc_history_line(chart.line_commands.into());
    window.set_cc_history_area(chart.area_commands.into());
    window.set_cc_history_data(chart.has_data);
    window.set_cc_history_max(format(Some(max)).into());
    window.set_cc_history_min(format(Some(min)).into());
    window.set_cc_history_current(format(history.samples().back().and_then(|s| s.value)).into());
}
pub fn export_json(snapshot: &TelemetrySnapshot) -> Result<String, String> {
    serde_json::to_string_pretty(&serde_json::json!({"application":"Voltix","version":env!("CARGO_PKG_VERSION"),"unix_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_secs(),"telemetry":snapshot})).map_err(|e|e.to_string())
}
pub fn save_export(directory: &std::path::Path, json: &str) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("cc_export_failed".into());
    }
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = directory.join(format!("Voltix-telemetry-{time}.json"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|e| e.to_string())?;
    file.write_all(json.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_sort_and_filter_are_read_only() {
        let p = vec![
            Process {
                pid: 1,
                name: "a".into(),
                cpu_percent: Some(5.0),
                memory: 10,
                ..Default::default()
            },
            Process {
                pid: 2,
                name: "b".into(),
                cpu_percent: None,
                memory: 20,
                ..Default::default()
            },
        ];
        assert_eq!(filter_processes(&p, "", 0)[0].pid, 1);
        assert_eq!(filter_processes(&p, "", 1)[0].pid, 2);
        assert_eq!(filter_processes(&p, "b", 0).len(), 1);
        assert!(filter_processes(&p, "missing", 0).is_empty());
    }
    #[test]
    fn battery_presentation_preserves_discharge_sign_and_estimate() {
        let loc = Localizer::new(crate::localization::LocaleCode::En);
        let battery = Battery {
            current_a: Some(-1.25),
            power_w: Some(-4.75),
            status: 3,
            ..Default::default()
        };
        let rows = battery_rows(&battery, &loc, 0);
        assert!(rows[1].value.contains("Discharging"));
        assert!(rows[5].value.starts_with("-1.25"));
        assert!(rows[6].value.starts_with("-4.75"));
        assert!(rows[6].label.contains("estimate"));
    }
    #[test]
    fn absent_battery_readings_do_not_become_zero() {
        let l = Localizer::new(crate::localization::LocaleCode::En);
        let rows = battery_rows(&Battery::default(), &l, 0);
        assert_eq!(rows[0].value.as_str(), l.tr("status_unavailable"));
    }
}
