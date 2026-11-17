//! Verify the original freeze and the additional reviewed observers.
use std::{collections::BTreeMap, process::Command};
fn main() {
    let old = Command::new("cargo")
        .args([
            "run",
            "--offline",
            "--no-default-features",
            "--example",
            "check_v1_contracts",
        ])
        .status()
        .expect("original guard");
    assert!(old.success(), "original requirements or tests changed");
    let files: BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read("tests/frozen-review-v1_1.sha256.json").unwrap())
            .unwrap();
    for (path, expected) in files {
        let result = Command::new("sha256sum").arg(&path).output().unwrap();
        assert!(result.status.success(), "cannot read {path}");
        let actual = String::from_utf8(result.stdout).unwrap();
        assert_eq!(
            actual.split_whitespace().next().unwrap(),
            expected,
            "reviewed test changed: {path}"
        );
    }
    let recovery = Command::new("sha256sum")
        .args(["-c", "tests/frozen-recovery-review.sha256"])
        .status()
        .unwrap();
    assert!(recovery.success(), "reviewed recovery tests changed");
    println!("Original tests, requirements, and reviewed observer tests are unchanged.");
}
