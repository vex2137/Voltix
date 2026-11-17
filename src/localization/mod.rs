use std::collections::HashMap;

use crate::settings::LanguagePreference;

mod ui_keys;
pub use ui_keys::UI_KEYS;

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
    #[cfg(target_os = "android")]
    {
        for prop in [
            "persist.sys.locale",
            "persist.sys.language",
            "ro.product.locale",
        ] {
            if let Some(val) = crate::platform::android::get_system_property(prop) {
                return LocaleCode::from_locale_tag(&val);
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

    /// Localize catalogued diagnostic templates while preserving paths, keys and OS details.
    pub fn diagnostic(&self, text: &str) -> String {
        self.diagnostic_at(text, 0)
    }
    fn diagnostic_at(&self, text: &str, depth: usize) -> String {
        if self.locale == LocaleCode::En || depth > 4 {
            return text.to_string();
        }
        if let Some((key, _)) = self.en.iter().find(|(_, value)| value.as_str() == text) {
            return self.tr(key).to_string();
        }
        for (key, template) in &self.en {
            if !template.contains("{0}") {
                continue;
            }
            if let Some(args) = diagnostic_arguments(template, text) {
                let mut translated = self.tr(key).to_string();
                for (i, value) in args.iter().enumerate() {
                    translated = translated
                        .replace(&format!("{{{i}}}"), &self.diagnostic_at(value, depth + 1));
                }
                return translated;
            }
        }
        if text.contains('\n') {
            return text
                .lines()
                .map(|line| self.diagnostic_at(line, depth + 1))
                .collect::<Vec<_>>()
                .join("\n");
        }
        text.to_string()
    }
    pub fn tr<'a>(&'a self, key: &'a str) -> &'a str {
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

fn diagnostic_arguments(template: &str, text: &str) -> Option<Vec<String>> {
    let mut rest = text;
    let mut tail = template;
    let mut args = Vec::new();
    while let Some(start) = tail.find('{') {
        rest = rest.strip_prefix(&tail[..start])?;
        let end = tail[start..].find('}')? + start;
        if tail[start + 1..end].parse::<usize>().ok()? != args.len() {
            return None;
        }
        tail = &tail[end + 1..];
        let delimiter = tail.split('{').next().unwrap_or("");
        let stop = if delimiter.is_empty() {
            rest.len()
        } else {
            rest.find(delimiter)?
        };
        args.push(rest[..stop].to_string());
        rest = &rest[stop..];
    }
    if rest == tail && !args.is_empty() {
        Some(args)
    } else {
        None
    }
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
        assert_eq!(
            loc_pl.tr("nonexistent_key_fallback"),
            "nonexistent_key_fallback"
        );
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
    #[test]
    fn catalogs_are_complete_and_templates_preserve_parameters() {
        let en = parse_locale_map(EN_JSON);
        let pl = parse_locale_map(PL_JSON);
        assert_eq!(en.len(), pl.len());
        for (key, value) in &en {
            let translated = pl
                .get(key)
                .unwrap_or_else(|| panic!("Missing Polish: {key}"));
            assert!(!translated.trim().is_empty());
            for n in 0..8 {
                let token = format!("{{{n}}}");
                assert_eq!(
                    value.matches(&token).count(),
                    translated.matches(&token).count(),
                    "{key}"
                );
            }
        }
        for key in UI_KEYS {
            assert!(en.contains_key(*key), "{key}");
        }
        let mut loc = Localizer::new(LocaleCode::Pl);
        let key = UI_KEYS[66];
        loc.active.remove(key);
        assert_eq!(loc.tr(key), en[key]);
        assert_eq!(loc.tr("missing_key"), "missing_key");
        loc.set_locale(LocaleCode::En);
        assert_eq!(loc.tr(key), en[key]);
        let loc = Localizer::new(LocaleCode::Pl);
        let message = loc.diagnostic("Review write to /tmp/MangoHud.conf. Existing bytes will be backed up. Confirm only after reviewing the editor and destination. Nothing written yet.");
        assert!(
            message.starts_with("Sprawdź zapis do /tmp/MangoHud.conf."),
            "{message}"
        );
        assert_eq!(
            loc.diagnostic("Error: Explicit confirmation is required"),
            "Błąd: Wymagane jest jawne potwierdzenie"
        );
    }
}
