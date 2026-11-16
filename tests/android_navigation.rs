//! Exercises the compiled production AppWindow with Android mode enabled.
//! Uses Slint's software renderer, not an Android device/backend.
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition, PhysicalSize, Rgb8Pixel, SharedPixelBuffer,
};
use std::{path::PathBuf, rc::Rc};
use voltix::app::{AppWindow, Theme};
struct TestPlatform(Rc<MinimalSoftwareWindow>);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn tap(window: &slint::Window, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
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
    window: &Rc<MinimalSoftwareWindow>,
    name: &str,
    width: u32,
    height: u32,
) -> SharedPixelBuffer<Rgb8Pixel> {
    slint::platform::update_timers_and_animations();
    assert_eq!(
        window.size(),
        PhysicalSize::new(width, height),
        "mobile page changed viewport size"
    );
    window.request_redraw();
    let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(pixels.make_mut_slice(), width as usize);
    }));
    if let Some(path) = std::env::var_os("VOLTIX_UI_SNAPSHOTS") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        bytes.extend_from_slice(pixels.as_bytes());
        std::fs::write(path.join(format!("{name}.ppm")), bytes).unwrap();
    }
    pixels
}
// Simulated presentation fixtures only; production collectors are untouched.
fn density_fixtures(ui: &AppWindow, loc: &voltix::localization::Localizer) {
    use slint::{ModelRc, VecModel};
    use voltix::{
        app::{ControlRow, ControlSummary},
        monitoring::extended::{Battery, Platform},
    };
    ui.set_cc_summaries(ModelRc::new(VecModel::from(
        [
            "CPU",
            "GPU",
            "RAM",
            "VRAM",
            loc.tr("cc_storage"),
            loc.tr("cc_network"),
        ]
        .into_iter()
        .map(|label| ControlSummary {
            label: label.into(),
            value: "42%".into(),
            detail: "73 / 128 MiB".into(),
            percent: 42.,
            status: 0,
        })
        .collect::<Vec<_>>(),
    )));
    ui.set_cc_network(ModelRc::new(VecModel::from(vec![ControlRow {
        label: "wlan0".into(),
        value: "1.2 MiB/s".into(),
        detail: "128 MiB".into(),
    }])));
    ui.set_cc_disks(ModelRc::new(VecModel::from(vec![ControlRow {
        label: "nvme0n1".into(),
        value: "8.3 MiB/s".into(),
        detail: String::new().into(),
    }])));
    let battery = Battery {
        percent: Some(73.),
        status: 3,
        source: 0,
        temperature_c: Some(31.4),
        voltage_v: Some(3.89),
        current_a: Some(-0.82),
        power_w: Some(-3.19),
        technology: "Li-ion".into(),
        health: 2,
    };
    ui.set_cc_detail_rows(ModelRc::new(VecModel::from(
        voltix::control_center::battery_rows(&battery, loc, 0),
    )));
    ui.set_cc_permission_rows(ModelRc::new(VecModel::from(
        voltix::control_center::android_permission_rows(
            &Platform {
                elapsed_ms: 10,
                network_permission: true,
                ..Default::default()
            },
            loc,
        ),
    )));
    ui.set_cc_gpu_diagnostic_rows(ModelRc::new(VecModel::from(vec![
        ControlRow {
            label: loc.tr("gpu_utilization").into(),
            value: loc.tr("status_permission_denied").into(),
            detail: loc.tr("hotfix_gpu_policy_detail").into(),
        },
        ControlRow {
            label: loc.tr("hotfix_gpu_vram").into(),
            value: loc.tr("status_unsupported").into(),
            detail: loc.tr("probe_uma").into(),
        },
    ])));
    ui.set_cc_platform_diagnostic_rows(ModelRc::new(VecModel::from(vec![ControlRow {
        label: loc.tr("cc_sensors").into(),
        value: "12".into(),
        detail: loc.tr("cc_sensors_note").into(),
    }])));
}

