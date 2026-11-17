//! Completeness and memory-evidence tests for the benchmark judge.
#[path = "support/performance.rs"]
mod gate;
use gate::*;
fn valid() -> Trial {
    Trial {
        number: 1,
        app_count: 1000,
        hz: 60,
        mode: "normal".into(),
        frames: (1..=600)
            .map(|i| Frame {
                deadline_ns: i * 16_666_667,
                presented_ns: Some(i * 16_666_667),
            })
            .collect(),
        input_ns: vec![100_000_000; 20],
        result_ns: vec![100_000_000; 20],
    }
}
fn full_evidence() -> Evidence {
    let mut trials = vec![];
    for app_count in [100, 1000, 10000] {
        for hz in [60, 90] {
            for mode in ["normal", "dependency-wait"] {
                for number in 1..=5 {
                    let mut t = valid();
                    t.app_count = app_count;
                    t.hz = hz;
                    t.number = number;
                    t.mode = mode.into();
                    let period = 1_000_000_000 / hz as u64;
                    t.frames = (1..=hz as u64 * 10)
                        .map(|i| Frame {
                            deadline_ns: i * period,
                            presented_ns: Some(i * period),
                        })
                        .collect();
                    trials.push(t);
                }
            }
        }
    }
    Evidence {
        physical_device: true,
        model: "OPPO Reno7 A".into(),
        android_api: 33,
        release: true,
        abi: "arm64-v8a".into(),
        source: "perfetto".into(),
        apk_sha256: "a".repeat(64),
        trace_sha256: "b".repeat(64),
        trials,
        cache_retained_bytes: 1024,
        cache_peak_bytes: 2048,
        total_pss_bytes: 4096,
    }
}
#[test]
fn every_required_mode_and_repetition_must_be_present() {
    let mut e = full_evidence();
    gate::validate(&e).unwrap();
    e.trials.pop();
    assert!(gate::validate(&e).is_err());
}
#[test]
fn an_extra_bad_trial_must_not_be_excluded_as_unrecognized() {
    let mut e = full_evidence();
    let mut t = valid();
    t.number = 6;
    t.input_ns = vec![900_000_000; 20];
    e.trials.push(t);
    assert!(gate::validate(&e).is_err());
}
#[test]
fn zero_component_memory_is_missing_evidence_rather_than_perfect_memory_usage() {
    let mut e = full_evidence();
    e.cache_retained_bytes = 0;
    e.cache_peak_bytes = 0;
    assert!(gate::validate(&e).is_err());
}
