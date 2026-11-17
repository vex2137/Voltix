use voltix::monitoring::extended::{
    parse_diskstats, parse_net_dev, parse_process, rate, Battery, Platform,
};
#[test]
fn missing_android_metrics_remain_optional_and_platform_status_is_not_a_percentage() {
    let p:Platform=serde_json::from_str(r#"{"foreground":true,"elapsed_ms":12000,"battery":{"percent":null,"health":2,"current_a":null,"voltage_v":4.2},"received":null,"transmitted":null}"#).unwrap();
    let b = p.battery.unwrap();
    assert_eq!(b.percent, None);
    assert_eq!(b.health, 2);
    assert_eq!(b.current_a, None);
    assert_eq!(b.power_w, None);
    assert_eq!(p.received, None);
    let text = serde_json::to_string(&b).unwrap();
    assert!(!text.contains("capacity"));
    assert!(!text.contains("cycle"));
}
#[test]
fn counters_use_elapsed_time_and_never_convert_resets_to_spikes() {
    assert_eq!(rate(100, 600, 2.5), Some(200.0));
    assert_eq!(rate(600, 5, 1.0), None);
    assert_eq!(rate(2, 2, 1.0), Some(0.0));
    assert_eq!(rate(2, 3, f64::INFINITY), None);
    assert!(parse_net_dev("bad data").is_empty());
    assert!(parse_diskstats("short 1").is_empty());
    assert!(parse_process(1, "invalid", 4096).is_none());
}
#[test]
fn battery_power_is_only_present_when_platform_supplies_it() {
    let b: Battery = serde_json::from_str(
        r#"{"voltage_v":4.1,"current_a":-0.5,"power_w":-2.05,"status":3,"percent":73.0}"#,
    )
    .unwrap();
    assert_eq!(b.power_w, Some(-2.05));
    assert_eq!(b.status, 3);
    assert_eq!(Battery::default().percent, None);
}
#[test]
fn exported_files_are_unique_and_preserve_previous_data() {
    let dir = std::env::temp_dir().join(format!("voltix-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = voltix::control_center::save_export(&dir, "{\"sample\":1}").unwrap();
    let b = voltix::control_center::save_export(&dir, "{\"sample\":2}").unwrap();
    assert_ne!(a, b);
    assert_eq!(std::fs::read_to_string(a).unwrap(), "{\"sample\":1}");
    assert_eq!(std::fs::read_to_string(b).unwrap(), "{\"sample\":2}");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn old_settings_keep_their_values_and_new_navigation_defaults_are_valid() {
    let s=voltix::settings::UserSettings::from_json_str(r#"{"theme":"light","language":"polish","polling_interval_ms":500,"last_page":999,"history_source":99}"#).unwrap();
    assert_eq!(s.last_page, 13);
    assert_eq!(s.history_source, 9);
    assert_eq!(s.polling_interval_ms, 500);
    assert_eq!(s.language, voltix::settings::LanguagePreference::Polish);
}
