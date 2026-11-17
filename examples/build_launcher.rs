//! Build a Rust launcher plus a constructor-only Android service adapter; no Java/Kotlin sources.
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn run(c: &mut Command) -> Result<()> {
    let status = c.status()?;
    anyhow::ensure!(status.success(), "command failed: {c:?}");
    Ok(())
}
fn utf(s: &str) -> Vec<u8> {
    let mut b = vec![1];
    b.extend((s.len() as u16).to_be_bytes());
    b.extend(s.as_bytes());
    b
}
fn u16s(b: &mut Vec<u8>, values: &[u16]) {
    for v in values {
        b.extend(v.to_be_bytes());
    }
}
fn service_class() -> Vec<u8> {
    let pool = vec![
        utf("dev/quiet/launcher/MediaAccessService"),
        vec![7, 0, 1],
        utf("android/service/notification/NotificationListenerService"),
        vec![7, 0, 3],
        utf("<init>"),
        utf("()V"),
        vec![12, 0, 5, 0, 6],
        vec![10, 0, 4, 0, 7],
        utf("Code"),
    ];
    let mut b = vec![0xca, 0xfe, 0xba, 0xbe];
    u16s(&mut b, &[0, 52, 10]);
    for entry in pool {
        b.extend(entry);
    }
    u16s(&mut b, &[0x21, 2, 4, 0, 0, 1, 1, 5, 6, 1, 9]);
    b.extend(17u32.to_be_bytes());
    u16s(&mut b, &[1, 1]);
    b.extend(5u32.to_be_bytes());
    b.extend([0x2a, 0xb7, 0, 8, 0xb1]);
    u16s(&mut b, &[0, 0, 0]);
    b
}
struct Pool(Vec<Vec<u8>>);
impl Pool {
    fn add(&mut self, b: Vec<u8>) -> u16 {
        self.0.push(b);
        self.0.len() as u16
    }
    fn utf(&mut self, s: &str) -> u16 {
        self.add(utf(s))
    }
    fn class(&mut self, s: &str) -> u16 {
        let index = self.utf(s);
        let mut b = vec![7];
        u16s(&mut b, &[index]);
        self.add(b)
    }
    fn method(&mut self, class: u16, name: u16, signature: u16) -> u16 {
        let mut b = vec![12];
        u16s(&mut b, &[name, signature]);
        let nt = self.add(b);
        let mut b = vec![10];
        u16s(&mut b, &[class, nt]);
        self.add(b)
    }
}
fn method_code(
    b: &mut Vec<u8>,
    access: u16,
    name: u16,
    signature: u16,
    code_index: u16,
    stack: u16,
    locals: u16,
    code: &[u8],
) {
    u16s(b, &[access, name, signature, 1, code_index]);
    b.extend((12u32 + code.len() as u32).to_be_bytes());
    u16s(b, &[stack, locals]);
    b.extend((code.len() as u32).to_be_bytes());
    b.extend(code);
    u16s(b, &[0, 0]);
}
fn widget_activity_class() -> Vec<u8> {
    let mut p = Pool(vec![]);
    let this = p.class("dev/quiet/launcher/WidgetRequestActivity");
    let parent = p.class("android/app/Activity");
    let init = p.utf("<init>");
    let void = p.utf("()V");
    let parent_init = p.method(parent, init, void);
    let code = p.utf("Code");
    let create = p.utf("onCreate");
    let bundle = p.utf("(Landroid/os/Bundle;)V");
    let parent_create = p.method(parent, create, bundle);
    let launch = p.utf("launchRequest");
    let launch_ref = p.method(this, launch, void);
    let result = p.utf("onActivityResult");
    let result_sig = p.utf("(IILandroid/content/Intent;)V");
    let system = p.class("java/lang/System");
    let load = p.utf("loadLibrary");
    let string_void = p.utf("(Ljava/lang/String;)V");
    let load_ref = p.method(system, load, string_void);
    let library = p.utf("quiet_launcher");
    let mut string = vec![8];
    u16s(&mut string, &[library]);
    let library = p.add(string);
    let clinit = p.utf("<clinit>");
    let mut b = vec![0xca, 0xfe, 0xba, 0xbe];
    u16s(&mut b, &[0, 52, p.0.len() as u16 + 1]);
    for e in p.0 {
        b.extend(e);
    }
    u16s(&mut b, &[0x21, this, parent, 0, 0, 5]);
    method_code(
        &mut b,
        1,
        init,
        void,
        code,
        1,
        1,
        &[
            0x2a,
            0xb7,
            (parent_init >> 8) as u8,
            parent_init as u8,
            0xb1,
        ],
    );
    method_code(
        &mut b,
        8,
        clinit,
        void,
        code,
        1,
        0,
        &[
            0x12,
            library as u8,
            0xb8,
            (load_ref >> 8) as u8,
            load_ref as u8,
            0xb1,
        ],
    );
    method_code(
        &mut b,
        4,
        create,
        bundle,
        code,
        2,
        2,
        &[
            0x2a,
            0x2b,
            0xb7,
            (parent_create >> 8) as u8,
            parent_create as u8,
            0x2a,
            0xb6,
            (launch_ref >> 8) as u8,
            launch_ref as u8,
            0xb1,
        ],
    );
    u16s(
        &mut b,
        &[0x101, launch, void, 0, 0x104, result, result_sig, 0, 0],
    );
    b
}
fn stamp(root: &Path, out: &Path) -> Result<()> {
    let mut files = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("android/media-service.xml"),
        root.join("examples/build_launcher.rs"),
    ];
    fn visit(p: &Path, v: &mut Vec<PathBuf>) -> Result<()> {
        for e in std::fs::read_dir(p)? {
            let p = e?.path();
            if p.is_dir() {
                visit(&p, v)?;
            } else {
                v.push(p);
            }
        }
        Ok(())
    }
    visit(&root.join("src"), &mut files)?;
    visit(&root.join("android/res"), &mut files)?;
    files.sort();
    let mut lines = String::new();
    for file in files {
        let relative = file.strip_prefix(root)?;
        let result = Command::new("sha256sum").arg(relative).output()?;
        anyhow::ensure!(result.status.success(), "cannot hash source");
        lines.push_str(std::str::from_utf8(&result.stdout)?);
    }
    std::fs::write(out, lines)?;
    Ok(())
}
fn main() -> Result<()> {
    let root = std::env::current_dir()?;
    let arm = std::env::args().any(|a| a == "--arm64");
    let target = if arm {
        "aarch64-linux-android"
    } else {
        "x86_64-linux-android"
    };
    let base = if arm {
        root.join("target")
    } else {
        root.join("target/emulator")
    };
    let sdk = PathBuf::from(std::env::var("ANDROID_HOME").context("ANDROID_HOME")?);
    let tools = sdk.join("build-tools/35.0.0");
    run(Command::new("cargo")
        .args([
            "apk",
            "build",
            "--release",
            "--lib",
            "--target",
            target,
            "--target-dir",
        ])
        .arg(&base))?;
    let apk_dir = base.join("release/apk");
    let apk = apk_dir.join("quiet-launcher.apk");
    let work = base.join("launcher-package");
    std::fs::create_dir_all(work.join("dex"))?;
    let class = work.join("MediaAccessService.class");
    std::fs::write(&class, service_class())?;
    let activity_class = work.join("WidgetRequestActivity.class");
    std::fs::write(&activity_class, widget_activity_class())?;
    run(Command::new(tools.join("d8"))
        .args(["--min-api", "28", "--lib"])
        .arg(sdk.join("platforms/android-35/android.jar"))
        .arg("--output")
        .arg(work.join("dex"))
        .arg(class)
        .arg(activity_class))?;
    let manifest = std::fs::read_to_string(apk_dir.join("AndroidManifest.xml"))?
        .replacen(
            "<application ",
            "<application android:allowBackup=\"false\" ",
            1,
        )
        .replace("android:hasCode=\"false\"", "android:hasCode=\"true\"")
        .replace(
            "</application>",
            &format!(
                "{}</application>",
                include_str!("../android/media-service.xml")
            ),
        );
    std::fs::write(work.join("AndroidManifest.xml"), manifest)?;
    run(Command::new("unzip")
        .args(["-oq"])
        .arg(&apk)
        .arg("lib/*")
        .arg("-d")
        .arg(&work))?;
    std::fs::copy(work.join("dex/classes.dex"), work.join("classes.dex"))?;
    run(Command::new(tools.join("aapt"))
        .args(["package", "-f", "-M"])
        .arg(work.join("AndroidManifest.xml"))
        .arg("-S")
        .arg(root.join("android/res"))
        .arg("-I")
        .arg(sdk.join("platforms/android-35/android.jar"))
        .arg("-F")
        .arg(work.join("unsigned.apk")))?;
    let abi = if arm { "arm64-v8a" } else { "x86_64" };
    run(Command::new(tools.join("aapt"))
        .current_dir(&work)
        .args(["add", "unsigned.apk", "classes.dex"])
        .arg(format!("lib/{abi}/libquiet_launcher.so")))?;
    run(Command::new(tools.join("zipalign"))
        .args(["-f", "-p", "4"])
        .arg(work.join("unsigned.apk"))
        .arg(work.join("aligned.apk")))?;
    run(Command::new(tools.join("apksigner"))
        .args(["sign", "--ks"])
        .arg(std::env::var("CARGO_APK_RELEASE_KEYSTORE").context("release signing key")?)
        .args([
            "--ks-pass",
            "env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD",
            "--out",
        ])
        .arg(&apk)
        .arg(work.join("aligned.apk")))?;
    run(Command::new(tools.join("apksigner"))
        .args(["verify"])
        .arg(&apk))?;
    if !arm {
        stamp(&root, &root.join("target/v1-sut-build-inputs.sha256"))?;
    }
    println!("{}", apk.display());
    Ok(())
}
