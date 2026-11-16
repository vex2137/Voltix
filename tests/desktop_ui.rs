//! Render the production desktop root using simulated telemetry, not monitoring overrides.
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, Model, ModelRc, PhysicalSize, Rgb8Pixel, SharedPixelBuffer, VecModel,
};
use std::rc::Rc;
use voltix::{
    app::{AppWindow, ControlRow, ControlSummary, ProcessItem, Theme},
    localization::{LocaleCode, Localizer},
    monitoring::extended::Sensor,
};
struct TestPlatform(Rc<MinimalSoftwareWindow>);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn tap(window: &slint::Window, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
fn render(
    adapter: &Rc<MinimalSoftwareWindow>,
    name: &str,
    w: u32,
    h: u32,
) -> SharedPixelBuffer<Rgb8Pixel> {
    slint::platform::update_timers_and_animations();
    adapter.request_redraw();
    assert_eq!(
        adapter.size(),
        PhysicalSize::new(w, h),
        "desktop layout enlarged the viewport: {name}"
    );
    let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    assert!(adapter.draw_if_needed(|r| {
        r.render(pixels.make_mut_slice(), w as usize);
    }));
    if let Some(dir) = std::env::var_os("VOLTIX_DESKTOP_SNAPSHOTS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = format!("P6\n{w} {h}\n255\n").into_bytes();
        bytes.extend_from_slice(pixels.as_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), bytes).unwrap();
    }
    pixels
}
#[test]
fn desktop_pages_fit_and_refresh_preserves_process_and_sensor_models() {
    let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(adapter.clone()))).unwrap();
    let ui = AppWindow::new().unwrap();
    voltix::app::configure_platform_window(&ui, false);
    ui.set_overlay_linux(true);
    ui.set_cc_processes_available(true);
    let processes = Rc::new(VecModel::from(
        (0..80)
            .map(|i| ProcessItem {
                key: format!("{i}:100").into(),
                name: format!("worker-{i} — long process name").into(),
                pid: (1000 + i).to_string().into(),
                cpu_text: "12.4%".into(),
                memory_text: "512.0".into(),
                user: "desktop-user".into(),
                state: "Sleeping".into(),
                threads: "12".into(),
                cpu: 12.4,
                ram: 31.,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ));
    ui.set_proc_rows(processes.clone().into());
    let summary = ModelRc::new(VecModel::from(
        (0..6)
            .map(|i| ControlSummary {
                label: ["Tasks", "Threads", "CPU", "RAM", "Top CPU", "Top RAM"][i].into(),
                value: "123".into(),
                detail: "worker process".into(),
                status: 0,
                percent: 32.,
            })
            .collect::<Vec<_>>(),
    ));
    ui.set_proc_summaries(summary.clone());
    ui.set_cc_summaries(summary);
    ui.set_proc_selected("".into());
    ui.set_proc_selected_index(15);
    ui.set_manager_profiles(ModelRc::new(VecModel::from(vec![
        "default".into(),
        "gaming".into(),
    ])));
    ui.set_manager_flags(ModelRc::new(VecModel::from(vec![true; 32])));
    ui.set_manager_values(ModelRc::new(VecModel::from(vec!["14".into(); 18])));
    ui.set_manager_config("fps\ngpu_stats\n".into());
    ui.set_desktop_sensor_groups(ModelRc::new(VecModel::default()));
    let performance: Vec<_> = (0..6)
        .map(|i| ControlRow {
            label: [
                "CPU temperature",
                "CPU frequency",
                "GPU temperature",
                "GPU core / memory",
                "GPU power",
                "GPU fan",
            ][i]
                .into(),
            value: [
                "46.0 °C",
                "3200 MHz",
                "51.0 °C",
                "1200 / 800 MHz",
                "25.0 W",
                "28% ",
            ][i]
                .into(),
            detail: "".into(),
        })
        .collect();
    let sensors = vec![
        Sensor {
            name: "coretemp / Package id 0".into(),
            value: 46.,
            unit: "°C".into(),
        },
        Sensor {
            name: "nct6798 / Fan 1".into(),
            value: 1234.,
            unit: "RPM".into(),
        },
        Sensor {
            name: "amdgpu / Power".into(),
            value: 25.,
            unit: "W".into(),
        },
        Sensor {
            name: "nct6798 / Voltage".into(),
            value: 1.2,
            unit: "V".into(),
        },
    ];
    for locale in [LocaleCode::En, LocaleCode::Pl] {
        let loc = Localizer::new(locale);
        voltix::app::apply_i18n_to_window(&ui, &loc);
        voltix::desktop_sensors::render(&ui, &performance, &sensors, &loc);
        let groups = ui.get_desktop_sensor_groups();
        assert_eq!(
            groups
                .row_data(2)
                .unwrap()
                .rows
                .row_data(0)
                .unwrap()
                .value
                .as_str(),
            "46.0 °C"
        );
        assert_eq!(
            groups
                .iter()
                .find(|g| g.title.as_str() == loc.tr("desktop_fans"))
                .unwrap()
                .rows
                .row_data(0)
                .unwrap()
                .value
                .as_str(),
            "1234 RPM"
        );
        let nested = groups.row_data(2).unwrap().rows;
        voltix::desktop_sensors::render(&ui, &performance, &sensors, &loc);
        assert!(
            nested == ui.get_desktop_sensor_groups().row_data(2).unwrap().rows,
            "refresh replaced sensor model"
        );
        for (w, h, dark) in [(740, 480, true), (1024, 768, false), (1280, 800, true)] {
            ui.global::<Theme>().set_dark_mode(dark);
            ui.global::<Theme>().set_interface_mode(1);
            adapter.set_size(PhysicalSize::new(w, h));
            ui.show().unwrap();
            ui.set_proc_selected("".into());
            for page in [0, 9, 10, 6, 11] {
                ui.set_cc_settings_category(if page == 11 { 3 } else { 0 });
                ui.set_current_page(page);
                render(
                    &adapter,
                    &format!("{}-{w}-page-{page}", locale.as_str()),
                    w,
                    h,
                );
            }
            if w == 1280 {
                for section in 0..5 {
                    tap(ui.window(), 233. + (section as f32 + 0.5) * 201., 157.);
                    render(
                        &adapter,
                        &format!("{}-studio-{section}", locale.as_str()),
                        w,
                        h,
                    );
                }
            }
            ui.set_current_page(10);
            ui.set_proc_selected("".into());
            let unscrolled = render(&adapter, "process-before-scroll", w, h);
            adapter.dispatch_event(WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(w as f32 - 100., h as f32 - 100.),
                delta_x: 0.,
                delta_y: -240.,
            });
            let before = render(&adapter, "process-scrolled", w, h);
            assert!(
                unscrolled.as_bytes() != before.as_bytes(),
                "table did not scroll"
            );
            let mut updated = processes.row_data(0).unwrap();
            updated.cpu += 0.1;
            updated.cpu_text = format!("{:.1}%", updated.cpu).into();
            processes.set_row_data(0, updated);
            let after = render(&adapter, "process-refreshed", w, h);
            assert!(
                before.as_bytes() == after.as_bytes(),
                "refresh reset scrolling or changed unchanged visible rows"
            );
            ui.set_proc_selected("15:100".into());
            processes.set_row_data(15, processes.row_data(15).unwrap());
            assert_eq!(ui.get_proc_selected().as_str(), "15:100");
            ui.global::<Theme>().set_interface_mode(2);
            ui.set_current_page(0);
            render(&adapter, &format!("{}-{w}-gaming", locale.as_str()), w, h);
        }
    }
}
