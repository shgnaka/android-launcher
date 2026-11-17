//! Generate a tiny native callback bridge from Rust, convert to DEX, build Rust activities.
//! All fixture behavior lives in tests/android-fixture/src/lib.rs; no Java/Kotlin sources.
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
struct Pool(Vec<Vec<u8>>);
impl Pool {
    fn add(&mut self, x: Vec<u8>) -> u16 {
        self.0.push(x);
        self.0.len() as u16
    }
    fn utf(&mut self, x: &str) -> u16 {
        let mut b = vec![1];
        b.extend((x.len() as u16).to_be_bytes());
        b.extend(x.as_bytes());
        self.add(b)
    }
    fn class(&mut self, x: &str) -> u16 {
        let name = self.utf(x);
        let mut b = vec![7];
        b.extend(name.to_be_bytes());
        self.add(b)
    }
    fn method(&mut self, class: u16, name: u16, descriptor: u16) -> u16 {
        let mut b = vec![12];
        b.extend(name.to_be_bytes());
        b.extend(descriptor.to_be_bytes());
        let nt = self.add(b);
        let mut b = vec![10];
        b.extend(class.to_be_bytes());
        b.extend(nt.to_be_bytes());
        self.add(b)
    }
}
fn u16s(b: &mut Vec<u8>, v: &[u16]) {
    for x in v {
        b.extend(x.to_be_bytes())
    }
}
fn bridge() -> Vec<u8> {
    let mut p = Pool(vec![]);
    let this = p.class("dev/quiet/fixture/MediaCallbacks");
    let parent = p.class("android/media/session/MediaSession$Callback");
    let init = p.utf("<init>");
    let void = p.utf("()V");
    let super_init = p.method(parent, init, void);
    let code = p.utf("Code");
    let methods = ["onPlay", "onPause", "onSkipToNext", "onSkipToPrevious"].map(|s| p.utf(s));
    let mut b = vec![0xca, 0xfe, 0xba, 0xbe];
    u16s(&mut b, &[0, 52, p.0.len() as u16 + 1]);
    for entry in p.0 {
        b.extend(entry)
    }
    u16s(&mut b, &[0x21, this, parent, 0, 0, 5]);
    u16s(&mut b, &[1, init, void, 1, code]);
    b.extend(17u32.to_be_bytes());
    u16s(&mut b, &[1, 1]);
    b.extend(5u32.to_be_bytes());
    b.extend([0x2a, 0xb7, (super_init >> 8) as u8, super_init as u8, 0xb1]);
    u16s(&mut b, &[0, 0]);
    for name in methods {
        u16s(&mut b, &[0x101, name, void, 0])
    }
    u16s(&mut b, &[0]);
    b
}
fn run(c: &mut Command) -> Result<()> {
    let out = c.output()?;
    anyhow::ensure!(
        out.status.success(),
        "{c:?}: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}
fn main() -> Result<()> {
    let sdk = PathBuf::from(std::env::var("ANDROID_HOME").context("ANDROID_HOME")?);
    let b = sdk.join("build-tools/35.0.0");
    let out = std::env::current_dir()?.join("target/acceptance-fixtures");
    std::fs::create_dir_all(out.join("dex"))?;
    let class = out.join("MediaCallbacks.class");
    std::fs::write(&class, bridge())?;
    run(Command::new(b.join("d8"))
        .args(["--min-api", "28", "--lib"])
        .arg(sdk.join("platforms/android-35/android.jar"))
        .arg("--output")
        .arg(out.join("dex"))
        .arg(class))?;
    run(Command::new("cargo")
        .args([
            "apk",
            "build",
            "--release",
            "--lib",
            "--target",
            "x86_64-linux-android",
            "--manifest-path",
            "tests/android-fixture/Cargo.toml",
            "--target-dir",
        ])
        .arg(out.join("native")))?;
    let so = out.join("native/x86_64-linux-android/release/libquiet_acceptance_fixture.so");
    anyhow::ensure!(so.is_file(), "native library missing");
    for (package, aliases) in [
        (
            "example.camera",
            vec![("Main", "Camera"), ("Tools", "Camera Tools")],
        ),
        ("example.other", vec![("Main", "Camera")]),
        ("example.notes", vec![("Main", "メモ")]),
        ("example.player1", vec![("Main", "PLAYER ONE")]),
        ("example.player2", vec![("Main", "PLAYER TWO")]),
    ] {
        let dir = out.join(package);
        std::fs::create_dir_all(dir.join("lib/x86_64"))?;
        std::fs::copy(&so, dir.join("lib/x86_64/libmain.so"))?;
        std::fs::copy(out.join("dex/classes.dex"), dir.join("classes.dex"))?;
        let aliases=aliases.iter().map(|(name,label)|format!(r#"<activity-alias android:name="{package}.{name}" android:label="{label}" android:targetActivity="android.app.NativeActivity" android:exported="true"><intent-filter><action android:name="android.intent.action.MAIN"/><category android:name="android.intent.category.LAUNCHER"/></intent-filter></activity-alias>"#)).collect::<String>();
        let manifest = format!(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{package}" android:versionCode="1" android:versionName="1"><uses-sdk android:minSdkVersion="28" android:targetSdkVersion="35"/><application android:label="{package}" android:debuggable="true" android:extractNativeLibs="true" android:allowBackup="false"><activity android:name="android.app.NativeActivity" android:exported="true"><meta-data android:name="android.app.lib_name" android:value="main"/></activity>{aliases}</application></manifest>"#
        );
        std::fs::write(dir.join("AndroidManifest.xml"), manifest)?;
        run(Command::new(b.join("aapt"))
            .args(["package", "-f", "-M"])
            .arg(dir.join("AndroidManifest.xml"))
            .arg("-I")
            .arg(sdk.join("platforms/android-35/android.jar"))
            .arg("-F")
            .arg(dir.join("unsigned.apk")))?;
        run(Command::new(b.join("aapt")).current_dir(&dir).args([
            "add",
            "unsigned.apk",
            "classes.dex",
            "lib/x86_64/libmain.so",
        ]))?;
        run(Command::new(b.join("zipalign"))
            .args(["-f", "-p", "4"])
            .arg(dir.join("unsigned.apk"))
            .arg(dir.join("aligned.apk")))?;
        run(Command::new(b.join("apksigner"))
            .args(["sign", "--ks"])
            .arg(
                std::env::var("CARGO_APK_RELEASE_KEYSTORE")
                    .unwrap_or_else(|_| ".local/android/debug.keystore".into()),
            )
            .args(["--ks-pass", "pass:android", "--out"])
            .arg(out.join(format!("{package}.apk")))
            .arg(dir.join("aligned.apk")))?;
        println!("{}", out.join(format!("{package}.apk")).display());
    }
    build_widgets(&sdk, &out)?;
    let hashes = Command::new("sha256sum")
        .args([
            "tests/android-fixture/src/lib.rs",
            "tests/android-fixture/Cargo.toml",
            "tests/android-fixture/Cargo.lock",
            "examples/build_acceptance_fixtures.rs",
        ])
        .args([
            "tests/fixtures/v1/widgets/description.xml",
            "tests/fixtures/v1/widgets/image.xml",
            "tests/fixtures/v1/widgets/hidden.xml",
            "tests/fixtures/v1/widgets/duplicates.xml",
            "tests/fixtures/v1/widgets/configuration.xml",
        ])
        .output()?;
    anyhow::ensure!(hashes.status.success(), "fixture provenance failed");
    std::fs::write(out.join("build-inputs.sha256"), hashes.stdout)?;
    let _ = Path::new("tests/android-fixture");
    Ok(())
}
fn build_widgets(sdk: &Path, out: &Path) -> Result<()> {
    let b = sdk.join("build-tools/35.0.0");
    for (name, label) in [
        ("description", "WIDGET DESC"),
        ("image", "WIDGET IMAGE"),
        ("hidden", "WIDGET HIDDEN"),
        ("duplicates", "WIDGET DUP"),
        ("configuration", "WIDGET CONFIG"),
    ] {
        let package = format!("example.widget.{name}");
        let dir = out.join(&package);
        std::fs::create_dir_all(dir.join("res/layout"))?;
        std::fs::create_dir_all(dir.join("res/xml"))?;
        std::fs::copy(
            format!("tests/fixtures/v1/widgets/{name}.xml"),
            dir.join("res/layout/content.xml"),
        )?;
        let configuration = if name == "configuration" {
            r#" android:configure="android.app.Activity""#
        } else {
            ""
        };
        let xml=format!(r#"<appwidget-provider xmlns:android="http://schemas.android.com/apk/res/android" android:minWidth="180dp" android:minHeight="48dp" android:initialLayout="@layout/content" android:updatePeriodMillis="0" android:widgetCategory="home_screen"{configuration}/ >"#).replace("/ >","/>");
        std::fs::write(dir.join("res/xml/provider.xml"), xml)?;
        let activity = if name == "configuration" {
            r#"<activity android:name="android.app.Activity" android:label="WIDGET CONFIGURATION" android:exported="true"/>"#
        } else {
            ""
        };
        let manifest = format!(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{package}" android:versionCode="1" android:versionName="1"><uses-sdk android:minSdkVersion="28" android:targetSdkVersion="35"/><application android:hasCode="false" android:allowBackup="false" android:label="{label}">{activity}<receiver android:name="android.appwidget.AppWidgetProvider" android:label="{label}" android:exported="true"><intent-filter><action android:name="android.appwidget.action.APPWIDGET_UPDATE"/></intent-filter><meta-data android:name="android.appwidget.provider" android:resource="@xml/provider"/></receiver></application></manifest>"#
        );
        std::fs::write(dir.join("AndroidManifest.xml"), manifest)?;
        run(Command::new(b.join("aapt"))
            .args(["package", "-f", "-M"])
            .arg(dir.join("AndroidManifest.xml"))
            .arg("-S")
            .arg(dir.join("res"))
            .arg("-I")
            .arg(sdk.join("platforms/android-35/android.jar"))
            .arg("-F")
            .arg(dir.join("unsigned.apk")))?;
        run(Command::new(b.join("zipalign"))
            .args(["-f", "4"])
            .arg(dir.join("unsigned.apk"))
            .arg(dir.join("aligned.apk")))?;
        run(Command::new(b.join("apksigner"))
            .args(["sign", "--ks"])
            .arg(
                std::env::var("CARGO_APK_RELEASE_KEYSTORE")
                    .unwrap_or_else(|_| ".local/android/debug.keystore".into()),
            )
            .args(["--ks-pass", "pass:android", "--out"])
            .arg(out.join(format!("{package}.apk")))
            .arg(dir.join("aligned.apk")))?;
        println!("{}", out.join(format!("{package}.apk")).display());
    }
    Ok(())
}
