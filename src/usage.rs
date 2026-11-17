/// Android UsageEvents screen states; timestamps use wall-clock milliseconds.
#[derive(Debug, Clone, Copy)]
pub enum ScreenEvent {
    On(i64),
    Off(i64),
    Reset(i64),
}

impl ScreenEvent {
    pub const fn timestamp(&self) -> i64 {
        match *self {
            ScreenEvent::On(t) | ScreenEvent::Off(t) | ScreenEvent::Reset(t) => t,
        }
    }
}

/// Count the union of interactive intervals, clipped to the local calendar day.
/// Events before midnight establish the state of a session crossing midnight.
pub fn screen_seconds(events: &[ScreenEvent], midnight: i64, now: i64) -> u64 {
    if now <= midnight {
        return 0;
    }
    let mut sorted = events.to_vec();
    sorted.sort_by_key(|e| e.timestamp());
    let mut started = None;
    let mut total = 0i64;
    for &event in &sorted {
        let timestamp = event.timestamp();
        if timestamp > now {
            break;
        }
        match event {
            ScreenEvent::On(t) => {
                if started.is_none() {
                    started = Some(t);
                }
            }
            ScreenEvent::Off(t) => {
                if let Some(start) = started.take() {
                    total += (t - start.max(midnight)).max(0);
                }
            }
            // An unclosed interval at shutdown has no reliable end time.
            ScreenEvent::Reset(_) => {
                started = None;
            }
        }
    }
    if let Some(start) = started {
        total += (now - start.max(midnight)).max(0);
    }
    (total.min(now - midnight).max(0) / 1000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_midnight_and_counts_open_session() {
        use ScreenEvent::*;
        assert_eq!(
            screen_seconds(&[On(0), Off(12_000), On(15_000)], 10_000, 20_000),
            7
        );
    }
    #[test]
    fn duplicate_events_and_shutdown_do_not_inflate_time() {
        use ScreenEvent::*;
        assert_eq!(
            screen_seconds(&[On(0), On(1000), Off(4000), Off(5000)], 0, 6000),
            4
        );
        assert_eq!(screen_seconds(&[On(0), Reset(4000), On(5000)], 0, 7000), 2);
        assert_eq!(screen_seconds(&[], 0, 7000), 0);
    }
}
