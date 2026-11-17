//! Actual mobile GPU root; simulated reports, never a claim of on-device access.
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, ModelRc, PhysicalSize, Rgb8Pixel, SharedPixelBuffer, VecModel,
};
use std::{cell::Cell, rc::Rc};
use voltix::{
    app::{AppWindow, Theme},
    localization::{LocaleCode, Localizer},
    platform::shizuku::{rows, Probe, Snapshot},
};
struct TestPlatform(Rc<MinimalSoftwareWindow>);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn render(adapter: &Rc<MinimalSoftwareWindow>, name: &str, w: u32, h: u32) {
    slint::platform::update_timers_and_animations();
    adapter.request_redraw();
    assert_eq!(
        adapter.size(),
        PhysicalSize::new(w, h),
        "advanced GPU page enlarged viewport"
    );
    let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    assert!(adapter.draw_if_needed(|r| {
        r.render(pixels.make_mut_slice(), w as usize);
    }));
    if let Some(dir) = std::env::var_os("VOLTIX_UI_SNAPSHOTS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = format!("P6\n{w} {h}\n255\n").into_bytes();
        bytes.extend_from_slice(pixels.as_bytes());
        std::fs::write(dir.join(format!("shizuku-{name}-{w}.ppm")), bytes).unwrap();
    }
}
#[test]
fn mobile_gpu_uses_optional_bridge_controls_at_narrow_widths() {
    let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(adapter.clone()))).unwrap();
    let ui = AppWindow::new().unwrap();
    voltix::app::configure_platform_window(&ui, true);
    ui.set_current_page(2);
    ui.global::<Theme>().set_dark_mode(true);
    let action = Rc::new(Cell::new(0));
    let copy = action.clone();
    ui.on_shizuku_action(move |code| copy.set(code));
    for locale in [LocaleCode::En, LocaleCode::Pl] {
        let loc = Localizer::new(locale);
        voltix::app::apply_i18n_to_window(&ui, &loc);
        for (w, h) in [(390, 844), (320, 640)] {
            adapter.set_size(PhysicalSize::new(w, h));
            ui.show().unwrap();
            for state in ["disabled", "permission_denied", "connected"] {
                let enabled = state != "disabled";
                let snapshot = Snapshot {
                    enabled,
                    installed: true,
                    service_running: true,
                    authorization: if state == "connected" {
                        "granted".into()
                    } else {
                        "denied".into()
                    },
                    state: state.into(),
                    uid: 2000,
                    sampled_elapsed_ms: if state == "connected" { 1000 } else { 0 },
                    records: if state == "connected" {
                        vec![
                            Probe {
                                kind: "cur_freq_hz".into(),
                                path: "/sys/class/devfreq/13000000.mali/cur_freq".into(),
                                status: "readable".into(),
                                text: "847000000".into(),
                                ..Default::default()
                            },
                            Probe {
                                kind: "ged_load_v1".into(),
                                path: "/sys/kernel/ged/hal/gpu_utilization".into(),
                                status: "readable".into(),
                                text: "62 0 38".into(),
                                ..Default::default()
                            },
                            Probe {
                                kind: "gpu_temp_mc".into(),
                                path: "/sys/class/thermal/thermal_zone12/temp".into(),
                                label: "gpu".into(),
                                status: "readable".into(),
                                text: "48500".into(),
                            },
                        ]
                    } else {
                        Vec::new()
                    },
                };
                let (v, values) = rows(&snapshot, true, 1500, &loc);
                ui.set_shizuku_enabled(enabled);
                ui.set_shizuku_running(true);
                ui.set_shizuku_authorized(state == "connected");
                ui.set_shizuku_installed(true);
                ui.set_shizuku_rows(ModelRc::new(VecModel::from(values)));
                ui.set_shizuku_sources(ModelRc::new(VecModel::from(v.source_rows)));
                render(&adapter, &format!("{}-{state}", locale.as_str()), w, h);
                if state == "disabled" {
                    action.set(0);
                    let position = slint::LogicalPosition::new(32., 116.);
                    ui.window().dispatch_event(WindowEvent::PointerPressed {
                        position,
                        button: PointerEventButton::Left,
                    });
                    ui.window().dispatch_event(WindowEvent::PointerReleased {
                        position,
                        button: PointerEventButton::Left,
                    });
                    assert_eq!(
                        action.get(),
                        1,
                        "real Advanced GPU checkbox did not request enabling the optional bridge"
                    );
                }
            }
        }
    }
}
