use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::localization::{resolve_locale, Localizer};
use crate::models::history::BoundedHistory;
use crate::models::metrics::{MetricStatus, MetricValue};
use crate::models::telemetry::{
    CpuTelemetry, CpuUtilizationScope, GpuTelemetry, MemoryTelemetry, StorageDeviceTelemetry,
    TelemetrySnapshot,
};
use crate::monitoring::MonitoringEngine;
use crate::platform::detect_system_dark_mode;
use crate::settings::{LanguagePreference, TemperatureUnit, ThemePreference, UserSettings};
use crate::utils::format::{
    format_bytes_binary, format_frequency_mhz, format_temperature, format_uptime_compact,
};

slint::include_modules!();

const HISTORY_CAPACITY: usize = 240;
const CHART_WINDOW: Duration = Duration::from_secs(60);
const CHART_VIEWBOX_W: f64 = 600.0;
const CHART_VIEWBOX_H: f64 = 140.0;

#[derive(Default)]
struct RenderCache {
    cpu_line: String,
    cpu_area: String,
    cpu_has: Option<bool>,
    ram_line: String,
    ram_area: String,
    ram_has: Option<bool>,
    gpu_line: String,
    gpu_area: String,
    gpu_has: Option<bool>,
    gpu_empty: String,
    sys_gpus: Option<Vec<(String, String, String)>>,
}

struct SharedState {
    shutdown: bool,
    settings: UserSettings,
    config_path: Option<PathBuf>,
    localizer: Localizer,
    selected_gpu_index: usize,
    latest_snapshot: Option<TelemetrySnapshot>,
    last_cpu_scope: CpuUtilizationScope,
    cpu_history: BoundedHistory,
    ram_history: BoundedHistory,
    gpu_histories: HashMap<String, BoundedHistory>,
    render_cache: RenderCache,
}

// Histories for disconnected devices are useless once they leave the chart
// window. Keeping them forever would grow the map across GPU hotplug events.
fn prune_stale_gpu_histories(
    histories: &mut HashMap<String, BoundedHistory>,
    now: std::time::Instant,
) {
    let retention = CHART_WINDOW + Duration::from_secs(5);
    histories.retain(|_, history| {
        history
            .samples()
            .back()
            .is_some_and(|last| now.saturating_duration_since(last.timestamp) <= retention)
    });
}

fn update_vec_model<T: Clone + PartialEq + 'static>(model: ModelRc<T>, items: Vec<T>) {
    if let Some(vec_model) = model.as_any().downcast_ref::<VecModel<T>>() {
        // Keep the existing model and its unchanged rows alive. A full reset causes
        // Slint to rebuild every repeated component, even when one value changed.
        let old_count = vec_model.row_count();
        let new_count = items.len();
        for (index, item) in items.into_iter().enumerate() {
            if index >= old_count {
                vec_model.push(item);
            } else if vec_model.row_data(index).as_ref() != Some(&item) {
                vec_model.set_row_data(index, item);
            }
        }
        for index in (new_count..old_count).rev() {
            vec_model.remove(index);
        }
    }
}

pub fn run(config_override: Option<PathBuf>) -> Result<(), slint::PlatformError> {
    crate::diagnostics::mark("application initialization begins");
    let config_path = config_override.or_else(UserSettings::config_file_path);
    let settings = UserSettings::load_or_default(config_path.as_deref());
    let locale = resolve_locale(settings.language);
    let localizer = Localizer::new(locale);

    let window = AppWindow::new()?;
    crate::diagnostics::mark("Slint component initialized (not presented)");

    window.set_cpu_core_rows(ModelRc::from(Rc::new(VecModel::default())));
    window.set_gpus(ModelRc::from(Rc::new(VecModel::default())));
    window.set_disks(ModelRc::from(Rc::new(VecModel::default())));
    window.set_system_gpus(ModelRc::from(Rc::new(VecModel::default())));

    let shared = Arc::new(Mutex::new(SharedState {
        shutdown: false,
        settings: settings.clone(),
        config_path: config_path.clone(),
        localizer,
        selected_gpu_index: 0,
        latest_snapshot: None,
        last_cpu_scope: CpuUtilizationScope::Unavailable,
        cpu_history: BoundedHistory::new(HISTORY_CAPACITY),
        ram_history: BoundedHistory::new(HISTORY_CAPACITY),
        gpu_histories: HashMap::new(),
        render_cache: RenderCache::default(),
    }));
    let condvar = Arc::new(Condvar::new());

    // Apply initial settings, theme, and translations
    if let Ok(mut state) = shared.lock() {
        apply_theme_to_window(&window, state.settings.theme);
        apply_i18n_to_window(&window, &state.localizer);
        apply_settings_to_window(&window, &state.settings, state.config_path.as_deref());
        window.set_cpu_header_label(SharedString::from(state.localizer.tr("header_cpu_short")));
        window.set_dash_cpu_card_label(SharedString::from(state.localizer.tr("dash_cpu_card")));
        window.set_dash_cpu_chart_title(SharedString::from(state.localizer.tr("dash_cpu_history")));
        window.set_cpu_chart_title(SharedString::from(state.localizer.tr("cpu_history_title")));
        window.set_selected_gpu(empty_gpu_item(&state.localizer));
        let sampling_msg = state.localizer.tr("status_sampling").to_string();
        window.set_gpu_chart_empty_text(SharedString::from(sampling_msg.as_str()));
        state.render_cache.gpu_empty = sampling_msg;
    }

    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_change_interface(move |mode| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.interface_mode = match mode {
                    1 => crate::settings::InterfaceMode::Professional,
                    2 => crate::settings::InterfaceMode::Gaming,
                    _ => crate::settings::InterfaceMode::Modern,
                };
                persist_settings(&state);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
                render_snapshot_to_window(&win, &mut state);
            }
        });
    }
    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_change_accent(move |index| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.accent = index.clamp(0, 3) as u8;
                state.settings.palette = crate::settings::PalettePreset::VoltixDefault;
                persist_settings(&state);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
            }
        });
    }

    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_change_palette(move |index| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.palette = match index {
                    1 => crate::settings::PalettePreset::Cyberpunk,
                    2 => crate::settings::PalettePreset::Emerald,
                    3 => crate::settings::PalettePreset::Minimal,
                    _ => crate::settings::PalettePreset::VoltixDefault,
                };
                // Built-in palettes describe dark surfaces. Default restores the
                // existing theme preference rather than silently changing it.
                persist_settings(&state);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
            }
        });
    }

    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_page_changed(move || {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                render_snapshot_to_window(&win, &mut state);
            }
        });
    }

    // Wire UI callbacks
    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_select_gpu(move |idx| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.selected_gpu_index = idx.max(0) as usize;
                render_snapshot_to_window(&win, &mut state);
            }
        });
    }

    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_change_theme(move |mode| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.palette = crate::settings::PalettePreset::VoltixDefault;
                state.settings.theme = match mode {
                    0 => ThemePreference::System,
                    2 => ThemePreference::Light,
                    _ => ThemePreference::Dark,
                };
                persist_settings(&state);
                apply_theme_to_window(&win, state.settings.theme);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
            }
        });
    }

    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_change_language(move |lang_idx| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.language = match lang_idx {
                    1 => LanguagePreference::English,
                    2 => LanguagePreference::Polish,
                    _ => LanguagePreference::System,
                };
                let resolved = resolve_locale(state.settings.language);
                state.localizer.set_locale(resolved);
                persist_settings(&state);
                apply_i18n_to_window(&win, &state.localizer);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
                render_snapshot_to_window(&win, &mut state);
            }
        });
    }

    {
        let shared_c = shared.clone();
        let weak = window.as_weak();
        window.on_change_temp_unit(move |unit_idx| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.temperature_unit = match unit_idx {
                    1 => TemperatureUnit::Fahrenheit,
                    _ => TemperatureUnit::Celsius,
                };
                persist_settings(&state);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
                render_snapshot_to_window(&win, &mut state);
            }
        });
    }

    {
        let shared_c = shared.clone();
        let condvar_c = condvar.clone();
        let weak = window.as_weak();
        window.on_change_polling_ms(move |ms| {
            if let (Ok(mut state), Some(win)) = (shared_c.lock(), weak.upgrade()) {
                state.settings.polling_interval_ms = ms.max(250) as u64;
                state.settings.validate();
                persist_settings(&state);
                apply_settings_to_window(&win, &state.settings, state.config_path.as_deref());
                condvar_c.notify_all();
            }
        });
    }

    // Spawn background monitoring thread
    let monitor_handle = spawn_monitor_thread(shared.clone(), condvar.clone(), window.as_weak());

    crate::diagnostics::mark("entering UI event loop (not first frame)");
    let run_result = window.run();

    // Signal shutdown and join background monitoring thread cleanly
    if let Ok(mut state) = shared.lock() {
        state.shutdown = true;
        condvar.notify_all();
    }
    if let Ok(handle) = monitor_handle {
        let _ = handle.join();
    }

    run_result
}

