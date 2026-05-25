use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    pub name: String,
    pub version: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub default_click_delay: f64,
    pub default_playback_speed: f64,
    pub screenshot_format: String,
    pub failsafe_enabled: bool,
    pub max_recording_duration: u64,
    pub auto_save_recordings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Display {
    pub show_coordinates_on_click: bool,
    pub show_progress_during_playback: bool,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directories {
    pub screenshots: String,
    pub recordings: String,
    pub coordinates: String,
    pub templates: String,
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub window_titles: Vec<String>,
    pub detection_timeout: u64,
    pub click_precision: u32,
    pub template_matching_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnalyzer {
    pub google_gemini_api_key: String,
    pub enabled: bool,
    pub min_gold: u64,
    pub min_elixir: u64,
    pub min_dark_elixir: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoAttacker {
    #[serde(default)]
    pub attack_sessions: Vec<String>,
    #[serde(default = "default_max_search_attempts")]
    pub max_search_attempts: u32,
}

fn default_max_search_attempts() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bot: BotInfo,
    pub automation: Automation,
    pub display: Display,
    pub directories: Directories,
    pub game: Game,
    pub ai_analyzer: AiAnalyzer,
    #[serde(default)]
    pub auto_attacker: AutoAttacker,

    #[serde(skip)]
    pub config_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bot: BotInfo {
                name: "COC Attack Bot".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                author: "COC Bot User".into(),
            },
            automation: Automation {
                default_click_delay: 0.1,
                default_playback_speed: 1.0,
                screenshot_format: "PNG".into(),
                failsafe_enabled: true,
                max_recording_duration: 300,
                auto_save_recordings: true,
            },
            display: Display {
                show_coordinates_on_click: true,
                show_progress_during_playback: true,
                log_level: "INFO".into(),
            },
            directories: Directories {
                screenshots: "screenshots".into(),
                recordings: "recordings".into(),
                coordinates: "coordinates".into(),
                templates: "templates".into(),
                logs: "logs".into(),
            },
            game: Game {
                window_titles: vec![
                    "Clash of Clans".into(),
                    "Google Play Games".into(),
                    "BlueStacks".into(),
                    "NoxPlayer".into(),
                    "LDPlayer".into(),
                    "MEmu".into(),
                ],
                detection_timeout: 10,
                click_precision: 5,
                template_matching_threshold: 0.8,
            },
            ai_analyzer: AiAnalyzer {
                google_gemini_api_key: "REPLACE_ME_WITH_YOUR_GEMINI_API_KEY".into(),
                enabled: false,
                min_gold: 300_000,
                min_elixir: 300_000,
                min_dark_elixir: 2_000,
            },
            auto_attacker: AutoAttacker::default(),
            config_path: PathBuf::from("config.json"),
        }
    }
}

impl Config {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            let bytes = std::fs::read(path)?;
            let mut cfg: Config = serde_json::from_slice(&bytes)?;
            cfg.config_path = path.to_path_buf();
            tracing::info!("Configuration loaded from {}", path.display());
            Ok(cfg)
        } else {
            let mut cfg = Config::default();
            cfg.config_path = path.to_path_buf();
            cfg.save()?;
            Ok(cfg)
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.config_path, json)?;
        tracing::info!("Configuration saved to {}", self.config_path.display());
        Ok(())
    }
}
