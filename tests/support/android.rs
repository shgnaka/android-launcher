//! Black-box emulator harness. Every case restores private settings and test permissions.
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};
static DEVICE: Mutex<()> = Mutex::new(());
const PACKAGE: &str = "dev.quiet.launcher";
const SETTINGS: &str = "/data/user/0/dev.quiet.launcher/files/settings.json";
const FIXTURES: [&str; 5] = [
    "example.camera",
    "example.other",
    "example.notes",
    "example.player1",
    "example.player2",
];
pub struct Android {
    _guard: MutexGuard<'static, ()>,
    pub adb: PathBuf,
    pub serial: String,
    pub out: PathBuf,
    original: Option<Vec<u8>>,
    backup: Option<Vec<u8>>,
    owner: String,
    pub listener: Option<String>,
    widget_packages: Mutex<Vec<String>>,
    bind_was_allowed: bool,
    pub width: i32,
    pub height: i32,
}
fn output(c: &mut Command) -> Result<Vec<u8>> {
    let mut child = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let a = std::thread::spawn(move || {
        let mut b = vec![];
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let e = std::thread::spawn(move || {
        let mut b = vec![];
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("ENVIRONMENT: command timed out: {c:?}")
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let bytes = a.join().unwrap()?;
    let errors = e.join().unwrap()?;
    anyhow::ensure!(
        status.success(),
        "{c:?}: {}",
        String::from_utf8_lossy(&errors)
    );
    Ok(bytes)
}
impl Android {
    pub fn new(name: &str, settings: serde_json::Value) -> Result<Self> {
        let guard = DEVICE.lock().unwrap_or_else(|p| p.into_inner());
        let adb = std::env::var_os("QUIET_ADB")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/tmp/android-launcher-sdk/platform-tools/adb".into());
        let serial = std::env::var("QUIET_SERIAL").unwrap_or_else(|_| "emulator-5554".into());
        anyhow::ensure!(
            serial.starts_with("emulator-"),
            "ENVIRONMENT: destructive acceptance cases require an emulator serial"
        );
        let apk = std::env::var_os("QUIET_TEST_APK")
            .map(PathBuf::from)
            .unwrap_or_else(|| "target/emulator/release/apk/quiet-launcher.apk".into());
        anyhow::ensure!(
            apk.is_file(),
            "ENVIRONMENT: build the x86_64 test APK first"
        );
        for stamp in [
            "target/acceptance-fixtures/build-inputs.sha256",
            "target/v1-sut-build-inputs.sha256",
        ] {
            let result = Command::new("sha256sum")
                .args(["-c", stamp])
                .output()
                .context("ENVIRONMENT: source provenance check")?;
            anyhow::ensure!(
                result.status.success(),
                "ENVIRONMENT: stale or missing build provenance {stamp}; rebuild first"
            );
        }
        let run = |args: &[&str]| output(Command::new(&adb).args(["-s", &serial]).args(args));
        run(&["root"])?;
        run(&["wait-for-device"])?;
        run(&["install", "-r", "--no-streaming", apk.to_str().unwrap()])?;
        for package in FIXTURES {
            let p = format!("target/acceptance-fixtures/{package}.apk");
            anyhow::ensure!(
                Path::new(&p).is_file(),
                "ENVIRONMENT: build_acceptance_fixtures is required: {p}"
            );
            run(&["install", "-r", "--no-streaming", &p])?;
            run(&["shell", "am", "force-stop", package])?;
        }
        let widgets = String::from_utf8(run(&["shell", "dumpsys", "appwidget"])?)?;
        let grants = widgets
            .split("Grants:")
            .nth(1)
            .context("ENVIRONMENT: cannot inspect prior widget grants")?;
        let bind_was_allowed = grants
            .lines()
            .any(|l| l.contains("package=dev.quiet.launcher"));
        run(&[
            "shell",
            "am",
            "start",
            "-W",
            "-n",
            "dev.quiet.launcher/android.app.NativeActivity",
        ])?;
        let original = run(&["shell", "cat", SETTINGS]).ok();
        let backup = run(&["shell", "cat", &format!("{SETTINGS}.bak")]).ok();
        let owner = String::from_utf8(run(&[
            "shell",
            "stat",
            "-c",
            "%u",
            "/data/user/0/dev.quiet.launcher",
        ])?)?
        .trim()
        .to_owned();
        let size = String::from_utf8(run(&["shell", "wm", "size"])?)?;
        let numbers = size
            .split_whitespace()
            .last()
            .context("screen size")?
            .split('x')
            .map(str::parse::<i32>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        anyhow::ensure!(numbers.len() == 2, "ENVIRONMENT: invalid wm size");
        let out = Path::new("target/v1-android").join(name);
        std::fs::create_dir_all(&out)?;
        let lab = Self {
            _guard: guard,
            adb,
            serial,
            out,
            original,
            backup,
            owner,
            listener: None,
            widget_packages: Mutex::new(vec![]),
            bind_was_allowed,
            width: numbers[0],
            height: numbers[1],
        };
        lab.shell(&["am", "force-stop", PACKAGE])?;
        lab.put_settings(&serde_json::to_vec(&settings)?)?;
        lab.start()?;
        Ok(lab)
    }
    pub fn run(&self, args: &[&str]) -> Result<Vec<u8>> {
        output(
            Command::new(&self.adb)
                .args(["-s", &self.serial])
                .args(args),
        )
    }
    pub fn shell(&self, args: &[&str]) -> Result<Vec<u8>> {
        let mut a = vec!["shell"];
        a.extend(args);
        self.run(&a)
    }
    pub fn put_settings(&self, bytes: &[u8]) -> Result<()> {
        let p = self.out.join("input-settings.json");
        std::fs::write(&p, bytes)?;
        self.run(&[
            "push",
            p.to_str().unwrap(),
            "/data/local/tmp/quiet-v1-settings.json",
        ])?;
        self.shell(&["cp", "/data/local/tmp/quiet-v1-settings.json", SETTINGS])?;
        self.shell(&["chown", &self.owner, SETTINGS])?;
        self.shell(&["chmod", "600", SETTINGS])?;
        Ok(())
    }
    pub fn start(&self) -> Result<()> {
        self.shell(&[
            "am",
            "start",
            "-W",
            "-n",
            "dev.quiet.launcher/android.app.NativeActivity",
        ])?;
        Ok(())
    }
    pub fn restart(&self) -> Result<()> {
        self.shell(&["am", "force-stop", PACKAGE])?;
        self.start()
    }
    pub fn drawer(&self) -> Result<()> {
        self.shell(&[
            "input",
            "swipe",
            &(self.width / 2).to_string(),
            &(self.height * 4 / 5).to_string(),
            &(self.width / 2).to_string(),
            &(self.height / 3).to_string(),
            "350",
        ])?;
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            self.shell(&["uiautomator", "dump", "/data/local/tmp/quiet-v1-field.xml"])?;
            let xml =
                String::from_utf8(self.shell(&["cat", "/data/local/tmp/quiet-v1-field.xml"])?)?;
            if xml.contains("android.widget.EditText") {
                break;
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "inline native search was not mounted"
            );
        }
        Ok(())
    }
    pub fn back(&self) -> Result<()> {
        self.shell(&["input", "keyevent", "KEYCODE_BACK"])?;
        Ok(())
    }
    pub fn type_text(&self, text: &str) -> Result<()> {
        self.shell(&["input", "text", text])?;
        Ok(())
    }
    pub fn image(&self, name: &str) -> Result<PathBuf> {
        let p = self.out.join(name);
        let bytes = self.run(&["exec-out", "screencap", "-p"])?;
        anyhow::ensure!(
            bytes.starts_with(b"\x89PNG"),
            "ENVIRONMENT: screenshot is not PNG"
        );
        std::fs::write(&p, bytes)?;
        Ok(p)
    }
    pub fn lines(&self, name: &str) -> Result<Vec<(String, [i32; 4])>> {
        self.lines_mode(name, "11")
    }
    fn lines_mode(&self, name: &str, psm: &str) -> Result<Vec<(String, [i32; 4])>> {
        let shot = self.image(name)?;
        let ocr = std::env::var("QUIET_TESSERACT")
            .unwrap_or_else(|_| "/tmp/quiet-ocr/root/usr/bin/tesseract".into());
        let data = std::env::var("QUIET_TESSDATA")
            .unwrap_or_else(|_| "/tmp/quiet-ocr/root/usr/share/tesseract-ocr/5/tessdata".into());
        let bytes = output(Command::new(ocr).arg(shot).arg("stdout").args([
            "--tessdata-dir",
            &data,
            "-l",
            "eng+jpn",
            "--psm",
            psm,
            "tsv",
        ]))?;
        let mut lines: BTreeMap<String, (String, [i32; 4])> = BTreeMap::new();
        for line in String::from_utf8(bytes)?.lines().skip(1) {
            let c: Vec<_> = line.split('\t').collect();
            if c.len() < 12 || c[0] != "5" || c[11].trim().is_empty() {
                continue;
            }
            let x = c[6].parse::<i32>()?;
            let y = c[7].parse::<i32>()?;
            let w = c[8].parse::<i32>()?;
            let h = c[9].parse::<i32>()?;
            let item = lines
                .entry(format!("{}:{}:{}", c[2], c[3], c[4]))
                .or_insert((String::new(), [x, y, x + w, y + h]));
            item.0.push_str(c[11]);
            item.1 = [
                item.1[0].min(x),
                item.1[1].min(y),
                item.1[2].max(x + w),
                item.1[3].max(y + h),
            ];
        }
        let result: Vec<_> = lines.into_values().collect();
        std::fs::write(
            self.out.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&result)?,
        )?;
        Ok(result)
    }
    pub fn dialog_text(&self, text: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            let lines = self.lines_mode("native-dialog.png", "6")?;
            if lines
                .iter()
                .any(|(s, _)| normalize(s).contains(&normalize(text)))
            {
                return Ok(());
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "expected native dialog text {text:?}; {lines:?}"
            );
        }
    }
    pub fn wait_text(&self, text: &str) -> Result<[i32; 4]> {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            let lines = self.lines("screen.png")?;
            if let Some((_, b)) = lines
                .iter()
                .find(|(s, _)| normalize(s).contains(&normalize(text)))
            {
                return Ok(*b);
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "expected visible text {text:?}; got {lines:?}"
            );
        }
    }
    pub fn assert_absent(&self, text: &str) -> Result<()> {
        let lines = self.lines("absent.png")?;
        anyhow::ensure!(
            !lines
                .iter()
                .any(|(s, _)| normalize(s).contains(&normalize(text))),
            "unexpected visible text {text:?}: {lines:?}"
        );
        Ok(())
    }
    pub fn app_bounds(&self, text: &str) -> Result<[i32; 4]> {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            let lines = self.lines("app-row.png")?;
            if let Some((_, b)) = lines
                .iter()
                .find(|(s, b)| normalize(s).contains(&normalize(text)) && b[1] > self.height / 7)
            {
                return Ok(*b);
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "expected app row below search field: {text}; {lines:?}"
            );
        }
    }
    pub fn tap_app(&self, text: &str) -> Result<()> {
        let b = self.app_bounds(text)?;
        self.shell(&[
            "input",
            "tap",
            &((b[0] + b[2]) / 2).to_string(),
            &((b[1] + b[3]) / 2).to_string(),
        ])?;
        Ok(())
    }
    pub fn long_press_app(&self, text: &str) -> Result<()> {
        let b = self.app_bounds(text)?;
        let x = ((b[0] + b[2]) / 2).to_string();
        let y = ((b[1] + b[3]) / 2).to_string();
        self.shell(&["input", "swipe", &x, &y, &x, &y, "900"])?;
        Ok(())
    }
    pub fn tap_text(&self, text: &str) -> Result<()> {
        let b = self.wait_text(text)?;
        self.shell(&[
            "input",
            "tap",
            &((b[0] + b[2]) / 2).to_string(),
            &((b[1] + b[3]) / 2).to_string(),
        ])?;
        Ok(())
    }
    pub fn long_press(&self, text: &str) -> Result<()> {
        let b = self.wait_text(text)?;
        let x = ((b[0] + b[2]) / 2).to_string();
        let y = ((b[1] + b[3]) / 2).to_string();
        self.shell(&["input", "swipe", &x, &y, &x, &y, "900"])?;
        Ok(())
    }
    pub fn settings(&self) -> Result<()> {
        let x = (self.width * 9 / 10).to_string();
        let y = (self.height / 7).to_string();
        self.shell(&["input", "swipe", &x, &y, &x, &y, "900"])?;
        self.wait_text("文字サイズ")?;
        Ok(())
    }
    pub fn scroll_to(&self, text: &str) -> Result<()> {
        for _ in 0..12 {
            if self
                .lines("scroll.png")?
                .iter()
                .any(|(s, _)| normalize(s).contains(&normalize(text)))
            {
                return Ok(());
            }
            self.shell(&[
                "input",
                "swipe",
                &(self.width / 2).to_string(),
                &(self.height * 4 / 5).to_string(),
                &(self.width / 2).to_string(),
                &(self.height / 4).to_string(),
                "300",
            ])?;
        }
        anyhow::bail!("expected settings entry not found: {text}")
    }
    pub fn saved(&self) -> Result<serde_json::Value> {
        Ok(serde_json::from_slice(&self.shell(&["cat", SETTINGS])?)?)
    }
    pub fn foreground(&self, package: &str) -> Result<()> {
        let state = String::from_utf8(self.shell(&["dumpsys", "activity", "activities"])?)?;
        std::fs::write(self.out.join("activity.txt"), &state)?;
        anyhow::ensure!(
            state.lines().any(|l| (l.contains("topResumedActivity")
                || l.contains("ResumedActivity"))
                && l.contains(&format!("{package}/"))),
            "wrong resumed activity: expected {package}"
        );
        Ok(())
    }
    pub fn fixture(&self, package: &str, activity: &str) -> Result<()> {
        self.shell(&[
            "am",
            "start",
            "-W",
            "-n",
            &format!("{package}/{package}.{activity}"),
        ])?;
        Ok(())
    }
    pub fn session(&self, package: &str) -> Result<serde_json::Value> {
        Ok(serde_json::from_slice(&self.shell(&[
            "run-as",
            package,
            "cat",
            "files/session.json",
        ])?)?)
    }
    pub fn add_widget(&self, kind: &str, label: &str) -> Result<()> {
        let package = format!("example.widget.{kind}");
        let apk = format!("target/acceptance-fixtures/{package}.apk");
        self.run(&["install", "-r", "--no-streaming", &apk])?;
        self.widget_packages.lock().unwrap().push(package);
        self.shell(&["appwidget", "grantbind", "--package", "dev.quiet.launcher"])?;
        self.restart()?;
        self.wait_text("メモ")?;
        self.settings()?;
        self.scroll_to("ウィジェットを選ぶ")?;
        self.tap_text("ウィジェットを選ぶ")?;
        self.scroll_to(label)?;
        self.tap_text(label)?;
        Ok(())
    }
    pub fn enable_listener(&mut self) -> Result<()> {
        let services = String::from_utf8(self.shell(&[
            "cmd",
            "package",
            "query-services",
            "--components",
            "-a",
            "android.service.notification.NotificationListenerService",
            "-p",
            PACKAGE,
        ])?)?;
        let component = services
            .lines()
            .find(|l| l.starts_with("dev.quiet.launcher/"))
            .context("required notification listener service is missing")?
            .to_owned();
        let old = String::from_utf8(self.shell(&[
            "settings",
            "get",
            "secure",
            "enabled_notification_listeners",
        ])?)?;
        if !old.contains(&component) {
            self.shell(&["cmd", "notification", "allow_listener", &component])?;
            self.listener = Some(component)
        }
        Ok(())
    }
}
impl Drop for Android {
    fn drop(&mut self) {
        let _ = self.shell(&["am", "force-stop", PACKAGE]);
        for package in self.widget_packages.lock().unwrap().iter() {
            let _ = self.run(&["uninstall", package]);
        }
        let _ = self.shell(&[
            "appwidget",
            if self.bind_was_allowed {
                "grantbind"
            } else {
                "revokebind"
            },
            "--package",
            PACKAGE,
        ]);

        if let Some(bytes) = &self.original {
            let _ = self.put_settings(bytes);
        } else {
            let _ = self.shell(&["rm", "-f", SETTINGS]);
        }
        if let Some(bytes) = &self.backup {
            let p = self.out.join("original-backup.json");
            if std::fs::write(&p, bytes).is_ok() {
                let _ = self.run(&[
                    "push",
                    p.to_str().unwrap(),
                    "/data/local/tmp/quiet-v1-backup.json",
                ]);
                let _ = self.shell(&[
                    "cp",
                    "/data/local/tmp/quiet-v1-backup.json",
                    &format!("{SETTINGS}.bak"),
                ]);
                let _ = self.shell(&["chown", &self.owner, &format!("{SETTINGS}.bak")]);
            }
        } else {
            let _ = self.shell(&["rm", "-f", &format!("{SETTINGS}.bak")]);
        }
        if let Some(c) = &self.listener {
            let _ = self.shell(&["cmd", "notification", "disallow_listener", c]);
        }
        for p in FIXTURES {
            let _ = self.shell(&["am", "force-stop", p]);
        }
        let _ = self.start();
    }
}
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}
pub fn settings() -> serde_json::Value {
    serde_json::json!({"favorites":["example.notes/example.notes.Main"],"hidden_packages":[],"widget_ids":[],"widget_names":{},"font_percent":100,"show_weather":false,"show_usage":false,"show_media":false,"dark":false})
}
