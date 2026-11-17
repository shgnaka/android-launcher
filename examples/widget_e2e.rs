//! Provisional widget experiment: actual UI actions + OCR of the rendered screen.
//! Only emulator serials are accepted. Original Quiet settings are restored on completion/failure.
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};
struct Lab {
    adb: PathBuf,
    serial: String,
    ocr: PathBuf,
    data: PathBuf,
    out: PathBuf,
}
impl Lab {
    fn run(&self, args: &[&str]) -> Result<Vec<u8>> {
        let out = Command::new(&self.adb)
            .args(["-s", &self.serial])
            .args(args)
            .output()?;
        anyhow::ensure!(
            out.status.success(),
            "adb {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out.stdout)
    }
    fn shell(&self, args: &[&str]) -> Result<Vec<u8>> {
        let mut cmd = vec!["shell"];
        cmd.extend(args);
        self.run(&cmd)
    }
    fn shot(&self, name: &str) -> Result<PathBuf> {
        let path = self.out.join(name);
        let bytes = self.run(&["exec-out", "screencap", "-p"])?;
        anyhow::ensure!(bytes.starts_with(b"\x89PNG"), "not a screenshot");
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
    fn lines(&self, image: &Path) -> Result<Vec<(String, [i32; 4])>> {
        let result = Command::new(&self.ocr)
            .arg(image)
            .arg("stdout")
            .args(["--tessdata-dir"])
            .arg(&self.data)
            .args(["-l", "eng+jpn", "--psm", "11", "tsv"])
            .output()?;
        anyhow::ensure!(
            result.status.success(),
            "OCR failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut lines: BTreeMap<String, (String, [i32; 4])> = BTreeMap::new();
        for line in String::from_utf8(result.stdout)?.lines().skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 12 || c[0] != "5" || c[11].trim().is_empty() {
                continue;
            }
            let key = format!("{}:{}:{}", c[2], c[3], c[4]);
            let x = c[6].parse::<i32>()?;
            let y = c[7].parse::<i32>()?;
            let w = c[8].parse::<i32>()?;
            let h = c[9].parse::<i32>()?;
            let item = lines
                .entry(key)
                .or_insert((String::new(), [x, y, x + w, y + h]));
            item.0.push_str(c[11]);
            item.1 = [
                item.1[0].min(x),
                item.1[1].min(y),
                item.1[2].max(x + w),
                item.1[3].max(y + h),
            ];
        }
        Ok(lines.into_values().collect())
    }
    fn tap_text(&self, text: &str) -> Result<()> {
        for step in 0..10 {
            let shot = self.shot(&format!("scan-{step}.png"))?;
            let lines = self.lines(&shot)?;
            if let Some((_, b)) = lines
                .iter()
                .find(|(line, _)| normalize(line).contains(&normalize(text)))
            {
                self.shell(&[
                    "input",
                    "tap",
                    &((b[0] + b[2]) / 2).to_string(),
                    &((b[1] + b[3]) / 2).to_string(),
                ])?;
                thread::sleep(Duration::from_secs(1));
                return Ok(());
            }
            self.shell(&["input", "swipe", "360", "1030", "360", "330", "400"])?;
            thread::sleep(Duration::from_secs(1));
        }
        anyhow::bail!("could not find UI text: {text}")
    }
    fn verify(&self, name: &str, visible: bool) -> Result<()> {
        let screenshot = self.shot(name)?;
        let lines = self.lines(&screenshot)?;
        let all = normalize(
            &lines
                .iter()
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
        std::fs::write(self.out.join(format!("{name}.txt")), &all)?;
        for expected in ["TEST PROVIDER", "QUIET WIDGET READY"] {
            anyhow::ensure!(
                all.contains(&normalize(expected)) == visible,
                "{name}: expected visible={visible}: {expected}; OCR={all}"
            );
        }
        Ok(())
    }
    fn start(&self) -> Result<()> {
        self.shell(&[
            "am",
            "start",
            "-W",
            "-n",
            "dev.quiet.launcher/android.app.NativeActivity",
        ])?;
        thread::sleep(Duration::from_secs(4));
        Ok(())
    }
    fn settings(&self) -> Result<()> {
        self.shell(&["input", "swipe", "620", "1000", "620", "1000", "1000"])?;
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }
    fn put_settings(&self, bytes: &[u8], owner: &str) -> Result<()> {
        let path = self.out.join("settings-input.json");
        std::fs::write(&path, bytes)?;
        self.run(&[
            "push",
            path.to_str().unwrap(),
            "/data/local/tmp/quiet-fixture-settings.json",
        ])?;
        let dest = "/data/user/0/dev.quiet.launcher/files/settings.json";
        self.shell(&["cp", "/data/local/tmp/quiet-fixture-settings.json", dest])?;
        self.shell(&["chown", owner, dest])?;
        self.shell(&["chmod", "600", dest])?;
        Ok(())
    }
}
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}
fn main() -> Result<()> {
    let mut lab = Lab {
        adb: "/tmp/android-launcher-sdk/platform-tools/adb".into(),
        serial: "emulator-5554".into(),
        ocr: "/tmp/quiet-ocr/root/usr/bin/tesseract".into(),
        data: "/tmp/quiet-ocr/root/usr/share/tesseract-ocr/5/tessdata".into(),
        out: "target/widget-e2e".into(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        let value = args.next().context("option needs a value")?;
        match key.as_str() {
            "--adb" => lab.adb = value.into(),
            "--serial" => lab.serial = value,
            "--ocr" => lab.ocr = value.into(),
            "--tessdata" => lab.data = value.into(),
            "--out" => lab.out = value.into(),
            _ => anyhow::bail!("unknown option {key}"),
        }
    }
    anyhow::ensure!(
        lab.serial.starts_with("emulator-"),
        "only emulators are accepted"
    );
    std::fs::create_dir_all(&lab.out)?;
    lab.run(&["root"])?;
    lab.run(&["wait-for-device"])?;
    lab.run(&[
        "install",
        "-r",
        "--no-streaming",
        "target/widget-fixture/fixture.apk",
    ])?;
    let original = lab.shell(&["cat", "/data/user/0/dev.quiet.launcher/files/settings.json"])?;
    let owner = String::from_utf8(lab.shell(&[
        "stat",
        "-c",
        "%u",
        "/data/user/0/dev.quiet.launcher/files/settings.json",
    ])?)?
    .trim()
    .to_owned();
    let experiment = (|| -> Result<()> {
        lab.shell(&["am", "force-stop", "dev.quiet.launcher"])?;
        let mut settings = quiet_launcher::config::Settings::default();
        settings.show_media = false;
        settings.show_usage = false;
        settings.show_weather = false;
        lab.put_settings(&serde_json::to_vec(&settings)?, &owner)?;
        lab.shell(&["appwidget", "grantbind", "--package", "dev.quiet.launcher"])?;
        lab.start()?;
        lab.settings()?;
        lab.tap_text("ウィジェットを選ぶ")?;
        lab.tap_text("TEST PROVIDER")?;
        lab.shell(&["input", "keyevent", "4"])?;
        thread::sleep(Duration::from_secs(4));
        lab.verify("01-added.png", true)?;
        println!("PASS widget provider and text are actually rendered");
        lab.shell(&["am", "force-stop", "dev.quiet.launcher"])?;
        lab.start()?;
        lab.verify("02-restarted.png", true)?;
        println!("PASS widget survives process restart");
        lab.settings()?;
        lab.tap_text("削除")?;
        lab.shell(&["input", "keyevent", "4"])?;
        thread::sleep(Duration::from_secs(3));
        lab.verify("03-deleted.png", false)?;
        println!("PASS deletion removes rendered widget");
        std::fs::write(
            lab.out.join("summary.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"status":"passed","provisional_acceptance":true,"provider_name":true,"actual_text":true,"restart":true,"delete":true,"method":"real UI actions and OCR of screenshots","fixture":"framework resource-only AppWidgetProvider"}),
            )?,
        )?;
        Ok(())
    })();
    lab.shell(&["am", "force-stop", "dev.quiet.launcher"])?;
    lab.put_settings(&original, &owner)?;
    lab.start()?;
    experiment
}
