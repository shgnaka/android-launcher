use crate::support::{screen::*, *};
use eframe::egui::{self, Pos2, Vec2};
use quiet_launcher::{model::*, search::AppSearch};
use serde_json::json;
use std::sync::atomic::Ordering;
fn basic() -> serde_json::Value {
    json!({})
}
fn alpha() -> Vec<AppEntry> {
    vec![
        app("a/Main", "ALPHA"),
        app("b/Main", "BETA"),
        app("c/Main", "GAMMA"),
    ]
}
// A05/A07/A08: wrong source/selection/header/long-press branch changes these outcomes.
#[test]
fn home_only_shows_registered_apps_without_navigation_headers() {
    let s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    assert!(s.has("ALPHA"));
    assert!(!s.has("BETA"));
    for t in ["ホーム", "アプリ ↑", "アプリ↑", "テキストウィジェット"] {
        assert!(!s.labels().iter().any(|x| x == t));
    }
}
#[test]
fn home_launch_uses_identity_after_the_catalog_order_changes() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.app
        .replace_apps(vec![app("b/Main", "BETA"), app("a/Main", "ALPHA")]);
    s.frame(vec![]);
    s.click("ALPHA");
    assert_eq!(*s.device.launched.lock().unwrap(), vec!["a/Main"]);
}
#[test]
fn home_name_long_press_does_not_open_settings_or_app_actions() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.secondary("ALPHA");
    assert!(s.has("ALPHA"));
    assert!(!s.has("文字サイズ"));
    assert!(!s.has("アプリを非表示"));
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn home_background_opens_settings_and_back_returns_home() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.settings();
    s.back();
    assert!(s.has("ALPHA"));
    assert!(!s.has("文字サイズ"));
}
#[test]
fn single_up_swipe_opens_the_drawer_without_launching_an_app() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    assert!(s.has("ALPHA") && s.has("BETA") && s.has("GAMMA"));
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn back_from_search_clears_query_and_reopens_all_visible_apps() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.search("ALPHA");
    assert!(s.has("ALPHA") && !s.has("BETA"));
    s.back();
    assert!(s.has("＋ アプリを追加"));
    s.drawer();
    assert!(s.has("BETA") && s.has("GAMMA"));
}
#[test]
fn back_on_home_keeps_the_launcher_home() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.back();
    assert!(s.has("ALPHA"));
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn back_from_an_app_menu_returns_home_in_one_operation() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.secondary("ALPHA");
    assert!(s.has("アプリを非表示"));
    s.back();
    assert!(s.has("＋ アプリを追加"));
    assert!(!s.has("BETA"));
    assert!(!s.has("アプリを非表示"));
}
#[test]
fn rotation_resets_drawer_and_query_to_home() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.search("ALPHA");
    s.size = Vec2::new(640., 360.);
    s.frame(vec![]);
    s.settle();
    assert!(
        s.has("＋ アプリを追加"),
        "rotation must reset page: {:?}",
        s.labels()
    );
    assert!(!s.has("BETA"));
}
#[test]
fn drawer_long_press_does_not_mutate_registration_until_an_action() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.secondary("ALPHA");
    assert!(s.has("アプリを非表示"));
    assert!(s.saved().favorites.is_empty());
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn drawer_menu_addition_persists_and_appears_on_home() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.secondary("ALPHA");
    s.click("ホームに追加");
    s.back();
    assert!(s.has("ALPHA"));
    assert_eq!(s.saved().favorites, vec!["a/Main"]);
}
#[test]
fn hidden_packages_are_absent_from_home_drawer_and_literal_search() {
    let mut s = Screen::new(
        json!({"favorites":["example.notes/Main","example.camera/Main","example.other/Main"],"hidden_packages":["example.camera"]}),
        apps(),
    );
    assert!(s.has("Camera") && s.has("メモ"));
    s.drawer();
    s.search("Camera");
    assert_eq!(
        s.labels().iter().filter(|t| *t == "Camera").count(),
        2,
        "input and exactly one visible app"
    );
    s.click("Camera");
    assert_eq!(
        *s.device.launched.lock().unwrap(),
        vec!["example.other/Main"]
    );
}
#[test]
fn hiding_via_the_drawer_menu_preserves_favorite_identity_and_order() {
    let mut s = Screen::new(json!({"favorites":["a/Main","b/Main","c/Main"]}), alpha());
    s.drawer();
    s.secondary("BETA");
    s.click("アプリを非表示");
    assert!(!s.has("BETA"));
    s.back();
    assert!(!s.has("BETA"));
    assert_eq!(s.saved().favorites, vec!["a/Main", "b/Main", "c/Main"]);
    assert!(s.saved().is_hidden("b/Other"));
}
#[test]
fn hiding_an_app_does_not_remove_its_registered_widget() {
    let mut s = Screen::new(
        json!({"favorites":["a/Main"],"widget_ids":[7],"widget_names":{"7":"TEST PROVIDER"}}),
        alpha(),
    );
    s.drawer();
    s.secondary("ALPHA");
    s.click("アプリを非表示");
    assert_eq!(s.saved().widget_ids, vec![7]);
    assert_eq!(s.saved().widget_names[&7], "TEST PROVIDER");
}
struct Everything;
impl AppSearch for Everything {
    fn search<'a>(&self, _: &str, apps: &'a [AppEntry]) -> Vec<&'a AppEntry> {
        apps.iter().collect()
    }
}
#[test]
fn swapped_engine_cannot_leak_hidden_results() {
    let mut s = Screen::new(json!({"hidden_packages":["a"]}), alpha());
    s.app.set_search_engine(Box::new(Everything));
    s.drawer();
    s.search("anything");
    assert!(!s.has("ALPHA"));
    assert!(s.has("BETA") && s.has("GAMMA"));
}
#[test]
fn default_search_returns_kana_for_romaji_and_not_other_apps() {
    let mut s = Screen::new(
        basic(),
        vec![app("a/Main", "カメラ"), app("b/Main", "別名")],
    );
    s.drawer();
    s.search("kamera");
    assert!(s.has("カメラ"));
    assert!(!s.has("別名"));
}
#[test]
fn enter_in_search_does_not_auto_launch() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.search("ALPHA");
    s.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(s.device.launched.lock().unwrap().is_empty());
    assert!(s.has("ALPHA"));
}
#[test]
fn same_named_apps_expose_distinguishing_package_information() {
    let mut s = Screen::new(basic(), apps());
    s.drawer();
    s.search("Camera");
    assert!(
        s.has("example.camera") && s.has("example.other"),
        "same-named destinations must be distinguishable: {:?}",
        s.labels()
    );
}
#[test]
fn app_list_has_a_stable_normalized_order() {
    let mut s = Screen::new(
        basic(),
        vec![
            app("z/Main", "Zulu"),
            app("b/Main", "beta"),
            app("a/Main", "ALPHA"),
        ],
    );
    s.drawer();
    let order: Vec<_> = s
        .labels()
        .into_iter()
        .filter(|x| ["Zulu", "beta", "ALPHA"].contains(&x.as_str()))
        .collect();
    assert_eq!(order, vec!["ALPHA", "beta", "Zulu"]);
}
#[test]
fn missing_labels_use_package_names_instead_of_blank_rows() {
    let mut s = Screen::new(
        basic(),
        vec![
            app("example.blank/Main", "   "),
            app("example.empty/Main", ""),
        ],
    );
    s.drawer();
    assert!(s.has("example.blank") && s.has("example.empty"));
}
#[test]
fn returning_to_home_and_drawer_does_not_enumerate_an_unchanged_catalog() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.wait_snapshot();
    for _ in 0..10 {
        s.drawer();
        s.back();
    }
    assert_eq!(s.device.enumerations.load(Ordering::SeqCst), 0);
    assert!(s.has("ALPHA"));
}
#[test]
fn focus_return_after_launch_is_home_and_keeps_cached_favorites() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.drawer();
    s.click("BETA");
    s.focused = false;
    s.frame(vec![]);
    s.focused = true;
    s.frame(vec![]);
    s.settle();
    assert!(s.has("ALPHA"));
    assert!(!s.has("BETA"));
}
#[test]
fn corrupted_privacy_settings_do_not_expose_cached_apps() {
    let mut s = Screen::new(basic(), alpha());
    std::fs::write(s.dir.settings(), b"bad settings").unwrap();
    s.app = quiet_launcher::ui::Launcher::from_context(&s.ctx, s.device.clone(), s.dir.settings());
    s.frame(vec![]);
    assert!(s.has("設定を読み取れません"));
    s.drawer();
    assert!(!s.has("ALPHA") && !s.has("BETA"));
}
#[test]
fn corrupted_settings_allow_a_confirmed_reset_when_backup_is_missing() {
    let mut s = Screen::new(basic(), alpha());
    std::fs::write(s.dir.settings(), b"bad settings").unwrap();
    let _ = std::fs::remove_file(s.dir.settings().with_extension("json.bak"));
    s.app = quiet_launcher::ui::Launcher::from_context(&s.ctx, s.device.clone(), s.dir.settings());
    s.frame(vec![]);
    s.settings();
    assert!(
        s.has("設定を初期化"),
        "recovery needs a usable path without a backup: {:?}",
        s.labels()
    );
}
#[test]
fn unknown_usage_is_not_displayed_as_zero_or_silently_hidden() {
    let mut s = Screen::new(json!({"show_usage":true}), alpha());
    s.wait_snapshot();
    assert!(
        s.has("—"),
        "usage unavailable needs an explicit unknown state: {:?}",
        s.labels()
    );
    assert!(!s.has("0時間00分"));
}
#[test]
fn confirmed_zero_usage_is_displayed_with_the_range_label() {
    let mut s = Screen::new(json!({"show_usage":true}), alpha());
    s.device.snapshot.lock().unwrap().screen_seconds = Some(0);
    s.focused = false;
    s.frame(vec![]);
    s.focused = true;
    s.frame(vec![]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !s.has("今日 0時間00分") {
        s.frame(vec![]);
        assert!(std::time::Instant::now() < deadline, "{:?}", s.labels());
        std::thread::yield_now();
    }
}
#[test]
fn missing_media_targets_do_not_leave_global_media_controls() {
    let s = Screen::new(json!({"show_media":true}), alpha());
    for t in ["前へ", "再生", "一時停止", "次へ"] {
        assert!(!s.labels().iter().any(|x| x == t));
    }
}
#[test]
fn settings_provide_notification_access_for_media_sessions() {
    let mut s = Screen::new(basic(), alpha());
    s.settings();
    assert!(
        s.has("通知アクセス"),
        "media integration needs its permission entry: {:?}",
        s.labels()
    );
}
#[test]
fn oversized_home_shows_error_without_rendering_an_arbitrary_subset() {
    let a: Vec<_> = (0..20)
        .map(|i| app(&format!("p/A{i}"), &format!("APP{i}")))
        .collect();
    let mut s = Screen::new(
        json!({"favorites":a.iter().map(|a|a.id.clone()).collect::<Vec<_>>()}),
        a,
    );
    s.frame(vec![]);
    assert!(s.has("収まりません"));
    assert!(!s.labels().iter().any(|t| t.starts_with("APP")));
    assert_eq!(s.saved().favorites.len(), 20);
}
#[test]
fn native_search_mounts_inside_the_safe_visible_width() {
    let mut s = Screen::new(json!({"font_percent":160}), alpha());
    s.device.native.store(true, Ordering::SeqCst);
    *s.device.insets.lock().unwrap() = [0., 40., 0., 24.];
    s.drawer();
    let field = s
        .device
        .field
        .lock()
        .unwrap()
        .clone()
        .expect("native inline search mounted");
    assert!(field.rect[0] >= 0 && field.rect[0] + field.rect[2] <= 360);
    assert!(field.rect[1] >= 40);
    assert_eq!(field.font_percent, 160);
}
#[test]
fn widgets_keep_real_provider_names_and_show_their_body() {
    let mut s = Screen::new(
        json!({"widget_ids":[7],"widget_names":{"7":"TEST PROVIDER"}}),
        alpha(),
    );
    s.device.widgets.lock().unwrap().push(TextWidget {
        id: 7,
        title: "TEST PROVIDER".into(),
        lines: vec!["QUIET WIDGET READY".into()],
        pending: false,
    });
    s.widget_poll();
    assert!(s.has("TEST PROVIDER") && s.has("QUIET WIDGET READY"));
}
#[test]
fn deleted_widget_content_does_not_reappear_from_late_updates() {
    let mut s = Screen::new(basic(), alpha());
    s.device.widgets.lock().unwrap().push(TextWidget {
        id: 7,
        title: "DELETED PROVIDER".into(),
        lines: vec!["LATE CONTENT".into()],
        pending: false,
    });
    s.widget_poll();
    assert!(!s.has("DELETED PROVIDER") && !s.has("LATE CONTENT"));
}
#[test]
fn back_during_settings_does_not_persist_unsent_weather_text() {
    let mut s = Screen::new(basic(), alpha());
    s.settings();
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("location".into(), "35.0,135.0".into()));
    s.frame(vec![]);
    s.back();
    assert!(s.saved().weather_location.is_none());
    assert!(s.has("＋ アプリを追加"));
}
#[test]
fn home_up_gesture_includes_the_exact_48dp_boundary() {
    let mut s = Screen::new(basic(), alpha());
    s.drag(Pos2::new(160., 500.), Pos2::new(160., 452.));
    assert!(s.has("BETA"), "48dp must open the drawer: {:?}", s.labels());
}
#[test]
fn home_up_gesture_below_threshold_does_not_open() {
    let mut s = Screen::new(basic(), alpha());
    s.drag(Pos2::new(160., 500.), Pos2::new(160., 453.));
    assert!(!s.has("BETA"));
    assert!(s.has("＋ アプリを追加"));
}
#[test]
fn hidden_names_are_only_in_the_dedicated_management_section() {
    let mut s = Screen::new(
        json!({"favorites":["a/Main","b/Main"],"hidden_packages":["b"]}),
        alpha(),
    );
    s.settings();
    s.find_scroll("ホームの並び順");
    let text = s.labels();
    let start = text.iter().position(|x| x == "ホームの並び順").unwrap();
    let end = text
        .iter()
        .position(|x| x == "非表示アプリ")
        .unwrap_or(text.len());
    assert!(!text[start..end].iter().any(|x| x == "BETA"));
    s.find_scroll("非表示アプリ");
    assert!(s.has("BETA"));
}
#[test]
fn unhide_from_settings_restores_the_original_home_order() {
    let mut s = Screen::new(
        json!({"favorites":["a/Main","b/Main","c/Main"],"hidden_packages":["b"]}),
        alpha(),
    );
    s.settings();
    s.find_scroll("再表示");
    s.click("再表示");
    s.back();
    let order: Vec<_> = s
        .labels()
        .into_iter()
        .filter(|x| ["ALPHA", "BETA", "GAMMA"].contains(&x.as_str()))
        .collect();
    assert_eq!(order, vec!["ALPHA", "BETA", "GAMMA"]);
    assert!(!s.saved().is_hidden("b"));
}
#[test]
fn failed_hide_save_keeps_the_app_visible_and_the_prior_file() {
    let mut s = Screen::new(basic(), alpha());
    let before = std::fs::read(s.dir.settings()).unwrap();
    std::fs::create_dir(s.dir.settings().with_extension("json.bak")).unwrap();
    s.drawer();
    s.secondary("ALPHA");
    s.click("アプリを非表示");
    assert!(s.has("ALPHA"));
    assert!(s.has("完了できません") || s.has("保存できません"));
    assert_eq!(std::fs::read(s.dir.settings()).unwrap(), before);
}
#[test]
fn stale_pointer_release_cannot_launch_the_replacement_row() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    let p = s.position("ALPHA");
    s.pointer(p, true);
    s.app.replace_apps(vec![
        app("b/Main", "BETA"),
        app("a/Main", "ALPHA"),
        app("c/Main", "GAMMA"),
    ]);
    s.frame(vec![]);
    s.pointer(p, false);
    assert!(!s
        .device
        .launched
        .lock()
        .unwrap()
        .iter()
        .any(|id| id == "b/Main"));
}
#[test]
fn front_return_updates_renamed_favorite_without_an_empty_home_frame() {
    let mut s = Screen::new(json!({"favorites":["a/Main"]}), alpha());
    s.wait_snapshot();
    *s.device.apps.lock().unwrap() = vec![app("a/Main", "RENAMED"), app("b/Main", "BETA")];
    s.device.revision.fetch_add(1, Ordering::SeqCst);
    s.focused = false;
    s.frame(vec![]);
    s.focused = true;
    s.frame(vec![]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !s.has("RENAMED") {
        assert!(
            s.has("ALPHA"),
            "normal cached favorite must not disappear: {:?}",
            s.labels()
        );
        s.frame(vec![]);
        assert!(
            std::time::Instant::now() < deadline,
            "catalog change not reflected within five seconds"
        );
        std::thread::yield_now();
    }
    assert_eq!(s.saved().favorites, vec!["a/Main"]);
}
#[test]
fn missing_favorites_are_grouped_for_explicit_management() {
    let mut s = Screen::new(json!({"favorites":["missing/Main"]}), alpha());
    assert!(!s.has("missing/Main"));
    s.settings();
    s.find_scroll("未インストールの登録");
    assert!(s.has("missing"));
    assert_eq!(s.saved().favorites, vec!["missing/Main"]);
}
#[test]
fn hidden_uninstalled_packages_remain_manageable_with_their_identity() {
    let mut s = Screen::new(
        json!({"favorites":["missing/Main"],"hidden_packages":["missing"]}),
        alpha(),
    );
    s.settings();
    s.find_scroll("再表示");
    assert!(s.has("missing"));
    assert!(s.has("未インストール"));
    assert!(
        !s.has("未インストール: missing/Main"),
        "hidden app must not leak into normal favorites management"
    );
}
#[test]
fn turning_on_an_oversized_section_is_rejected_without_changing_preferences() {
    let a: Vec<_> = (0..8)
        .map(|i| app(&format!("p/A{i}"), &format!("APP{i}")))
        .collect();
    let mut s = Screen::new(
        json!({"favorites":a.iter().map(|a|a.id.clone()).collect::<Vec<_>>()}),
        a,
    );
    s.settings();
    s.click("メディアコントロール");
    assert!(!s.saved().show_media);
    assert!(s.has("収まりません"));
    assert_eq!(s.saved().favorites.len(), 8);
}
#[test]
fn all_apps_hidden_leaves_a_usable_empty_home_and_settings() {
    let mut s = Screen::new(
        json!({"favorites":["a/Main","b/Main"],"hidden_packages":["a","b","c"]}),
        alpha(),
    );
    assert!(s.has("＋ アプリを追加"));
    s.drawer();
    assert!(!s.has("ALPHA") && !s.has("BETA") && !s.has("GAMMA"));
    s.back();
    s.settings();
    assert!(s.has("文字サイズ"));
}
#[test]
fn dark_theme_remains_saved_after_a_visible_toggle() {
    let mut s = Screen::new(basic(), alpha());
    s.settings();
    s.click("暗い背景");
    assert!(s.saved().dark);
    s.back();
    assert!(s.ctx.style().visuals.dark_mode);
}
#[test]
fn unknown_battery_is_explicit_instead_of_inventing_a_percentage() {
    let mut s = Screen::new(basic(), alpha());
    s.wait_snapshot();
    assert!(s.has("電池 —"));
    assert!(!s.has("電池 0%"));
}
#[test]
fn registered_widget_deletion_persists_and_late_content_stays_absent() {
    let mut s = Screen::new(
        json!({"widget_ids":[7],"widget_names":{"7":"TEST PROVIDER"}}),
        alpha(),
    );
    s.device.widgets.lock().unwrap().push(TextWidget {
        id: 7,
        title: "TEST PROVIDER".into(),
        lines: vec!["QUIET WIDGET READY".into()],
        pending: false,
    });
    s.widget_poll();
    s.settings();
    s.find_scroll("削除");
    s.click("削除");
    assert!(s.saved().widget_ids.is_empty());
    s.back();
    s.device.widgets.lock().unwrap().push(TextWidget {
        id: 7,
        title: "TEST PROVIDER".into(),
        lines: vec!["LATE CONTENT".into()],
        pending: false,
    });
    s.widget_poll();
    assert!(!s.has("TEST PROVIDER") && !s.has("LATE CONTENT"));
}
#[test]
fn scriptless_native_search_can_be_cleared_by_one_home_event() {
    let mut s = Screen::new(basic(), alpha());
    s.device.native.store(true, Ordering::SeqCst);
    s.drawer();
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("search".into(), "ALPHA".into()));
    s.frame(vec![]);
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("home".into(), String::new()));
    s.frame(vec![]);
    s.settle();
    assert!(s.has("＋ アプリを追加"));
    s.drawer();
    assert!(s.has("BETA"));
    let field = s.device.field.lock().unwrap().clone().unwrap();
    assert!(field.text.is_empty());
}
fn touch_event(p: Pos2, phase: egui::TouchPhase) -> egui::Event {
    egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(1),
        phase,
        pos: p,
        force: None,
    }
}
fn hold(s: &mut Screen, label: &str, seconds: f64, movement: f32) {
    let p = s.position(label);
    s.frame(vec![
        touch_event(p, egui::TouchPhase::Start),
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    let start = s.now;
    s.now = start + seconds - 1. / 60.;
    let end = p + Vec2::new(movement, 0.);
    s.frame(vec![
        touch_event(end, egui::TouchPhase::Move),
        egui::Event::PointerMoved(end),
    ]);
}
#[test]
fn touch_hold_of_499ms_does_not_open_the_app_menu() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    hold(&mut s, "ALPHA", 0.499, 0.);
    assert!(!s.has("アプリを非表示"));
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn touch_hold_of_500ms_opens_the_menu_without_launching() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    hold(&mut s, "ALPHA", 0.500, 0.);
    assert!(s.has("アプリを非表示"), "{:?}", s.labels());
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn ten_dp_movement_cancels_the_long_press_candidate() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    hold(&mut s, "ALPHA", 0.6, 10.);
    assert!(!s.has("アプリを非表示"));
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn cancelled_touch_does_not_launch_the_previously_pressed_row() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    let p = s.position("ALPHA");
    s.frame(vec![
        touch_event(p, egui::TouchPhase::Start),
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    s.frame(vec![
        touch_event(p, egui::TouchPhase::Cancel),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerGone,
    ]);
    assert!(s.device.launched.lock().unwrap().is_empty());
}
#[test]
fn downward_close_includes_the_exact_112dp_boundary() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.drag(Pos2::new(160., 250.), Pos2::new(160., 362.));
    assert!(s.has("＋ アプリを追加"));
    assert!(!s.has("BETA"));
}
#[test]
fn downward_close_below_112dp_does_not_close() {
    let mut s = Screen::new(basic(), alpha());
    s.drawer();
    s.drag(Pos2::new(160., 250.), Pos2::new(160., 361.));
    assert!(s.has("BETA"));
}
#[test]
fn vertical_dominance_threshold_includes_exact_one_point_five() {
    let mut s = Screen::new(basic(), alpha());
    s.drag(Pos2::new(160., 500.), Pos2::new(210., 425.));
    assert!(s.has("BETA"));
}
#[test]
fn horizontal_drag_does_not_open_the_app_drawer() {
    let mut s = Screen::new(basic(), alpha());
    s.drag(Pos2::new(100., 500.), Pos2::new(250., 450.));
    assert!(s.has("＋ アプリを追加"));
    assert!(!s.has("BETA"));
}
#[test]
fn removing_a_search_query_after_scrolling_makes_the_first_result_reachable() {
    let a: Vec<_> = (0..200)
        .map(|i| app(&format!("p/A{i:03}"), &format!("APP{i:03}")))
        .collect();
    let mut s = Screen::new(basic(), a);
    s.drawer();
    s.drag(Pos2::new(180., 500.), Pos2::new(180., 150.));
    s.device.native.store(true, Ordering::SeqCst);
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("search".into(), "APP000".into()));
    s.frame(vec![]);
    s.settle();
    assert!(s.has("APP000"));
    s.device
        .text
        .lock()
        .unwrap()
        .push_back(("search".into(), String::new()));
    s.frame(vec![]);
    s.settle();
    assert!(s.has("APP000"));
}
#[test]
fn ten_thousand_entries_are_virtualized_and_can_find_a_far_away_app() {
    let a: Vec<_> = (0..10000)
        .map(|i| app(&format!("p/A{i:05}"), &format!("APP{i:05}")))
        .collect();
    let mut s = Screen::new(basic(), a);
    s.drawer();
    assert!(s.labels().iter().filter(|x| x.starts_with("APP")).count() < 20);
    s.search("APP09999");
    assert!(s.has("APP09999"));
    assert!(!s.has("APP00000"));
}
