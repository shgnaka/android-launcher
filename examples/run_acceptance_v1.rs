//! Execute every compiled test and each available Android API; retain all red/environment errors.
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn execute(c: &mut Command, log: &Path) -> Result<bool> {
    let f = std::fs::File::create(log)?;
    let status = c
        .stdout(Stdio::from(f.try_clone()?))
        .stderr(Stdio::from(f))
        .status()?;
    Ok(status.success())
}
fn isolated(root: &Path, api: u32) -> Result<PathBuf> {
    let d = root.join(format!(".local/acceptance-runs/api{api}"));
    std::fs::create_dir_all(d.join("target"))?;
    for name in [
        "src",
        "android",
        "tests",
        "examples",
        "vendor",
        "Cargo.toml",
        "Cargo.lock",
        "dist",
    ] {
        let p = d.join(name);
        if !p.exists() {
            std::os::unix::fs::symlink(root.join(name), p)?;
        }
    }
    for name in [
        "emulator",
        "release",
        "acceptance-fixtures",
        "widget-fixture",
        "v1-sut-build-inputs.sha256",
    ] {
        let p = d.join("target").join(name);
        if !p.exists() {
            std::os::unix::fs::symlink(root.join("target").join(name), p)?;
        }
    }
    Ok(d)
}
fn summary(text: &str) -> Value {
    let mut pass = vec![];
    let mut fail = vec![];
    for l in text.lines() {
        if let Some(v) = l.strip_prefix("test ") {
            if let Some(n) = v.strip_suffix(" ... ok") {
                pass.push(n);
            } else if let Some(n) = v.strip_suffix(" ... FAILED") {
                fail.push(n);
            }
        }
    }
    json!({"passed":pass,"failed":fail,"environment_error":text.contains("ENVIRONMENT:")})
}
fn main() -> Result<()> {
    let root = std::env::current_dir()?;
    let out = root.join("target/v1-run");
    std::fs::create_dir_all(&out)?;
    anyhow::ensure!(
        execute(
            Command::new("cargo").args([
                "run",
                "--offline",
                "--no-default-features",
                "--example",
                "check_v1_contracts"
            ]),
            &out.join("freeze.txt")
        )?,
        "test freeze verification failed"
    );
    let build = Command::new("cargo")
        .args([
            "test",
            "--offline",
            "--features",
            "android-check",
            "--no-run",
            "--message-format",
            "json",
        ])
        .output()?;
    std::fs::write(out.join("build.jsonl"), &build.stdout)?;
    std::fs::write(out.join("build.stderr"), &build.stderr)?;
    anyhow::ensure!(
        build.status.success(),
        "test compilation failed; not a behavior Red"
    );
    let mut binaries = BTreeMap::new();
    for line in String::from_utf8(build.stdout)?.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["reason"] == "compiler-artifact" && v["profile"]["test"] == true {
            if let (Some(name), Some(path)) =
                (v["target"]["name"].as_str(), v["executable"].as_str())
            {
                binaries.insert(name.to_owned(), PathBuf::from(path));
            }
        }
    }
    let mut results = vec![];
    for (name, binary) in &binaries {
        if name == "android_requirements_v1" {
            continue;
        }
        println!("RUN {name}");
        let log = out.join(format!("{name}.txt"));
        let ok = execute(&mut Command::new(binary), &log)?;
        let data = summary(&std::fs::read_to_string(&log)?);
        results.push(json!({"target":name,"success":ok,"results":data,"log":log}));
    }
    let adb = std::env::var("QUIET_ADB")
        .unwrap_or_else(|_| "/tmp/android-launcher-sdk/platform-tools/adb".into());
    let devices = Command::new(&adb).arg("devices").output();
    let mut apis = BTreeMap::new();
    if let Ok(d) = devices {
        for line in String::from_utf8_lossy(&d.stdout).lines().skip(1) {
            let mut fields = line.split_whitespace();
            if let (Some(serial), Some("device")) = (fields.next(), fields.next()) {
                if !serial.starts_with("emulator-") {
                    continue;
                }
                let o = Command::new(&adb)
                    .args(["-s", serial, "shell", "getprop", "ro.build.version.sdk"])
                    .output()?;
                if let Ok(api) = String::from_utf8_lossy(&o.stdout).trim().parse::<u32>() {
                    apis.insert(api, serial.to_owned());
                }
            }
        }
    }
    let android = binaries
        .get("android_requirements_v1")
        .context("Android tests are missing from build targets")?;
    for api in [28, 33, 35] {
        if let Some(serial) = apis.get(&api) {
            println!("RUN Android API {api}: {serial}");
            let log = out.join(format!("android-api{api}.txt"));
            let ok = execute(
                Command::new(android)
                    .current_dir(isolated(&root, api)?)
                    .env("QUIET_SERIAL", serial),
                &log,
            )?;
            results.push(json!({"target":"android_requirements_v1","api":api,"serial":serial,"success":ok,"results":summary(&std::fs::read_to_string(&log)?),"log":log}));
        } else {
            results.push(json!({"target":"android_requirements_v1","api":api,"success":false,"environment_error":"required emulator is not running"}));
        }
    }
    if let Some(serial) = apis.get(&35) {
        println!("RUN approved widget E2E minimum on API35");
        let log = out.join("widget-minimum.txt");
        let passed = widget_minimum(&root, &out, &adb, serial, &log)?;
        results
            .push(json!({"target":"approved_widget_minimum","success":passed,"api":35,"log":log}));
    } else {
        results.push(json!({"target":"approved_widget_minimum","success":false,"environment_error":"API35 emulator unavailable"}));
    }
    let successful = results.iter().all(|v| v["success"] == true);
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(
            &json!({"all_passed":successful,"results":results,"note":"Judge self-tests are not real phone performance; API matrix never skips missing environments."}),
        )?,
    )?;
    anyhow::ensure!(successful,"one or more acceptance targets failed or lack required evidence; see target/v1-run/summary.json");
    Ok(())
}
fn adb(adb: &str, serial: &str, args: &[&str]) -> Result<std::process::Output> {
    Ok(Command::new(adb).args(["-s", serial]).args(args).output()?)
}
fn widget_minimum(
    root: &Path,
    out: &Path,
    program: &str,
    serial: &str,
    log: &Path,
) -> Result<bool> {
    let settings = "/data/user/0/dev.quiet.launcher/files/settings.json";
    let backup = format!("{settings}.bak");
    let original = adb(program, serial, &["shell", "cat", settings])?;
    let old_backup = adb(program, serial, &["shell", "cat", &backup])?;
    let uid = adb(
        program,
        serial,
        &[
            "shell",
            "stat",
            "-c",
            "%u",
            "/data/user/0/dev.quiet.launcher",
        ],
    )?;
    anyhow::ensure!(
        uid.status.success(),
        "widget prerequisite: Quiet not installed"
    );
    let uid = String::from_utf8(uid.stdout)?.trim().to_owned();
    if !original.status.success() {
        let seed = out.join("widget-seed.json");
        std::fs::write(&seed,br#"{"favorites":[],"widget_ids":[],"show_weather":false,"show_usage":false,"show_media":false,"dark":false}"#)?;
        anyhow::ensure!(adb(
            program,
            serial,
            &[
                "push",
                seed.to_str().unwrap(),
                "/data/local/tmp/widget-seed.json"
            ]
        )?
        .status
        .success());
        anyhow::ensure!(adb(
            program,
            serial,
            &["shell", "cp", "/data/local/tmp/widget-seed.json", settings]
        )?
        .status
        .success());
        adb(program, serial, &["shell", "chown", &uid, settings])?;
    }
    let outcome = execute(
        Command::new(root.join("target/debug/examples/widget_e2e"))
            .current_dir(isolated(root, 35)?)
            .args(["--adb", program, "--serial", serial]),
        log,
    );
    adb(
        program,
        serial,
        &["shell", "am", "force-stop", "dev.quiet.launcher"],
    )?;
    for (path, previous) in [(settings, original), (backup.as_str(), old_backup)] {
        if previous.status.success() {
            let local = out.join(if path == settings {
                "widget-original.json"
            } else {
                "widget-original-backup.json"
            });
            std::fs::write(&local, previous.stdout)?;
            adb(
                program,
                serial,
                &[
                    "push",
                    local.to_str().unwrap(),
                    "/data/local/tmp/widget-restore.json",
                ],
            )?;
            adb(
                program,
                serial,
                &["shell", "cp", "/data/local/tmp/widget-restore.json", path],
            )?;
            adb(program, serial, &["shell", "chown", &uid, path])?;
        } else {
            adb(program, serial, &["shell", "rm", "-f", path])?;
        }
    }
    adb(
        program,
        serial,
        &[
            "shell",
            "am",
            "start",
            "-W",
            "-n",
            "dev.quiet.launcher/android.app.NativeActivity",
        ],
    )?;
    outcome
}
