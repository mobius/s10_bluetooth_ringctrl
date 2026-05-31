use anyhow::Result;
use clap::Parser;
use std::path::Path;
use tracing::{info, warn};

mod gesture;
use gesture::{GestureDetector, TouchEvent};

#[derive(Parser, Debug)]
#[command(name = "s10-ringctrl")]
#[command(about = "S10 Bluetooth Remote Input Remapper (Linux MVP)")]
struct Cli {
    /// Touchpad input device path
    #[arg(short, long, default_value = "/dev/input/event22")]
    touch: String,

    /// Consumer control input device path
    #[arg(short, long, default_value = "/dev/input/event23")]
    consumer: String,

    /// Swipe detection threshold
    #[arg(long, default_value_t = 80)]
    threshold: i32,

    /// Double-tap detection timeout (ms)
    #[arg(long, default_value_t = 500)]
    double_tap_ms: u64,

    /// Enable active remapping (default is debug-only)
    #[arg(long)]
    remap: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    info!("S10 RingCtrl starting...");
    info!("Touch device: {}", cli.touch);
    info!("Consumer device: {}", cli.consumer);
    info!("Threshold: {}, Double-tap: {}ms", cli.threshold, cli.double_tap_ms);
    if cli.remap {
        info!("Mode: REMAP");
    } else {
        info!("Mode: DEBUG (output only)");
    }

    // Open touch device
    let mut touch_dev = open_device(&cli.touch, "touch")?;
    let mut consumer_dev = open_device(&cli.consumer, "consumer")?;

    let mut detector = GestureDetector::new(cli.threshold, cli.double_tap_ms);

    info!("Capturing events. Press Ctrl+C to stop.");

    loop {
        // Poll both devices
        if let Ok(events) = touch_dev.fetch_events() {
            for ev in events {
                handle_touch_event(&ev, &mut detector, cli.remap);
            }
        }
        if let Ok(events) = consumer_dev.fetch_events() {
            for ev in events {
                handle_consumer_event(&ev, cli.remap);
            }
        }
    }
}

fn open_device(path: &str, label: &str) -> Result<evdev::Device> {
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

fn handle_touch_event(ev: &evdev::InputEvent, detector: &mut GestureDetector, remap: bool) {
    use evdev::AbsoluteAxisType;

    match ev.kind() {
        evdev::InputEventKind::AbsAxis(axis) => {
            match axis {
                AbsoluteAxisType::ABS_MT_TRACKING_ID => {
                    if ev.value() >= 0 {
                        detector.feed(TouchEvent::TrackingStart);
                    } else {
                        if let Some(gesture) = detector.feed(TouchEvent::TrackingEnd) {
                            let name = format!("{:?}", gesture).to_uppercase();
                            if remap {
                                info!("[GESTURE] {} -> (remap not yet implemented)", name);
                            } else {
                                println!("[DEBUG] >>> {} DETECTED <<<", name);
                            }
                        }
                    }
                }
                AbsoluteAxisType::ABS_MT_POSITION_X => {
                    detector.feed(TouchEvent::Position { x: Some(ev.value()), y: None });
                }
                AbsoluteAxisType::ABS_MT_POSITION_Y => {
                    detector.feed(TouchEvent::Position { x: None, y: Some(ev.value()) });
                }
                AbsoluteAxisType::ABS_X => {
                    detector.feed(TouchEvent::Position { x: Some(ev.value()), y: None });
                }
                AbsoluteAxisType::ABS_Y => {
                    detector.feed(TouchEvent::Position { x: None, y: Some(ev.value()) });
                }
                _ => {}
            }
        }
        evdev::InputEventKind::Key(_) => {
            // BTN_TOUCH etc.
        }
        _ => {}
    }
}

fn handle_consumer_event(ev: &evdev::InputEvent, remap: bool) {
    if let evdev::InputEventKind::Key(key) = ev.kind() {
        let state = match ev.value() {
            0 => "RELEASE",
            1 => "PRESS",
            2 => "REPEAT",
            _ => "UNKNOWN",
        };
        let name = format!("{:?}", key);
        if remap {
            info!("[CONSUMER] {} {} -> (remap not yet implemented)", state, name);
        } else {
            println!("[DEBUG] [CONSUMER] {} {}", state, name);
        }
    }
}
