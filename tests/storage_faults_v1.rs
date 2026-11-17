//! A33: real filesystem syscalls fail or the writer process dies at each commit boundary.
//! This proves process-crash consistency on the host; it does not simulate physical power loss.
#![cfg(all(target_os = "linux", target_env = "gnu"))]
mod support;
use quiet_launcher::config::Settings;
use std::{path::PathBuf, process::Command, sync::OnceLock};
use support::Directory;
static TOOLS: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
fn tools() -> &'static (PathBuf, PathBuf) {
    TOOLS.get_or_init(|| {
        let root = std::env::current_dir().unwrap();
        let out = root.join("target/v1-storage-tools");
        std::fs::create_dir_all(&out).unwrap();
        let library = out.join("storage_fault.so");
        let result = Command::new("rustc")
            .args([
                "--edition=2021",
                "--crate-type=cdylib",
                "tests/fixtures/v1/storage_fault.rs",
                "-o",
            ])
            .arg(&library)
            .output()
            .expect("ENVIRONMENT: rustc required for failure fixture");
        assert!(
            result.status.success(),
            "ENVIRONMENT: failure fixture did not compile: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let build = Command::new("cargo")
            .args([
                "build",
                "--offline",
                "--no-default-features",
                "--example",
                "save_probe_v1",
            ])
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "ENVIRONMENT: probe did not compile: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        (library, root.join("target/debug/examples/save_probe_v1"))
    })
}
fn fault(stage: &str, action: &str) {
    let (library, probe) = tools();
    let dir = Directory::new("crash");
    let old = br#"{"favorites":["old/Main"],"hidden_packages":["secret"],"dark":false}"#;
    let older = br#"{"favorites":["older/Main"],"hidden_packages":["secret"],"dark":false}"#;
    dir.write(old);
    std::fs::write(dir.settings().with_extension("json.bak"), older).unwrap();
    let candidate = dir.0.join("candidate.json");
    std::fs::write(
        &candidate,
        br#"{"favorites":["new/Main","old/Main"],"hidden_packages":["secret","new"],"dark":true}"#,
    )
    .unwrap();
    let result = Command::new(probe)
        .arg(dir.settings())
        .arg(candidate)
        .env("LD_PRELOAD", library)
        .env("QUIET_FAULT_ROOT", &dir.0)
        .env("QUIET_FAULT_STAGE", stage)
        .env("QUIET_FAULT_ACTION", action)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains(&format!("FAULT_FIRED:{stage}")),
        "ENVIRONMENT: injection point was not reached: {stderr}"
    );
    assert!(
        !result.status.success(),
        "failed operation must never report save success"
    );
    if action == "fail" {
        assert_eq!(
            result.status.code(),
            Some(7),
            "save must return an error: {stderr}"
        );
    } else {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(result.status.signal(), Some(9));
    }
    let loaded =
        Settings::load(&dir.settings()).expect("primary must remain complete after failure/crash");
    if stage == "primary.directory_after_replace" {
        assert_eq!(loaded.favorites, vec!["new/Main", "old/Main"]);
        assert!(loaded.dark && loaded.is_hidden("new"));
    } else {
        assert_eq!(std::fs::read(dir.settings()).unwrap(), old);
        assert_eq!(loaded.favorites, vec!["old/Main"]);
        assert!(!loaded.dark && !loaded.is_hidden("new"));
    }
    assert!(loaded.is_hidden("secret"));
    let backup = std::fs::read(dir.settings().with_extension("json.bak")).unwrap();
    assert!(
        backup == older || backup == old,
        "backup cannot contain a partial or uncommitted candidate"
    );
    // A fresh process can retry after any interrupted save, without changing the test expectation.
    let mut retry = loaded;
    retry.dark = true;
    retry.save(&dir.settings()).unwrap();
    assert!(Settings::load(&dir.settings()).unwrap().dark);
}
macro_rules! cases {
    ($fail:ident,$kill:ident,$stage:literal) => {
        #[test]
        fn $fail() {
            fault($stage, "fail");
        }
        #[test]
        fn $kill() {
            fault($stage, "kill");
        }
    };
}
cases!(backup_write_error, backup_write_crash, "backup.write");
cases!(
    backup_file_sync_error,
    backup_file_sync_crash,
    "backup.file_sync"
);
cases!(
    backup_directory_before_error,
    backup_directory_before_crash,
    "backup.directory_before_replace"
);
cases!(backup_replace_error, backup_replace_crash, "backup.replace");
cases!(
    backup_directory_after_error,
    backup_directory_after_crash,
    "backup.directory_after_replace"
);
cases!(primary_write_error, primary_write_crash, "primary.write");
cases!(
    primary_file_sync_error,
    primary_file_sync_crash,
    "primary.file_sync"
);
cases!(
    primary_directory_before_error,
    primary_directory_before_crash,
    "primary.directory_before_replace"
);
cases!(
    primary_replace_error,
    primary_replace_crash,
    "primary.replace"
);
cases!(
    primary_directory_after_error,
    primary_directory_after_crash,
    "primary.directory_after_replace"
);
