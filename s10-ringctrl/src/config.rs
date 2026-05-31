use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_device")]
    pub device: DeviceConfig,
    #[serde(default = "default_gesture")]
    pub gesture: GestureConfig,
    #[serde(default)]
    pub mappings: HashMap<String, String>,
    /// Path to alternative config file (switched on long-press)
    #[serde(default)]
    pub alt_config: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(default = "default_touch")]
    pub touch: String,
    #[serde(default = "default_consumer")]
    pub consumer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GestureConfig {
    #[serde(default = "default_threshold")]
    pub threshold: i32,
    #[serde(default = "default_double_tap_ms")]
    pub double_tap_ms: u64,
    #[serde(default = "default_long_press_ms")]
    pub long_press_ms: u64,
}

fn default_device() -> DeviceConfig {
    DeviceConfig {
        touch: "/dev/input/event22".to_string(),
        consumer: "/dev/input/event23".to_string(),
    }
}

fn default_gesture() -> GestureConfig {
    GestureConfig {
        threshold: 80,
        double_tap_ms: 500,
        long_press_ms: 3000,
    }
}

fn default_touch() -> String { "/dev/input/event22".to_string() }
fn default_consumer() -> String { "/dev/input/event23".to_string() }
fn default_threshold() -> i32 { 80 }
fn default_double_tap_ms() -> u64 { 500 }
fn default_long_press_ms() -> u64 { 3000 }

impl Default for Config {
    fn default() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert("UP".to_string(), "command:hyprctl dispatch cyclenext prev".to_string());
        mappings.insert("DOWN".to_string(), "command:hyprctl dispatch cyclenext".to_string());
        mappings.insert("LEFT".to_string(), "command:hyprctl dispatch workspace e-1".to_string());
        mappings.insert("RIGHT".to_string(), "command:hyprctl dispatch workspace e+1".to_string());
        mappings.insert("TAP".to_string(), "text:继续\n".to_string());
        mappings.insert("DOUBLE_TAP".to_string(), "command:hyprctl dispatch togglespecialworkspace".to_string());
        
        Config {
            device: default_device(),
            gesture: default_gesture(),
            mappings,
            alt_config: None,
        }
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save_default(path: &str) -> anyhow::Result<()> {
        let config = Config::default();
        let content = toml::to_string_pretty(&config)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_mapping(&self, gesture: &str) -> Option<&str> {
        self.mappings.get(gesture).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.device.touch, "/dev/input/event22");
        assert_eq!(cfg.gesture.threshold, 80);
        assert!(cfg.mappings.contains_key("UP"));
    }

    #[test]
    fn test_toml_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.mappings.len(), cfg.mappings.len());
    }
}
