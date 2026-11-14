use std::time::{Duration, Instant};

use voltix::localization::{LocaleCode, Localizer, resolve_locale};
use voltix::models::history::BoundedHistory;
use voltix::models::metrics::{MetricStatus, MetricValue};
use voltix::monitoring::cpu::{compute_utilization, parse_proc_cpuinfo, parse_proc_stat};
use voltix::monitoring::disk::parse_and_deduplicate_mounts;
use voltix::monitoring::memory::parse_proc_meminfo;
use voltix::monitoring::system::{parse_os_release, parse_proc_uptime};
use voltix::settings::{LanguagePreference, TemperatureUnit, UserSettings};
use voltix::utils::format::{celsius_to_fahrenheit, format_temperature};

#[test]
fn end_to_end_cpu_and_history_pipeline() {
    let stat_a = "cpu  1000 100 300 8600 0 0 0 0 0 0\ncpu0 500 50 150 4300 0 0 0 0 0 0\n";
    let stat_b = "cpu  1400 100 500 9000 0 0 0 0 0 0\ncpu0 750 50 250 4450 0 0 0 0 0 0\n";

    let p1 = parse_proc_stat(stat_a).expect("valid stat A");
    let p2 = parse_proc_stat(stat_b).expect("valid stat B");
    let overall = compute_utilization(p1.overall, p2.overall);
    assert_eq!(overall, MetricValue::Available(60.0));

    let t0 = Instant::now();
    let mut history = BoundedHistory::new(60);
    history.push(t0, overall.copied());
    history.push(t0 + Duration::from_secs(1), Some(65.0));

    let paths = history.build_chart_paths(
        t0 + Duration::from_secs(1),
        Duration::from_secs(60),
        600.0,
        140.0,
        0.0,
        100.0,
        Duration::from_secs(3),
    );
    assert!(paths.has_data);
    assert_eq!(paths.min_value, Some(60.0));
    assert_eq!(paths.max_value, Some(65.0));
}

#[test]
fn memory_disk_system_and_localization_integration() {
    let mem = parse_proc_meminfo(
        "MemTotal: 16000000 kB\nMemAvailable: 8000000 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n",
    );
    assert_eq!(mem.utilization_percent, MetricValue::Available(50.0));
    assert_eq!(mem.swap_utilization_percent.status(), MetricStatus::Unsupported);

    let mounts = parse_and_deduplicate_mounts(
        "/dev/nvme0n1p2 /home btrfs rw 0 0\n/dev/nvme0n1p2 / btrfs rw 0 0\n",
    );
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].mount_point, "/");

    let cpu_info = parse_proc_cpuinfo("processor : 0\nmodel name : Test CPU\n");
    assert_eq!(cpu_info.logical_cpus, MetricValue::Available(1));

    assert_eq!(parse_os_release("PRETTY_NAME=\"CachyOS\"").as_deref(), Some("CachyOS"));
    assert_eq!(parse_proc_uptime("42.9 10.0"), Some(42));

    let mut settings = UserSettings {
        polling_interval_ms: 100,
        ..UserSettings::default()
    };
    settings.validate();
    assert_eq!(settings.polling_interval_ms, 250);

    assert!((celsius_to_fahrenheit(20.0) - 68.0).abs() < 1e-6);
    assert_eq!(format_temperature(20.0, TemperatureUnit::Fahrenheit), "68.0 °F");

    let loc = Localizer::new(resolve_locale(LanguagePreference::Polish));
    assert_eq!(loc.locale(), LocaleCode::Pl);
    assert_eq!(loc.tr("nav_settings"), "Ustawienia");
}
