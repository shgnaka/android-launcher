//! Reviewed observer revision: old frozen tests stay available and unchanged.
#![cfg(feature = "ui")]
mod support;
#[path = "support/visible_v1_1.rs"]
mod visible;
use eframe::egui::{self, Pos2, Vec2};
use serde_json::json;
use support::{app, screen::Screen};
use visible::fully_visible;
#[test]
fn cached_home_rejects_overflow_without_hiding_the_cause() {
    let apps = vec![
        app("a/Main", "ALPHA"),
        app("b/Main", "BETA"),
        app("c/Main", "GAMMA"),
        app("d/Main", "DELTA"),
        app("e/Main", "Browser"),
    ];
    let mut s = Screen::new(
        json!({"favorites":["a/Main","b/Main","c/Main","d/Main"]}),
        apps,
    );
    s.size = Vec2::new(360., 440.);
    s.frame(vec![]);
    s.settle();
    assert!(fully_visible(&s.output)
        .iter()
        .any(|(text, _)| text == "ALPHA"));
    let before = std::fs::read(s.dir.settings()).unwrap();
    s.drawer();
    s.search("Browser");
    let position = fully_visible(&s.output)
        .into_iter()
        .find(|(text, _)| text == "+")
        .expect("fully visible add control")
        .1
        .center();
    s.pointer(position, true);
    s.pointer(position, false);
    s.settle();
    assert!(s.has("ホームに収まりません"));
    assert_eq!(std::fs::read(s.dir.settings()).unwrap(), before);
}
#[test]
fn clipped_text_is_not_an_actionable_full_label() {
    let ctx = egui::Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let clip = egui::Rect::from_min_size(Pos2::new(20., 20.), Vec2::new(100., 1.));
            ui.painter().with_clip_rect(clip).text(
                Pos2::new(20., 20.),
                egui::Align2::LEFT_TOP,
                "PARTIAL",
                egui::FontId::proportional(20.),
                egui::Color32::BLACK,
            );
        });
    });
    assert!(fully_visible(&output).is_empty());
}
#[test]
fn fully_visible_text_can_supply_an_action_position() {
    let ctx = egui::Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("FULL LABEL");
        });
    });
    assert!(fully_visible(&output)
        .iter()
        .any(|(s, _)| s == "FULL LABEL"));
}
