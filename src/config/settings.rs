use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LcdDisplayMode {
    #[serde(alias = "DualInfographic")]
    CpuGpuLoad,
    LiquidGauge,
    SimulatedClock,
    CustomImage,
}

impl Default for LcdDisplayMode {
    fn default() -> Self {
        Self::CpuGpuLoad
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoolingProfile {
    Silent,
    Performance,
    Fixed(u8),
}

impl Default for CoolingProfile {
    fn default() -> Self {
        Self::Silent
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub start_in_tray: bool,
    pub minimize_to_tray: bool,
    pub polling_interval_ms: u64,
    pub lcd_mode: LcdDisplayMode,
    pub lcd_brightness: u8,
    pub pump_profile: CoolingProfile,
    pub fan_profile: CoolingProfile,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            start_in_tray: false,
            minimize_to_tray: true,
            polling_interval_ms: 1000,
            lcd_mode: LcdDisplayMode::CpuGpuLoad,
            lcd_brightness: 80,
            pump_profile: CoolingProfile::Silent,
            fan_profile: CoolingProfile::Silent,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Option<PathBuf> {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let mut path = PathBuf::from(app_data);
            path.push("AIPulse");
            path.push("config.json");
            Some(path)
        } else {
            None
        }
    }

    pub fn load_or_default() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                        Ok(config) => {
                            info!("Loaded configuration v{} from {:?}", config.version, path);
                            return config;
                        }
                        Err(err) => {
                            warn!("Failed to parse config file {:?}: {}. Using defaults.", path, err);
                        }
                    },
                    Err(err) => {
                        warn!("Failed to read config file {:?}: {}. Using defaults.", path, err);
                    }
                }
            }
        }
        let config = Self::default();
        config.save();
        config
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match serde_json::to_string_pretty(self) {
                Ok(json) => {
                    if let Err(err) = fs::write(&path, json) {
                        error!("Failed to write config to {:?}: {}", path, err);
                    } else {
                        info!("Saved configuration to {:?}", path);
                    }
                }
                Err(err) => error!("Failed to serialize config: {}", err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        assert_eq!(config.version, 1);
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.polling_interval_ms, 1000);
        assert_eq!(deserialized.lcd_brightness, 80);
        assert_eq!(deserialized.lcd_mode, LcdDisplayMode::CpuGpuLoad);
    }
}
