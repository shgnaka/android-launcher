use quiet_launcher::{config::Settings, home::HomeLayout};
// A18/A37: hand-calculated heights and ordering, independent of implementation helpers.
#[test]
fn height_boundaries_are_inclusive_without_silent_truncation() {
    for (font, n, w, weather, usage, media, h) in [
        (100, 0, 0, false, false, false, 192.0),
        (100, 4, 0, false, false, false, 368.0),
        (100, 1, 1, true, true, true, 380.0),
        (80, 1, 1, true, true, true, 332.0),
        (160, 1, 1, true, true, true, 527.2),
        (100, 0, 3, false, false, false, 336.0),
    ] {
        let x = HomeLayout {
            height: h,
            font_percent: font,
            favorites: n,
            widgets: w,
            weather,
            usage,
            media,
        };
        assert!(
            !HomeLayout {
                height: h - 1.0,
                ..x
            }
            .fits(),
            "{font}%, {n} apps"
        );
        assert!(x.fits(), "{font}%, {n} apps at equality");
        assert!(HomeLayout {
            height: h + 1.0,
            ..x
        }
        .fits());
    }
}
fn settings() -> Settings {
    Settings {
        favorites: ["a/Main", "b/Main", "c/Main", "d/Main"]
            .map(String::from)
            .to_vec(),
        ..Settings::default()
    }
}
#[test]
fn hiding_one_activity_hides_the_entire_package_not_same_named_neighbors() {
    let mut s = settings();
    s.set_hidden("b/Main", true);
    assert!(s.is_hidden("b/Tools"));
    assert!(!s.is_hidden("bb/Main"));
    assert_eq!(s.visible_favorites(), vec!["a/Main", "c/Main", "d/Main"]);
    assert_eq!(s.favorites, vec!["a/Main", "b/Main", "c/Main", "d/Main"]);
}
#[test]
fn reorder_of_visible_items_preserves_hidden_slots() {
    let mut s = settings();
    s.set_hidden("b", true);
    s.set_hidden("d", true);
    s.move_visible_favorite("c/Main", -1);
    s.set_hidden("b", false);
    s.set_hidden("d", false);
    assert_eq!(s.favorites, vec!["c/Main", "b/Main", "a/Main", "d/Main"]);
}
#[test]
fn different_unhide_orders_have_the_same_final_favorites() {
    let mut outputs = vec![];
    for order in [["b", "d"], ["d", "b"]] {
        let mut s = settings();
        s.set_hidden("b", true);
        s.set_hidden("d", true);
        s.move_visible_favorite("c/Main", -1);
        for p in order {
            s.set_hidden(p, false)
        }
        outputs.push(s.favorites);
    }
    assert_eq!(outputs[0], vec!["c/Main", "b/Main", "a/Main", "d/Main"]);
    assert_eq!(outputs[1], vec!["c/Main", "b/Main", "a/Main", "d/Main"]);
}
#[test]
fn removing_visible_neighbor_does_not_remove_hidden_registration() {
    let mut s = settings();
    s.set_hidden("b", true);
    s.toggle_favorite("a/Main");
    s.set_hidden("b", false);
    assert_eq!(s.favorites, vec!["b/Main", "c/Main", "d/Main"]);
}
#[test]
fn new_registration_is_appended_after_hidden_positions() {
    let mut s = settings();
    s.set_hidden("b", true);
    s.toggle_favorite("e/Main");
    s.set_hidden("b", false);
    assert_eq!(
        s.favorites,
        vec!["a/Main", "b/Main", "c/Main", "d/Main", "e/Main"]
    );
}
#[test]
fn repeated_hide_unhide_does_not_duplicate_or_remove_favorites() {
    let mut s = settings();
    for _ in 0..100 {
        s.set_hidden("b", true);
        s.set_hidden("b/Tools", true);
        assert_eq!(s.hidden_packages, vec!["b"]);
        s.set_hidden("b", false);
    }
    assert_eq!(s.favorites, vec!["a/Main", "b/Main", "c/Main", "d/Main"]);
}
