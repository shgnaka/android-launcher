//! Requirement: an overflowing addition reports its reason without opening details,
//! and preserves the last accepted home configuration.
#![cfg(feature = "ui")]
use eframe::egui::{self, Pos2, Vec2};
use quiet_launcher::{config::Settings, platform::desktop::Desktop, ui::Launcher};
use std::sync::Arc;

fn frame(app: &mut Launcher, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(360.0, 440.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.render(ctx),
    )
}
fn pointer(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
fn labels(output: &egui::FullOutput) -> Vec<String> {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}
#[test]
fn rejected_home_addition_reports_capacity_directly_and_keeps_preferences() {
    let path = std::env::temp_dir()
        .join(format!("quiet-overflow-notice-{}", std::process::id()))
        .join("settings.json");
    let settings = Settings {
        favorites: [
            "preview/phone",
            "preview/messages",
            "preview/camera",
            "preview/calendar",
        ]
        .map(String::from)
        .to_vec(),
        show_weather: false,
        show_usage: false,
        show_media: false,
        ..Settings::default()
    };
    settings.save(&path).unwrap();
    let ctx = egui::Context::default();
    let mut app = Launcher::from_context(&ctx, Arc::new(Desktop), path.clone());
    std::thread::sleep(std::time::Duration::from_millis(100));
    let home = labels(&frame(&mut app, &ctx, vec![]));
    assert!(
        home.iter().any(|s| s == "電話"),
        "initial configuration must fit: {home:?}"
    );
    frame(&mut app, &ctx, pointer(Pos2::new(160.0, 360.0), true));
    frame(&mut app, &ctx, pointer(Pos2::new(160.0, 160.0), false));
    for _ in 0..40 {
        frame(&mut app, &ctx, vec![]);
    }
    frame(&mut app, &ctx, vec![egui::Event::Text("Browser".into())]);
    let drawer = frame(&mut app, &ctx, vec![]);
    assert!(labels(&drawer).iter().any(|s| s == "Browser"));
    let add = drawer
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "+" => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .expect("the single unregistered result has an add control");
    frame(&mut app, &ctx, pointer(add, true));
    frame(&mut app, &ctx, pointer(add, false));
    let notice = labels(&frame(&mut app, &ctx, vec![]));
    assert_eq!(Settings::load(&path).unwrap().favorites, settings.favorites);
    assert!(
        notice.iter().any(|s| s.contains("収まりません")),
        "show the cause without expanding details: {notice:?}"
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
