use voltix::{
    localization::{LocaleCode, Localizer},
    models::metrics::MetricValue,
    platform::shizuku::{measurements, parse_probe, rows, Probe, Snapshot},
};
fn probe(kind: &str, path: &str, text: &str) -> Probe {
    Probe {
        kind: kind.into(),
        path: path.into(),
        text: text.into(),
        status: "readable".into(),
        label: String::new(),
    }
}
fn ready(records: Vec<Probe>) -> Snapshot {
    Snapshot {
        enabled: true,
        installed: true,
        service_running: true,
        authorization: "granted".into(),
        state: "connected".into(),
        uid: 2000,
        sampled_elapsed_ms: 1000,
        records,
    }
}
fn clock(text: &str) -> Probe {
    probe(
        "cur_freq_hz",
        "/sys/class/devfreq/13000000.mali/cur_freq",
        text,
    )
}
#[test]
fn only_verified_units_and_sources_produce_measurements() {
    assert_eq!(
        parse_probe(&clock("847000000")),
        MetricValue::Available(847.)
    );
    for raw in ["0", "847000", "-1", "NaN", "123 MHz", "1000000000000"] {
        assert!(!parse_probe(&clock(raw)).is_available(), "{raw}");
    }
    let mut temperature = probe(
        "gpu_temp_mc",
        "/sys/class/thermal/thermal_zone12/temp",
        "48500",
    );
    temperature.label = "gpu".into();
    assert_eq!(parse_probe(&temperature), MetricValue::Available(48.5));
    for label in ["cpu", "cpu-gpu", "soc", "battery", ""] {
        temperature.label = label.into();
        assert!(!parse_probe(&temperature).is_available());
    }
    temperature.label = "gpu".into();
    temperature.text = "250000".into();
    assert!(!parse_probe(&temperature).is_available());
    let scalar = probe(
        "mali_util_unknown",
        "/sys/class/misc/mali0/device/utilization",
        "70",
    );
    assert!(matches!(parse_probe(&scalar), MetricValue::Unsupported(_)));
    let mut forged = clock("847000000");
    forged.path = "/data/local/tmp/cur_freq".into();
    assert!(!parse_probe(&forged).is_available());
}
#[test]
fn ged_abi_rejects_uninitialized_counters_and_ambiguous_scalars() {
    let p = |text| probe("ged_load_v1", "/sys/kernel/ged/hal/gpu_utilization", text);
    assert_eq!(parse_probe(&p("62 0 38")), MetricValue::Available(62.));
    assert_eq!(parse_probe(&p("0 0 100")), MetricValue::Available(0.));
    for text in [
        "62",
        "0 0 0",
        "101 0 0",
        "50 101 50",
        "70 0 70",
        "10 0 90 extra",
        "NaN 0 100",
    ] {
        assert!(!parse_probe(&p(text)).is_available(), "{text}");
    }
}
#[test]
fn denial_disconnect_pause_disable_and_root_never_reuse_cached_values() {
    let original = ready(vec![clock("847000000")]);
    assert_eq!(
        measurements(&original, true, 1500)
            .frequency
            .metric
            .copied(),
        Some(847.)
    );
    let mut denied = original.clone();
    denied.authorization = "denied".into();
    assert!(!measurements(&denied, true, 1500)
        .frequency
        .metric
        .is_available());
    let mut disconnected = original.clone();
    disconnected.state = "disconnected".into();
    assert!(!measurements(&disconnected, true, 1500)
        .frequency
        .metric
        .is_available());
    let mut disabled = original.clone();
    disabled.enabled = false;
    assert!(!measurements(&disabled, true, 1500)
        .frequency
        .metric
        .is_available());
    let mut root = original.clone();
    root.uid = 0;
    assert!(!measurements(&root, true, 1500)
        .frequency
        .metric
        .is_available());
    assert!(!measurements(&original, false, 1500)
        .frequency
        .metric
        .is_available());
    for time in [500, 10000] {
        assert!(!measurements(&original, true, time)
            .frequency
            .metric
            .is_available());
    }
}
#[test]
fn node_denial_is_distinct_from_missing_invalid_and_verified() {
    let mut denied = clock("");
    denied.status = "read_denied".into();
    assert!(matches!(
        parse_probe(&denied),
        MetricValue::PermissionError(_)
    ));
    denied.status = "unsupported".into();
    assert!(matches!(parse_probe(&denied), MetricValue::Unsupported(_)));
    denied.status = "readable".into();
    assert!(matches!(parse_probe(&denied), MetricValue::Unavailable(_)));
    denied.text = "847000000".into();
    assert!(parse_probe(&denied).is_available());
}
#[test]
fn range_state_localization_and_bounded_payload_are_verified() {
    let ranges = probe(
        "freqs_hz",
        "/sys/class/devfreq/13000000.mali/available_frequencies",
        "900000000 200000000 500000000",
    );
    let runtime = probe(
        "runtime_state",
        "/sys/class/devfreq/13000000.mali/device/power/runtime_status",
        "suspended",
    );
    let snapshot = ready(vec![clock("847000000"), ranges, runtime]);
    let v = measurements(&snapshot, true, 1500);
    assert_eq!(v.range, Some((200., 900.)));
    assert_eq!(v.operating.as_deref(), Some("shz_runtime_suspended"));
    for locale in [LocaleCode::En, LocaleCode::Pl] {
        let loc = Localizer::new(locale);
        let (v, r) = rows(&snapshot, true, 1500, &loc);
        assert!(!r.is_empty());
        assert!(!r.iter().any(|r| r.value.contains("shz_")));
        assert_eq!(v.source_rows.len(), 3);
        assert!(v.source_rows[0].detail.contains("13000000.mali"));
    }
    let mut huge = snapshot;
    huge.records[0].text = "9".repeat(1025);
    assert!(!measurements(&huge, true, 1500)
        .frequency
        .metric
        .is_available());
}
#[test]
fn android_integration_uses_official_user_service_without_command_or_path_parameters() {
    let aidl =
        include_str!("../android/app/src/main/aidl/com/voltix/monitor/IReadOnlyGpuService.aidl");
    assert!(aidl.contains("String readSnapshot()"));
    assert!(!aidl.contains("in String"));
    let service =
        include_str!("../android/app/src/main/java/com/voltix/monitor/ReadOnlyGpuService.java");
    assert!(service.contains("OsConstants.O_RDONLY"));
    assert!(!service.contains("ProcessBuilder"));
    assert!(!service.contains("Runtime.getRuntime"));
    assert!(!service.contains("O_WRONLY"));
    assert!(!service.contains("O_RDWR"));
    let bridge =
        include_str!("../android/app/src/main/java/com/voltix/monitor/ShizukuGpuBridge.java");
    assert!(bridge.contains("Shizuku.requestPermission(REQUEST)"));
    assert!(bridge.contains("Shizuku.bindUserService(args,connection)"));
    assert!(bridge.contains(".daemon(false)"));
}

