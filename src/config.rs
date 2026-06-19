use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl Config {
    /// Standard config file path for the platform:
    ///   Linux:   ~/.config/liana/config.json
    ///   macOS:   ~/Library/Application Support/liana/config.json
    ///   Windows: %APPDATA%\liana\config.json
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("liana").join("config.json"))
    }

    /// Load config from the standard path. Returns `None` if no file exists.
    pub fn load() -> Option<Config> {
        let path = Self::path()?;
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save config to the standard path, creating parent directories as needed.
    pub fn save(&self) -> Result<(), String> {
        let path =
            Self::path().ok_or_else(|| "Could not determine config directory".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {e}"))?;
        }
        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("Serialize error: {e}"))?;
        std::fs::write(&path, content).map_err(|e| format!("Write error: {e}"))?;
        Ok(())
    }

    /// Run an interactive step-by-step setup wizard when no config file exists.
    pub fn setup() -> Config {
        println!("No config file found — let's set one up.");
        println!();

        // API key
        let api_key = loop {
            print!("API Key (sk-...): ");
            std::io::stdout().flush().ok();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            let trimmed = input.trim().to_string();
            if !trimmed.is_empty() {
                break trimmed;
            }
            println!("  API key cannot be empty.");
        };

        // Base URL (with default)
        print!("Base URL [https://api.deepseek.com]: ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let base_url = {
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                "https://api.deepseek.com".to_string()
            } else {
                trimmed
            }
        };

        // Model (with default)
        print!("Model [deepseek-v4-flash]: ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let model = {
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                "deepseek-v4-flash".to_string()
            } else {
                trimmed
            }
        };

        let config = Config {
            api_key,
            base_url,
            model,
        };

        println!();
        match config.save() {
            Ok(()) => {
                let path = Self::path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "?".to_string());
                println!("Configuration saved to: {path}");
            }
            Err(e) => {
                eprintln!("Warning: could not save config file: {e}");
                eprintln!("Using config for this session only.");
            }
        }
        println!();

        config
    }
}
