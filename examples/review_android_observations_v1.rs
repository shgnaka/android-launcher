//! Proposed corrected observations. Original frozen tests still run and retain their failures.
#[path = "../tests/support/android.rs"]
mod device;
#[path = "../tests/support/ocr_v1_revision.rs"]
mod observer;
use anyhow::{Context, Result};
use device::{settings, Android};
use std::process::Command;
fn tap_control(lab: &Android, text: &str) -> Result<()> {
    lab.wait_text(text)?;
    let image = lab.image("word-selector.png")?;
    let ocr = std::env::var("QUIET_TESSERACT")
        .unwrap_or_else(|_| "/tmp/quiet-ocr/root/usr/bin/tesseract".into());
    let data = std::env::var("QUIET_TESSDATA")
        .unwrap_or_else(|_| "/tmp/quiet-ocr/root/usr/share/tesseract-ocr/5/tessdata".into());
    let result = Command::new(ocr)
        .arg(&image)
        .arg("stdout")
        .args([
            "--tessdata-dir",
            &data,
            "-l",
            "eng+jpn",
            "--psm",
            "11",
            "tsv",
        ])
        .output()?;
    anyhow::ensure!(result.status.success(), "ENVIRONMENT: OCR failed");
    std::fs::write(lab.out.join("word-selector.tsv"), &result.stdout)?;
    let bounds = observer::control_bounds(&String::from_utf8(result.stdout)?, text)?;
    std::fs::write(
        lab.out.join("selected-control.json"),
        serde_json::to_vec(&bounds)?,
    )?;
    lab.shell(&[
        "input",
        "tap",
        &((bounds[0] + bounds[2]) / 2).to_string(),
        &((bounds[1] + bounds[3]) / 2).to_string(),
    ])?;
    Ok(())
}
fn back() -> Result<()> {
    let lab = Android::new("review-A11", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera")?;
    lab.wait_text("Camera")?;
    lab.back()?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.shell(&["uiautomator", "dump", "/data/local/tmp/quiet-review.xml"])?;
    let xml = String::from_utf8(lab.shell(&["cat", "/data/local/tmp/quiet-review.xml"])?)?;
    std::fs::write(lab.out.join("input-after-back.xml"), &xml)?;
    let node = xml
        .split("<node")
        .filter_map(|n| n.split('>').next())
        .find(|n| n.contains("class=\"android.widget.EditText\""))
        .context("native search is missing after reopening")?;
    anyhow::ensure!(
        node.contains("text=\"\""),
        "search query must be empty after Back: {node}"
    );
    lab.type_text("Camera%sTools")?;
    lab.tap_app("Camera Tools")?;
    lab.dialog_text("FIXTURE CAMERA TOOLS")?;
    lab.foreground("example.camera")
}
fn hide() -> Result<()> {
    let lab = Android::new("review-A16", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.long_press_app("Camera Tools")?;
    tap_control(&lab, "アプリを非表示")?;
    // The visible committed state is the barrier; adb input completion is not save completion.
    lab.wait_text("一致するアプリはありません")?;
    anyhow::ensure!(
        lab.saved()?["hidden_packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "example.camera"),
        "visible hide must already be persisted"
    );
    lab.restart()?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.wait_text("一致するアプリはありません")?;
    Ok(())
}
fn unhide() -> Result<()> {
    let mut s = settings();
    s["hidden_packages"] = serde_json::json!(["example.camera"]);
    let lab = Android::new("review-A19", s)?;
    lab.wait_text("メモ")?;
    lab.settings()?;
    lab.scroll_to("非表示アプリ")?;
    lab.scroll_to("再表示")?;
    tap_control(&lab, "再表示")?;
    lab.back()?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.tap_app("Camera Tools")?;
    lab.dialog_text("FIXTURE CAMERA TOOLS")?;
    anyhow::ensure!(
        lab.saved()?["hidden_packages"]
            .as_array()
            .unwrap()
            .is_empty(),
        "successfully unhidden app must be persisted"
    );
    lab.foreground("example.camera")
}
fn main() -> Result<()> {
    let mut results = vec![];
    for (name, test) in [
        ("A11", back as fn() -> Result<()>),
        ("A16", hide),
        ("A19", unhide),
    ] {
        let result = test();
        println!("{name}: {result:?}");
        results.push(serde_json::json!({"case":name,"passed":result.is_ok(),"error":result.err().map(|e|format!("{e:#}"))}));
    }
    std::fs::create_dir_all("target/v1-observation-review")?;
    std::fs::write(
        "target/v1-observation-review/results.json",
        serde_json::to_vec_pretty(&results)?,
    )?;
    anyhow::ensure!(
        results.iter().all(|r| r["passed"] == true),
        "one or more corrected observations failed"
    );
    Ok(())
}
