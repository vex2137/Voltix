use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::models::metrics::MetricValue;
use crate::models::telemetry::{GpuTelemetry, GpuVendor, StorageDeviceTelemetry};

const PROP_VALUE_MAX: usize = 92;

extern "C" {
    fn __system_property_get(name: *const libc::c_char, value: *mut libc::c_char) -> libc::c_int;
}

pub fn get_system_property(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    let mut buf = [0_u8; PROP_VALUE_MAX];
    let len = unsafe { __system_property_get(c_name.as_ptr(), buf.as_mut_ptr().cast()) };
    if len <= 0 {
        return None;
    }
    let slice = &buf[..(len as usize).min(PROP_VALUE_MAX - 1)];
    let val = String::from_utf8_lossy(slice).trim().to_string();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

pub fn android_os_name() -> MetricValue<String> {
    let release = get_system_property("ro.build.version.release");
    let sdk = get_system_property("ro.build.version.sdk");
    match (release, sdk) {
        (Some(rel), Some(api)) => MetricValue::Available(format!("Android {rel} (API {api})")),
        (Some(rel), None) => MetricValue::Available(format!("Android {rel}")),
        (None, Some(api)) => MetricValue::Available(format!("Android (API {api})")),
        (None, None) => MetricValue::Available("Android".to_string()),
    }
}

pub fn android_device_name() -> MetricValue<String> {
    let mfg = get_system_property("ro.product.manufacturer");
    let model = get_system_property("ro.product.model");
    match (mfg, model) {
        (Some(m), Some(mdl)) => {
            if mdl
                .to_ascii_lowercase()
                .starts_with(&m.to_ascii_lowercase())
            {
                MetricValue::Available(mdl)
            } else {
                MetricValue::Available(format!("{m} {mdl}"))
            }
        }
        (None, Some(mdl)) => MetricValue::Available(mdl),
        (Some(m), None) => MetricValue::Available(m),
        (None, None) => MetricValue::Unavailable("Device model unavailable".to_string()),
    }
}

pub fn android_kernel_version() -> MetricValue<String> {
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::uname(&mut uts) };
    if rc != 0 {
        return MetricValue::Unavailable("uname failed".to_string());
    }
    let rel = unsafe { CStr::from_ptr(uts.release.as_ptr()) };
    let s = rel.to_string_lossy().trim().to_string();
    if s.is_empty() {
        MetricValue::Unavailable("Empty kernel release".to_string())
    } else {
        MetricValue::Available(s)
    }
}

pub fn android_uptime_seconds() -> MetricValue<u64> {
    let mut info: libc::sysinfo = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::sysinfo(&mut info) };
    if rc == 0 && info.uptime >= 0 {
        MetricValue::Available(info.uptime as u64)
    } else {
        MetricValue::Unavailable("sysinfo uptime unavailable".to_string())
    }
}

pub fn android_soc_model() -> Option<String> {
    get_system_property("ro.soc.model")
        .or_else(|| get_system_property("ro.board.platform"))
        .or_else(|| get_system_property("ro.hardware"))
}

pub fn android_soc_vendor() -> Option<String> {
    get_system_property("ro.soc.manufacturer")
        .or_else(|| get_system_property("ro.hardware.chipname"))
}

