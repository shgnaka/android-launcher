use crate::support::{controlled::*, screen::Screen};
use quiet_launcher::model::{MediaAction, MediaSession, Playback};
use serde_json::json;
use std::sync::Arc;
fn sessions() -> Vec<MediaSession> {
    vec![
        MediaSession {
            id: "p1:1".into(),
            package: "example.player1".into(),
            provider: "PLAYER ONE".into(),
            title: Some("TRACK B".into()),
            state: Playback::Playing,
            previous: true,
            play: true,
            pause: true,
            next: true,
        },
        MediaSession {
            id: "p2:1".into(),
            package: "example.player2".into(),
            provider: "PLAYER TWO".into(),
            title: Some("TRACK X".into()),
            state: Playback::Paused,
            previous: false,
            play: true,
            pause: true,
            next: true,
        },
    ]
}
fn setup() -> (Screen, Arc<Time>) {
    let t = Arc::new(Time::default());
    let f = Arc::new(Forecast::default());
    let mut s =
        Screen::with_environment(json!({"show_media":true}), vec![], f.environment(t.clone()));
    *s.device.sessions.lock().unwrap() = sessions();
    t.advance(2);
    // The media controller runs on a worker thread. A fixed number of simulated
    // frames does not guarantee that the worker has been scheduled under load.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !s.has("PLAYER ONE") && std::time::Instant::now() < deadline {
        s.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(s.has("PLAYER ONE"), "initial media session did not arrive");
    (s, t)
}
fn swipe(s: &mut Screen) {
    let p = s.position("PLAYER ONE");
    s.drag(
        p + eframe::egui::Vec2::new(120., 0.),
        p - eframe::egui::Vec2::new(20., 0.),
    );
}
#[test]
fn playing_session_is_initially_selected_and_its_track_is_visible() {
    let (s, _) = setup();
    assert!(s.has("PLAYER ONE") && s.has("TRACK B"));
    assert!(!s.has("TRACK X"));
}
#[test]
fn horizontal_swipe_selects_the_next_session_without_sending_a_command() {
    let (mut s, _) = setup();
    swipe(&mut s);
    assert!(s.has("PLAYER TWO") && s.has("TRACK X"));
    assert!(s.device.media_commands.lock().unwrap().is_empty());
}
#[test]
fn enumeration_order_does_not_change_the_selected_session() {
    let (mut s, t) = setup();
    swipe(&mut s);
    s.device.sessions.lock().unwrap().reverse();
    t.advance(1);
    s.settle();
    assert!(s.has("PLAYER TWO") && s.has("TRACK X"));
}
#[test]
fn removal_of_selected_session_falls_back_to_remaining_playback() {
    let (mut s, t) = setup();
    swipe(&mut s);
    s.device.sessions.lock().unwrap().retain(|x| x.id == "p1:1");
    t.advance(1);
    s.settle();
    assert!(s.has("PLAYER ONE"));
    assert!(!s.has("PLAYER TWO"));
}
#[test]
fn no_sessions_hides_the_media_region() {
    let (mut s, t) = setup();
    s.device.sessions.lock().unwrap().clear();
    t.advance(1);
    s.settle();
    assert!(!s.has("PLAYER ONE") && !s.has("PLAYER TWO"));
    assert!(!s.has("TRACK B"));
}
#[test]
fn stopped_error_and_none_sessions_are_not_control_targets() {
    let (mut s, t) = setup();
    for x in s.device.sessions.lock().unwrap().iter_mut() {
        x.state = Playback::Stopped;
    }
    t.advance(1);
    s.settle();
    assert!(!s.has("PLAYER ONE") && !s.has("PLAYER TWO"));
}
#[test]
fn permission_revocation_hides_previously_valid_session_data() {
    let (mut s, t) = setup();
    assert!(s.has("PLAYER ONE"));
    s.device
        .media_denied
        .store(true, std::sync::atomic::Ordering::SeqCst);
    t.advance(1);
    s.settle();
    assert!(!s.has("PLAYER ONE") && !s.has("TRACK B"));
}

fn tab(s: &mut Screen) {
    s.frame(vec![eframe::egui::Event::Key {
        key: eframe::egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: eframe::egui::Modifiers::NONE,
    }]);
}
fn focus(s: &mut Screen, label: &str) {
    for _ in 0..15 {
        tab(s);
        if s.output.platform_output.events.iter().any(|e| {
            e.widget_info()
                .label
                .as_ref()
                .is_some_and(|l| l.contains(label))
        }) {
            return;
        }
    }
    panic!("accessible media action missing: {label}; {:?}", s.labels());
}
fn enter(s: &mut Screen) {
    s.frame(vec![eframe::egui::Event::Key {
        key: eframe::egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: eframe::egui::Modifiers::NONE,
    }]);
    s.settle();
}
#[test]
fn media_controls_are_symbols_with_accessible_action_names() {
    let (mut s, _) = setup();
    assert!(s.has("PLAYER ONE"));
    for text in ["前へ", "再生", "一時停止", "次へ"] {
        assert!(!s.labels().iter().any(|t| t == text));
    }
    for text in ["前", "停止", "次"] {
        focus(&mut s, text);
    }
}
#[test]
fn next_targets_only_selected_player_and_displays_its_real_update() {
    let (mut s, t) = setup();
    assert!(s.has("PLAYER ONE"));
    focus(&mut s, "次");
    enter(&mut s);
    t.advance(1);
    s.settle();
    assert_eq!(
        *s.device.media_commands.lock().unwrap(),
        vec![("p1:1".into(), MediaAction::Next)]
    );
    assert!(s.has("TRACK C"));
    assert!(!s.has("TRACK Y"));
}
#[test]
fn pause_command_and_playback_state_belong_to_selected_player() {
    let (mut s, t) = setup();
    assert!(s.has("PLAYER ONE"));
    focus(&mut s, "停止");
    enter(&mut s);
    t.advance(1);
    s.settle();
    assert_eq!(
        *s.device.media_commands.lock().unwrap(),
        vec![("p1:1".into(), MediaAction::Pause)]
    );
    focus(&mut s, "再生");
}
#[test]
fn disappeared_target_does_not_redirect_a_focused_action_to_another_player() {
    let (mut s, _) = setup();
    assert!(s.has("PLAYER ONE"));
    focus(&mut s, "次");
    s.device.sessions.lock().unwrap().retain(|x| x.id == "p2:1");
    enter(&mut s);
    assert!(!s
        .device
        .media_commands
        .lock()
        .unwrap()
        .iter()
        .any(|(id, _)| id == "p2:1"));
}
#[test]
fn successful_command_submission_does_not_invent_external_playback_state() {
    let (mut s, t) = setup();
    assert!(s.has("PLAYER ONE"));
    s.device
        .ignore_media_commands
        .store(true, std::sync::atomic::Ordering::SeqCst);
    focus(&mut s, "停止");
    enter(&mut s);
    t.advance(1);
    s.settle();
    assert_eq!(
        *s.device.media_commands.lock().unwrap(),
        vec![("p1:1".into(), MediaAction::Pause)]
    );
    focus(&mut s, "停止");
}
