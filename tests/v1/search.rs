use crate::support::*;
use quiet_launcher::search::{AppSearch, ReadingSearch};
use std::collections::BTreeMap;
// A14/A39: catches asymmetric transliteration, missing dictionary entries, duplicate loss.
macro_rules! equivalent { ($name:ident,$label:expr,[$($q:expr),+])=>{#[test] fn $name(){for q in [$($q),+]{
    assert_eq!(ids(ReadingSearch::default().search(q,&[app("a/Main",$label),app("b/Main","別名")])),vec!["a/Main"],"{} -> {}",$label,q);
    assert_eq!(ids(ReadingSearch::default().search($label,&[app("a/Main",q),app("b/Main","別名")])),vec!["a/Main"],"reverse {} -> {}",q,$label);
}}};}
equivalent!(
    camera,
    "かめら",
    ["カメラ", "ｶﾒﾗ", "kamera", "camera", " ＫＡＭＥＲＡ "]
);
equivalent!(smartphone, "スマホ", ["すまほ", "sumaho"]);
equivalent!(photo, "しゃしん", ["シャシン", "shashin", "syasin"]);
equivalent!(school, "がっこう", ["ガッコウ", "gakkou", "gakko"]);
equivalent!(coffee, "コーヒー", ["こーひー", "koohii", "kohii"]);
equivalent!(line, "LINE", ["らいん", "ライン", "rain"]);
equivalent!(chrome, "Chrome", ["くろーむ", "クローム", "kuroomu"]);
equivalent!(
    youtube,
    "YouTube",
    ["ゆーちゅーぶ", "ユーチューブ", "yuuchuubu"]
);
equivalent!(gmail, "Gmail", ["じーめーる", "ジーメール", "jiimeeru"]);
equivalent!(google, "Google", ["ぐーぐる", "グーグル", "guuguru"]);
#[test]
fn partial_match_does_not_launch_or_select_a_prefix() {
    for (label, q) in [
        ("カメラ", "kame"),
        ("カメラアプリ", "カメラ"),
        ("Camera Tools", "camera"),
        ("App2", "App3"),
        ("A-B", "AB"),
    ] {
        assert!(
            ReadingSearch::default()
                .search(q, &[app("a/Main", label)])
                .is_empty(),
            "{label}: {q}"
        );
    }
}
#[test]
fn duplicate_names_and_readings_keep_all_identities() {
    assert_eq!(
        ids(ReadingSearch::default().search(
            "kamera",
            &[
                app("a/Main", "Camera"),
                app("b/Main", "カメラ"),
                app("a/Other", "かめら"),
                app("c/Main", "Camera Tools")
            ]
        )),
        vec!["a/Main", "b/Main", "a/Other"]
    );
}
#[test]
fn empty_and_blank_queries_preserve_all_identities() {
    for q in ["", " ", "　\t\n"] {
        assert_eq!(
            ids(ReadingSearch::default().search(q, &apps())),
            vec![
                "example.camera/Main",
                "example.camera/Tools",
                "example.other/Main",
                "example.notes/Main"
            ]
        );
    }
}
#[test]
fn custom_reading_adds_a_key_without_removing_original_name() {
    let e =
        ReadingSearch::with_readings(BTreeMap::from([("custom/Main".into(), "ほげほげ".into())]));
    for q in ["UnknownBrand", "ホゲホゲ", "hogehoge"] {
        assert_eq!(
            ids(e.search(
                q,
                &[
                    app("custom/Main", "UnknownBrand"),
                    app("other/Main", "別名")
                ]
            )),
            vec!["custom/Main"]
        );
    }
}
#[test]
fn removing_custom_reading_keeps_literal_name_but_not_the_old_alias() {
    let a = [app("custom/Main", "UnknownBrand")];
    let e = ReadingSearch::default();
    assert!(e.search("ほげほげ", &a).is_empty());
    assert_eq!(ids(e.search("UnknownBrand", &a)), vec!["custom/Main"]);
}
#[test]
fn changing_labels_with_equal_list_length_invalidates_old_readings() {
    let e = ReadingSearch::default();
    assert_eq!(e.search("kamera", &[app("a/Main", "カメラ")]).len(), 1);
    assert!(e.search("kamera", &[app("a/Main", "メモ")]).is_empty());
    assert_eq!(
        ids(e.search("memo", &[app("a/Main", "メモ")])),
        vec!["a/Main"]
    );
}
#[test]
fn swapping_positions_does_not_reuse_another_apps_reading() {
    let e = ReadingSearch::default();
    e.search("kamera", &[app("a/Main", "カメラ"), app("b/Main", "メモ")]);
    assert_eq!(
        ids(e.search("kamera", &[app("b/Main", "メモ"), app("a/Main", "カメラ")])),
        vec!["a/Main"]
    );
}
#[test]
fn long_query_returns_no_invented_match() {
    assert!(ReadingSearch::default()
        .search(&"あ".repeat(1024), &apps())
        .is_empty());
}
