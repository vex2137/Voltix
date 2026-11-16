use voltix::{
    control_center::android_permission_rows,
    localization::{LocaleCode, Localizer},
    monitoring::extended::Platform,
};

#[test]
fn permissions_are_separate_from_gpu_support_and_initial_sampling() {
    for code in [LocaleCode::En, LocaleCode::Pl] {
        let loc = Localizer::new(code);
        let pending = android_permission_rows(&Platform::default(), &loc);
        assert_eq!(pending[0].value.as_str(), loc.tr("hotfix_not_required"));
        assert_eq!(pending[1].value.as_str(), loc.tr("status_sampling"));
        let rows = android_permission_rows(
            &Platform {
                elapsed_ms: 42,
                network_permission: true,
                ..Default::default()
            },
            &loc,
        );
        assert_eq!(rows[1].value.as_str(), loc.tr("cc_granted"));
        assert!(rows[1].detail.contains("ACCESS_NETWORK_STATE"));
        assert_eq!(rows[0].detail.as_str(), loc.tr("cc_permission_note"));
        assert!(!loc.tr("hotfix_gpu_policy_detail").contains("hotfix_"));
    }
}

#[test]
fn android_gpu_diagnostic_templates_translate_without_losing_source_paths() {
    let en = Localizer::new(LocaleCode::En);
    let pl = Localizer::new(LocaleCode::Pl);
    let cases=[
        "Device/driver does not expose /sys/class/misc/mali0/device/utilization",
        "Permission denied: /sys/gpu. Android sandbox or SELinux policy may restrict this node; the cause cannot be identified from EACCES alone.",
        "Temporarily unavailable: /sys/gpu",
        "Sensor read failure: /sys/gpu: I/O error (os error 5)",
        "Temporarily unavailable: counter interval is zero",
        "Sensor read failure: invalid KGSL busy/total counters",
        "Mali utilization interface exists, but its units and sampling semantics are not verified for this driver",
        "Sensor read failure: invalid GPU frequency in /sys/gpu",
        "Temporarily unavailable: GPU clock is zero (possibly idle)",
        "Shared unified system memory (UMA); no dedicated VRAM",
        "Permission denied reading thermal sysfs; Android sandbox or SELinux may restrict access",
        "No readable thermal zone",
        "Not exposed via unprivileged sysfs",
    ];
    for input in cases {
        assert_eq!(en.diagnostic(input), input);
        let translated = pl.diagnostic(input);
        assert_ne!(translated, input, "missing Polish diagnostic: {input}");
        if input.contains("/sys/") {
            assert!(translated.contains("/sys/"));
        }
        assert!(!translated.contains("{0}"));
    }
}
