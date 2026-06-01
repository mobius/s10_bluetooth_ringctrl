use anyhow::Result;
use clap::Parser;
use std::sync::mpsc;
use std::thread;
use tracing::{info, warn};

mod action;
mod config;
mod gesture;

use action::ActionExecutor;
use config::Config;
use gesture::{Gesture, GestureDetector, TouchEvent};

#[derive(Parser, Debug)]
#[command(name = "s10-ringctrl")]
#[command(about = "S10 Bluetooth Remote Input Remapper (Linux MVP)")]
struct Cli {
    #[arg(short, long, default_value = "s10-ringctrl.toml")]
    config: String,
    #[arg(short, long)]
    touch: Option<String>,
    #[arg(long)]
    consumer: Option<String>,
    #[arg(long)]
    generate_config: bool,
    #[arg(long)]
    remap: bool,
    #[arg(short, long)]
    quiet: bool,
}

enum Event {
    Touch(evdev::InputEvent),
    Consumer(evdev::InputEvent),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.generate_config {
        Config::save_default(&cli.config)?;
        println!("Default config saved to: {}", cli.config);
        return Ok(());
    }

    if cli.quiet {
        tracing_subscriber::fmt().with_max_level(tracing::Level::WARN).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let mut config = Config::load(&cli.config)?;
    if let Some(touch) = cli.touch {
        config.device.touch = touch;
    }
    if let Some(consumer) = cli.consumer {
        config.device.consumer = consumer;
    }

    let primary_config_path = cli.config.clone();
    let mut profiles = vec![primary_config_path.clone()];
    if let Some(ref alt) = config.alt_config {
        profiles.push(alt.clone());
    }
    for alt in &config.alt_configs {
        if !profiles.contains(alt) {
            profiles.push(alt.clone());
        }
    }
    let mut profile_index = 0usize;

    info!("S10 RingCtrl starting...");
    info!("Touch device: {}", config.device.touch);
    info!("Consumer device: {}", config.device.consumer);
    info!("Threshold: {}", config.gesture.threshold);
    info!("Mode: {}", if cli.remap { "REMAP" } else { "DEBUG" });
    if profiles.len() > 1 {
        info!("Profiles ({}): {:?}", profiles.len(), profiles);
    }

    let (tx, rx) = mpsc::channel::<Event>();

    let touch_path = config.device.touch.clone();
    let tx_touch = tx.clone();
    thread::spawn(move || {
        run_device_loop(&touch_path, "touch", tx_touch, Event::Touch);
    });

    let consumer_path = config.device.consumer.clone();
    let tx_consumer = tx;
    thread::spawn(move || {
        run_device_loop(&consumer_path, "consumer", tx_consumer, Event::Consumer);
    });

    let mut detector = GestureDetector::new(config.gesture.threshold);
    let executor = ActionExecutor::new();

    info!("Capturing events. Press Ctrl+C to stop.");

    loop {
        match rx.recv() {
            Ok(Event::Touch(ev)) => {
                if let Some(gesture) = handle_touch_event(&ev, &mut detector) {
                    if gesture == Gesture::Tap {
                        let (_x, y) = detector.last_position();
                        let is_mod = y.map_or(false, |v| v > 600);
                        if is_mod && profiles.len() > 1 {
                            executor.stop_repeat();
                            profile_index = (profile_index + 1) % profiles.len();
                            let next_path = profiles[profile_index].clone();
                            match Config::load(&next_path) {
                                Ok(new_config) => {
                                    config = new_config;
                                    info!("Config switched to: {} ({}/{})", next_path, profile_index + 1, profiles.len());
                                }
                                Err(e) => {
                                    warn!("Failed to load config '{}': {}", next_path, e);
                                }
                            }
                        } else {
                            executor.stop_repeat();
                            let name = gesture.as_str();
                            if cli.remap {
                                if let Some(action) = config.get_mapping(name) {
                                    executor.execute(action, name);
                                }
                            } else {
                                println!("[DEBUG] >>> {} DETECTED <<<", name);
                            }
                        }
                    } else {
                        let name = gesture.as_str();
                        executor.stop_repeat();
                        if cli.remap {
                            if let Some(action) = config.get_mapping(name) {
                                if action.starts_with("key:") {
                                    let (count, key) = parse_repeat_key(&action[4..]);
                                    executor.repeat_key(key, count, name);
                                } else {
                                    executor.execute(action, name);
                                }
                            }
                        } else {
                            println!("[DEBUG] >>> {} DETECTED <<<", name);
                        }
                    }
                }
            }
            Ok(Event::Consumer(ev)) => {
                executor.stop_repeat();
                handle_consumer_event(&ev, cli.remap, &config, &executor);
            }
            Err(_) => {
                info!("Event channel closed, exiting.");
                break;
            }
        }
    }

    Ok(())
}

/// Parse a key string with optional repeat prefix, e.g. "20W" -> (20, "W"), "A" -> (1, "A").
fn parse_repeat_key(s: &str) -> (u32, &str) {
    let first_non_digit = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if first_non_digit == 0 {
        (1, s)
    } else {
        let count = s[..first_non_digit].parse::<u32>().unwrap_or(1);
        (count.max(1), &s[first_non_digit..])
    }
}

fn open_device(path: &str, label: &str) -> Result<evdev::Device> {
    use std::path::Path;
    if !Path::new(path).exists() {
        anyhow::bail!("{} device not found: {}", label, path);
    }
    let mut dev = evdev::Device::open(path)?;
    if let Err(e) = dev.grab() {
        warn!("Failed to grab {} ({}): {}", label, path, e);
    } else {
        info!("Grabbed {}: {}", label, path);
    }
    Ok(dev)
}

fn run_device_loop(
    path: &str,
    label: &str,
    tx: mpsc::Sender<Event>,
    wrap: fn(evdev::InputEvent) -> Event,
) {
    use std::path::Path;
    let mut reported_missing = false;
    loop {
        if !Path::new(path).exists() {
            if !reported_missing {
                info!("{} device disconnected, waiting for reconnect...", label);
                reported_missing = true;
            }
            thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }
        reported_missing = false;
        match open_device(path, label) {
            Ok(mut dev) => {
                info!("{} device connected", label);
                let mut backoff = 0u32;
                loop {
                    match dev.fetch_events() {
                        Ok(events) => {
                            backoff = 0;
                            for ev in events {
                                if tx.send(wrap(ev)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            if !Path::new(path).exists() {
                                info!("{} device disconnected", label);
                                break;
                            }
                            let delay_ms = 100u64 * 2u64.pow(backoff.min(6));
                            warn!("{} device error (retry in {}ms): {}", label, delay_ms, e);
                            thread::sleep(std::time::Duration::from_millis(delay_ms));
                            backoff += 1;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to open {} device: {}", label, e);
                thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
}

fn handle_touch_event(
    ev: &evdev::InputEvent,
    detector: &mut GestureDetector,
) -> Option<Gesture> {
    use evdev::AbsoluteAxisType;

    match ev.kind() {
        evdev::InputEventKind::AbsAxis(axis) => match axis {
            AbsoluteAxisType::ABS_MT_TRACKING_ID => {
                if ev.value() >= 0 {
                    detector.feed(TouchEvent::TrackingStart);
                    None
                } else {
                    detector.feed(TouchEvent::TrackingEnd)
                }
            }
            AbsoluteAxisType::ABS_MT_POSITION_X | AbsoluteAxisType::ABS_X => {
                detector.feed(TouchEvent::Position {
                    x: Some(ev.value()),
                    y: None,
                });
                None
            }
            AbsoluteAxisType::ABS_MT_POSITION_Y | AbsoluteAxisType::ABS_Y => {
                detector.feed(TouchEvent::Position {
                    x: None,
                    y: Some(ev.value()),
                });
                None
            }
            _ => None,
        },
        _ => None,
    }
}

fn handle_consumer_event(
    ev: &evdev::InputEvent,
    remap: bool,
    config: &Config,
    executor: &ActionExecutor,
) {
    if let evdev::InputEventKind::Key(key) = ev.kind() {
        let name = if key == evdev::Key::KEY_VOLUMEDOWN {
            "POWER".to_string()
        } else {
            format!("{:?}", key)
        };
        if remap {
            if ev.value() == 1 {
                if let Some(action) = config.get_mapping(&name) {
                    executor.execute(action, &name);
                }
            }
        } else {
            let state = match ev.value() {
                0 => "RELEASE",
                1 => "PRESS",
                2 => "REPEAT",
                _ => "UNKNOWN",
            };
            println!("[DEBUG] [CONSUMER] {} {}", state, name);
        }
    }
}
