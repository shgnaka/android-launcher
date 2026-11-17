//! Run against a booted/running Android emulator, never an attached phone.
//! cargo run --example emulator_smoke --no-default-features -- \
//!   --adb /path/to/adb --apk target/emulator/release/apk/quiet-launcher.apk
use anyhow::{bail, Context, Result};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Device {
    adb: PathBuf,
    serial: String,
    out: PathBuf,
}

fn timed_output(command: &mut Command, timeout: Duration) -> Result<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out.join();
            let _ = err.join();
            bail!("command timed out after {}s", timeout.as_secs());
        }
        thread::sleep(Duration::from_millis(100));
    };
    Ok(Output {
        status,
        stdout: out.join().unwrap()?,
        stderr: err.join().unwrap()?,
    })
}

impl Device {
    fn run(&self, args: &[&str], seconds: u64) -> Result<Output> {
        timed_output(
            Command::new(&self.adb)
                .args(["-s", &self.serial])
                .args(args),
            Duration::from_secs(seconds),
        )
    }
    fn text(&self, args: &[&str], seconds: u64) -> Result<String> {
        let result = self.run(args, seconds)?;
        if !result.status.success() {
            bail!("adb {args:?}: {}", String::from_utf8_lossy(&result.stderr));
        }
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    }
    fn capture(&self, name: &str) -> Result<(u32, u32)> {
        let output = self.run(&["exec-out", "screencap", "-p"], 45)?;
        if !output.status.success()
            || !output.stdout.starts_with(b"\x89PNG\r\n\x1a\n")
            || output.stdout.len() < 24
        {
            bail!(
                "screencap did not produce a PNG: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let width = u32::from_be_bytes(output.stdout[16..20].try_into()?);
        let height = u32::from_be_bytes(output.stdout[20..24].try_into()?);
        std::fs::write(self.out.join(name), output.stdout)?;
        Ok((width, height))
    }
    fn assert_launcher_foreground(&self, name: &str) -> Result<()> {
        let activity = self.text(&["shell", "dumpsys", "activity", "activities"], 30)?;
        std::fs::write(self.out.join(name), &activity)?;
        if !activity.lines().any(|line| {
            (line.contains("ResumedActivity") || line.contains("topResumedActivity"))
                && line.contains("dev.quiet.launcher/")
        }) {
            bail!("Quiet is not the resumed activity; see {name}");
        }
        let windows = self.text(&["shell", "dumpsys", "window", "windows"], 30)?;
        std::fs::write(self.out.join(format!("{name}-windows.txt")), &windows)?;
        if !windows.split("\n  Window #").any(|window| {
            window
                .lines()
                .next()
                .is_some_and(|line| line.contains("dev.quiet.launcher/android.app.NativeActivity"))
                && window.contains("Surface: shown=true")
                && window.contains("mDrawState=HAS_DRAWN")
        }) {
            bail!("Quiet is resumed but its window has not drawn; see {name}-windows.txt");
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut adb = PathBuf::from("adb");
    let mut serial = "emulator-5554".to_owned();
    let mut apk = PathBuf::from("target/emulator/release/apk/quiet-launcher.apk");
    let mut out = PathBuf::from("target/android-smoke");
    let mut boot_timeout = 900u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next().context("each option requires a value")?;
        match arg.as_str() {
            "--adb" => adb = value.into(),
            "--serial" => serial = value,
            "--apk" => apk = value.into(),
            "--out" => out = value.into(),
            "--boot-timeout" => boot_timeout = value.parse()?,
            _ => bail!("unknown option: {arg}"),
        }
    }
    if !serial.starts_with("emulator-") {
        bail!("only emulator serials are accepted");
    }
    if !Path::new(&apk).is_file() {
        bail!("APK not found: {}", apk.display());
    }
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("summary.json"), b"{\"status\":\"running\"}")?;
    let device = Device { adb, serial, out };
    let start = Instant::now();
    let mut last_progress = Instant::now() - Duration::from_secs(30);
    println!("Waiting for sys.boot_completed=1 and Android package manager...");
    loop {
        if start.elapsed() > Duration::from_secs(boot_timeout) {
            bail!("Android did not finish booting");
        }
        let ready = device
            .text(&["shell", "getprop", "sys.boot_completed"], 5)
            .is_ok_and(|value| value.trim() == "1");
        if ready
            && device
                .text(&["shell", "pm", "path", "android"], 10)
                .is_ok_and(|value| value.contains("package:"))
        {
            break;
        }
        if last_progress.elapsed() >= Duration::from_secs(30) {
            println!("Still booting: {}s", start.elapsed().as_secs());
            last_progress = Instant::now();
        }
        thread::sleep(Duration::from_secs(5));
    }
    println!("PASS boot_ready ({}s)", start.elapsed().as_secs());
    let install = device.text(
        &[
            "install",
            "-r",
            "--no-streaming",
            apk.to_str().context("APK path is not UTF-8")?,
        ],
        120,
    )?;
    std::fs::write(device.out.join("install.txt"), &install)?;
    if !install.contains("Success") {
        bail!("install did not report Success: {install}");
    }
    println!("PASS install");
    device.text(&["shell", "input", "keyevent", "KEYCODE_WAKEUP"], 15)?;
    device.text(&["shell", "wm", "dismiss-keyguard"], 15)?;
    device.text(&["logcat", "-c"], 10)?;
    let launch = device.text(
        &[
            "shell",
            "am",
            "start",
            "-W",
            "-n",
            "dev.quiet.launcher/android.app.NativeActivity",
        ],
        90,
    )?;
    std::fs::write(device.out.join("launch.txt"), launch)?;
    thread::sleep(Duration::from_secs(5));
    device.assert_launcher_foreground("home-activity.txt")?;
    let (width, height) = device.capture("01-home.png")?;
    println!("PASS launch_and_capture ({width}x{height})");
    for (start, end) in [(height * 4 / 5, height / 3), (height * 9 / 20, height / 5)] {
        device.text(
            &[
                "shell",
                "input",
                "swipe",
                &(width / 2).to_string(),
                &start.to_string(),
                &(width / 2).to_string(),
                &end.to_string(),
                "450",
            ],
            30,
        )?;
        thread::sleep(Duration::from_secs(3));
    }
    device.assert_launcher_foreground("drawer-activity.txt")?;
    device.capture("02-drawer.png")?;
    println!("PASS swipe_without_accidental_launch");
    // The first Back may dismiss the IME; the next closes the separate drawer.
    for _ in 0..2 {
        device.text(&["shell", "input", "keyevent", "KEYCODE_BACK"], 15)?;
        thread::sleep(Duration::from_secs(1));
    }
    let previous_home = device.text(
        &[
            "shell",
            "cmd",
            "package",
            "resolve-activity",
            "--brief",
            "-a",
            "android.intent.action.MAIN",
            "-c",
            "android.intent.category.HOME",
        ],
        15,
    )?;
    let previous_home = previous_home
        .lines()
        .rev()
        .find(|line| line.contains('/') && !line.contains(' '))
        .unwrap_or("com.android.launcher3/com.android.launcher3.uioverrides.QuickstepLauncher")
        .to_owned();
    device.text(
        &[
            "shell",
            "cmd",
            "package",
            "set-home-activity",
            "dev.quiet.launcher/android.app.NativeActivity",
        ],
        15,
    )?;
    let home_result = (|| -> Result<()> {
        for iteration in 0..3 {
            device.text(
                &[
                    "shell",
                    "am",
                    "start",
                    "-W",
                    "-a",
                    "android.settings.SETTINGS",
                ],
                30,
            )?;
            device.text(&["shell", "input", "keyevent", "KEYCODE_HOME"], 15)?;
            thread::sleep(Duration::from_secs(3));
            device.assert_launcher_foreground(&format!("home-return-{iteration}.txt"))?;
        }
        device.capture("08-return-home.png")?;
        Ok(())
    })();
    if home_result.is_err() {
        let fallback = if previous_home.starts_with("dev.quiet.launcher/") {
            "com.android.launcher3/com.android.launcher3.uioverrides.QuickstepLauncher"
        } else {
            &previous_home
        };
        let _ = device.text(
            &["shell", "cmd", "package", "set-home-activity", fallback],
            15,
        );
        let _ = device.text(&["shell", "am", "force-stop", "dev.quiet.launcher"], 15);
    }
    home_result?;
    println!("PASS default_home_return_with_drawn_window (3 cycles)");
    let logs = device.text(&["logcat", "-d", "-t", "3000"], 30)?;
    std::fs::write(device.out.join("app-logcat.txt"), &logs)?;
    if logs.contains("Fatal signal")
        || logs.contains("thread 'main' panicked")
        || logs.contains("FATAL EXCEPTION")
    {
        bail!("fatal error appears in logcat; inspect app-logcat.txt");
    }
    std::fs::write(
        device.out.join("summary.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "passed", "serial": device.serial, "boot_ready": true, "install": true,
            "version": env!("CARGO_PKG_VERSION"),
            "launch": true, "swipe_keeps_launcher_foreground": true,
            "default_home_return_cycles": 3, "home_window_shown_and_drawn": true,
            "screenshots": ["01-home.png", "02-drawer.png", "08-return-home.png"],
            "note": "Screenshots require visual review. Emulator timings do not measure phone FPS."
        }))?,
    )?;
    println!("PASS smoke; screenshots and logs: {}", device.out.display());
    Ok(())
}
