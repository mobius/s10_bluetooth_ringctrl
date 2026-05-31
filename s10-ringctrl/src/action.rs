use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Action executor supporting command/key/text/combo
pub struct ActionExecutor {
    wtype_available: bool,
    hypr_env: HashMap<String, String>,
}

impl ActionExecutor {
    pub fn new() -> Self {
        let wtype_available = Command::new("which")
            .arg("wtype")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if wtype_available {
            info!("wtype found, key/text injection enabled");
        } else {
            warn!("wtype not found, key/text injection disabled. Install: sudo pacman -S wtype");
        }

        let hypr_env = discover_hypr_env();
        Self { wtype_available, hypr_env }
    }

    pub fn execute(&self, action: &str, source: &str) {
        if action == "passthrough" || action.is_empty() {
            return;
        }

        if action.starts_with("command:") {
            let cmd = &action[8..];
            self.run_command(cmd, source);
        } else if action.starts_with("text:") {
            let text = &action[5..];
            self.type_text(text, source);
        } else if action.starts_with("key:") {
            let key = &action[4..];
            self.type_key(key, source);
        } else if action.starts_with("combo:") {
            let combo = &action[6..];
            self.type_combo(combo, source);
        } else {
            warn!("Unknown action format: {}", action);
        }
    }

    fn run_command(&self, cmd: &str, source: &str) {
        let mut env_vars: HashMap<String, String> = std::env::vars().collect();
        env_vars.extend(self.hypr_env.clone());
        if !env_vars.contains_key("DISPLAY") {
            env_vars.insert("DISPLAY".to_string(), ":1".to_string());
        }
        if !env_vars.contains_key("XDG_CURRENT_DESKTOP") {
            env_vars.insert("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string());
        }

        debug!("Executing command: {}", cmd);
        match Command::new("sh").arg("-c").arg(cmd).envs(&env_vars).output() {
            Ok(output) => {
                if output.status.success() {
                    info!("[command] {} -> {}", source, cmd);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!("[command err] {}: {}", source, stderr.trim());
                }
            }
            Err(e) => {
                error!("[command fail] {}: {}", source, e);
            }
        }
    }

    fn type_text(&self, text: &str, source: &str) {
        if !self.wtype_available {
            warn!("[wtype not found] cannot type: {}", text);
            return;
        }
        let parts: Vec<&str> = text.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if !part.is_empty() {
                let _ = Command::new("wtype")
                    .arg(part)
                    .envs(&self.hypr_env)
                    .output();
            }
            if i < parts.len() - 1 || text.ends_with("\\n") {
                let _ = Command::new("wtype")
                    .args(["-k", "Return"])
                    .envs(&self.hypr_env)
                    .output();
            }
        }
        info!("[wtype text] {} -> {}", source, text.replace("\\n", "[Enter]"));
    }

    fn type_key(&self, key: &str, source: &str) {
        if !self.wtype_available {
            warn!("[wtype not found] cannot emit key: {}", key);
            return;
        }
        let wtype_name = if key.starts_with("KEY_") { &key[4..] } else { key };
        let _ = Command::new("wtype")
            .arg(wtype_name)
            .envs(&self.hypr_env)
            .output();
        info!("[wtype key] {} -> {}", source, wtype_name);
    }

    fn type_combo(&self, combo: &str, source: &str) {
        if !self.wtype_available {
            warn!("[wtype not found] cannot emit combo: {}", combo);
            return;
        }
        let keys: Vec<String> = combo.split('+').map(|k| {
            let k = k.trim();
            if k.starts_with("KEY_") { k[4..].to_string() } else { k.to_string() }
        }).collect();
        
        let mut args = Vec::new();
        for k in &keys {
            args.push("-k");
            args.push(k.as_str());
        }
        let _ = Command::new("wtype")
            .args(&args)
            .envs(&self.hypr_env)
            .output();
        info!("[wtype combo] {} -> {}", source, combo);
    }
}

fn discover_hypr_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    
    // Find HYPRLAND_INSTANCE_SIGNATURE from /run/user/*/hypr/
    for entry in std::fs::read_dir("/run/user").ok().into_iter().flatten() {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let hypr_dir = entry.path().join("hypr");
        if !hypr_dir.exists() { continue; }
        
        for sub in std::fs::read_dir(&hypr_dir).ok().into_iter().flatten() {
            let sub = match sub { Ok(s) => s, Err(_) => continue };
            if sub.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let sig = sub.file_name().to_string_lossy().to_string();
                if sig.contains('_') {
                    env.insert("HYPRLAND_INSTANCE_SIGNATURE".to_string(), sig);
                    env.insert("XDG_RUNTIME_DIR".to_string(), 
                        entry.path().to_string_lossy().to_string());
                    break;
                }
            }
        }
    }
    
    // Find WAYLAND_DISPLAY
    for wayland in ["wayland-1", "wayland-0"] {
        for entry in std::fs::read_dir("/run/user").ok().into_iter().flatten() {
            let entry = match entry { Ok(e) => e, Err(_) => continue };
            if entry.path().join(wayland).exists() {
                env.insert("WAYLAND_DISPLAY".to_string(), wayland.to_string());
                if !env.contains_key("XDG_RUNTIME_DIR") {
                    env.insert("XDG_RUNTIME_DIR".to_string(),
                        entry.path().to_string_lossy().to_string());
                }
                break;
            }
        }
    }
    
    env
}