fn spawn_monitor_thread(
    shared: Arc<Mutex<SharedState>>,
    condvar: Arc<Condvar>,
    weak_window: slint::Weak<AppWindow>,
) -> std::io::Result<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name("voltix-monitor".to_string())
        .spawn(move || {
            let mut engine = MonitoringEngine::new();
            crate::diagnostics::mark("monitoring engine initialized");

            // Brief non-busy warmup so the initial /proc/stat delta has elapsed ticks
            if let Ok(guard) = shared.lock() {
                if !guard.shutdown {
                    let _ = condvar.wait_timeout(guard, Duration::from_millis(160));
                }
            }

            loop {
                let (should_stop, interval) = match shared.lock() {
                    Ok(guard) => (
                        guard.shutdown,
                        Duration::from_millis(guard.settings.polling_interval_ms),
                    ),
                    Err(_) => break,
                };
                if should_stop {
                    break;
                }

                let snapshot = engine.collect_snapshot();

                {
                    let Ok(mut state) = shared.lock() else {
                        break;
                    };
                    if state.shutdown {
                        break;
                    }
                    let ts = snapshot.sampled_at;
                    if state.last_cpu_scope != snapshot.cpu.utilization_scope {
                        state.cpu_history = BoundedHistory::new(HISTORY_CAPACITY);
                        state.last_cpu_scope = snapshot.cpu.utilization_scope;
                    }
                    let cpu_sample = match snapshot.cpu.utilization_scope {
                        CpuUtilizationScope::SystemWide => {
                            snapshot.cpu.overall_utilization.copied()
                        }
                        CpuUtilizationScope::ProcessOnly => {
                            snapshot.cpu.process_utilization.copied()
                        }
                        CpuUtilizationScope::Unavailable => None,
                    };
                    state.cpu_history.push(ts, cpu_sample);
                    state
                        .ram_history
                        .push(ts, snapshot.memory.utilization_percent.copied());

                    prune_stale_gpu_histories(&mut state.gpu_histories, ts);
                    for gpu in &snapshot.gpus {
                        let entry = state
                            .gpu_histories
                            .entry(gpu.id.clone())
                            .or_insert_with(|| BoundedHistory::new(HISTORY_CAPACITY));
                        entry.push(ts, gpu.utilization_percent.copied());
                    }

                    state.latest_snapshot = Some(snapshot);
                }

                let shared_ui = shared.clone();
                let weak_ui = weak_window.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let (Ok(mut state), Some(win)) = (shared_ui.lock(), weak_ui.upgrade()) {
                        render_snapshot_to_window(&win, &mut state);
                        crate::diagnostics::first_sample();
                    }
                });

                let Ok(guard) = shared.lock() else {
                    break;
                };
                if guard.shutdown {
                    break;
                }
                let _ = condvar.wait_timeout(guard, interval);
            }
        })
}

fn persist_settings(state: &SharedState) {
    if let Some(path) = &state.config_path {
        if let Err(err) = state.settings.save_to(path) {
            log::warn!("Failed to persist settings: {err}");
        }
    }
}

fn apply_theme_to_window(window: &AppWindow, pref: ThemePreference) {
    let is_dark = match pref {
        ThemePreference::Dark => true,
        ThemePreference::Light => false,
        ThemePreference::System => detect_system_dark_mode(),
    };
    window.global::<Theme>().set_dark_mode(is_dark);
}

fn apply_settings_to_window(
    window: &AppWindow,
    settings: &UserSettings,
    config_path: Option<&std::path::Path>,
) {
    window
        .global::<Theme>()
        .set_interface_mode(match settings.interface_mode {
            crate::settings::InterfaceMode::Modern => 0,
            crate::settings::InterfaceMode::Professional => 1,
            crate::settings::InterfaceMode::Gaming => 2,
        });
    window
        .global::<Theme>()
        .set_palette(match settings.palette {
            crate::settings::PalettePreset::VoltixDefault => 0,
            crate::settings::PalettePreset::Cyberpunk => 1,
            crate::settings::PalettePreset::Emerald => 2,
            crate::settings::PalettePreset::Minimal => 3,
        });
    window
        .global::<Theme>()
        .set_accent_index(settings.accent as i32);
    window
        .global::<Theme>()
        .set_polish(resolve_locale(settings.language) == crate::localization::LocaleCode::Pl);
    window.set_theme_mode(match settings.theme {
        ThemePreference::System => 0,
        ThemePreference::Dark => 1,
        ThemePreference::Light => 2,
    });
    window.set_language_mode(match settings.language {
        LanguagePreference::System => 0,
        LanguagePreference::English => 1,
        LanguagePreference::Polish => 2,
    });
    window.set_temp_unit(match settings.temperature_unit {
        TemperatureUnit::Celsius => 0,
        TemperatureUnit::Fahrenheit => 1,
    });
    window.set_polling_ms(settings.polling_interval_ms as i32);
    window.set_polling_badge_text(SharedString::from(format!(
        "{} ms",
        settings.polling_interval_ms
    )));
    window.set_config_path(SharedString::from(
        config_path
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "-".to_string()),
    ));
}

