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
    /// Path to alternative config file (legacy, use alt_configs)
    #[serde(default)]
    pub alt_config: Option<String>,
    /// List of alternative config files cycled on Mod tap
    #[serde(default)]
    pub alt_configs: Vec<String>,
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
}

fn default_device() -> DeviceConfig {
    DeviceConfig {
        touch: "/dev/input/event22".to_string(),
        consumer: "/dev/input/event23".to_string(),
    }
}

fn default_gesture() -> GestureConfig {
    GestureConfig { threshold: 80 }
}

fn default_touch() -> String { "/dev/input/event22".to_string() }
fn default_consumer() -> String { "/dev/input/event23".to_string() }
fn default_threshold() -> i32 { 80 }

impl Default for Config {
    fn default() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert("UP".to_string(), "command:hyprctl dispatch cyclenext prev".to_string());
        mappings.insert("DOWN".to_string(), "command:hyprctl dispatch cyclenext".to_string());
        mappings.insert("LEFT".to_string(), "command:hyprctl dispatch workspace e-1".to_string());
        mappings.insert("RIGHT".to_string(), "command:hyprctl dispatch workspace e+1".to_string());
        
        Config {
            device: default_device(),
            gesture: default_gesture(),
            mappings,
            alt_config: None,
            alt_configs: vec![],
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

    #[test]
    fn test_alt_config_parsing() {
        let toml_str = r#"
alt_config = "s10-ringctrl-hyprland.toml"

[gesture]
threshold = 80

[mappings]
UP = "command:hyprctl"
"#;
        let parsed: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.alt_config, Some("s10-ringctrl-hyprland.toml".to_string()));
    }

    #[test]
    fn test_alt_configs_parsing() {
        let toml_str = r#"
alt_configs = ["s10-ringctrl-hyprland.toml", "s10-ringctrl-gaming.toml"]

[gesture]
threshold = 80
"#;
        let parsed: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.alt_configs, vec!["s10-ringctrl-hyprland.toml", "s10-ringctrl-gaming.toml"]);
    }
}
