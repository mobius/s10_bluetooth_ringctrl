use anyhow::Result;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
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
    /// Config file path
    #[arg(short, long, default_value = "s10-ringctrl.toml")]
    config: String,

    /// Touchpad input device path
    #[arg(short, long)]
    touch: Option<String>,

    /// Consumer control input device path
    #[arg(long)]
    consumer: Option<String>,

    /// Generate default config file and exit
    #[arg(long)]
    generate_config: bool,

    /// Enable active remapping (default is debug-only)
    #[arg(long)]
    remap: bool,

    /// Quiet output (errors only)
    #[arg(short, long)]
    quiet: bool,
}

enum Event {
    Touch(evdev::InputEvent),
    Consumer(evdev::InputEvent),
    LongPress,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.generate_config {
        Config::save_default(&cli.config)?;
        println!("Default config saved to: {}", cli.config);
        return Ok(());
    }

    // Setup tracing
    if cli.quiet {
        tracing_subscriber::fmt().with_max_level(tracing::Level::WARN).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    // Load config
    let mut config = Config::load(&cli.config)?;
    if let Some(touch) = cli.touch {
        config.device.touch = touch;
    }
    if let Some(consumer) = cli.consumer {
        config.device.consumer = consumer;
    }

    let primary_config_path = cli.config.clone();
    let mut active_config_path = primary_config_path.clone();

    info!("S10 RingCtrl starting...");
    info!("Touch device: {}", config.device.touch);
    info!("Consumer device: {}", config.device.consumer);
    info!(
        "Threshold: {}, Double-tap: {}ms",
        config.gesture.threshold, config.gesture.double_tap_ms
    );
    info!("Mode: {}", if cli.remap { "REMAP" } else { "DEBUG" });
    if let Some(ref alt) = config.alt_config {
        info!("Alt config: {}", alt);
    }

    let (tx, rx) = mpsc::channel::<Event>();

    let touch_path = config.device.touch.clone();
    let long_press_ms = config.gesture.long_press_ms;
    let tx_touch = tx.clone();
    thread::spawn(move || {
        match open_device(&touch_path, "touch") {
            Ok(mut dev) => {
                info!("Touch thread started");
                let mut timer_active: Option<Arc<AtomicBool>> = None;
                loop {
                    match dev.fetch_events() {
                        Ok(events) => {
                            for ev in events {
                                if let evdev::InputEventKind::AbsAxis(axis) = ev.kind() {
                                    if axis == evdev::AbsoluteAxisType::ABS_MT_TRACKING_ID {
                                        if ev.value() >= 0 {
                                            // Finger down: start long-press timer
                                            let active = Arc::new(AtomicBool::new(true));
                                            timer_active = Some(active.clone());
                                            let tx = tx_touch.clone();
                                            let ms = long_press_ms;
                                            thread::spawn(move || {
                                                thread::sleep(Duration::from_millis(ms));
                                                if active.load(Ordering::SeqCst) {
                                                    info!("[timer] LongPress fired after {}ms", ms);
                                                    let _ = tx.send(Event::LongPress);
                                                } else {
                                                    info!("[timer] LongPress cancelled (finger up before {}ms)", ms);
                                                }
                                            });
                                            info!("[touch] TrackingStart, timer started ({}ms)", ms);
                                        } else {
                                            // Finger up: cancel timer
                                            if let Some(ref active) = timer_active {
                                                active.store(false, Ordering::SeqCst);
                                            }
                                            timer_active = None;
                                            info!("[touch] TrackingEnd, timer cancelled");
                                        }
                                    }
                                }
                                if tx_touch.send(Event::Touch(ev)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Touch device error: {}", e);
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to open touch device: {}", e);
            }
        }
    });

    let consumer_path = config.device.consumer.clone();
    let tx_consumer = tx;
    thread::spawn(move || {
        match open_device(&consumer_path, "consumer") {
            Ok(mut dev) => {
                info!("Consumer thread started");
                loop {
                    match dev.fetch_events() {
                        Ok(events) => {
                            for ev in events {
                                if tx_consumer.send(Event::Consumer(ev)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Consumer device error: {}", e);
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to open consumer device: {}", e);
            }
        }
    });

    let mut detector = GestureDetector::new(config.gesture.threshold, config.gesture.double_tap_ms);
    let executor = ActionExecutor::new();
    let mut long_press_active = false;

    info!("Capturing events. Press Ctrl+C to stop.");

    loop {
        match rx.recv() {
            Ok(Event::Touch(ev)) => {
                // If this TrackingEnd follows a LongPress, consume it silently
                if let evdev::InputEventKind::AbsAxis(axis) = ev.kind() {
                    if axis == evdev::AbsoluteAxisType::ABS_MT_TRACKING_ID && ev.value() < 0 {
                        if long_press_active {
                            long_press_active = false;
                            continue;
                        }
                    }
                    if axis == evdev::AbsoluteAxisType::ABS_MT_TRACKING_ID && ev.value() >= 0 {
                        long_press_active = false;
                    }
                }

                if let Some(gesture) = handle_touch_event(&ev, &mut detector) {
                    if gesture == Gesture::DoubleTap && config.alt_config.is_some() {
                        // Double-tap mode: swap config profile
                        let next_path = if active_config_path == primary_config_path {
                            config.alt_config.clone().unwrap()
                        } else {
                            primary_config_path.clone()
                        };
                        match Config::load(&next_path) {
                            Ok(new_config) => {
                                config = new_config;
                                active_config_path = next_path;
                                info!("Config switched to: {}", active_config_path);
                            }
                            Err(e) => {
                                warn!("Failed to load config '{}': {}", next_path, e);
                            }
                        }
                    } else {
                        let name = gesture.as_str();
                        if cli.remap {
                            if let Some(action) = config.get_mapping(name) {
                                executor.execute(action, name);
                            }
                        } else {
                            println!("[DEBUG] >>> {} DETECTED <<<", name);
                        }
                    }
                }
            }
            Ok(Event::Consumer(ev)) => {
                handle_consumer_event(&ev, cli.remap, &config, &executor);
            }
            Ok(Event::LongPress) => {
                long_press_active = true;
                info!("Long press detected (no action configured).");
            }
            Err(_) => {
                info!("Event channel closed, exiting.");
                break;
            }
        }
    }

    Ok(())
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

/// Parse a touch event into the gesture detector. Returns `Some(Gesture)` when
/// a gesture is resolved (on finger lift).
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
        let state = match ev.value() {
            0 => "RELEASE",
            1 => "PRESS",
            2 => "REPEAT",
            _ => "UNKNOWN",
        };
        let name = format!("{:?}", key);
        if remap {
            if ev.value() == 1 {
                if let Some(action) = config.get_mapping(&name) {
                    executor.execute(action, &name);
                }
            }
        } else {
            println!("[DEBUG] [CONSUMER] {} {}", state, name);
        }
    }
}
