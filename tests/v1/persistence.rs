use crate::support::*;
use quiet_launcher::config::Settings;
use serde_json::json;
// A32/A33/A45: catches destructive migration, future-version overwrite, missing validation.
#[test]
fn missing_file_defaults_do_not_hide_or_register_apps() {
    let d = Directory::new("first");
    let s = Settings::load(&d.settings()).unwrap();
    assert!(s.favorites.is_empty());
    assert!(s.hidden_packages.is_empty());
    assert!(s.widget_ids.is_empty());
    assert_eq!(s.font_percent, 100);
    assert!(!s.usage_last_24h);
    assert!(!s.dark);
    assert!(s.show_weather && s.show_usage && s.show_media);
    assert!(!d.settings().exists());
}
#[test]
fn legacy_notes_do_not_destroy_favorites_or_widgets() {
    let d = Directory::new("legacy");
    let p=d.write(r#"{"favorites":["example.notes/Main"],"widget_ids":[7],"widget_names":{"7":"時計"},"notes":[{"text":"old memo"}]}"#.as_bytes());
    let s = Settings::load(&p).unwrap();
    s.save(&p).unwrap();
    let r = Settings::load(&p).unwrap();
    assert_eq!(r.favorites, vec!["example.notes/Main"]);
    assert_eq!(r.widget_names[&7], "時計");
    assert!(r.hidden_packages.is_empty());
}
#[test]
fn unsupported_schema_never_loads_as_a_valid_empty_configuration() {
    let d = Directory::new("future");
    let bytes = br#"{"schema_version":2,"favorites":["secret/Main"],"hidden_packages":["secret"]}"#;
    let p = d.write(bytes);
    assert!(
        Settings::load(&p).is_err(),
        "future schema must not lose privacy settings"
    );
    assert_eq!(std::fs::read(&p).unwrap(), bytes);
}
#[test]
fn ordinary_save_cannot_overwrite_a_future_schema() {
    let d = Directory::new("future-write");
    let bytes = br#"{"schema_version":2,"hidden_packages":["secret"]}"#;
    let p = d.write(bytes);
    assert!(Settings::default().save(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), bytes);
}
#[test]
fn a_new_save_emits_the_supported_schema_version() {
    let d = Directory::new("version");
    Settings::default().save(&d.settings()).unwrap();
    let j: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.settings()).unwrap()).unwrap();
    assert_eq!(j["schema_version"], 1);
}
#[test]
fn invalid_important_fields_preserve_the_original_file() {
    for v in [
        json!({"hidden_packages":"secret"}),
        json!({"font_percent":161}),
        json!({"font_percent":79}),
        json!({"favorites":42}),
        json!({"hidden_packages":["a/Main"]}),
    ] {
        let d = Directory::new("invalid");
        let p = d.write(&serde_json::to_vec(&v).unwrap());
        let original = std::fs::read(&p).unwrap();
        assert!(Settings::load(&p).is_err(), "{v}");
        assert!(Settings::default().save(&p).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), original);
    }
}
#[test]
fn duplicate_favorites_are_normalized_without_clearing_hidden_state() {
    let d = Directory::new("duplicates");
    let p = d.write(br#"{"favorites":["a/Main","b/Main","a/Main"],"hidden_packages":["a"]}"#);
    let s = Settings::load(&p).unwrap();
    assert_eq!(s.favorites, vec!["a/Main", "b/Main"]);
    assert!(s.is_hidden("a/Main"));
}
#[test]
fn reading_over_128_characters_is_rejected_before_overwrite() {
    let d = Directory::new("reading-limit");
    let mut s = Settings::default();
    s.search_readings.insert("a/Main".into(), "元の読み".into());
    s.save(&d.settings()).unwrap();
    let before = std::fs::read(d.settings()).unwrap();
    s.search_readings.insert("a/Main".into(), "あ".repeat(129));
    assert!(s.save(&d.settings()).is_err());
    assert_eq!(std::fs::read(d.settings()).unwrap(), before);
}
#[test]
fn reading_of_128_characters_can_be_saved() {
    let d = Directory::new("reading-boundary");
    let mut s = Settings::default();
    s.search_readings.insert("a/Main".into(), "あ".repeat(128));
    s.save(&d.settings()).unwrap();
    assert_eq!(
        Settings::load(&d.settings()).unwrap().search_readings["a/Main"]
            .chars()
            .count(),
        128
    );
}
#[test]
fn backup_is_the_previous_successful_configuration() {
    let d = Directory::new("backup");
    let mut s = Settings::default();
    s.favorites = vec!["a/Main".into()];
    s.save(&d.settings()).unwrap();
    s.favorites.push("b/Main".into());
    s.save(&d.settings()).unwrap();
    assert_eq!(
        Settings::load(&d.settings().with_extension("json.bak"))
            .unwrap()
            .favorites,
        vec!["a/Main"]
    );
}
#[test]
fn failed_backup_write_keeps_the_previous_primary() {
    let d = Directory::new("backup-failure");
    let p = d.write(BASIC.as_bytes());
    let before = std::fs::read(&p).unwrap();
    std::fs::create_dir(p.with_extension("json.bak")).unwrap();
    let mut s = Settings::default();
    s.dark = true;
    assert!(s.save(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), before);
}
#[test]
fn failed_primary_replacement_does_not_publish_partial_json() {
    let d = Directory::new("replace-failure");
    let p = d.settings();
    std::fs::create_dir(&p).unwrap();
    assert!(Settings::default().save(&p).is_err());
    assert!(p.is_dir());
}
#[test]
fn invalid_backup_does_not_replace_the_corrupt_primary() {
    let d = Directory::new("bad-backup");
    let p = d.write(b"corrupt primary");
    std::fs::write(p.with_extension("json.bak"), b"corrupt backup").unwrap();
    assert!(Settings::recover(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"corrupt primary");
}
#[test]
fn repeated_recovery_preserves_each_corrupt_original() {
    let d = Directory::new("archive");
    let p = d.settings();
    Settings::default().save(&p).unwrap();
    for bytes in [b"first corrupt".as_slice(), b"second corrupt".as_slice()] {
        std::fs::write(&p, bytes).unwrap();
        Settings::recover(&p).unwrap();
    }
    assert_eq!(
        std::fs::read(p.with_extension("json.corrupt")).unwrap(),
        b"first corrupt"
    );
    assert_eq!(
        std::fs::read(p.with_extension("json.corrupt.1")).unwrap(),
        b"second corrupt"
    );
}
#[test]
fn all_persistent_preferences_survive_restart() {
    let d = Directory::new("all");
    let p=d.write(r#"{"schema_version":1,"favorites":["a/Main","b/Main"],"hidden_packages":["a"],"font_percent":160,"usage_last_24h":true,"search_readings":{"b/Main":"びー"},"widget_ids":[7,8],"widget_names":{"7":"時計","8":"予定"},"weather_location":{"latitude":35.0,"longitude":135.0},"show_weather":true,"show_usage":false,"show_media":false,"dark":true}"#.as_bytes());
    let s = Settings::load(&p).unwrap();
    s.save(&p).unwrap();
    let r = Settings::load(&p).unwrap();
    assert_eq!(r.favorites, vec!["a/Main", "b/Main"]);
    assert!(r.is_hidden("a/Tools"));
    assert_eq!(r.font_percent, 160);
    assert!(r.usage_last_24h && r.dark && r.show_weather);
    assert!(!r.show_usage && !r.show_media);
    assert_eq!(r.search_readings["b/Main"], "びー");
    assert_eq!(r.widget_ids, vec![7, 8]);
    assert_eq!(r.widget_names[&8], "予定");
    assert_eq!(r.weather_location.unwrap().latitude, 35.0);
}
