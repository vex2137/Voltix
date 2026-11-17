//! Regression checks for the ordinary Android telemetry path and packaged surface.
use voltix::localization::{LocaleCode, Localizer};
#[test]
fn android_sources_have_no_privileged_bridge_or_dependency() {
    for source in [
        include_str!("../android/app/build.gradle.kts"),
        include_str!("../android/app/src/main/AndroidManifest.xml"),
        include_str!("../android/app/src/main/java/com/voltix/monitor/VoltixActivity.java"),
        include_str!("../src/platform/android_bridge.rs"),
        include_str!("../ui/app-window.slint"),
        include_str!("../ui/pages/gpu.slint"),
    ] {
        let source = source.to_ascii_lowercase();
        for marker in [
            "shizuku",
            "nativegpucommand",
            "readonlygpuservice",
            "interact_across_users_full",
            "shz_",
        ] {
            assert!(
                !source.contains(marker),
                "obsolete Android integration: {marker}"
            );
        }
    }
}
#[test]
fn gpu_identification_and_non_privileged_sources_remain_available() {
    let source = include_str!("../src/platform/android/mod.rs");
    for marker in [
        "fn probe_vulkan_gpu",
        "fn probe_egl_gpu",
        "libvulkan.so",
        "libEGL.so",
        "read_android_gpu_temperature()",
        "read_android_gpu_clock_mhz()",
        "read_android_gpu_utilization()",
    ] {
        assert!(
            source.contains(marker),
            "missing existing GPU source: {marker}"
        );
    }
    let activity =
        include_str!("../android/app/src/main/java/com/voltix/monitor/VoltixActivity.java");
    for marker in [
        "ACTION_BATTERY_CHANGED",
        "ActivityManager.MemoryInfo",
        "nativeSnapshot",
        "nativePollingInterval",
    ] {
        assert!(activity.contains(marker));
    }
}
#[test]
fn unavailable_explanation_is_short_and_localized_and_gpu_zones_are_safe() {
    for locale in [LocaleCode::En, LocaleCode::Pl] {
        let loc = Localizer::new(locale);
        let note = loc.tr("android_gpu_na_note");
        assert!(note.starts_with("N/A:"));
        assert!(note.chars().count() < 110);
        assert!(!note.contains("android_gpu_"));
    }
    for good in ["gpu", "gpu-therm", "gpuss-0", "mali"] {
        assert!(voltix::platform::gpu_probe::gpu_thermal_type(good));
    }
    for bad in ["cpu", "soc", "cpu_gpu", "gpu_soc", "battery-gpu"] {
        assert!(!voltix::platform::gpu_probe::gpu_thermal_type(bad));
    }
}
