//! A37: change the real settings slider through keyboard focus; no direct model mutation.
#![cfg(feature = "ui")]
mod support;
use eframe::egui::{self, Event, Key, WidgetType};
use serde_json::json;
use support::{app, screen::Screen};
fn key(s: &mut Screen, key: Key) {
    for pressed in [true, false] {
        s.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    s.settle();
}
fn focus_slider(s: &mut Screen) {
    s.settings();
    for _ in 0..25 {
        s.frame(vec![Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        let found = s
            .output
            .platform_output
            .events
            .iter()
            .any(|e| e.widget_info().typ == WidgetType::Slider);
        s.frame(vec![Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        if found {
            return;
        }
    }
    panic!("font slider must be keyboard reachable");
}
#[test]
fn slider_selects_and_saves_every_ten_percent_step_in_both_directions() {
    let mut s = Screen::new(json!({"font_percent":80}), vec![]);
    focus_slider(&mut s);
    for expected in [90, 100, 110, 120, 130, 140, 150, 160] {
        key(&mut s, Key::ArrowRight);
        assert_eq!(s.saved().font_percent, expected);
    }
    key(&mut s, Key::ArrowRight);
    assert_eq!(s.saved().font_percent, 160);
    for expected in [150, 140, 130, 120, 110, 100, 90, 80] {
        key(&mut s, Key::ArrowLeft);
        assert_eq!(s.saved().font_percent, expected);
    }
    key(&mut s, Key::ArrowLeft);
    assert_eq!(s.saved().font_percent, 80);
}
#[test]
fn enlarging_font_that_cannot_fit_keeps_font_and_order_and_shows_error() {
    let apps: Vec<_> = (0..8)
        .map(|i| app(&format!("p/A{i}"), &format!("APP{i}")))
        .collect();
    let mut s = Screen::new(
        json!({"favorites":apps.iter().map(|a|a.id.clone()).collect::<Vec<_>>(),"font_percent":100}),
        apps,
    );
    assert!(s.has("APP7"), "the original eight registrations must fit");
    let before = std::fs::read(s.dir.settings()).unwrap();
    s.settings();
    s.click("100%");
    s.frame(vec![Event::Text("160".into())]);
    key(&mut s, Key::Enter);
    assert!(
        s.has("ホームに収まりません"),
        "rejection needs a directly visible cause: {:?}",
        s.labels()
    );
    assert_eq!(s.saved().font_percent, 100);
    assert_eq!(
        s.saved().favorites,
        vec!["p/A0", "p/A1", "p/A2", "p/A3", "p/A4", "p/A5", "p/A6", "p/A7"]
    );
    assert_eq!(std::fs::read(s.dir.settings()).unwrap(), before);
}