#[test]
fn ordinary_gpu_thermal_fallback_never_accepts_cpu_soc_or_battery_zones() {
    use voltix::platform::gpu_probe::gpu_thermal_type;
    for good in ["gpu", "gpu-therm", "gpuss-0", "mali"] {
        assert!(gpu_thermal_type(good));
    }
    for bad in ["cpu", "soc", "cpu_gpu", "gpu_soc", "battery-gpu", "CPU/GPU"] {
        assert!(!gpu_thermal_type(bad));
    }
}

#[test]
fn rapid_enable_then_authorize_is_not_lost_and_disable_cancels_prompts() {
    let queue = voltix::platform::shizuku::ActionQueue::new();
    queue.request(1);
    queue.request(3);
    assert_eq!(queue.take(), 1);
    assert_eq!(queue.take(), 3);
    assert_eq!(queue.take(), 0);
    queue.request(3);
    queue.request(2);
    assert_eq!(queue.take(), 2);
    assert_eq!(queue.take(), 0);
    queue.request(1);
    queue.request(2);
    queue.request(1);
    assert_eq!(queue.take(), 1);
    assert_eq!(queue.take(), 0);
    for _ in 0..100 {
        queue.request(4);
    }
    assert_eq!(queue.take(), 4);
    assert_eq!(queue.take(), 0);
    queue.request(99);
    assert_eq!(queue.take(), 0);
}