fn apply_i18n_to_window(window: &AppWindow, loc: &Localizer) {
    let i18n = window.global::<I18n>();
    i18n.set_app_name(SharedString::from(loc.tr("app_name")));
    i18n.set_app_version(SharedString::from(loc.tr("app_version")));
    i18n.set_nav_dashboard(SharedString::from(loc.tr("nav_dashboard")));
    i18n.set_nav_cpu(SharedString::from(loc.tr("nav_cpu")));
    i18n.set_nav_gpu(SharedString::from(loc.tr("nav_gpu")));
    i18n.set_nav_memory(SharedString::from(loc.tr("nav_memory")));
    i18n.set_nav_storage(SharedString::from(loc.tr("nav_storage")));
    i18n.set_nav_system(SharedString::from(loc.tr("nav_system")));
    i18n.set_nav_settings(SharedString::from(loc.tr("nav_settings")));

    i18n.set_nav_short_dashboard(SharedString::from(loc.tr("nav_short_dashboard")));
    i18n.set_nav_short_cpu(SharedString::from(loc.tr("nav_short_cpu")));
    i18n.set_nav_short_gpu(SharedString::from(loc.tr("nav_short_gpu")));
    i18n.set_nav_short_memory(SharedString::from(loc.tr("nav_short_memory")));
    i18n.set_nav_short_storage(SharedString::from(loc.tr("nav_short_storage")));
    i18n.set_nav_short_system(SharedString::from(loc.tr("nav_short_system")));
    i18n.set_nav_short_settings(SharedString::from(loc.tr("nav_short_settings")));

    i18n.set_status_polling(SharedString::from(loc.tr("status_polling")));
    i18n.set_status_unavailable(SharedString::from(loc.tr("status_unavailable")));
    i18n.set_status_unsupported(SharedString::from(loc.tr("status_unsupported")));
    i18n.set_status_permission_denied(SharedString::from(loc.tr("status_permission_denied")));
    i18n.set_status_sampling(SharedString::from(loc.tr("status_sampling")));
    i18n.set_status_no_data(SharedString::from(loc.tr("status_no_data")));

    i18n.set_header_cpu_short(SharedString::from(loc.tr("header_cpu_short")));
    i18n.set_header_app_cpu_short(SharedString::from(loc.tr("header_app_cpu_short")));
    i18n.set_header_subtitle_dashboard(SharedString::from(loc.tr("header_subtitle_dashboard")));
    i18n.set_header_subtitle_cpu(SharedString::from(loc.tr("header_subtitle_cpu")));
    i18n.set_header_subtitle_gpu(SharedString::from(loc.tr("header_subtitle_gpu")));
    i18n.set_header_subtitle_memory(SharedString::from(loc.tr("header_subtitle_memory")));
    i18n.set_header_subtitle_storage(SharedString::from(loc.tr("header_subtitle_storage")));
    i18n.set_header_subtitle_system(SharedString::from(loc.tr("header_subtitle_system")));
    i18n.set_header_subtitle_settings(SharedString::from(loc.tr("header_subtitle_settings")));

    i18n.set_dash_cpu_card(SharedString::from(loc.tr("dash_cpu_card")));
    i18n.set_dash_cpu_app_card(SharedString::from(loc.tr("dash_cpu_app_card")));
    i18n.set_dash_cpu_sys_restricted(SharedString::from(loc.tr("dash_cpu_sys_restricted")));
    i18n.set_dash_gpu_card(SharedString::from(loc.tr("dash_gpu_card")));
    i18n.set_dash_ram_card(SharedString::from(loc.tr("dash_ram_card")));
    i18n.set_dash_vram_card(SharedString::from(loc.tr("dash_vram_card")));
    i18n.set_dash_uptime_card(SharedString::from(loc.tr("dash_uptime_card")));
    i18n.set_dash_cpu_history(SharedString::from(loc.tr("dash_cpu_history")));
    i18n.set_dash_cpu_app_history(SharedString::from(loc.tr("dash_cpu_app_history")));
    i18n.set_dash_ram_history(SharedString::from(loc.tr("dash_ram_history")));
    i18n.set_dash_gpu_history(SharedString::from(loc.tr("dash_gpu_history")));

    i18n.set_cpu_model(SharedString::from(loc.tr("cpu_model")));
    i18n.set_cpu_vendor(SharedString::from(loc.tr("cpu_vendor")));
    i18n.set_cpu_arch(SharedString::from(loc.tr("cpu_arch")));
    i18n.set_cpu_overall_util(SharedString::from(loc.tr("cpu_overall_util")));
    i18n.set_cpu_process_util(SharedString::from(loc.tr("cpu_process_util")));
    i18n.set_cpu_frequency(SharedString::from(loc.tr("cpu_frequency")));
    i18n.set_cpu_temperature(SharedString::from(loc.tr("cpu_temperature")));
    i18n.set_cpu_cores_threads(SharedString::from(loc.tr("cpu_cores_threads")));
    i18n.set_cpu_per_core(SharedString::from(loc.tr("cpu_per_core")));
    i18n.set_cpu_history_title(SharedString::from(loc.tr("cpu_history_title")));
    i18n.set_cpu_app_history_title(SharedString::from(loc.tr("cpu_app_history_title")));

    i18n.set_gpu_selector(SharedString::from(loc.tr("gpu_selector")));
    i18n.set_gpu_none_detected(SharedString::from(loc.tr("gpu_none_detected")));
    i18n.set_gpu_name(SharedString::from(loc.tr("gpu_name")));
    i18n.set_gpu_vendor(SharedString::from(loc.tr("gpu_vendor")));
    i18n.set_gpu_driver(SharedString::from(loc.tr("gpu_driver")));
    i18n.set_gpu_utilization(SharedString::from(loc.tr("gpu_utilization")));
    i18n.set_gpu_vram(SharedString::from(loc.tr("gpu_vram")));
    i18n.set_gpu_temperature(SharedString::from(loc.tr("gpu_temperature")));
    i18n.set_gpu_core_clock(SharedString::from(loc.tr("gpu_core_clock")));
    i18n.set_gpu_mem_clock(SharedString::from(loc.tr("gpu_mem_clock")));
    i18n.set_gpu_power_draw(SharedString::from(loc.tr("gpu_power_draw")));
    i18n.set_gpu_power_limit(SharedString::from(loc.tr("gpu_power_limit")));
    i18n.set_gpu_fan_speed(SharedString::from(loc.tr("gpu_fan_speed")));
    i18n.set_gpu_history_title(SharedString::from(loc.tr("gpu_history_title")));

    i18n.set_mem_total(SharedString::from(loc.tr("mem_total")));
    i18n.set_mem_used(SharedString::from(loc.tr("mem_used")));
    i18n.set_mem_available(SharedString::from(loc.tr("mem_available")));
    i18n.set_mem_utilization(SharedString::from(loc.tr("mem_utilization")));
    i18n.set_mem_swap_total(SharedString::from(loc.tr("mem_swap_total")));
    i18n.set_mem_swap_used(SharedString::from(loc.tr("mem_swap_used")));
    i18n.set_mem_swap_utilization(SharedString::from(loc.tr("mem_swap_utilization")));
    i18n.set_mem_history_title(SharedString::from(loc.tr("mem_history_title")));

    i18n.set_storage_empty(SharedString::from(loc.tr("storage_empty")));
    i18n.set_storage_device(SharedString::from(loc.tr("storage_device")));
    i18n.set_storage_mount(SharedString::from(loc.tr("storage_mount")));
    i18n.set_storage_fs(SharedString::from(loc.tr("storage_fs")));
    i18n.set_storage_used(SharedString::from(loc.tr("storage_used")));
    i18n.set_storage_available(SharedString::from(loc.tr("storage_available")));
    i18n.set_storage_total(SharedString::from(loc.tr("storage_total")));

    i18n.set_sys_os_section(SharedString::from(loc.tr("sys_os_section")));
    i18n.set_sys_hw_section(SharedString::from(loc.tr("sys_hw_section")));
    i18n.set_sys_os_name(SharedString::from(loc.tr("sys_os_name")));
    i18n.set_sys_kernel(SharedString::from(loc.tr("sys_kernel")));
    i18n.set_sys_arch(SharedString::from(loc.tr("sys_arch")));
    i18n.set_sys_hostname(SharedString::from(loc.tr("sys_hostname")));
    i18n.set_sys_uptime(SharedString::from(loc.tr("sys_uptime")));
    i18n.set_sys_cpu(SharedString::from(loc.tr("sys_cpu")));
    i18n.set_sys_ram(SharedString::from(loc.tr("sys_ram")));
    i18n.set_sys_gpus(SharedString::from(loc.tr("sys_gpus")));

    i18n.set_settings_appearance(SharedString::from(loc.tr("settings_appearance")));
    i18n.set_settings_theme(SharedString::from(loc.tr("settings_theme")));
    i18n.set_settings_theme_system(SharedString::from(loc.tr("settings_theme_system")));
    i18n.set_settings_theme_dark(SharedString::from(loc.tr("settings_theme_dark")));
    i18n.set_settings_theme_light(SharedString::from(loc.tr("settings_theme_light")));
    i18n.set_settings_localization(SharedString::from(loc.tr("settings_localization")));
    i18n.set_settings_language(SharedString::from(loc.tr("settings_language")));
    i18n.set_settings_lang_system(SharedString::from(loc.tr("settings_lang_system")));
    i18n.set_settings_lang_en(SharedString::from(loc.tr("settings_lang_en")));
    i18n.set_settings_lang_pl(SharedString::from(loc.tr("settings_lang_pl")));
    i18n.set_settings_temp_unit(SharedString::from(loc.tr("settings_temp_unit")));
    i18n.set_settings_monitoring(SharedString::from(loc.tr("settings_monitoring")));
    i18n.set_settings_polling_interval(SharedString::from(loc.tr("settings_polling_interval")));
    i18n.set_settings_config_path(SharedString::from(loc.tr("settings_config_path")));
    i18n.set_settings_saved_auto(SharedString::from(loc.tr("settings_saved_auto")));

    i18n.set_chart_now(SharedString::from(loc.tr("chart_now")));
    i18n.set_chart_60s(SharedString::from(loc.tr("chart_60s")));
    i18n.set_chart_30s(SharedString::from(loc.tr("chart_30s")));
    i18n.set_diag_notice_title(SharedString::from(loc.tr("diag_notice_title")));
}

