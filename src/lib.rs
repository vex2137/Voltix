pub mod app;
pub mod control_center;
pub mod custom_theme;
pub mod diagnostics;
pub mod localization;
pub mod mangohud;
pub mod models;
pub mod monitoring;
pub mod overlay;
pub mod platform;
pub mod process_ui;
pub mod settings;
pub mod utils;

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: slint::android::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let config_path = app.internal_data_path().map(|p| p.join("settings.json"));

    if let Err(err) = slint::android::init(app) {
        log::error!("Failed to initialize Slint Android backend: {err}");
        return;
    }

    if let Err(err) = app::run(config_path) {
        log::error!("Voltix Android exited with UI error: {err}");
    }
}
