//! A10/A11/A12/A49: remaining gesture and pending-input state boundaries from v1.
#![cfg(feature = "ui")]
mod support;
use eframe::egui::{self, Event, Pos2, Vec2};
use serde_json::json;
use std::sync::atomic::Ordering;
use support::{app, screen::Screen};
fn screen() -> Screen {
    Screen::new(
        json!({}),
        vec![app("a/Main", "ALPHA"), app("b/Main", "BETA")],
    )
}
fn touch(id: u64, pos: Pos2, phase: egui::TouchPhase) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    }
}
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
#[test]
fn second_finger_cancels_the_pending_app_tap() {
    let mut s = screen();
    s.drawer();
    let p = s.position("ALPHA");
    s.frame(vec![
        touch(1, p, egui::TouchPhase::Start),
        Event::PointerMoved(p),
        button(p, true),
    ]);
    let second = p + Vec2::new(0., 60.);
    s.frame(vec![touch(2, second, egui::TouchPhase::Start)]);
    s.frame(vec![
        touch(2, second, egui::TouchPhase::End),
        touch(1, p, egui::TouchPhase::End),
        button(p, false),
        Event::PointerGone,
    ]);
    s.settle();
    assert!(
        s.device.launched.lock().unwrap().is_empty(),
        "a multi-touch sequence cannot launch an app"
    );
    assert!(s.saved().favorites.is_empty());
}
#[test]
fn nine_dp_movement_still_allows_a_long_press_after_600ms() {
    let mut s = screen();
    s.drawer();
    let p = s.position("ALPHA");
    s.frame(vec![
        touch(1, p, egui::TouchPhase::Start),
        Event::PointerMoved(p),
        button(p, true),
    ]);
    s.now += 0.6;
    let end = p + Vec2::new(9., 0.);
    s.frame(vec![
        touch(1, end, egui::TouchPhase::Move),
        Event::PointerMoved(end),
    ]);
    assert!(
        s.has("アプリを非表示"),
        "movement below 10dp must not cancel long press: {:?}",
        s.labels()
    );
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn vertical_ratio_below_one_point_five_keeps_home() {
    let mut s = screen();
    s.drag(Pos2::new(160., 500.), Pos2::new(210., 426.));
    assert!(s.has("＋ アプリを追加"));
    assert!(!s.has("BETA"));
}
#[test]
fn reaching_top_from_mid_list_does_not_close_in_the_same_gesture() {
    let apps = (0..100)
        .map(|i| app(&format!("p/A{i:03}"), &format!("APP{i:03}")))
        .collect();
    let mut s = Screen::new(json!({}), apps);
    s.drawer();
    s.frame(vec![
        Event::PointerMoved(Pos2::new(160., 300.)),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0., -120.),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    s.settle();
    assert!(!s.has("APP000"), "test must start away from the top");
    s.drag(Pos2::new(160., 210.), Pos2::new(160., 590.));
    assert!(
        !s.has("＋ アプリを追加"),
        "this gesture began below the top and cannot dismiss the drawer"
    );
    assert!(
        s.has("APP000"),
        "the same gesture must have reached the top"
    );
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn touch_release_keeps_scroll_momentum_without_launch_or_registration() {
    let apps = (0..100)
        .map(|i| app(&format!("p/A{i:03}"), &format!("APP{i:03}")))
        .collect();
    let mut s = Screen::new(json!({}), apps);
    s.drawer();
    let start = Pos2::new(160., 510.);
    s.frame(vec![
        touch(1, start, egui::TouchPhase::Start),
        Event::PointerMoved(start),
        button(start, true),
    ]);
    let mut end = start;
    for i in 1..=8 {
        end = start - Vec2::new(0., i as f32 * 25.);
        s.frame(vec![
            touch(1, end, egui::TouchPhase::Move),
            Event::PointerMoved(end),
        ]);
    }
    s.frame(vec![
        touch(1, end, egui::TouchPhase::End),
        button(end, false),
        Event::PointerGone,
    ]);
    let before = s
        .texts()
        .into_iter()
        .find(|(t, _)| t.starts_with("APP"))
        .expect("visible row at release");
    for _ in 0..10 {
        s.frame(vec![]);
    }
    let after = s
        .texts()
        .into_iter()
        .find(|(t, _)| t.starts_with("APP"))
        .expect("visible row after release");
    assert!(
        before.0 != after.0 || (before.1.top() - after.1.top()).abs() > 1.,
        "list stopped as soon as the finger was released"
    );
    assert!(s.device.launched.lock().unwrap().is_empty());
    assert!(s.saved().favorites.is_empty());
}
#[test]
fn back_discards_unsubmitted_location_text_when_settings_reopen() {
    let mut s = screen();
    s.settings();
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("location".into(), "35.123,139.456".into()));
    s.frame(vec![]);
    s.back();
    s.settings();
    assert!(
        !s.has("35.123,139.456"),
        "Back must discard the unsent editor value"
    );
    assert!(s.saved().weather_location.is_none());
}
#[test]
fn home_from_settings_removes_native_editor_and_keeps_saved_preferences() {
    let mut s = screen();
    s.settings();
    s.click("暗い背景");
    s.device.native.store(true, Ordering::SeqCst);
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("home".into(), String::new()));
    s.frame(vec![]);
    s.settle();
    assert!(s.has("＋ アプリを追加"));
    assert!(!s.has("文字サイズ"));
    assert!(s.saved().dark);
    assert!(s.device.field.lock().unwrap().is_none());
}
#[test]
fn reading_reply_from_an_editor_cancelled_by_back_cannot_change_saved_settings() {
    let mut s = screen();
    s.drawer();
    s.secondary("ALPHA");
    s.click("読みを設定");
    s.back();
    let before = std::fs::read(s.dir.settings()).unwrap();
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("reading:a/Main".into(), "あるふぁ".into()));
    s.frame(vec![]);
    s.settle();
    assert_eq!(
        std::fs::read(s.dir.settings()).unwrap(),
        before,
        "cancelled editor reply must not commit after returning home"
    );
    assert!(s.has("＋ アプリを追加"));
}
