//! The physical benchmark evidence gate. Sample validation is separate from data collection.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Frame {
    pub deadline_ns: u64,
    pub presented_ns: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Trial {
    pub number: u8,
    pub app_count: u32,
    pub hz: u32,
    pub mode: String,
    pub frames: Vec<Frame>,
    pub input_ns: Vec<u64>,
    pub result_ns: Vec<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub physical_device: bool,
    pub model: String,
    pub android_api: u32,
    pub release: bool,
    pub abi: String,
    pub source: String,
    pub apk_sha256: String,
    pub trace_sha256: String,
    pub trials: Vec<Trial>,
    pub cache_retained_bytes: u64,
    pub cache_peak_bytes: u64,
    pub total_pss_bytes: u64,
}
fn p95(a: &[u64]) -> Result<u64> {
    ensure!(a.len() >= 20, "need at least 20 input samples per trial");
    let mut a = a.to_vec();
    a.sort_unstable();
    Ok(a[(95 * a.len()).div_ceil(100) - 1])
}
pub fn trial(t: &Trial) -> Result<()> {
    ensure!([60, 90].contains(&t.hz), "unverified refresh rate");
    ensure!(
        [100, 1000, 10000].contains(&t.app_count),
        "wrong dataset size"
    );
    ensure!(p95(&t.input_ns)? <= 100_000_000, "input p95 over 100 ms");
    ensure!(
        p95(&t.result_ns)?
            <= if t.app_count == 10000 {
                300_000_000
            } else {
                100_000_000
            },
        "result p95 exceeds contract"
    );
    ensure!(!t.frames.is_empty(), "missing requested frame periods");
    ensure!(
        t.frames
            .windows(2)
            .all(|f| f[1].deadline_ns > f[0].deadline_ns),
        "frame periods are not strictly ordered"
    );
    if t.app_count != 10000 {
        ensure!(
            t.frames.len() >= t.hz as usize * 10,
            "ten-second requested-period denominator is incomplete"
        );
        let good = t
            .frames
            .iter()
            .filter(|f| f.presented_ns.is_some_and(|p| p <= f.deadline_ns))
            .count();
        ensure!(
            good * 100 >= t.frames.len() * 95,
            "over 5% of periods missed presentation, including missing frames"
        );
        let period = 1_000_000_000u64 / t.hz as u64;
        let first = t.frames.first().unwrap().deadline_ns;
        let span = t.frames.last().unwrap().deadline_ns - first + period;
        ensure!(
            span.abs_diff(10_000_000_000) <= period,
            "recorded periods do not represent ten seconds at the claimed refresh rate"
        );
        let mut last = Some(first.saturating_sub(period));
        for f in &t.frames {
            if let Some(p) = f.presented_ns {
                ensure!(
                    last.is_none_or(|old| p >= old),
                    "presentations are not monotonic"
                );
                if let Some(old) = last {
                    ensure!(p - old < 100_000_000, "100 ms display stall")
                }
                last = Some(p);
            } else if let Some(old) = last {
                ensure!(
                    f.deadline_ns.saturating_sub(old) < 100_000_000,
                    "missing presentations caused a 100 ms stall"
                );
            }
        }
    }
    Ok(())
}
pub fn validate(e: &Evidence) -> Result<()> {
    ensure!(
        e.physical_device
            && e.model == "OPPO Reno7 A"
            && e.android_api == 33
            && e.release
            && e.abi == "arm64-v8a",
        "not the release build on the reference physical phone"
    );
    ensure!(
        e.source == "perfetto"
            && [&e.apk_sha256, &e.trace_sha256]
                .iter()
                .all(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())),
        "missing measured-source provenance"
    );
    ensure!(
        e.cache_retained_bytes > 0
            && e.cache_peak_bytes >= e.cache_retained_bytes
            && e.cache_retained_bytes <= 8 * 1024 * 1024
            && e.cache_peak_bytes <= 16 * 1024 * 1024
            && e.total_pss_bytes > 0,
        "invalid component/PSS memory evidence"
    );
    ensure!(
        e.trials.len() == 60,
        "unexpected trial count; no extra bad trial may be dropped"
    );
    for n in [100, 1000, 10000] {
        for hz in [60, 90] {
            for mode in ["normal", "dependency-wait"] {
                for number in 1..=5 {
                    let matches: Vec<_> = e
                        .trials
                        .iter()
                        .filter(|t| {
                            t.app_count == n && t.hz == hz && t.mode == mode && t.number == number
                        })
                        .collect();
                    ensure!(matches.len()==1,"missing or duplicate trial: apps={n}, hz={hz}, mode={mode}, number={number}");
                    trial(matches[0])?;
                }
            }
        }
    }
    Ok(())
}
