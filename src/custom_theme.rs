//! Validated user-defined colors. Input is data only; no paths or commands are executed.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomTheme {
    pub name: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
    pub cpu: String,
    pub gpu: String,
    pub ram: String,
    pub vram: String,
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self {
            name: "My theme".into(),
            background: "#0B1019".into(),
            surface: "#192637".into(),
            text: "#F1F5F9".into(),
            accent: "#38BDF8".into(),
            cpu: "#38BDF8".into(),
            gpu: "#A78BFA".into(),
            ram: "#4ADE80".into(),
            vram: "#F59E0B".into(),
        }
    }
}

pub fn rgb(value: &str) -> Option<(u8, u8, u8)> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let parse = |range: std::ops::Range<usize>| u8::from_str_radix(value.get(range)?, 16).ok();
    Some((parse(1..3)?, parse(3..5)?, parse(5..7)?))
}

fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let linear = |channel: u8| {
        let x = f64::from(channel) / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

impl CustomTheme {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || self.name.len() > 40
            || self.name.chars().any(char::is_control)
        {
            return Err("name must be 1–40 characters without control characters");
        }
        let background = rgb(&self.background).ok_or("colors must use #RRGGBB")?;
        let surface = rgb(&self.surface).ok_or("colors must use #RRGGBB")?;
        let text = rgb(&self.text).ok_or("colors must use #RRGGBB")?;
        if contrast(text, background) < 4.5 || contrast(text, surface) < 4.5 {
            return Err("text must have at least 4.5:1 contrast on background and surface");
        }
        for color in [&self.accent, &self.cpu, &self.gpu, &self.ram, &self.vram] {
            let color = rgb(color).ok_or("colors must use #RRGGBB")?;
            if contrast(color, background) < 4.5 || contrast(color, surface) < 4.5 {
                return Err(
                    "metric and accent colors must have at least 4.5:1 contrast on both surfaces",
                );
            }
        }
        Ok(())
    }

    pub fn from_json(raw: &str) -> Result<Self, &'static str> {
        if raw.len() > 4096 {
            return Err("theme JSON is too large");
        }
        let theme: Self = serde_json::from_str(raw).map_err(|_| "invalid theme JSON")?;
        theme.validate()?;
        Ok(theme)
    }

    pub fn color(value: &str) -> slint::Color {
        let (r, g, b) = rgb(value).unwrap_or((255, 255, 255));
        slint::Color::from_rgb_u8(r, g, b)
    }

    pub fn dark_background(&self) -> bool {
        rgb(&self.background).is_some_and(|color| luminance(color) < 0.179)
    }

    pub fn accent_foreground(&self) -> slint::Color {
        let accent = rgb(&self.accent).unwrap_or((255, 255, 255));
        if contrast(accent, (0, 0, 0)) >= contrast(accent, (255, 255, 255)) {
            slint::Color::from_rgb_u8(0, 0, 0)
        } else {
            slint::Color::from_rgb_u8(255, 255, 255)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_theme_json_rejects_invalid_colors_contrast_and_unknown_fields() {
        let valid = CustomTheme::default();
        let json = serde_json::to_string(&valid).unwrap();
        assert_eq!(CustomTheme::from_json(&json).unwrap(), valid);
        let mut invalid = valid.clone();
        invalid.cpu = "#XYZ123".into();
        assert!(invalid.validate().is_err());
        invalid = valid.clone();
        invalid.text = "#111111".into();
        assert!(invalid.validate().is_err());
        invalid = valid;
        invalid.name = "\n".into();
        assert!(invalid.validate().is_err());
        assert!(
            CustomTheme::from_json(&json.replace("\"cpu\":", "\"unknown\":1,\"cpu\":")).is_err()
        );
        assert!(CustomTheme::from_json(&" ".repeat(4097)).is_err());
    }
}
