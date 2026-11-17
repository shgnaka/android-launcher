//! Additional v1 boundaries. Literal expected outcomes come from A17/A19/A35/A37/A39/A50.
#![cfg(feature = "ui")]
mod support;
use eframe::egui::{self, Pos2, Vec2};
use quiet_launcher::{
    config::Settings,
    model::{widget_title, TextWidget, WidgetNames},
};
use serde_json::json;
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use support::{
    app,
    controlled::{Forecast, Time},
    screen::Screen,
    Directory,
};

// A35: actual native-field bounds, not clipped text returned by the screen helper.
fn field_geometry(width: f32, height: f32, font: u16) {
    for dark in [false, true] {
        let mut s = Screen::new(
            json!({"font_percent":font,"dark":dark}),
            vec![app("a/Main", "ALPHA")],
        );
        s.size = Vec2::new(width, height);
        s.device.native.store(true, Ordering::SeqCst);
        *s.device.insets.lock().unwrap() = [0., 40., 0., 24.];
        s.frame(vec![]);
        s.drag(
            Pos2::new(width / 2., height - 60.),
            Pos2::new(width / 2., height - 180.),
        );
        // Full-height IME is tested in portrait. Landscape/split here test bars and cutout;
        // a 280dp IME plus 40dp cutout leaves no possible target in a 320dp viewport.
        let keyboard = if height >= 640. { 280. } else { 0. };
        *s.device.keyboard.lock().unwrap() = keyboard;
        s.frame(vec![]);
        let field = s
            .device
            .field
            .lock()
            .unwrap()
            .clone()
            .expect("inline search must remain mounted above IME");
        let [x, y, w, h] = field.rect;
        assert!(w > 0 && h > 0, "empty search target: {field:?}");
        assert!(
            x >= 0 && x + w <= width as i32,
            "search exceeds screen width: {field:?}"
        );
        assert!(
            y >= 40 && y + h <= (height - keyboard - 24.) as i32,
            "search overlaps cutout, system bar or IME: {field:?}, height={height}"
        );
        assert_eq!(field.font_percent, font);
        assert_eq!(field.dark, dark);
        s.back();
        assert!(
            s.device.field.lock().unwrap().is_none(),
            "Back must remove native input"
        );
    }
}
macro_rules! geometry {
    ($name:ident,$w:expr,$h:expr,$f:expr) => {
        #[test]
        fn $name() {
            field_geometry($w, $h, $f);
        }
    };
}
geometry!(a35_narrow_240_font80, 240., 640., 80);
geometry!(a35_narrow_240_font100, 240., 640., 100);
geometry!(a35_narrow_240_font160, 240., 640., 160);
geometry!(a35_small_320_font80, 320., 640., 80);
geometry!(a35_small_320_font100, 320., 640., 100);
geometry!(a35_small_320_font160, 320., 640., 160);
geometry!(a35_standard_360_font80, 360., 640., 80);
geometry!(a35_standard_360_font100, 360., 640., 100);
geometry!(a35_standard_360_font160, 360., 640., 160);
geometry!(a35_landscape_font80, 640., 360., 80);
geometry!(a35_landscape_font100, 640., 360., 100);
geometry!(a35_landscape_font160, 640., 360., 160);
geometry!(a35_split_font80, 320., 320., 80);
geometry!(a35_split_font100, 320., 320., 100);
geometry!(a35_split_font160, 320., 320., 160);

