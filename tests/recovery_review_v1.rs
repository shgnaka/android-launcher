//! DATA-03 / A45: expectations derived from the confirmed-recovery requirement.
#![cfg(feature = "ui")]
mod support;
use serde_json::json;
use support::{app, screen::Screen};
fn corrupted() -> Screen {
    let mut s = Screen::new(json!({}), vec![app("a/Main", "ALPHA")]);
    std::fs::write(s.dir.settings(), b"private corrupt data").unwrap();
    let _ = std::fs::remove_file(s.dir.settings().with_extension("json.bak"));
    s.app = quiet_launcher::ui::Launcher::from_context(&s.ctx, s.device.clone(), s.dir.settings());
    s.frame(vec![]);
    s.settings();
    s
}
#[test]
fn reset_cancel_keeps_corrupt_original_and_stays_in_settings() {
    let mut s = corrupted();
    s.click("設定を初期化");
    assert_eq!(
        std::fs::read(s.dir.settings()).unwrap(),
        b"private corrupt data"
    );
    s.click("キャンセル");
    assert_eq!(
        std::fs::read(s.dir.settings()).unwrap(),
        b"private corrupt data"
    );
    assert!(s.has("設定を読み取れません"));
}
#[test]
fn confirmed_reset_archives_original_and_restores_a_usable_home() {
    let mut s = corrupted();
    s.click("設定を初期化");
    assert_eq!(
        std::fs::read(s.dir.settings()).unwrap(),
        b"private corrupt data"
    );
    s.click("初期化する");
    assert_eq!(
        std::fs::read(s.dir.settings().with_extension("json.corrupt")).unwrap(),
        b"private corrupt data"
    );
    assert!(quiet_launcher::config::Settings::load(&s.dir.settings()).is_ok());
    s.back();
    assert!(s.has("＋ アプリを追加"));
}
