use crate::support::*;
use quiet_launcher::catalog::AppCatalog;
#[test]
fn unchanged_generation_survives_disk_reload_without_enumeration() {
    let d = Directory::new("catalog");
    let p = d.0.join("apps.json");
    let mut c = AppCatalog::default();
    c.refresh("boot1:ja:1", || Ok(apps())).unwrap();
    c.save(&p).unwrap();
    let mut r = AppCatalog::load(&p).unwrap();
    assert!(!r
        .refresh("boot1:ja:1", || panic!(
            "same generation must not enumerate"
        ))
        .unwrap());
    assert_eq!(r.apps()[0].id, "example.camera/Main");
}
#[test]
fn language_boot_and_package_generations_each_replace_stale_names() {
    let mut c = AppCatalog::default();
    for revision in ["boot1:ja:1", "boot1:en:1", "boot2:en:1", "boot2:en:2"] {
        assert!(c
            .refresh(revision, || Ok(vec![app("a/Main", revision)]))
            .unwrap());
        assert_eq!(c.apps()[0].label, revision);
    }
}
#[test]
fn failed_source_retains_data_and_does_not_commit_new_generation() {
    let mut c = AppCatalog::default();
    c.refresh("1", || Ok(vec![app("a/Main", "OLD")])).unwrap();
    assert!(c.refresh("2", || anyhow::bail!("unavailable")).is_err());
    assert_eq!(c.apps()[0].label, "OLD");
    assert!(c.refresh("2", || Ok(vec![app("a/Main", "NEW")])).unwrap());
    assert_eq!(c.apps()[0].label, "NEW");
}
#[test]
fn corrupt_cache_does_not_destroy_the_independent_settings() {
    let d = Directory::new("corrupt-cache");
    d.write(br#"{"hidden_packages":["secret"]}"#);
    let p = d.0.join("apps.json");
    std::fs::write(&p, b"corrupt cache").unwrap();
    assert!(AppCatalog::load(&p).is_err());
    assert_eq!(
        std::fs::read(d.settings()).unwrap(),
        br#"{"hidden_packages":["secret"]}"#
    );
}
#[test]
fn explicit_rebuild_ignores_unchanged_revision() {
    let mut c = AppCatalog::default();
    c.refresh("1", || Ok(vec![app("a/Main", "OLD")])).unwrap();
    c.invalidate();
    assert!(c.refresh("1", || Ok(vec![app("a/Main", "NEW")])).unwrap());
    assert_eq!(c.apps()[0].label, "NEW");
}
