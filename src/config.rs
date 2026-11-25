use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub dark_mode: bool,
    pub language: crate::i18n::Language,
    pub last_path: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dark_mode: crate::i18n::detect_system_theme(),
            language: crate::i18n::detect_system_language(),
            last_path: None,
        }
    }
}

pub fn config_file_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|dir| dir.join("cedar-folder-size-analyzer").join("config.json"))
}

pub fn save_config(config: &AppConfig) {
    let Some(config_file) = config_file_path() else {
        return;
    };

    let Some(config_dir) = config_file.parent() else {
        return;
    };

    if let Err(e) = std::fs::create_dir_all(config_dir) {
        eprintln!("Failed to create config directory: {}", e);
        return;
    }

    match serde_json::to_string_pretty(config) {
        Ok(json) => {
            if let Err(e) = std::fs::write(config_file, json) {
                eprintln!("Failed to save config: {}", e);
            }
        }
        Err(e) => eprintln!("Failed to serialize config: {}", e),
    }
}

pub fn load_config() -> AppConfig {
    if let Some(config_file) = config_file_path() {
        if let Ok(json) = std::fs::read_to_string(&config_file) {
            if let Ok(config) = serde_json::from_str(&json) {
                return config;
            }
        }
    }

    AppConfig::default()
}
