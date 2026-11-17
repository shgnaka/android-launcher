//! Validate the built artifact, not source strings or a mocked package manager.
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn apk() -> PathBuf {
    std::env::var_os("QUIET_PHONE_APK")
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/release/apk/quiet-launcher.apk".into())
}
fn run(c: &mut Command) -> String {
    let o = c
        .output()
        .unwrap_or_else(|e| panic!("ENVIRONMENT: {c:?}: {e}"));
    assert!(
        o.status.success(),
        "{c:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap()
}
fn tool(name: &str) -> PathBuf {
    let sdk = std::env::var_os("ANDROID_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/tmp/android-launcher-sdk".into());
    sdk.join("build-tools/35.0.0").join(name)
}
fn badging(p: &Path) -> String {
    run(Command::new(tool("aapt")).args(["dump", "badging"]).arg(p))
}
fn signature(p: &Path) -> String {
    let text = run(Command::new(tool("apksigner"))
        .args(["verify", "--print-certs"])
        .arg(p));
    text.lines()
        .find(|l| l.starts_with("Signer #1 certificate SHA-256 digest:"))
        .expect("signer digest")
        .to_owned()
}
#[test]
fn a01_release_artifact_declares_the_required_android_abi_and_minimum_api() {
    let text = badging(&apk());
    assert!(text.contains("package: name='dev.quiet.launcher'"));
    assert!(text.contains("sdkVersion:'28'"));
    assert!(text.contains("targetSdkVersion:'35'"));
    assert!(text.contains("native-code: 'arm64-v8a'"));
}
#[test]
fn a04_update_artifact_uses_the_same_certificate_as_the_installed_old_release() {
    assert_eq!(
        signature(&apk()),
        signature(Path::new("dist/quiet-launcher-0.3.0.apk"))
    );
}
#[test]
fn a04_download_archive_contains_exactly_the_installable_apk() {
    let archive = std::env::var_os("QUIET_DOWNLOAD_ZIP")
        .map(PathBuf::from)
        .unwrap_or_else(|| "dist/quiet-launcher-download.zip".into());
    let names = run(Command::new("unzip").args(["-Z1"]).arg(&archive));
    assert_eq!(
        names.lines().collect::<Vec<_>>(),
        vec!["quiet-launcher.apk"]
    );
    let o = Command::new("unzip")
        .args(["-p"])
        .arg(archive)
        .arg("quiet-launcher.apk")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(o.stdout, std::fs::read(apk()).unwrap());
}
#[test]
fn a52_android_automatic_backup_is_disabled_in_the_binary_manifest() {
    let text = run(Command::new(tool("aapt"))
        .args(["dump", "xmltree"])
        .arg(apk())
        .arg("AndroidManifest.xml"));
    let backup = text.lines().find(|l| l.contains("android:allowBackup("));
    assert!(
        backup.is_some_and(|l| l.contains("0x0")),
        "binary manifest must disable automatic backup; found {backup:?}"
    );
}
#[test]
fn a01_installed_package_has_a_home_entry_and_launcher_visibility() {
    let text = run(Command::new(tool("aapt"))
        .args(["dump", "xmltree"])
        .arg(apk())
        .arg("AndroidManifest.xml"));
    for category in [
        "android.intent.category.HOME",
        "android.intent.category.DEFAULT",
        "android.intent.category.LAUNCHER",
        "android.appwidget.action.APPWIDGET_UPDATE",
    ] {
        assert!(
            text.contains(category),
            "missing actual package declaration: {category}"
        );
    }
}
#[test]
fn a52_declared_permissions_do_not_add_location_tracking_or_usage_transmission_permissions() {
    let text = badging(&apk());
    for p in [
        "android.permission.ACCESS_FINE_LOCATION",
        "android.permission.ACCESS_COARSE_LOCATION",
        "android.permission.READ_CONTACTS",
        "android.permission.READ_SMS",
        "android.permission.SEND_SMS",
    ] {
        assert!(!text.contains(p), "unexpected permission {p}");
    }
}
