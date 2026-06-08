use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub save_folder: PathBuf,
    pub hotkey: String,
    pub notify: bool,
}

impl Default for Config {
    fn default() -> Self {
        let save_folder = dirs::picture_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("cc-clipboard");
        Config {
            save_folder,
            hotkey: "ctrl+shift+s".to_string(),
            notify: true,
        }
    }
}

impl Config {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cc-clipboard")
            .join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<Config>(&data) {
                    let _ = std::fs::create_dir_all(&cfg.save_folder);
                    return cfg;
                }
            }
        }
        let cfg = Config::default();
        cfg.save();
        let _ = std::fs::create_dir_all(&cfg.save_folder);
        cfg
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}