fn visible_charts(page: i32, mode: crate::settings::InterfaceMode) -> (bool, bool, bool) {
    use crate::settings::InterfaceMode;
    (
        page == 0 || page == 1,
        page == 3 || (page == 0 && mode == InterfaceMode::Modern),
        page == 0 || page == 2,
    )
}

fn render_snapshot_to_window(window: &AppWindow, state: &mut SharedState) {
    let Some(snap) = state.latest_snapshot.as_ref() else {
        return;
    };

    let loc = &state.localizer;
    let temp_unit = state.settings.temperature_unit;
    let max_gap = Duration::from_millis((state.settings.polling_interval_ms * 3).max(2500));

    // OS & System
    let (os_text, os_status) = format_metric_str(&snap.system.os_name, loc, |s| s.clone());
    window.set_os_name(os_text.clone());
    window.set_os_status(os_status);
    window.set_os_badge_text(if os_status == 0 {
        os_text
    } else if cfg!(target_os = "android") {
        SharedString::from("Android")
    } else {
        SharedString::from("Linux")
    });

    let (kernel_text, kernel_status) =
        format_metric_str(&snap.system.kernel_version, loc, |s| s.clone());
    window.set_kernel_text(kernel_text);
    window.set_kernel_status(kernel_status);

    let (host_text, host_status) = format_metric_str(&snap.system.hostname, loc, |s| s.clone());
    window.set_hostname(host_text);
    window.set_hostname_status(host_status);

    let (uptime_text, uptime_status) = format_metric_str(&snap.system.uptime_seconds, loc, |s| {
        format_uptime_compact(*s)
    });
    window.set_uptime_text(uptime_text);
    window.set_uptime_status(uptime_status);

    // CPU
    let (sys_util_text, sys_util_status) =
        format_metric_str(&snap.cpu.overall_utilization, loc, |v| format!("{v:.1}%"));
    window.set_cpu_sys_util_text(sys_util_text.clone());
    window.set_cpu_sys_util_percent(snap.cpu.overall_utilization.copied().unwrap_or(0.0) as f32);
    window.set_cpu_sys_util_status(sys_util_status);

    let (proc_util_text, proc_util_status) =
        format_metric_str(&snap.cpu.process_utilization, loc, |v| format!("{v:.1}%"));
    window.set_cpu_process_util_text(proc_util_text.clone());
    window
        .set_cpu_process_util_percent(snap.cpu.process_utilization.copied().unwrap_or(0.0) as f32);
    window.set_cpu_process_util_status(proc_util_status);

    let fmt_temp = |c: &f64| format_temperature(*c, temp_unit);
    let (cpu_temp_text, cpu_temp_status) =
        format_metric_str(&snap.cpu.temperature_celsius, loc, fmt_temp);
    window.set_cpu_temp_text(cpu_temp_text.clone());
    window.set_cpu_temp_status(cpu_temp_status);
    window.set_cpu_temp_source(SharedString::from(
        snap.cpu.temperature_source.clone().unwrap_or_default(),
    ));

    match snap.cpu.utilization_scope {
        CpuUtilizationScope::ProcessOnly => {
            window.set_cpu_header_label(SharedString::from(loc.tr("header_app_cpu_short")));
            window.set_dash_cpu_card_label(SharedString::from(loc.tr("dash_cpu_app_card")));
            window.set_dash_cpu_util_text(proc_util_text);
            window.set_dash_cpu_util_percent(
                snap.cpu.process_utilization.copied().unwrap_or(0.0) as f32
            );
            window.set_dash_cpu_util_status(proc_util_status);
            let secondary = if cpu_temp_status == 0 {
                format!("{} • {}", cpu_temp_text, loc.tr("dash_cpu_sys_restricted"))
            } else {
                loc.tr("dash_cpu_sys_restricted").to_string()
            };
            window.set_dash_cpu_secondary_text(SharedString::from(secondary));
            window.set_dash_cpu_chart_title(SharedString::from(loc.tr("dash_cpu_app_history")));
            window.set_cpu_chart_title(SharedString::from(loc.tr("cpu_app_history_title")));
        }
        CpuUtilizationScope::SystemWide | CpuUtilizationScope::Unavailable => {
            window.set_cpu_header_label(SharedString::from(loc.tr("header_cpu_short")));
            window.set_dash_cpu_card_label(SharedString::from(loc.tr("dash_cpu_card")));
            window.set_dash_cpu_util_text(sys_util_text);
            window.set_dash_cpu_util_percent(
                snap.cpu.overall_utilization.copied().unwrap_or(0.0) as f32
            );
            window.set_dash_cpu_util_status(sys_util_status);
            window.set_dash_cpu_secondary_text(if cpu_temp_status == 0 {
                cpu_temp_text
            } else {
                SharedString::default()
            });
            window.set_dash_cpu_chart_title(SharedString::from(loc.tr("dash_cpu_history")));
            window.set_cpu_chart_title(SharedString::from(loc.tr("cpu_history_title")));
        }
    }

    window.set_cpu_notice_text(SharedString::from(build_cpu_notice(&snap.cpu, loc)));

    let (cpu_model_text, cpu_model_status) =
        format_metric_str(&snap.cpu.model_name, loc, |s| s.clone());
    window.set_cpu_model(cpu_model_text);
    window.set_cpu_model_status(cpu_model_status);

    let (cpu_vendor_text, cpu_vendor_status) =
        format_metric_str(&snap.cpu.vendor, loc, |s| s.clone());
    window.set_cpu_vendor(cpu_vendor_text);
    window.set_cpu_vendor_status(cpu_vendor_status);
    window.set_cpu_arch(SharedString::from(snap.cpu.architecture.as_str()));

    let cores_threads = format!(
        "{} / {}",
        snap.cpu
            .physical_cores
            .copied()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string()),
        snap.cpu
            .logical_cpus
            .copied()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    window.set_cpu_cores_threads(SharedString::from(cores_threads));

    let (cpu_freq_text, cpu_freq_status) =
        format_metric_str(&snap.cpu.average_frequency_mhz, loc, |mhz| {
            format_frequency_mhz(*mhz)
        });
    window.set_cpu_freq_text(cpu_freq_text);
    window.set_cpu_freq_status(cpu_freq_status);

    // Per-core pairs for 2-column grid
    let mut core_rows = Vec::with_capacity(snap.cpu.per_core.len().div_ceil(2));
    for pair in snap.cpu.per_core.chunks(2) {
        let left = &pair[0];
        let (l_val, l_st) =
            format_metric_str(&left.utilization_percent, loc, |v| format!("{v:.1}%"));
        let l_freq = left
            .frequency_mhz
            .copied()
            .map(format_frequency_mhz)
            .unwrap_or_default();

        let (has_right, r_lbl, r_val, r_freq, r_pct, r_st) = if pair.len() > 1 {
            let right = &pair[1];
            let (rv, rst) =
                format_metric_str(&right.utilization_percent, loc, |v| format!("{v:.1}%"));
            let rf = right
                .frequency_mhz
                .copied()
                .map(format_frequency_mhz)
                .unwrap_or_default();
            (
                true,
                SharedString::from(format!("CPU {}", right.id)),
                rv,
                SharedString::from(rf),
                right.utilization_percent.copied().unwrap_or(0.0) as f32,
                rst,
            )
        } else {
            (
                false,
                SharedString::default(),
                SharedString::default(),
                SharedString::default(),
                0.0,
                1,
            )
        };

        core_rows.push(CoreRowItem {
            left_label: SharedString::from(format!("CPU {}", left.id)),
            left_value_text: l_val,
            left_freq_text: SharedString::from(l_freq),
            left_percent: left.utilization_percent.copied().unwrap_or(0.0) as f32,
            left_status: l_st,
            has_right,
            right_label: r_lbl,
            right_value_text: r_val,
            right_freq_text: r_freq,
            right_percent: r_pct,
            right_status: r_st,
        });
    }
    update_vec_model(window.get_cpu_core_rows(), core_rows);

    // Memory
    let (ram_util_text, ram_util_status) =
        format_metric_str(&snap.memory.utilization_percent, loc, |v| {
            format!("{v:.1}%")
        });
    window.set_ram_util_text(ram_util_text);
    window.set_ram_util_percent(snap.memory.utilization_percent.copied().unwrap_or(0.0) as f32);
    window.set_ram_util_status(ram_util_status);

    let (mem_total_text, mem_total_status) =
        format_metric_str(&snap.memory.total_bytes, loc, |b| format_bytes_binary(*b));
    let (mem_used_text, mem_used_status) =
        format_metric_str(&snap.memory.used_bytes, loc, |b| format_bytes_binary(*b));
    let (mem_avail_text, mem_avail_status) =
        format_metric_str(&snap.memory.available_bytes, loc, |b| {
            format_bytes_binary(*b)
        });
    window.set_mem_total_text(mem_total_text.clone());
    window.set_mem_total_status(mem_total_status);
    window.set_mem_used_text(mem_used_text.clone());
    window.set_mem_used_status(mem_used_status);
    window.set_mem_available_text(mem_avail_text);
    window.set_mem_available_status(mem_avail_status);
    window.set_ram_detail_text(if mem_used_status == 0 && mem_total_status == 0 {
        SharedString::from(format!("{mem_used_text} / {mem_total_text}"))
    } else {
        SharedString::default()
    });

    let (swap_total_text, swap_total_status) =
        format_metric_str(&snap.memory.swap_total_bytes, loc, |b| {
            format_bytes_binary(*b)
        });
    let (swap_used_text, swap_used_status) =
        format_metric_str(&snap.memory.swap_used_bytes, loc, |b| {
            format_bytes_binary(*b)
        });
    let (swap_util_text, swap_util_status) =
        format_metric_str(&snap.memory.swap_utilization_percent, loc, |v| {
            format!("{v:.1}%")
        });
    window.set_swap_total_text(swap_total_text);
    window.set_swap_total_status(swap_total_status);
    window.set_swap_used_text(swap_used_text);
    window.set_swap_used_status(swap_used_status);
    window.set_swap_util_text(swap_util_text);
    let swap_pct = snap.memory.swap_utilization_percent.copied().unwrap_or(0.0) as f32;
    window.set_swap_util_percent(swap_pct);
    window.set_swap_util_status(swap_util_status);
    window.set_mem_notice_text(SharedString::from(build_mem_notice(&snap.memory, loc)));

    // GPUs
    let gpu_items: Vec<GpuItem> = snap
        .gpus
        .iter()
        .map(|g| gpu_to_ui_item(g, loc, temp_unit))
        .collect();
    let has_gpus = !gpu_items.is_empty();
    let selected_idx = if has_gpus {
        state.selected_gpu_index.min(gpu_items.len() - 1)
    } else {
        0
    };

    let selected_gpu_item = gpu_items
        .get(selected_idx)
        .cloned()
        .unwrap_or_else(|| empty_gpu_item(loc));

    let sys_gpu_keys: Vec<(String, String, String)> = snap
        .gpus
        .iter()
        .map(|g| {
            let (drv, _) = format_metric_str(&g.driver_version, loc, |s| s.clone());
            (g.name.clone(), g.vendor_name.clone(), drv.to_string())
        })
        .collect();

    update_vec_model(window.get_gpus(), gpu_items);
    if state.render_cache.sys_gpus.as_ref() != Some(&sys_gpu_keys) {
        let sys_gpus: Vec<SystemGpuItem> = sys_gpu_keys
            .iter()
            .map(|(name, vendor, driver)| SystemGpuItem {
                name: SharedString::from(name.as_str()),
                vendor: SharedString::from(vendor.as_str()),
                driver: SharedString::from(driver.as_str()),
            })
            .collect();
        update_vec_model(window.get_system_gpus(), sys_gpus);
        state.render_cache.sys_gpus = Some(sys_gpu_keys);
    }
    window.set_has_gpus(has_gpus);
    window.set_selected_gpu_index(selected_idx as i32);
    window.set_selected_gpu(selected_gpu_item.clone());

    if has_gpus {
        window.set_dash_gpu_util_text(selected_gpu_item.util_text.clone());
        window.set_dash_gpu_util_percent(selected_gpu_item.util_percent);
        window.set_dash_gpu_util_status(selected_gpu_item.util_status);
        window.set_dash_gpu_temp_text(SharedString::from(format!(
            "{} • {}",
            selected_gpu_item.name, selected_gpu_item.temp_text
        )));
        window.set_dash_vram_util_text(selected_gpu_item.vram_text.clone());
        window.set_dash_vram_util_percent(selected_gpu_item.vram_percent);
        window.set_dash_vram_util_status(selected_gpu_item.vram_status);
        window.set_dash_vram_detail_text(selected_gpu_item.vram_subtext.clone());
    } else {
        window.set_dash_gpu_util_text(SharedString::from(loc.tr("status_unavailable")));
        window.set_dash_gpu_util_percent(0.0);
        window.set_dash_gpu_util_status(1);
        window.set_dash_gpu_temp_text(SharedString::from(loc.tr("gpu_none_detected")));
        window.set_dash_vram_util_text(SharedString::from(loc.tr("status_unavailable")));
        window.set_dash_vram_util_percent(0.0);
        window.set_dash_vram_util_status(1);
        window.set_dash_vram_detail_text(SharedString::default());
    }

    // Storage
    let disk_items: Vec<StorageItem> = snap
        .storage
        .iter()
        .map(|d| {
            let (used, _) = format_metric_str(&d.used_bytes, loc, |b| format_bytes_binary(*b));
            let (avail, _) =
                format_metric_str(&d.available_bytes, loc, |b| format_bytes_binary(*b));
            let (total, _) = format_metric_str(&d.total_bytes, loc, |b| format_bytes_binary(*b));
            let (pct_text, st) =
                format_metric_str(&d.utilization_percent, loc, |v| format!("{v:.1}%"));
            StorageItem {
                device: SharedString::from(d.device.as_str()),
                mount_point: SharedString::from(d.mount_point.as_str()),
                filesystem: SharedString::from(d.filesystem.as_str()),
                used_text: used,
                available_text: avail,
                total_text: total,
                percent_text: pct_text,
                percent: d.utilization_percent.copied().unwrap_or(0.0) as f32,
                status: st,
            }
        })
        .collect();
    update_vec_model(window.get_disks(), disk_items);
    window.set_gpu_notice_text(SharedString::from(build_gpu_notice(
        snap.gpus.get(selected_idx),
        loc,
    )));
    window.set_storage_notice_text(SharedString::from(build_storage_notice(&snap.storage, loc)));

    // Real-time bounded 60s charts (update Slint paths only when changed)
    let now = snap.sampled_at;

    let (show_cpu, show_ram, show_gpu) =
        visible_charts(window.get_current_page(), state.settings.interface_mode);
    if show_cpu {
        let cpu_paths = state.cpu_history.build_chart_paths(
            now,
            CHART_WINDOW,
            CHART_VIEWBOX_W,
            CHART_VIEWBOX_H,
            0.0,
            100.0,
            max_gap,
        );
        if state.render_cache.cpu_line != cpu_paths.line_commands {
            window.set_cpu_line_commands(SharedString::from(cpu_paths.line_commands.as_str()));
            state.render_cache.cpu_line = cpu_paths.line_commands;
        }
        if state.render_cache.cpu_area != cpu_paths.area_commands {
            window.set_cpu_area_commands(SharedString::from(cpu_paths.area_commands.as_str()));
            state.render_cache.cpu_area = cpu_paths.area_commands;
        }
        if state.render_cache.cpu_has != Some(cpu_paths.has_data) {
            window.set_cpu_has_history(cpu_paths.has_data);
            state.render_cache.cpu_has = Some(cpu_paths.has_data);
        }
    }
    if show_ram {
        let ram_paths = state.ram_history.build_chart_paths(
            now,
            CHART_WINDOW,
            CHART_VIEWBOX_W,
            CHART_VIEWBOX_H,
            0.0,
            100.0,
            max_gap,
        );
        if state.render_cache.ram_line != ram_paths.line_commands {
            window.set_ram_line_commands(SharedString::from(ram_paths.line_commands.as_str()));
            state.render_cache.ram_line = ram_paths.line_commands;
        }
        if state.render_cache.ram_area != ram_paths.area_commands {
            window.set_ram_area_commands(SharedString::from(ram_paths.area_commands.as_str()));
            state.render_cache.ram_area = ram_paths.area_commands;
        }
        if state.render_cache.ram_has != Some(ram_paths.has_data) {
            window.set_ram_has_history(ram_paths.has_data);
            state.render_cache.ram_has = Some(ram_paths.has_data);
        }
    }
    if show_gpu {
        let (gpu_line, gpu_area, gpu_has, gpu_empty) =
            if let Some(gpu) = snap.gpus.get(selected_idx) {
                if let Some(hist) = state.gpu_histories.get(&gpu.id) {
                    let gpu_paths = hist.build_chart_paths(
                        now,
                        CHART_WINDOW,
                        CHART_VIEWBOX_W,
                        CHART_VIEWBOX_H,
                        0.0,
                        100.0,
                        max_gap,
                    );
                    let empty_msg = if gpu_paths.has_data {
                        String::new()
                    } else {
                        match gpu.utilization_percent.status() {
                            MetricStatus::Unsupported => loc.tr("status_unsupported").to_string(),
                            MetricStatus::PermissionError => {
                                loc.tr("status_permission_denied").to_string()
                            }
                            _ => loc.tr("status_unavailable").to_string(),
                        }
                    };
                    (
                        gpu_paths.line_commands,
                        gpu_paths.area_commands,
                        gpu_paths.has_data,
                        empty_msg,
                    )
                } else {
                    (
                        String::new(),
                        String::new(),
                        false,
                        loc.tr("status_sampling").to_string(),
                    )
                }
            } else {
                (
                    String::new(),
                    String::new(),
                    false,
                    loc.tr("gpu_none_detected").to_string(),
                )
            };

        if state.render_cache.gpu_line != gpu_line {
            window.set_gpu_line_commands(SharedString::from(gpu_line.as_str()));
            state.render_cache.gpu_line = gpu_line;
        }
        if state.render_cache.gpu_area != gpu_area {
            window.set_gpu_area_commands(SharedString::from(gpu_area.as_str()));
            state.render_cache.gpu_area = gpu_area;
        }
        if state.render_cache.gpu_has != Some(gpu_has) {
            window.set_gpu_has_history(gpu_has);
            state.render_cache.gpu_has = Some(gpu_has);
        }
        if !gpu_empty.is_empty() && state.render_cache.gpu_empty != gpu_empty {
            window.set_gpu_chart_empty_text(SharedString::from(gpu_empty.as_str()));
            state.render_cache.gpu_empty = gpu_empty;
        }
    }
}

