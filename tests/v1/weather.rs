use crate::support::{app, controlled::*, screen::Screen};
use quiet_launcher::runtime::Clock;
use serde_json::json;
use std::sync::Arc;
fn setup() -> (Screen, Arc<Time>, Arc<Forecast>) {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let now = t.wall_time();
    f.reply(19.5, 0, now, now - chrono::Duration::minutes(15));
    let s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.0,"longitude":135.0}}),
        vec![app("a/Main", "ALPHA")],
        f.environment(t.clone()),
    );
    (s, t, f)
}
fn until(s: &mut Screen, condition: impl Fn(&Screen) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        s.frame(vec![]);
        if condition(s) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "weather condition failed: {:?}",
            s.labels()
        );
        std::thread::yield_now();
    }
}
#[test]
fn scripted_weather_is_shown_with_the_actual_fetch_time() {
    let (mut s, _, _) = setup();
    until(&mut s, |s| s.has("晴れ 20°C") && s.has("09:00"));
}
#[test]
fn successful_weather_is_not_refetched_before_thirty_minutes() {
    let (mut s, t, f) = setup();
    until(&mut s, |s| s.has("晴れ 20°C"));
    t.advance(1799);
    s.settle();
    assert_eq!(
        f.calls.lock().unwrap().len(),
        1,
        "successful values remain fresh for 30 minutes"
    );
}
#[test]
fn weather_is_refetched_at_the_exact_thirty_minute_boundary() {
    let (mut s, t, f) = setup();
    until(&mut s, |s| s.has("晴れ 20°C"));
    t.advance(1800);
    let now = t.wall_time();
    f.reply(23., 0, now, now);
    until(&mut s, |s| s.has("23°C"));
    assert_eq!(f.calls.lock().unwrap().len(), 2);
}
#[test]
fn failed_refresh_keeps_the_old_value_and_marks_it_stale() {
    let (mut s, t, f) = setup();
    until(&mut s, |s| s.has("20°C"));
    f.fail();
    t.advance(1800);
    until(&mut s, |s| s.has("更新待ち"));
    assert!(s.has("20°C") && s.has("09:00"));
}
#[test]
fn automatic_retry_waits_five_minutes_after_a_failure() {
    let (mut s, t, f) = setup();
    until(&mut s, |s| s.has("20°C"));
    f.fail();
    t.advance(1800);
    until(&mut s, |s| s.has("更新待ち"));
    t.advance(299);
    s.settle();
    assert_eq!(f.calls.lock().unwrap().len(), 2);
    let now = t.wall_time();
    f.reply(24., 0, now, now);
    t.advance(1);
    until(&mut s, |s| s.has("24°C"));
    assert_eq!(f.calls.lock().unwrap().len(), 3);
}
#[test]
fn old_weather_numbers_expire_at_twenty_four_hours() {
    let (mut s, t, f) = setup();
    until(&mut s, |s| s.has("20°C"));
    f.fail();
    t.advance(86400);
    until(&mut s, |s| s.has("天気 —"));
    assert!(!s.has("20°C"));
}
#[test]
fn weather_off_does_not_make_a_network_request() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let mut s = Screen::with_environment(
        json!({"show_weather":false,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t.clone()),
    );
    s.wait_snapshot();
    t.advance(86400);
    s.settle();
    assert!(f.calls.lock().unwrap().is_empty());
    assert!(!s.has("天気 —"));
}
#[test]
fn missing_location_does_not_make_a_network_request() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let mut s = Screen::with_environment(
        json!({"show_weather":true}),
        vec![],
        f.environment(t.clone()),
    );
    s.wait_snapshot();
    t.advance(86400);
    s.settle();
    assert!(f.calls.lock().unwrap().is_empty());
}
#[test]
fn old_weather_is_restored_across_process_recreation() {
    let (mut s, t, f) = setup();
    until(&mut s, |s| s.has("20°C"));
    let count = f.calls.lock().unwrap().len();
    s.app = quiet_launcher::ui::Launcher::with_environment(
        &s.ctx,
        s.device.clone(),
        s.dir.settings(),
        f.environment(t),
    );
    until(&mut s, |s| s.has("20°C"));
    assert_eq!(f.calls.lock().unwrap().len(), count);
}
#[test]
fn future_observation_is_not_accepted_as_current_weather() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let now = t.wall_time();
    f.reply(99., 0, now, now + chrono::Duration::minutes(11));
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t),
    );
    until(&mut s, |s| s.has("天気 —"));
    assert!(!s.has("99°C"));
}
#[test]
fn observation_more_than_three_hours_old_is_not_current_weather() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let now = t.wall_time();
    f.reply(99., 0, now, now - chrono::Duration::seconds(10801));
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t),
    );
    until(&mut s, |s| s.has("天気 —"));
    assert!(!s.has("99°C"));
}
#[test]
fn unsupported_weather_code_is_explicitly_unknown() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let now = t.wall_time();
    f.reply(19., 999, now, now);
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t),
    );
    until(&mut s, |s| s.has("天気不明"));
    assert!(s.has("19°C"));
}
#[test]
fn negative_half_degree_is_rounded_away_from_zero() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let now = t.wall_time();
    f.reply(-0.5, 0, now, now);
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t),
    );
    until(&mut s, |s| s.has("-1°C"));
}
#[test]
fn first_failure_has_an_explicit_unknown_weather_state() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    f.fail();
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t),
    );
    until(&mut s, |s| s.has("天気 —"));
}
fn controlled_value(t: &Time, temp: f64) -> quiet_launcher::weather::Weather {
    let now = t.wall_time().with_timezone(&chrono::Local);
    quiet_launcher::weather::Weather {
        temperature: temp,
        code: 0,
        fetched_at: now,
        observed_at: Some(now),
    }
}
#[test]
fn pending_network_does_not_block_drawer_or_search() {
    use crate::support::gated::*;
    let t = Arc::new(Time::default());
    let f = Arc::new(GatedForecast::default());
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(vec![gate.clone()]);
    f.push(controlled_value(&t, 20.), Some(gate));
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![app("a/Main", "カメラ"), app("b/Main", "別名")],
        quiet_launcher::runtime::Environment {
            clock: t,
            weather: f.clone(),
        },
    );
    f.wait_calls(1);
    s.drawer();
    s.search("kamera");
    assert!(s.has("カメラ") && !s.has("別名"));
}
#[test]
fn returning_to_the_same_location_cannot_accept_the_previous_request_generation() {
    use crate::support::gated::*;
    let t = Arc::new(Time::default());
    let f = Arc::new(GatedForecast::default());
    let first = Arc::new(Gate::default());
    let second = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(vec![first.clone(), second.clone()]);
    f.push(controlled_value(&t, 10.), Some(first.clone()));
    f.push(controlled_value(&t, 20.), Some(second));
    f.push(controlled_value(&t, 30.), None);
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        quiet_launcher::runtime::Environment {
            clock: t.clone(),
            weather: f.clone(),
        },
    );
    f.wait_calls(1);
    s.settings();
    for text in ["34.0,136.0", "35.0,135.0"] {
        s.device
            .text
            .lock()
            .unwrap()
            .push_back(("location".into(), text.into()));
        s.frame(vec![]);
        s.find_scroll("地域を保存");
        s.click("地域を保存");
    }
    s.back();
    first.release();
    f.wait_calls(2);
    for _ in 0..30 {
        s.frame(vec![]);
        assert!(
            !s.has("10°C"),
            "old A response must not overwrite newer A generation"
        );
    }
}
#[test]
fn response_finishing_after_ten_seconds_is_not_published_as_success() {
    use crate::support::gated::*;
    let t = Arc::new(Time::default());
    let f = Arc::new(GatedForecast::default());
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(vec![gate.clone()]);
    f.push(controlled_value(&t, 99.), Some(gate.clone()));
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        quiet_launcher::runtime::Environment {
            clock: t.clone(),
            weather: f.clone(),
        },
    );
    f.wait_calls(1);
    t.advance(11);
    s.settle();
    assert!(
        s.has("天気 —"),
        "expired request needs a visible unavailable state"
    );
    gate.release();
    s.settle();
    assert!(!s.has("99°C"));
}
#[test]
fn nonfinite_temperature_is_rejected_instead_of_becoming_visible_text() {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let now = t.wall_time();
    f.reply(f64::NAN, 0, now, now);
    let mut s = Screen::with_environment(
        json!({"show_weather":true,"weather_location":{"latitude":35.,"longitude":135.}}),
        vec![],
        f.environment(t),
    );
    until(&mut s, |s| s.has("天気 —"));
    assert!(!s.has("NaN"));
}
