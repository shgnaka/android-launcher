//! Verify both the original frozen acceptance baseline and the v1 additions.
use std::{collections::BTreeMap, path::Path, process::Command};
fn digest(path: &str) -> String {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum");
    assert!(out.status.success(), "cannot hash {path}");
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
fn main() {
    let legacy = Command::new("cargo")
        .args([
            "run",
            "--offline",
            "--no-default-features",
            "--example",
            "check_contracts",
        ])
        .status()
        .expect("original freeze guard");
    assert!(legacy.success(), "original frozen tests changed");
    for manifest in [
        "tests/frozen-v1.sha256.json",
        "tests/frozen-v1-infrastructure.sha256.json",
    ] {
        assert!(
            Path::new(manifest).is_file(),
            "freeze manifest missing: {manifest}"
        );
        let files: BTreeMap<String, String> =
            serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
        for (p, h) in files {
            assert_eq!(digest(&p), h, "frozen v1 input changed: {p}");
        }
    }
    let status = Command::new("sha256sum")
        .args(["-c", "docs/requirements-v1.sha256"])
        .status()
        .unwrap();
    assert!(status.success(), "normative v1 requirements changed");
    println!("Original and v1 tests/helpers/fixtures, and normative requirements are unchanged.");
}
