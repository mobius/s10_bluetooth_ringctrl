use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Action executor supporting command/key/text/combo
pub struct ActionExecutor {
    wtype_available: bool,
    hypr_env: HashMap<String, String>,
    repeat_id: Arc<Mutex<u64>>,
}

impl ActionExecutor {
    pub fn new() -> Self {
        let wtype_available = Command::new("which")
            .arg("wtype")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let ydotool_available = Command::new("which")
            .arg("ydotool")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if wtype_available {
            info!("wtype found, key/text injection enabled");
        } else {
            warn!("wtype not found, key/text injection disabled. Install: sudo pacman -S wtype");
        }
        if ydotool_available {
            info!("ydotool found, mouse injection enabled");
        } else {
            warn!("ydotool not found, mouse injection disabled. Install: sudo pacman -S ydotool");
        }

        let hypr_env = discover_hypr_env();
        Self { wtype_available, hypr_env, repeat_id: Arc::new(Mutex::new(0)) }
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
        } else if action.starts_with("mouse:") {
            let btn = &action[6..];
            self.type_mouse(btn, source);
        } else {
            warn!("Unknown action format: {}", action);
        }
    }

    /// Repeatedly emit a key (e.g. WASD in gaming mode).
    /// count: how many times to send (e.g. 20 from "key:20W").
    /// A new call auto-cancels the previous repeat.
    pub fn repeat_key(&self, key: &str, count: u32, source: &str) {
        if !self.wtype_available {
            warn!("[wtype not found] cannot repeat key: {}", key);
            return;
        }
        let wtype_name = if key.starts_with("KEY_") { key[4..].to_string() } else { key.to_string() };

        let mut guard = self.repeat_id.lock().unwrap();
        *guard += 1;
        let my_id = *guard;
        drop(guard);

        let env = self.hypr_env.clone();
        let repeat_id = self.repeat_id.clone();
        let source = source.to_string();

        thread::spawn(move || {
            for i in 0..count {
                if *repeat_id.lock().unwrap() != my_id { break; }
                let mut cmd = Command::new("wtype");
                cmd.envs(&env);
                cmd.arg(&wtype_name);
                let _ = cmd.output();
                if i == 0 {
                    info!("[wtype repeat] {} -> {} (x{})", source, wtype_name, count);
                }
                thread::sleep(Duration::from_millis(25));
            }
        });
    }

    /// Cancel any ongoing repeat_key sequence.
    pub fn stop_repeat(&self) {
        let mut guard = self.repeat_id.lock().unwrap();
        *guard += 1;
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
                let _ = self.wtype_cmd().arg(part).output();
            }
            if i < parts.len() - 1 || text.ends_with("\\n") {
                let _ = self.wtype_cmd().args(["-k", "Return"]).output();
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
        let _ = self.wtype_cmd().arg(wtype_name).output();
        info!("[wtype key] {} -> {}", source, wtype_name);
    }

    fn type_mouse(&self, btn: &str, source: &str) {
        let code = match btn {
            "left" | "Left" | "LEFT" => "0xC0",
            "right" | "Right" | "RIGHT" => "0xC1",
            "middle" | "Middle" | "MIDDLE" => "0xC2",
            _ => {
                warn!("Unknown mouse button: {}", btn);
                return;
            }
        };
        let mut cmd = if let Some(user) = std::env::var("SUDO_USER").ok() {
            let mut c = Command::new("sudo");
            c.arg("-u").arg(&user).arg("-E").arg("ydotool");
            c
        } else {
            Command::new("ydotool")
        };
        cmd.arg("click").arg(code);
        cmd.envs(&self.hypr_env);
        let _ = cmd.output();
        info!("[ydotool] {} -> {}", source, btn);
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
        let _ = self.wtype_cmd().args(&args).output();
        info!("[wtype combo] {} -> {}", source, combo);
    }

    fn wtype_cmd(&self) -> Command {
        if let Some(user) = std::env::var("SUDO_USER").ok() {
            let mut cmd = Command::new("sudo");
            cmd.arg("-u").arg(&user).arg("-E").arg("wtype");
            cmd.envs(&self.hypr_env);
            cmd
        } else {
            let mut cmd = Command::new("wtype");
            cmd.envs(&self.hypr_env);
            cmd
        }
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
