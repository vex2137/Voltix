use crate::settings::TemperatureUnit;

pub fn celsius_to_fahrenheit(celsius: f64) -> f64 {
    (celsius * 9.0 / 5.0) + 32.0
}

pub fn format_temperature(celsius: f64, unit: TemperatureUnit) -> String {
    match unit {
        TemperatureUnit::Celsius => format!("{celsius:.1} °C"),
        TemperatureUnit::Fahrenheit => format!("{:.1} °F", celsius_to_fahrenheit(celsius)),
    }
}

pub fn format_bytes_binary(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;

    let b = bytes as f64;
    if b >= TIB {
        format!("{:.2} TiB", b / TIB)
    } else if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_frequency_mhz(mhz: f64) -> String {
    if mhz >= 1000.0 {
        format!("{:.2} GHz", mhz / 1000.0)
    } else {
        format!("{mhz:.0} MHz")
    }
}

pub fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let mins = (seconds % 3_600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{days}d {hours:02}h {mins:02}m {secs:02}s")
    } else if hours > 0 {
        format!("{hours:02}h {mins:02}m {secs:02}s")
    } else {
        format!("{mins:02}m {secs:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_conversions_are_accurate() {
        assert!((celsius_to_fahrenheit(0.0) - 32.0).abs() < 1e-6);
        assert!((celsius_to_fahrenheit(100.0) - 212.0).abs() < 1e-6);
        assert!((celsius_to_fahrenheit(-40.0) - (-40.0)).abs() < 1e-6);
        assert_eq!(format_temperature(45.5, TemperatureUnit::Celsius), "45.5 °C");
        assert_eq!(
            format_temperature(25.0, TemperatureUnit::Fahrenheit),
            "77.0 °F"
        );
    }

    #[test]
    fn byte_formatting_uses_binary_units() {
        assert_eq!(format_bytes_binary(512), "512 B");
        assert_eq!(format_bytes_binary(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes_binary(16 * 1024 * 1024 * 1024), "16.00 GiB");
    }

    #[test]
    fn uptime_formatting_handles_days_and_hours() {
        assert_eq!(format_uptime(65), "01m 05s");
        assert_eq!(format_uptime(3661), "01h 01m 01s");
        assert_eq!(format_uptime(90061), "1d 01h 01m 01s");
    }
}