fn gpu_to_ui_item(gpu: &GpuTelemetry, loc: &Localizer, temp_unit: TemperatureUnit) -> GpuItem {
    let (driver_text, driver_status) = format_metric_str(&gpu.driver_version, loc, |s| s.clone());
    let (util_text, util_status) =
        format_metric_str(&gpu.utilization_percent, loc, |v| format!("{v:.0}%"));
    let (vram_text, vram_status) =
        format_metric_str(&gpu.vram_utilization_percent, loc, |v| format!("{v:.1}%"));

    let vram_subtext = match (&gpu.vram_used_bytes, &gpu.vram_total_bytes) {
        (MetricValue::Available(u), MetricValue::Available(t)) => {
            format!("{} / {}", format_bytes_binary(*u), format_bytes_binary(*t))
        }
        _ => String::new(),
    };

    let (temp_text, temp_status) = format_metric_str(&gpu.temperature_celsius, loc, |c| {
        format_temperature(*c, temp_unit)
    });
    let (core_clock_text, core_clock_status) =
        format_metric_str(&gpu.core_clock_mhz, loc, |m| format!("{m} MHz"));
    let (mem_clock_text, mem_clock_status) =
        format_metric_str(&gpu.memory_clock_mhz, loc, |m| format!("{m} MHz"));
    let (power_draw_text, power_draw_status) =
        format_metric_str(&gpu.power_draw_watts, loc, |w| format!("{w:.1} W"));
    let (power_limit_text, power_limit_status) =
        format_metric_str(&gpu.power_limit_watts, loc, |w| format!("{w:.0} W"));
    let (fan_text, fan_status) =
        format_metric_str(&gpu.fan_speed_percent, loc, |p| format!("{p}%"));

    GpuItem {
        id: SharedString::from(gpu.id.as_str()),
        name: SharedString::from(gpu.name.as_str()),
        vendor_name: SharedString::from(gpu.vendor_name.as_str()),
        pci_slot: SharedString::from(gpu.pci_slot.clone().unwrap_or_default()),
        driver_text,
        driver_status,
        util_text,
        util_percent: gpu.utilization_percent.copied().unwrap_or(0.0) as f32,
        util_status,
        vram_text,
        vram_subtext: SharedString::from(vram_subtext),
        vram_percent: gpu.vram_utilization_percent.copied().unwrap_or(0.0) as f32,
        vram_status,
        temp_text,
        temp_status,
        core_clock_text,
        core_clock_status,
        mem_clock_text,
        mem_clock_status,
        power_draw_text,
        power_draw_status,
        power_limit_text,
        power_limit_status,
        fan_text,
        fan_status,
    }
}

