use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Встраиваем языковые файлы в бинарник
const LANG_EN: &str = include_str!("../languages/en.json");
const LANG_RU: &str = include_str!("../languages/ru.json");
const LANG_DE: &str = include_str!("../languages/de.json");
const LANG_ZH: &str = include_str!("../languages/zh.json");
const LANG_ES: &str = include_str!("../languages/es.json");
const LANG_FR: &str = include_str!("../languages/fr.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    English,
    Russian,
    German,
    Chinese,
    Spanish,
    French,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Russian => "ru",
            Language::German => "de",
            Language::Chinese => "zh",
            Language::Spanish => "es",
            Language::French => "fr",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Russian => "Русский",
            Language::German => "Deutsch",
            Language::Chinese => "中文",
            Language::Spanish => "Español",
            Language::French => "Français",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "ru" | "ru-RU" => Language::Russian,
            "de" | "de-DE" => Language::German,
            "zh" | "zh-CN" | "zh-TW" => Language::Chinese,
            "es" | "es-ES" | "es-MX" => Language::Spanish,
            "fr" | "fr-FR" => Language::French,
            _ => Language::English, // Default
        }
    }

    pub fn all() -> Vec<Language> {
        vec![
            Language::English,
            Language::Russian,
            Language::German,
            Language::Chinese,
            Language::Spanish,
            Language::French,
        ]
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Translations {
    translations: HashMap<String, String>,
}

impl Translations {
    pub fn load(lang: Language) -> Self {
        // Получаем встроенный JSON для выбранного языка
        let content = match lang {
            Language::English => LANG_EN,
            Language::Russian => LANG_RU,
            Language::German => LANG_DE,
            Language::Chinese => LANG_ZH,
            Language::Spanish => LANG_ES,
            Language::French => LANG_FR,
        };
        
        match serde_json::from_str(content) {
            Ok(translations) => Self { translations },
            Err(e) => {
                eprintln!("Failed to parse language {}: {}", lang.code(), e);
                Self::fallback()
            }
        }
    }

    fn fallback() -> Self {
        // Minimal English fallback
        let mut translations = HashMap::new();
        translations.insert("app_title".to_string(), "Cedar Folder Size Analyzer".to_string());
        Self { translations }
    }

    pub fn get(&self, key: &str) -> String {
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("[{}]", key))
    }

    pub fn get_fmt(&self, key: &str, args: &[&str]) -> String {
        let template = self.get(key);
        let mut result = template;
        for (i, arg) in args.iter().enumerate() {
            result = result.replace(&format!("%{}", i + 1), arg);
            result = result.replace("%d", arg);
        }
        result
    }
}

/// Определение системного языка
pub fn detect_system_language() -> Language {
    if let Some(locale) = sys_locale::get_locale() {
        Language::from_code(&locale)
    } else {
        Language::English
    }
}

/// Определение системной темы (тёмная/светлая)
pub fn detect_system_theme() -> bool {
    // Попытка определить тему Windows
    #[cfg(windows)]
    {
        use std::process::Command;
        
        // Проверяем реестр Windows для темы
        if let Ok(output) = Command::new("reg")
            .args(&["query", "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", "/v", "AppsUseLightTheme"])
            .output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);
            if output_str.contains("0x0") {
                return true; // Dark mode
            }
        }
    }
    
    // По умолчанию тёмная тема
    true
}

