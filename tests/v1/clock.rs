use crate::support::{controlled::*, screen::Screen};
use chrono::TimeZone;
use serde_json::json;
use std::sync::Arc;
#[test]
fn clock_uses_the_selected_local_day_and_updates_at_a_minute_boundary() {
    let t = Arc::new(Time::default());
    *t.wall.lock().unwrap() = chrono::FixedOffset::east_opt(9 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 11, 16, 9, 5, 0)
        .unwrap();
    let f = Arc::new(Forecast::default());
    let mut s = Screen::with_environment(json!({}), vec![], f.environment(t.clone()));
    assert!(s.has("09:05") && s.has("2026年11月16日（月）"));
    t.advance(60);
    s.frame(vec![]);
    assert!(s.has("09:06"));
}
#[test]
fn midnight_updates_the_date_and_weekday() {
    let t = Arc::new(Time::default());
    *t.wall.lock().unwrap() = chrono::FixedOffset::east_opt(9 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 11, 16, 23, 59, 59)
        .unwrap();
    let f = Arc::new(Forecast::default());
    let mut s = Screen::with_environment(json!({}), vec![], f.environment(t.clone()));
    t.advance(1);
    s.frame(vec![]);
    assert!(s.has("00:00") && s.has("2026年11月17日（火）"));
}
#[test]
fn timezone_change_uses_the_new_local_calendar_date() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let mut s = Screen::with_environment(json!({}), vec![], f.environment(t.clone()));
    let old = *t.wall.lock().unwrap();
    *t.wall.lock().unwrap() = old.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());
    s.frame(vec![]);
    assert!(s.has("00:00") && s.has("2026年11月16日（月）"));
}