fn empty_gpu_item(loc: &Localizer) -> GpuItem {
    let unavail = SharedString::from(loc.tr("status_unavailable"));
    GpuItem {
        id: SharedString::default(),
        name: unavail.clone(),
        vendor_name: unavail.clone(),
        pci_slot: SharedString::default(),
        driver_text: unavail.clone(),
        driver_status: 1,
        util_text: unavail.clone(),
        util_percent: 0.0,
        util_status: 1,
        vram_text: unavail.clone(),
        vram_subtext: SharedString::default(),
        vram_percent: 0.0,
        vram_status: 1,
        temp_text: unavail.clone(),
        temp_status: 1,
        core_clock_text: unavail.clone(),
        core_clock_status: 1,
        mem_clock_text: unavail.clone(),
        mem_clock_status: 1,
        power_draw_text: unavail.clone(),
        power_draw_status: 1,
        power_limit_text: unavail.clone(),
        power_limit_status: 1,
        fan_text: unavail,
        fan_status: 1,
    }
}

fn format_metric_str<T, F>(metric: &MetricValue<T>, loc: &Localizer, fmt: F) -> (SharedString, i32)
where
    F: FnOnce(&T) -> String,
{
    match metric {
        MetricValue::Available(v) => {
            let text = SharedString::from(fmt(v));
            (text, MetricStatus::Available.as_code())
        }
        MetricValue::Unavailable(_) => (
            SharedString::from(loc.tr("status_unavailable")),
            MetricStatus::Unavailable.as_code(),
        ),
        MetricValue::Unsupported(_) => (
            SharedString::from(loc.tr("status_unsupported")),
            MetricStatus::Unsupported.as_code(),
        ),
        MetricValue::PermissionError(_) => (
            SharedString::from(loc.tr("status_permission_denied")),
            MetricStatus::PermissionError.as_code(),
        ),
    }
}

