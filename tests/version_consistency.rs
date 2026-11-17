#[test]
fn cargo_version_is_authoritative_and_metadata_is_consistent() {
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(version, "0.1.0-alpha.4");
    let desktop = include_str!("../packaging/linux/voltix.desktop");
    assert!(desktop
        .lines()
        .any(|line| line == format!("X-AppImage-Version={version}")));
    let gradle = include_str!("../android/app/build.gradle.kts");
    assert!(gradle.contains("../Cargo.toml"));
    assert!(!gradle.contains("versionName = \""));
    assert!(!include_str!("../ui/i18n/strings.slint").contains("alpha.3"));
    for raw in [
        include_str!("../locales/en.json"),
        include_str!("../locales/pl.json"),
    ] {
        let catalog: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert!(catalog.get("app_version").is_none());
    }
}
