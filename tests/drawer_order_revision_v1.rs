//! Separate revision for the stale positional assertion in the frozen legacy module at
//! `src/ui.rs` (`電話` at y < 200). The v1 contract requires a stable NFKC-lowercased
//! label order, which puts `電話` sixth for the desktop preview catalog. The original
//! test, its expectation and its last result are preserved unchanged; this revision
//! checks the original intent (inline search, visible app rows, no navigation headings)
//! against the correct order instead.
#![cfg(feature = "ui")]
mod support;

use serde_json::json;
use support::{app, screen::Screen};

fn desktop_apps() -> Vec<quiet_launcher::model::AppEntry> {
    vec![
        app("preview/phone", "電話"),
        app("preview/messages", "メッセージ"),
        app("preview/camera", "カメラ"),
        app("preview/calendar", "カレンダー"),
        app("preview/settings", "設定"),
        app("preview/browser", "Browser"),
    ]
}

#[test]
fn drawer_keeps_inline_search_and_visible_rows_in_normalized_order() {
    let mut s = Screen::new(json!({}), desktop_apps());
    s.drawer();

    let labels = s.labels();
    assert!(
        labels.iter().any(|text| text == "電話"),
        "the app row must be visible: {labels:?}"
    );
    let order: Vec<_> = labels
        .iter()
        .filter(|text| {
            matches!(
                text.as_str(),
                "Browser" | "カメラ" | "カレンダー" | "メッセージ" | "設定" | "電話"
            )
        })
        .cloned()
        .collect();
    assert_eq!(
        order,
        vec![
            "Browser",
            "カメラ",
            "カレンダー",
            "メッセージ",
            "設定",
            "電話"
        ],
        "drawer order is defined by the normalized label key"
    );
    assert!(
        !labels
            .iter()
            .any(|text| matches!(text.as_str(), "ホーム" | "アプリ" | "↑ アプリ")),
        "the drawer must not show navigation headings: {labels:?}"
    );

    s.search("電話");
    assert!(s.has("電話"), "inline search must still find the app");
    assert!(!s.has("カメラ"), "unmatched rows must be filtered out");
}