fn build_cpu_notice(cpu: &CpuTelemetry, loc: &Localizer) -> String {
    let mut notes = Vec::new();
    if cpu.overall_utilization.status() == MetricStatus::PermissionError {
        notes.push(loc.tr("diag_cpu_sys_restricted"));
    }
    match cpu.average_frequency_mhz.status() {
        MetricStatus::PermissionError => notes.push(loc.tr("diag_cpu_freq_restricted")),
        MetricStatus::Unsupported => notes.push(loc.tr("diag_cpu_freq_na")),
        MetricStatus::Available | MetricStatus::Unavailable => {}
    }
    match cpu.temperature_celsius.status() {
        MetricStatus::PermissionError => notes.push(loc.tr("diag_cpu_temp_restricted")),
        MetricStatus::Unsupported | MetricStatus::Unavailable => {
            notes.push(loc.tr("diag_cpu_temp_na"))
        }
        MetricStatus::Available => {}
    }
    notes.join("\n")
}

fn build_gpu_notice(gpu: Option<&GpuTelemetry>, loc: &Localizer) -> String {
    let Some(gpu) = gpu else {
        return String::new();
    };
    let mut notes = Vec::new();
    if gpu.utilization_percent.status() == MetricStatus::PermissionError
        || gpu.core_clock_mhz.status() == MetricStatus::PermissionError
        || gpu.temperature_celsius.status() == MetricStatus::PermissionError
    {
        notes.push(loc.tr("diag_gpu_util_restricted"));
    }
    if gpu.vram_utilization_percent.status() == MetricStatus::Unsupported {
        notes.push(loc.tr("diag_gpu_vram_uma"));
    }
    if gpu.power_draw_watts.status() == MetricStatus::Unsupported
        || gpu.power_limit_watts.status() == MetricStatus::Unsupported
        || gpu.memory_clock_mhz.status() == MetricStatus::Unsupported
        || gpu.fan_speed_percent.status() == MetricStatus::Unsupported
    {
        notes.push(loc.tr("diag_gpu_sensors_na"));
    }
    notes.join("\n")
}

