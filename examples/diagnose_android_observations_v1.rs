//! Diagnostic only: preserve frozen failing cases and record independent observations.
#[path = "../tests/support/android.rs"]
mod device;
use anyhow::Result;
use device::{settings, Android};
use std::time::{Duration, Instant};
fn observe(lab: &Android, hidden: bool) -> Result<()> {
    let start = Instant::now();
    let mut rows = vec![];
    for _ in 0..20 {
        let saved = lab.saved()?;
        let actual = saved["hidden_packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "example.camera");
        rows.push(serde_json::json!({"elapsed_ms":start.elapsed().as_millis(),"hidden":actual,"settings":saved}));
        if actual == hidden {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::fs::write(
        lab.out.join("save-timeline.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    lab.image("after-operation.png")?;
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap();
    match mode.as_str() {
        "hide" => {
            let lab = Android::new("diagnostic-hide-commit", settings())?;
            lab.wait_text("メモ")?;
            lab.drawer()?;
            lab.type_text("Camera%sTools")?;
            lab.long_press_app("Camera Tools")?;
            lab.tap_text("アプリを非表示")?;
            observe(&lab, true)?;
        }
        "unhide" => {
            let mut s = settings();
            s["hidden_packages"] = serde_json::json!(["example.camera"]);
            let lab = Android::new("diagnostic-unhide-commit", s)?;
            lab.wait_text("メモ")?;
            lab.settings()?;
            lab.scroll_to("非表示アプリ")?;
            lab.scroll_to("再表示")?;
            lab.tap_text("再表示")?;
            observe(&lab, false)?;
        }
        "back" => {
            let lab = Android::new("diagnostic-ime-clear", settings())?;
            lab.wait_text("メモ")?;
            lab.drawer()?;
            lab.type_text("Camera")?;
            lab.wait_text("Camera")?;
            lab.back()?;
            lab.wait_text("メモ")?;
            lab.drawer()?;
            lab.shell(&[
                "uiautomator",
                "dump",
                "/data/local/tmp/quiet-v1-after-back.xml",
            ])?;
            let xml =
                String::from_utf8(lab.shell(&["cat", "/data/local/tmp/quiet-v1-after-back.xml"])?)?;
            std::fs::write(lab.out.join("input-after-back.xml"), &xml)?;
            for node in xml
                .split("<node")
                .filter(|n| n.contains("class=\"android.widget.EditText\""))
            {
                println!("{}", node.split('>').next().unwrap());
            }
            lab.image("reopened.png")?;
        }
        _ => anyhow::bail!("expected hide/unhide/back"),
    }
    Ok(())
}
