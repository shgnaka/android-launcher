//! The real phone's numerical acceptance gate. Lack of evidence is an error, not a skip.
#[path = "support/performance.rs"]
mod gate;
#[test]
fn a36_a47_reference_phone_measurements_are_present_and_satisfy_the_frozen_budget() {
    let path = std::env::var_os("QUIET_PERFORMANCE_EVIDENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/v1-reference-phone/evidence.json".into());
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "ENVIRONMENT: reference OPPO Reno7 A Android 13 benchmark evidence required at {}: {e}",
            path.display()
        )
    });
    let evidence: gate::Evidence =
        serde_json::from_slice(&bytes).expect("actual measurement schema");
    let trace = path.parent().unwrap().join("trace.pftrace");
    let result = std::process::Command::new("sha256sum")
        .arg(&trace)
        .output()
        .expect("raw trace hash");
    assert!(
        result.status.success(),
        "ENVIRONMENT: raw Perfetto trace missing"
    );
    let digest = String::from_utf8(result.stdout).unwrap();
    assert_eq!(
        digest.split_whitespace().next().unwrap(),
        evidence.trace_sha256,
        "raw trace must match reported provenance"
    );
    gate::validate(&evidence)
        .expect("real reference-phone measurements must satisfy the immutable v1 contract");
}
