use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Up, Down, Left, Right, Tap, DoubleTap,
}

#[derive(Debug, Clone, Copy)]
pub enum TouchEvent {
    TrackingStart,
    TrackingEnd,
    Position { x: Option<i32>, y: Option<i32> },
}

pub struct GestureDetector {
    threshold: i32,
    double_tap_ms: u64,
    state: State,
    last_tap: Option<Instant>,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Idle,
    Tracking { start_x: Option<i32>, start_y: Option<i32>, current_x: Option<i32>, current_y: Option<i32> },
}

impl GestureDetector {
    pub fn new(threshold: i32, double_tap_ms: u64) -> Self {
        Self { threshold, double_tap_ms, state: State::Idle, last_tap: None }
    }

    pub fn feed(&mut self, event: TouchEvent) -> Option<Gesture> {
        match event {
            TouchEvent::TrackingStart => {
                self.state = State::Tracking { start_x: None, start_y: None, current_x: None, current_y: None };
                None
            }
            TouchEvent::Position { x, y } => {
                if let State::Tracking { ref mut start_x, ref mut start_y, ref mut current_x, ref mut current_y } = self.state {
                    if let Some(vx) = x { if start_x.is_none() { *start_x = Some(vx); } *current_x = Some(vx); }
                    if let Some(vy) = y { if start_y.is_none() { *start_y = Some(vy); } *current_y = Some(vy); }
                }
                None
            }
            TouchEvent::TrackingEnd => {
                let result = self.resolve();
                self.state = State::Idle;
                result
            }
        }
    }

    fn resolve(&mut self) -> Option<Gesture> {
        let State::Tracking { start_x, start_y, current_x, current_y } = self.state else { return None; };
        let has_x = start_x.is_some() && current_x.is_some();
        let has_y = start_y.is_some() && current_y.is_some();
        let has_pos = has_x || has_y;
        let dx = if has_x { current_x.unwrap() - start_x.unwrap() } else { 0 };
        let dy = if has_y { current_y.unwrap() - start_y.unwrap() } else { 0 };

        if has_pos && (dx.abs() >= self.threshold || dy.abs() >= self.threshold) {
            self.last_tap = None;
            return Some(if has_x && has_y {
                if dx.abs() > dy.abs() { if dx > 0 { Gesture::Left } else { Gesture::Right } }
                else { if dy > 0 { Gesture::Up } else { Gesture::Down } }
            } else if has_x { if dx > 0 { Gesture::Left } else { Gesture::Right } }
            else { if dy > 0 { Gesture::Up } else { Gesture::Down } });
        }

        let now = Instant::now();
        if let Some(last) = self.last_tap {
            if now.duration_since(last) < Duration::from_millis(self.double_tap_ms) {
                self.last_tap = None;
                return Some(Gesture::DoubleTap);
            }
        }
        self.last_tap = Some(now);
        Some(Gesture::Tap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swipe_up() {
        let mut d = GestureDetector::new(80, 500);
        assert_eq!(d.feed(TouchEvent::TrackingStart), None);
        assert_eq!(d.feed(TouchEvent::Position { x: None, y: Some(500) }), None);
        assert_eq!(d.feed(TouchEvent::Position { x: None, y: Some(900) }), None);
        assert_eq!(d.feed(TouchEvent::TrackingEnd), Some(Gesture::Up));
    }

    #[test]
    fn test_swipe_right() {
        let mut d = GestureDetector::new(80, 500);
        d.feed(TouchEvent::TrackingStart);
        d.feed(TouchEvent::Position { x: Some(700), y: None });
        d.feed(TouchEvent::Position { x: Some(500), y: None });
        assert_eq!(d.feed(TouchEvent::TrackingEnd), Some(Gesture::Right));
    }

    #[test]
    fn test_tap() {
        let mut d = GestureDetector::new(80, 500);
        d.feed(TouchEvent::TrackingStart);
        assert_eq!(d.feed(TouchEvent::TrackingEnd), Some(Gesture::Tap));
    }

    #[test]
    fn test_double_tap() {
        let mut d = GestureDetector::new(80, 500);
        d.feed(TouchEvent::TrackingStart);
        assert_eq!(d.feed(TouchEvent::TrackingEnd), Some(Gesture::Tap));
        d.feed(TouchEvent::TrackingStart);
        assert_eq!(d.feed(TouchEvent::TrackingEnd), Some(Gesture::DoubleTap));
    }
}
