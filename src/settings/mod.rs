use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MIN_POLLING_INTERVAL_MS: u64 = 250;
pub const MAX_POLLING_INTERVAL_MS: u64 = 5000;
pub const DEFAULT_POLLING_INTERVAL_MS: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    System,
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    #[default]
    System,
    English,
    Polish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceMode {
    #[default]
    Modern,
    Professional,
    Gaming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub theme: ThemePreference,
    pub interface_mode: InterfaceMode,
    pub accent: u8,
    pub language: LanguagePreference,
    pub polling_interval_ms: u64,
    pub temperature_unit: TemperatureUnit,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            interface_mode: InterfaceMode::Modern,
            accent: 0,
            language: LanguagePreference::System,
            polling_interval_ms: DEFAULT_POLLING_INTERVAL_MS,
            temperature_unit: TemperatureUnit::Celsius,
        }
    }
}

impl UserSettings {
    pub fn validate(&mut self) {
        self.accent = self.accent.min(3);
        self.polling_interval_ms = self
            .polling_interval_ms
            .clamp(MIN_POLLING_INTERVAL_MS, MAX_POLLING_INTERVAL_MS);
    }

    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        let mut parsed: Self = serde_json::from_str(raw)?;
        parsed.validate();
        Ok(parsed)
    }

    pub fn config_file_path() -> Option<PathBuf> {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let p = PathBuf::from(xdg);
            if !p.as_os_str().is_empty() {
                return Some(p.join("voltix").join("settings.json"));
            }
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join(".config").join("voltix").join("settings.json"))
    }

    pub fn load_or_default(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        match fs::read_to_string(path) {
            Ok(content) => match Self::from_json_str(&content) {
                Ok(settings) => settings,
                Err(err) => {
                    log::warn!(
                        "Corrupted settings file at {}, using defaults: {err}",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                log::warn!(
                    "Unable to read settings file at {}, using defaults: {err}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let mut validated = self.clone();
        validated.validate();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir {}: {e}", parent.display()))?;
        }

        let json = serde_json::to_string_pretty(&validated)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write settings to {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_preferences_migrate_and_mode_roundtrips() {
        let old = UserSettings::from_json_str(r#"{"theme":"light"}"#).unwrap();
        assert_eq!(old.interface_mode, InterfaceMode::Modern);
        assert_eq!(old.accent, 0);
        for mode in [
            InterfaceMode::Modern,
            InterfaceMode::Professional,
            InterfaceMode::Gaming,
        ] {
            let settings = UserSettings {
                interface_mode: mode,
                accent: 3,
                ..UserSettings::default()
            };
            let restored =
                UserSettings::from_json_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(restored, settings);
        }
        assert_eq!(
            UserSettings::from_json_str(r#"{"accent":255}"#)
                .unwrap()
                .accent,
            3
        );
    }

    #[test]
    fn settings_clamp_polling_interval() {
        let mut too_low = UserSettings {
            polling_interval_ms: 50,
            ..UserSettings::default()
        };
        too_low.validate();
        assert_eq!(too_low.polling_interval_ms, MIN_POLLING_INTERVAL_MS);

        let mut too_high = UserSettings {
            polling_interval_ms: 99_000,
            ..UserSettings::default()
        };
        too_high.validate();
        assert_eq!(too_high.polling_interval_ms, MAX_POLLING_INTERVAL_MS);
    }

    #[test]
    fn corrupted_settings_json_falls_back_cleanly() {
        let parsed = UserSettings::from_json_str("{ not valid json");
        assert!(parsed.is_err());

        let tmp = std::env::temp_dir().join("voltix_test_corrupted_settings.json");
        let _ = fs::write(&tmp, "{ corrupted");
        let loaded = UserSettings::load_or_default(Some(&tmp));
        assert_eq!(loaded, UserSettings::default());
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn settings_roundtrip_save_and_load() {
        let tmp = std::env::temp_dir().join("voltix_test_roundtrip_settings.json");
        let _ = fs::remove_file(&tmp);

        let initial = UserSettings {
            theme: ThemePreference::Light,
            language: LanguagePreference::Polish,
            polling_interval_ms: 500,
            temperature_unit: TemperatureUnit::Fahrenheit,
            interface_mode: InterfaceMode::Gaming,
            accent: 2,
        };
        initial.save_to(&tmp).expect("save should succeed");
        let reloaded = UserSettings::load_or_default(Some(&tmp));
        assert_eq!(reloaded, initial);
        let _ = fs::remove_file(&tmp);
    }
}
