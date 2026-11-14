use std::collections::HashMap;

use crate::settings::LanguagePreference;

const EN_JSON: &str = include_str!("../../locales/en.json");
const PL_JSON: &str = include_str!("../../locales/pl.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleCode {
    En,
    Pl,
}

impl LocaleCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Pl => "pl",
        }
    }

    pub fn from_locale_tag(tag: &str) -> Self {
        let normalized = tag.trim().to_ascii_lowercase();
        if normalized == "pl" || normalized.starts_with("pl_") || normalized.starts_with("pl-") {
            Self::Pl
        } else {
            Self::En
        }
    }
}

pub fn detect_system_locale() -> LocaleCode {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() && trimmed != "C" && trimmed != "POSIX" {
                return LocaleCode::from_locale_tag(trimmed);
            }
        }
    }
    LocaleCode::En
}

pub fn resolve_locale(pref: LanguagePreference) -> LocaleCode {
    match pref {
        LanguagePreference::System => detect_system_locale(),
        LanguagePreference::English => LocaleCode::En,
        LanguagePreference::Polish => LocaleCode::Pl,
    }
}

#[derive(Debug, Clone)]
pub struct Localizer {
    locale: LocaleCode,
    en: HashMap<String, String>,
    active: HashMap<String, String>,
}

impl Localizer {
    pub fn new(locale: LocaleCode) -> Self {
        let en = parse_locale_map(EN_JSON);
        let active = match locale {
            LocaleCode::En => en.clone(),
            LocaleCode::Pl => parse_locale_map(PL_JSON),
        };
        Self { locale, en, active }
    }

    pub fn locale(&self) -> LocaleCode {
        self.locale
    }

    pub fn set_locale(&mut self, locale: LocaleCode) {
        if self.locale == locale {
            return;
        }
        self.locale = locale;
        self.active = match locale {
            LocaleCode::En => self.en.clone(),
            LocaleCode::Pl => parse_locale_map(PL_JSON),
        };
    }

    pub fn tr(&self, key: &str) -> &str {
        if let Some(val) = self.active.get(key) {
            if !val.is_empty() {
                return val.as_str();
            }
        }
        if let Some(val) = self.en.get(key) {
            return val.as_str();
        }
        key
    }
}

fn parse_locale_map(raw: &str) -> HashMap<String, String> {
    serde_json::from_str::<HashMap<String, String>>(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_tag_detection_supports_polish_and_english_fallback() {
        assert_eq!(LocaleCode::from_locale_tag("pl_PL.UTF-8"), LocaleCode::Pl);
        assert_eq!(LocaleCode::from_locale_tag("pl-PL"), LocaleCode::Pl);
        assert_eq!(LocaleCode::from_locale_tag("en_US.UTF-8"), LocaleCode::En);
        assert_eq!(LocaleCode::from_locale_tag("de_DE.UTF-8"), LocaleCode::En);
    }

    #[test]
    fn polish_translations_load_and_fallback_to_english() {
        let loc_en = Localizer::new(LocaleCode::En);
        let loc_pl = Localizer::new(LocaleCode::Pl);
        assert_eq!(loc_en.tr("nav_dashboard"), "Dashboard");
        assert_eq!(loc_pl.tr("nav_dashboard"), "Pulpit");
        assert_eq!(loc_pl.tr("nonexistent_key_fallback"), "nonexistent_key_fallback");
    }

    #[test]
    fn all_english_keys_exist_in_polish_locale() {
        let en = parse_locale_map(EN_JSON);
        let pl = parse_locale_map(PL_JSON);
        for key in en.keys() {
            assert!(
                pl.contains_key(key),
                "Missing Polish translation for key '{key}'"
            );
        }
    }
}