#[test]
fn a37_every_supported_font_step_survives_a_real_file_round_trip() {
    for font in [80, 90, 100, 110, 120, 130, 140, 150, 160] {
        let dir = Directory::new("font-step");
        let mut settings = Settings::default();
        settings.font_percent = font;
        settings.save(&dir.settings()).unwrap();
        assert_eq!(Settings::load(&dir.settings()).unwrap().font_percent, font);
    }
}
#[test]
fn a19_unhide_overflow_keeps_hidden_registration_and_reports_the_cause() {
    let apps: Vec<_> = (0..9)
        .map(|i| app(&format!("p{i}/Main"), &format!("APP{i}")))
        .collect();
    let mut s = Screen::new(
        json!({"favorites":apps.iter().map(|a|a.id.clone()).collect::<Vec<_>>(),"hidden_packages":["p8"]}),
        apps,
    );
    let before = std::fs::read(s.dir.settings()).unwrap();
    s.settings();
    s.find_scroll("再表示");
    s.click("再表示");
    assert!(
        s.has("ホームに収まりません"),
        "error must be directly visible: {:?}",
        s.labels()
    );
    assert_eq!(std::fs::read(s.dir.settings()).unwrap(), before);
    assert!(s.saved().is_hidden("p8"));
    assert_eq!(s.saved().favorites.len(), 9);
}
#[test]
fn a17_outside_tap_cancels_menu_and_preserves_query_and_file() {
    let mut s = Screen::new(
        json!({}),
        vec![app("a/Main", "ALPHA"), app("b/Main", "BETA")],
    );
    let before = std::fs::read(s.dir.settings()).unwrap();
    s.drawer();
    s.search("ALPHA");
    s.secondary("ALPHA");
    assert!(s.has("アプリを非表示"));
    let p = Pos2::new(330., 580.);
    s.pointer(p, true);
    s.pointer(p, false);
    s.settle();
    assert!(!s.has("アプリを非表示"));
    assert!(s.has("ALPHA") && !s.has("BETA"));
    assert_eq!(std::fs::read(s.dir.settings()).unwrap(), before);
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn a39_reading_removed_by_empty_save_keeps_literal_name_match() {
    use quiet_launcher::search::{AppSearch, ReadingSearch};
    let dir = Directory::new("reading-clear");
    let mut settings = Settings::default();
    settings
        .search_readings
        .insert("a/Main".into(), "ほげほげ".into());
    settings.save(&dir.settings()).unwrap();
    let apps = vec![app("a/Main", "UnknownBrand")];
    let search =
        ReadingSearch::with_readings(Settings::load(&dir.settings()).unwrap().search_readings);
    assert_eq!(search.search("hogehoge", &apps)[0].id, "a/Main");
    settings.search_readings.remove("a/Main");
    settings.save(&dir.settings()).unwrap();
    let search =
        ReadingSearch::with_readings(Settings::load(&dir.settings()).unwrap().search_readings);
    assert!(search.search("hogehoge", &apps).is_empty());
    assert_eq!(search.search("UnknownBrand", &apps)[0].id, "a/Main");
}
#[test]
fn a13_1024_character_query_can_be_cleared_without_losing_the_first_result() {
    let mut s = Screen::new(json!({}), vec![app("a/Main", "ALPHA")]);
    s.drawer();
    s.device.native.store(true, Ordering::SeqCst);
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("search".into(), "あ".repeat(1024)));
    s.frame(vec![]);
    s.settle();
    assert!(!s.has("ALPHA"));
    assert!(s.device.launched.lock().unwrap().is_empty());
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("search".into(), String::new()));
    s.frame(vec![]);
    s.settle();
    assert!(s.has("ALPHA"));
}
#[test]
fn a31_real_label_widget_is_distinct_from_unknown_name() {
    let names = WidgetNames::from([(7, "以前の名前".into())]);
    assert_eq!(
        widget_title(7, &names, Some("ウィジェット")),
        "ウィジェット"
    );
    assert_eq!(widget_title(7, &names, Some("  ")), "以前の名前");
    assert_eq!(widget_title(8, &names, None), "未連携 (8)");
    assert_ne!(widget_title(8, &names, None), "ウィジェット");
}
#[test]
fn a50_received_widget_state_is_displayed_within_two_seconds() {
    let clock = Arc::new(Time::default());
    let forecast = Arc::new(Forecast::default());
    let mut s = Screen::with_environment(
        json!({"widget_ids":[7],"widget_names":{"7":"TEST PROVIDER"}}),
        vec![],
        forecast.environment(clock.clone()),
    );
    s.device.widgets.lock().unwrap().push(TextWidget {
        id: 7,
        title: "TEST PROVIDER".into(),
        lines: vec!["STATE A".into()],
        pending: false,
    });
    clock.advance(2);
    s.widget_poll();
    assert!(s.has("STATE A"));
    s.device.widgets.lock().unwrap()[0].lines = vec!["STATE B".into()];
    clock.advance(2);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        s.frame(vec![]);
        if s.has("STATE B") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "received update not shown within two seconds: {:?}",
            s.labels()
        );
        std::thread::yield_now();
    }
    assert!(!s.has("STATE A"));
}
