//! Tests the benchmark judge with deliberately valid and invalid records; not phone performance.
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
#[test]
fn exact_input_threshold_and_complete_periods_are_valid() {
    gate::trial(&valid()).unwrap();
}
#[test]
fn missing_periods_are_in_the_failure_denominator() {
    let mut t = valid();
    for f in t.frames.iter_mut().take(31) {
        f.presented_ns = None;
    }
    assert!(gate::trial(&t).is_err());
}
#[test]
fn an_empty_capture_is_not_a_performance_success() {
    let mut t = valid();
    t.frames.clear();
    assert!(gate::trial(&t).is_err());
}
#[test]
fn one_hundred_ms_input_p95_is_not_rounded_down() {
    let mut t = valid();
    t.input_ns = vec![100_000_001; 20];
    assert!(gate::trial(&t).is_err());
}
#[test]
fn ten_thousand_results_have_their_separate_300ms_budget() {
    let mut t = valid();
    t.app_count = 10000;
    t.result_ns = vec![300_000_000; 20];
    gate::trial(&t).unwrap();
    t.result_ns = vec![300_000_001; 20];
    assert!(gate::trial(&t).is_err());
}
#[test]
fn slow_ten_thousand_input_is_not_excused_by_result_budget() {
    let mut t = valid();
    t.app_count = 10000;
    t.input_ns = vec![101_000_000; 20];
    t.result_ns = vec![200_000_000; 20];
    assert!(gate::trial(&t).is_err());
}
#[test]
fn five_percent_missing_with_a_long_stall_is_still_a_failure() {
    let mut t = valid();
    for f in t.frames.iter_mut().skip(30).take(7) {
        f.presented_ns = None;
    }
    assert!(gate::trial(&t).is_err());
}
#[test]
fn emulator_and_missing_trials_are_never_promoted_to_phone_success() {
    let e = Evidence {
        physical_device: false,
        model: "OPPO Reno7 A".into(),
        android_api: 33,
        release: true,
        abi: "arm64-v8a".into(),
        source: "perfetto".into(),
        apk_sha256: "a".repeat(64),
        trace_sha256: "b".repeat(64),
        trials: vec![valid()],
        cache_retained_bytes: 1,
        cache_peak_bytes: 2,
        total_pss_bytes: 100,
    };
    assert!(gate::validate(&e).is_err());
}
#[test]
fn a_long_stall_at_the_start_is_not_hidden_by_missing_initial_frames() {
    let mut t = valid();
    for f in t.frames.iter_mut().take(6) {
        f.presented_ns = None;
    }
    assert!(gate::trial(&t).is_err());
}
#[test]
fn ninety_hz_claim_with_sixty_hz_periods_is_rejected() {
    let mut t = valid();
    t.hz = 90;
    t.frames = (1..=900)
        .map(|i| Frame {
            deadline_ns: i * 16_666_667,
            presented_ns: Some(i * 16_666_667),
        })
        .collect();
    assert!(gate::trial(&t).is_err());
}