fn build_mem_notice(mem: &MemoryTelemetry, loc: &Localizer) -> String {
    let mut notes = Vec::new();
    if mem.total_bytes.status() == MetricStatus::PermissionError
        || mem.available_bytes.status() == MetricStatus::PermissionError
        || mem.used_bytes.status() == MetricStatus::PermissionError
    {
        notes.push(loc.tr("diag_mem_restricted"));
    }
    if mem.swap_utilization_percent.status() == MetricStatus::Unsupported {
        notes.push(loc.tr("diag_mem_swap_na"));
    }
    notes.join("\n")
}

fn build_storage_notice(storage: &[StorageDeviceTelemetry], loc: &Localizer) -> String {
    let has_perm = storage
        .iter()
        .any(|d| d.utilization_percent.status() == MetricStatus::PermissionError);
    if cfg!(target_os = "android") || has_perm {
        loc.tr("diag_storage_note").to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod model_update_tests {
    use super::*;

    #[test]
    fn stale_gpu_histories_are_pruned_without_losing_recent_samples() {
        let now = std::time::Instant::now();
        let mut recent = BoundedHistory::new(4);
        recent.push(now - Duration::from_secs(60), Some(42.0));
        let mut stale = BoundedHistory::new(4);
        stale.push(now - Duration::from_secs(66), Some(20.0));
        let mut histories = HashMap::from([
            ("recent".to_string(), recent),
            ("stale".to_string(), stale),
            ("empty".to_string(), BoundedHistory::new(4)),
        ]);
        prune_stale_gpu_histories(&mut histories, now);
        assert_eq!(histories.len(), 1);
        assert_eq!(
            histories["recent"].samples().back().unwrap().value,
            Some(42.0)
        );
    }

    #[test]
    fn chart_visibility_tracks_page_and_mode() {
        use crate::settings::InterfaceMode::{Gaming, Modern, Professional};
        assert_eq!(visible_charts(0, Modern), (true, true, true));
        assert_eq!(visible_charts(0, Gaming), (true, false, true));
        assert_eq!(visible_charts(0, Professional), (true, false, true));
        assert_eq!(visible_charts(1, Modern), (true, false, false));
        assert_eq!(visible_charts(2, Modern), (false, false, true));
        assert_eq!(visible_charts(3, Modern), (false, true, false));
        assert_eq!(visible_charts(6, Modern), (false, false, false));
    }

    #[test]
    fn updates_rows_and_handles_growth_and_shrinkage() {
        let rows = Rc::new(VecModel::from(vec![1, 2, 3]));
        let model = ModelRc::from(rows.clone());
        update_vec_model(model.clone(), vec![1, 4, 3, 5]);
        assert_eq!(rows.row_count(), 4);
        assert_eq!(rows.row_data(1), Some(4));
        assert_eq!(rows.row_data(3), Some(5));
        update_vec_model(model.clone(), vec![7]);
        assert_eq!(rows.row_count(), 1);
        assert_eq!(rows.row_data(0), Some(7));
        update_vec_model(model, Vec::new());
        assert_eq!(rows.row_count(), 0);
    }
}