#[test]
fn android_root_renders_and_routes_real_mobile_components() {
    let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(adapter.clone()))).unwrap();
    let ui = AppWindow::new().unwrap();
    voltix::app::configure_platform_window(&ui, true);
    voltix::app::apply_i18n_to_window(
        &ui,
        &voltix::localization::Localizer::new(voltix::localization::LocaleCode::En),
    );
    ui.global::<Theme>().set_dark_mode(true);
    adapter.set_size(PhysicalSize::new(390, 844));
    ui.show().unwrap();
    assert!(ui.get_build_identifier().contains("Mobile UI R2"));
    assert_eq!(
        ui.get_source_commit().as_str(),
        voltix::build_info::SOURCE_COMMIT
    );
    // All five real bottom destinations must respond to pointer events.
    for (index, page) in [0, 1, 2, 8, 12].into_iter().enumerate() {
        let pixels = render(&adapter, &format!("tab-{page}"), 390, 844);
        // Each 24px icon must contain accent/foreground pixels above its label.
        let center = 39 + index * 78;
        let colored = (center.saturating_sub(14)..center + 14)
            .flat_map(|x| (785..811).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let p = pixels.as_slice()[y * 390 + x];
                p.r > 130 || p.g > 130 || p.b > 130
            })
            .count();
        assert!(colored > 8, "bottom icon {index} did not render");
        tap(ui.window(), center as f32, 810.);
        assert_eq!(
            ui.get_current_page(),
            page,
            "bottom destination {index} is inactive"
        );
    }
    render(&adapter, "more", 390, 844);
    // Locate the actual Settings row by tapping each visible Application row.
    // Row positions are intentionally fixed by the compact grouped layout.
    tap(ui.window(), 190., 612.);
    render(&adapter, "after-settings-tap", 390, 844);
    assert_eq!(
        ui.get_current_page(),
        6,
        "More Settings row did not open Settings"
    );
    assert_eq!(
        ui.get_cc_settings_category(),
        -1,
        "Settings must open category home"
    );
    for category in 0..6 {
        tap(ui.window(), 180., 160. + category as f32 * 64.);
        render(&adapter, &format!("settings-category-{category}"), 390, 844);
        assert_eq!(
            ui.get_cc_settings_category(),
            category,
            "category row did not open its controls"
        );
        tap(ui.window(), 130., 78.);
        render(&adapter, "settings-home", 390, 844);
        assert_eq!(
            ui.get_cc_settings_category(),
            -1,
            "category back must return to Settings home"
        );
    }
    for (page, y) in [
        (3, 179.),
        (4, 243.),
        (7, 307.),
        (5, 371.),
        (9, 435.),
        (13, 547.),
    ] {
        ui.invoke_navigate_to(12);
        render(&adapter, "more", 390, 844);
        tap(ui.window(), 180., y);
        assert_eq!(ui.get_current_page(), page, "More row is inactive");
        render(&adapter, &format!("more-destination-{page}"), 390, 844);
    }
    ui.invoke_navigate_to(12);
    render(&adapter, "more", 390, 844);
    tap(ui.window(), 180., 675.);
    render(&adapter, "about", 390, 844);
    assert_eq!(ui.get_current_page(), 6);
    assert_eq!(ui.get_cc_settings_category(), 4);
    assert!(ui.get_build_short().len() < 20);
    assert!(!ui
        .get_build_short()
        .contains(ui.get_source_commit().as_str()));
    // Populated narrow screens exercise row density instead of empty placeholders.
    for code in [
        voltix::localization::LocaleCode::En,
        voltix::localization::LocaleCode::Pl,
    ] {
        let loc = voltix::localization::Localizer::new(code);
        voltix::app::apply_i18n_to_window(&ui, &loc);
        ui.global::<Theme>().set_dark_mode(true);
        adapter.set_size(PhysicalSize::new(320, 640));
        density_fixtures(&ui, &loc);
        ui.invoke_navigate_to(0);
        render(
            &adapter,
            &format!("density-dashboard-{}", code.as_str()),
            320,
            640,
        );
        ui.global::<Theme>().set_interface_mode(1);
        render(
            &adapter,
            &format!("density-dashboard-professional-{}", code.as_str()),
            320,
            640,
        );
        ui.global::<Theme>().set_interface_mode(0);
        ui.set_cc_detail_title(loc.tr("cc_battery").into());
        ui.invoke_navigate_to(8);
        let image = render(
            &adapter,
            &format!("density-battery-{}", code.as_str()),
            320,
            640,
        );
        let mut blank = 0;
        let mut longest = 0;
        for y in 120..470 {
            let has_text = (24..296).any(|x| {
                let p = image.as_slice()[y * 320 + x];
                p.r > 110 && p.g > 110 && p.b > 110
            });
            if has_text {
                blank = 0;
            } else {
                blank += 1;
                longest = longest.max(blank);
            }
        }
        assert!(
            longest < 50,
            "battery contains excessive empty vertical space: {longest}px"
        );
        ui.invoke_navigate_to(13);
        let collapsed = render(
            &adapter,
            &format!("density-diagnostics-{}", code.as_str()),
            320,
            640,
        );
        let badge = (15..130)
            .flat_map(|x| (30..46).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let p = collapsed.as_slice()[y * 320 + x];
                p.b > 150 && p.g > 120 && p.r < 120
            })
            .count();
        assert!(
            badge > 10,
            "diagnostics title clipped the short build identifier"
        );
        tap(ui.window(), 140., 110.);
        let expanded = render(
            &adapter,
            &format!("density-diagnostics-expanded-{}", code.as_str()),
            320,
            640,
        );
        assert_ne!(
            collapsed.as_bytes(),
            expanded.as_bytes(),
            "technical explanation did not expand"
        );
        tap(ui.window(), 140., 110.);
        render(&adapter, "density-diagnostics-collapsed", 320, 640);
    }
    // Every page must keep navigation usable on a small phone and in both themes.
    for (locale, dark) in [
        (voltix::localization::LocaleCode::Pl, false),
        (voltix::localization::LocaleCode::En, true),
    ] {
        voltix::app::apply_i18n_to_window(&ui, &voltix::localization::Localizer::new(locale));
        density_fixtures(&ui, &voltix::localization::Localizer::new(locale));
        ui.global::<Theme>().set_dark_mode(dark);
        adapter.set_size(PhysicalSize::new(320, 640));
        for page in [0, 1, 2, 8, 12, 6] {
            ui.invoke_navigate_to(page);
            render(
                &adapter,
                &format!("small-{}-{page}", if dark { "dark-en" } else { "light-pl" }),
                320,
                640,
            );
            tap(ui.window(), 288., 610.);
            assert_eq!(
                ui.get_current_page(),
                12,
                "bottom More became inaccessible on page {page}"
            );
        }
    }
    ui.hide().unwrap();
}
