//! Expected behavior authored from docs/contracts-2026-11-16.md, before implementation.
use quiet_launcher::{
    catalog::AppCatalog,
    config::Settings,
    model::AppEntry,
    search::{AppSearch, ReadingSearch},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
fn path(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir()
        .join(format!(
            "quiet-contract-{}-{}-{name}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
        .join("settings.json")
}
fn app(id: &str, name: &str) -> AppEntry {
    AppEntry {
        id: id.into(),
        label: name.into(),
    }
}

#[test]
fn kana_and_romaji_are_equivalent_without_substring_matching() {
    let cases = [
        (
            "かめら",
            vec!["カメラ", "kamera", "camera", " ＫＡＭＥＲＡ "],
        ),
        ("スマホ", vec!["すまほ", "sumaho"]),
        ("しゃしん", vec!["シャシン", "shashin", "syasin"]),
        ("がっこう", vec!["ガッコウ", "gakkou", "gakko"]),
        ("コーヒー", vec!["こーひー", "koohii", "kohii"]),
    ];
    for (label, queries) in cases {
        for query in queries {
            let apps = vec![app("a/Main", label), app("b/Main", "別のアプリ")];
            assert_eq!(
                ReadingSearch::default()
                    .search(query, &apps)
                    .iter()
                    .map(|a| a.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["a/Main"],
                "{label}: {query}"
            );
            let reversed = vec![app("a/Main", query)];
            assert_eq!(
                ReadingSearch::default().search(label, &reversed).len(),
                1,
                "reverse {label}: {query}"
            );
        }
    }
    assert!(ReadingSearch::default()
        .search("kame", &[app("a/Main", "カメラ")])
        .is_empty());
}
#[test]
fn english_brand_readings_and_explicit_unknown_readings_work_both_ways() {
    for (name, reading) in [
        ("LINE", "らいん"),
        ("Chrome", "くろーむ"),
        ("YouTube", "ゆーちゅーぶ"),
    ] {
        for (label, query) in [(name, reading), (reading, name)] {
            assert_eq!(
                ReadingSearch::default()
                    .search(query, &[app("p/Main", label)])
                    .len(),
                1,
                "{label}: {query}"
            );
        }
    }
    let engine =
        ReadingSearch::with_readings(BTreeMap::from([("custom/Main".into(), "ほげほげ".into())]));
    assert_eq!(
        engine
            .search("ホゲホゲ", &[app("custom/Main", "UnknownBrand")])
            .len(),
        1
    );
}
#[test]
fn reading_search_keeps_duplicate_names_and_empty_query_shows_everything() {
    let apps = vec![
        app("a/Main", "カメラ"),
        app("b/Main", "かめら"),
        app("c/Main", "別"),
    ];
    assert_eq!(ReadingSearch::default().search("kamera", &apps).len(), 2);
    assert_eq!(ReadingSearch::default().search("  ", &apps).len(), 3);
}
#[test]
fn settings_roundtrip_includes_new_preferences_and_legacy_data() {
    let p = path("new");
    let mut s = Settings::default();
    s.favorites = vec!["a/Main".into(), "b/Main".into()];
    s.font_percent = 130;
    s.usage_last_24h = true;
    s.hidden_packages = vec!["a".into()];
    s.search_readings.insert("a/Main".into(), "えー".into());
    s.save(&p).unwrap();
    let r = Settings::load(&p).unwrap();
    assert_eq!(r.font_percent, 130);
    assert!(r.usage_last_24h);
    assert!(r.is_hidden("a/Main"));
    assert_eq!(r.search_readings["a/Main"], "えー");
    let old: Settings = serde_json::from_str(
        r#"{"favorites":["a/Main"],"widget_ids":[3],"notes":[{"text":"old"}]}"#,
    )
    .unwrap();
    assert_eq!(old.font_percent, 100);
    assert!(!old.usage_last_24h);
    assert!(old.hidden_packages.is_empty());
    assert_eq!(old.widget_ids, vec![3]);
}
#[test]
fn failed_validation_does_not_overwrite_the_last_settings() {
    let p = path("validation");
    let s = Settings::default();
    s.save(&p).unwrap();
    let before = std::fs::read(&p).unwrap();
    let mut bad = s;
    bad.font_percent = 0;
    assert!(bad.save(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), before);
}
#[test]
fn corrupt_primary_is_preserved_and_explicit_recovery_uses_last_good_backup() {
    let p = path("recover");
    let mut s = Settings::default();
    s.favorites = vec!["a/Main".into()];
    s.save(&p).unwrap();
    s.favorites.push("b/Main".into());
    s.save(&p).unwrap();
    std::fs::write(&p, b"broken settings").unwrap();
    assert!(Settings::load(&p).is_err());
    assert!(s.save(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"broken settings");
    let r = Settings::recover(&p).unwrap();
    assert_eq!(r.favorites, vec!["a/Main"]);
    assert_eq!(Settings::load(&p).unwrap().favorites, r.favorites);
    assert!(p.with_extension("json.corrupt").exists());
}
#[test]
fn failed_recovery_never_replaces_corrupt_primary_with_defaults() {
    let p = path("no-backup");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"bad").unwrap();
    assert!(Settings::recover(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"bad");
}
#[test]
fn catalog_is_reused_across_navigation_and_restarts_and_only_invalidates_by_revision() {
    let p = path("catalog");
    let mut c = AppCatalog::load(&p).unwrap_or_default();
    let calls = AtomicUsize::new(0);
    let source = || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![app("b/Main", "B"), app("a/Main", "A")])
    };
    assert!(c.refresh("boot1:en:1", source).unwrap());
    c.save(&p).unwrap();
    for _ in 0..20 {
        assert!(!c
            .refresh("boot1:en:1", || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            })
            .unwrap());
        assert_eq!(c.apps().len(), 2);
    }
    let mut r = AppCatalog::load(&p).unwrap();
    assert!(!r
        .refresh("boot1:en:1", || panic!("cache must avoid enumeration"))
        .unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(r.apps()[1].id, "a/Main");
    assert!(r
        .refresh("boot1:ja:2", || Ok(vec![app("a/Main", "えー")]))
        .unwrap());
    assert_eq!(r.apps()[0].label, "えー");
}
#[test]
fn failed_catalog_refresh_retains_last_good_data_and_revision() {
    let mut c = AppCatalog::default();
    c.refresh("1", || Ok(vec![app("a/Main", "A")])).unwrap();
    assert!(c.refresh("2", || anyhow::bail!("offline")).is_err());
    assert_eq!(c.apps()[0].label, "A");
    assert!(!c
        .refresh("1", || panic!("failed refresh must not change revision"))
        .unwrap());
}
#[test]
fn hidden_apps_keep_favorites_and_are_excluded_from_reorder_candidates() {
    let mut s = Settings::default();
    s.favorites = vec!["a/Main".into(), "b/Main".into(), "c/Main".into()];
    s.set_hidden("b", true);
    assert!(s.is_hidden("b/Other"));
    assert!(!s.is_hidden("bb/Main"));
    assert_eq!(s.visible_favorites(), vec!["a/Main", "c/Main"]);
    s.move_visible_favorite("c/Main", -1);
    assert_eq!(s.visible_favorites(), vec!["c/Main", "a/Main"]);
    s.set_hidden("b", false);
    assert_eq!(s.favorites, vec!["c/Main", "b/Main", "a/Main"]);
}
#[test]
fn rolling_usage_clips_to_the_requested_24_hour_window() {
    use quiet_launcher::usage::{screen_seconds, ScreenEvent::*};
    let hour = 3_600_000;
    let now = 30 * hour;
    let start = now - 24 * hour;
    assert_eq!(
        screen_seconds(&[On(0), Off(8 * hour), On(28 * hour)], start, now),
        4 * 3600
    );
    assert_eq!(
        screen_seconds(&[On(0), Off(8 * hour), On(28 * hour)], 24 * hour, now),
        2 * 3600
    );
}

#[test]
fn home_rejects_overflow_instead_of_hiding_registered_apps() {
    use quiet_launcher::home::HomeLayout;
    let one = HomeLayout {
        height: 560.0,
        font_percent: 100,
        favorites: 1,
        widgets: 1,
        weather: true,
        usage: true,
        media: true,
    };
    assert!(one.fits());
    assert!(!HomeLayout {
        favorites: 20,
        ..one
    }
    .fits());
    let small = HomeLayout {
        height: 420.0,
        favorites: 3,
        widgets: 0,
        media: false,
        ..one
    };
    assert!(small.fits());
    assert!(!HomeLayout {
        font_percent: 160,
        ..small
    }
    .fits());
}
