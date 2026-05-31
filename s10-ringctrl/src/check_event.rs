#[cfg(test)]
mod tests {
    #[test]
    fn check_abs_mt_tracking_id() {
        use evdev::AbsoluteAxisType;
        println!("ABS_MT_TRACKING_ID code = {}", AbsoluteAxisType::ABS_MT_TRACKING_ID.0);
    }
}
