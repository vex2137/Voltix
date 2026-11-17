//! Desktop-only presentation of existing sensor readings; no collection or inference.
use crate::{
    app::{AppWindow, ControlRow, DesktopSensorGroup},
    localization::Localizer,
    monitoring::extended::Sensor,
};
use slint::{Model, ModelRc, VecModel};

pub fn render(window: &AppWindow, performance: &[ControlRow], sensors: &[Sensor], loc: &Localizer) {
    let model = window.get_desktop_sensor_groups();
    let mut groups = vec![Vec::new(); 7];
    groups[0].extend(performance.iter().take(2).cloned());
    groups[1].extend(performance.iter().skip(2).cloned());
    for sensor in sensors {
        let group = match sensor.unit.as_str() {
            "°C" | "°F" => 2,
            "Hz" | "kHz" | "MHz" | "GHz" => 3,
            "RPM" => 4,
            "W" | "mW" => 5,
            _ => 6,
        };
        groups[group].push(ControlRow {
            label: sensor.name.clone().into(),
            value: match sensor.unit.as_str() {
                "RPM" => format!("{:.0} {}", sensor.value, sensor.unit),
                "°C" | "°F" | "Hz" | "kHz" | "MHz" | "GHz" | "W" | "mW" => {
                    format!("{:.1} {}", sensor.value, sensor.unit)
                }
                _ => format!("{:.2} {}", sensor.value, sensor.unit),
            }
            .into(),
            detail: "".into(),
        });
    }
    let keys = [
        "nav_cpu",
        "nav_gpu",
        "desktop_temperatures",
        "desktop_clocks",
        "desktop_fans",
        "desktop_power",
        "desktop_other_sensors",
    ];
    let mut result = Vec::new();
    for (index, rows) in groups.into_iter().enumerate() {
        if rows.is_empty() {
            continue;
        }
        // Reuse nested row models, so refresh does not recreate the sections.
        let existing = model
            .iter()
            .find(|g| g.title.as_str() == loc.tr(keys[index]))
            .map(|g| g.rows)
            .unwrap_or_else(|| ModelRc::new(VecModel::default()));
        crate::app::update_vec_model(existing.clone(), rows);
        result.push(DesktopSensorGroup {
            title: loc.tr(keys[index]).into(),
            rows: existing,
        });
    }
    crate::app::update_vec_model(model, result);
}
