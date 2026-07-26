use serde::Deserialize;
use std::path::PathBuf;
#[derive(Deserialize)]
pub struct Config {
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub model: Option<String>,
}

pub fn load_config() -> Config {
    let path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("liana")
        .join("config.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read config from {}", path.display()));
    serde_json::from_str(&content).expect("Failed to parse config")
}
