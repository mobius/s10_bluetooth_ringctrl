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
use gesture::{GestureDetector, TouchEvent};

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

enum DeviceType {
    Touch,
    Consumer,
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

    info!("S10 RingCtrl starting...");
    info!("Touch device: {}", config.device.touch);
    info!("Consumer device: {}", config.device.consumer);
    info!(
        "Threshold: {}, Double-tap: {}ms",
        config.gesture.threshold, config.gesture.double_tap_ms
    );
    info!("Mode: {}", if cli.remap { "REMAP" } else { "DEBUG" });

    let (tx, rx) = mpsc::channel::<(DeviceType, evdev::InputEvent)>();

    let touch_path = config.device.touch.clone();
    let tx_touch = tx.clone();
    thread::spawn(move || {
        match open_device(&touch_path, "touch") {
            Ok(mut dev) => {
                info!("Touch thread started");
                loop {
                    match dev.fetch_events() {
                        Ok(events) => {
                            for ev in events {
                                if tx_touch.send((DeviceType::Touch, ev)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Touch device error: {}", e);
                            thread::sleep(std::time::Duration::from_millis(100));
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
                                if tx_consumer.send((DeviceType::Consumer, ev)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Consumer device error: {}", e);
                            thread::sleep(std::time::Duration::from_millis(100));
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

    info!("Capturing events. Press Ctrl+C to stop.");

    loop {
        match rx.recv() {
            Ok((DeviceType::Touch, ev)) => {
                handle_touch_event(&ev, &mut detector, cli.remap, &config, &executor);
            }
            Ok((DeviceType::Consumer, ev)) => {
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

fn handle_touch_event(
    ev: &evdev::InputEvent,
    detector: &mut GestureDetector,
    remap: bool,
    config: &Config,
    executor: &ActionExecutor,
) {
    use evdev::AbsoluteAxisType;

    match ev.kind() {
        evdev::InputEventKind::AbsAxis(axis) => match axis {
            AbsoluteAxisType::ABS_MT_TRACKING_ID => {
                if ev.value() >= 0 {
                    detector.feed(TouchEvent::TrackingStart);
                } else {
                    if let Some(gesture) = detector.feed(TouchEvent::TrackingEnd) {
                        let name = gesture.as_str();
                        if remap {
                            if let Some(action) = config.get_mapping(name) {
                                executor.execute(action, name);
                            }
                        } else {
                            println!("[DEBUG] >>> {} DETECTED <<<", name);
                        }
                    }
                }
            }
            AbsoluteAxisType::ABS_MT_POSITION_X | AbsoluteAxisType::ABS_X => {
                detector.feed(TouchEvent::Position {
                    x: Some(ev.value()),
                    y: None,
                });
            }
            AbsoluteAxisType::ABS_MT_POSITION_Y | AbsoluteAxisType::ABS_Y => {
                detector.feed(TouchEvent::Position {
                    x: None,
                    y: Some(ev.value()),
                });
            }
            _ => {}
        },
        _ => {}
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