pub fn android_cpu_temperature() -> (MetricValue<f64>, Option<String>) {
    let thermal_root = Path::new("/sys/class/thermal");
    let entries = match fs::read_dir(thermal_root) {
        Ok(e) => e,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            return (
                MetricValue::PermissionError(
                    "Thermal sysfs restricted on unprivileged Android".to_string(),
                ),
                None,
            );
        }
        Err(_) => {
            return (
                MetricValue::Unsupported("Thermal zones unavailable".to_string()),
                None,
            );
        }
    };

    let mut saw_permission_denied = false;
    for entry in entries.flatten() {
        let dir = entry.path();
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let Ok(zone_type) = fs::read_to_string(dir.join("type")) else {
            continue;
        };
        let zt = zone_type.trim();
        let lower = zt.to_ascii_lowercase();
        if !(lower.contains("cpu") || lower.contains("soc") || lower.contains("tsens")) {
            continue;
        }

        match fs::read_to_string(dir.join("temp")) {
            Ok(raw) => {
                if let Ok(val) = raw.trim().parse::<i64>() {
                    // Android thermal zones are typically millidegrees C, occasionally degrees C
                    let celsius = if val > 200 {
                        val as f64 / 1000.0
                    } else {
                        val as f64
                    };
                    if (0.0..=140.0).contains(&celsius) {
                        return (MetricValue::Available(celsius), Some(zt.to_string()));
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                saw_permission_denied = true;
            }
            Err(_) => {}
        }
    }

    if saw_permission_denied {
        (
            MetricValue::PermissionError(
                "CPU thermal sensor restricted by Android SELinux".to_string(),
            ),
            None,
        )
    } else {
        (
            MetricValue::Unsupported("No readable CPU thermal zone".to_string()),
            None,
        )
    }
}

pub fn collect_android_gpus() -> Vec<GpuTelemetry> {
    let egl = get_system_property("ro.hardware.egl");
    let vulkan = get_system_property("ro.hardware.vulkan");
    let board = get_system_property("ro.board.platform");

    let hint = egl
        .as_deref()
        .or(vulkan.as_deref())
        .or(board.as_deref())
        .unwrap_or("Mobile GPU");
    let lower = hint.to_ascii_lowercase();

    let (vendor, vendor_name, display_name) = if lower.contains("adreno") || lower.contains("qcom")
    {
        (
            GpuVendor::Other,
            "Qualcomm".to_string(),
            format!("Qualcomm Adreno ({hint})"),
        )
    } else if lower.contains("mali") {
        (
            GpuVendor::Other,
            "ARM".to_string(),
            format!("ARM Mali ({hint})"),
        )
    } else if lower.contains("powervr") {
        (
            GpuVendor::Other,
            "Imagination".to_string(),
            format!("PowerVR ({hint})"),
        )
    } else {
        (
            GpuVendor::Other,
            "Android".to_string(),
            format!("Integrated GPU ({hint})"),
        )
    };

    let driver = match (egl, vulkan) {
        (Some(e), Some(v)) if e != v => MetricValue::Available(format!("EGL: {e} / Vulkan: {v}")),
        (Some(e), _) => MetricValue::Available(format!("EGL: {e}")),
        (_, Some(v)) => MetricValue::Available(format!("Vulkan: {v}")),
        _ => MetricValue::Unavailable("GPU driver property unavailable".to_string()),
    };

    let restricted = "Restricted on unprivileged Android".to_string();
    vec![GpuTelemetry {
        id: "android:gpu0".to_string(),
        name: display_name,
        vendor,
        vendor_name,
        pci_slot: None,
        driver_version: driver,
        utilization_percent: MetricValue::PermissionError(restricted.clone()),
        vram_total_bytes: MetricValue::Unsupported("Shared system memory".to_string()),
        vram_used_bytes: MetricValue::Unsupported("Shared system memory".to_string()),
        vram_free_bytes: MetricValue::Unsupported("Shared system memory".to_string()),
        vram_utilization_percent: MetricValue::Unsupported("Shared system memory".to_string()),
        temperature_celsius: MetricValue::PermissionError(restricted.clone()),
        core_clock_mhz: MetricValue::PermissionError(restricted.clone()),
        memory_clock_mhz: MetricValue::Unsupported("Not exposed on Android".to_string()),
        power_draw_watts: MetricValue::Unsupported(
            "Not exposed via unprivileged sysfs".to_string(),
        ),
        power_limit_watts: MetricValue::Unsupported(
            "Not exposed via unprivileged sysfs".to_string(),
        ),
        fan_speed_percent: MetricValue::Unsupported("Not applicable".to_string()),
    }]
}

pub fn collect_android_storage() -> Vec<StorageDeviceTelemetry> {
    let candidates = [
        ("/data", "Internal Storage", "userdata"),
        ("/storage/emulated/0", "User Storage", "fuse/sdcardfs"),
    ];

    let mut seen_fsid: HashSet<(u64, u64)> = HashSet::new();
    let mut results = Vec::new();

    for (mount_point, device_label, fs_label) in candidates {
        let Ok(c_path) = CString::new(mount_point) else {
            continue;
        };
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
        if rc != 0 {
            continue;
        }

        let block_size = if stat.f_frsize > 0 {
            stat.f_frsize as u64
        } else {
            stat.f_bsize as u64
        };
        let total = (stat.f_blocks as u64).saturating_mul(block_size);
        if total == 0 {
            continue;
        }
        let available = (stat.f_bavail as u64).saturating_mul(block_size);
        let dev_id = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(mount_point).map(|m| m.dev()).unwrap_or(0)
        };
        let key = (total, dev_id);
        if !seen_fsid.insert(key) {
            continue;
        }

        let free_root = (stat.f_bfree as u64).saturating_mul(block_size);
        let used = total.saturating_sub(free_root);
        let pct = ((used as f64) / (total as f64)) * 100.0;

        results.push(StorageDeviceTelemetry {
            device: device_label.to_string(),
            mount_point: mount_point.to_string(),
            filesystem: fs_label.to_string(),
            total_bytes: MetricValue::Available(total),
            used_bytes: MetricValue::Available(used),
            available_bytes: MetricValue::Available(available),
            utilization_percent: MetricValue::Available(pct.clamp(0.0, 100.0)),
        });
    }

    results
}
