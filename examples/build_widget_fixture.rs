//! A resource-only widget provider, using the Android framework's concrete provider class.
use anyhow::{Context, Result};
use std::{path::PathBuf, process::Command};
fn run(command: &mut Command) -> Result<()> {
    let out = command.output()?;
    anyhow::ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}
fn main() -> Result<()> {
    let sdk = PathBuf::from(std::env::var("ANDROID_HOME").context("ANDROID_HOME")?);
    let build = sdk.join("build-tools/35.0.0");
    let out = PathBuf::from("target/widget-fixture");
    std::fs::create_dir_all(&out)?;
    let unsigned = out.join("unsigned.apk");
    let aligned = out.join("aligned.apk");
    let apk = out.join("fixture.apk");
    run(Command::new(build.join("aapt"))
        .args([
            "package",
            "-f",
            "-M",
            "tests/fixtures/widget/AndroidManifest.xml",
            "-S",
            "tests/fixtures/widget/res",
            "-I",
        ])
        .arg(sdk.join("platforms/android-35/android.jar"))
        .arg("-F")
        .arg(&unsigned))?;
    run(Command::new(build.join("zipalign"))
        .args(["-f", "4"])
        .arg(unsigned)
        .arg(&aligned))?;
    run(Command::new(build.join("apksigner"))
        .args(["sign", "--ks"])
        .arg(
            std::env::var("CARGO_APK_RELEASE_KEYSTORE")
                .unwrap_or_else(|_| ".local/android/debug.keystore".into()),
        )
        .args(["--ks-pass", "pass:android", "--out"])
        .arg(&apk)
        .arg(aligned))?;
    println!("{}", apk.display());
    Ok(())
}
