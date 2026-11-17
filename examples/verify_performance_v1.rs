//! Validate collected physical benchmark records. No evidence means failure, never pass.
#[path = "../tests/support/performance.rs"]
mod gate;
fn main() -> anyhow::Result<()> {
    let path=std::env::args_os().nth(1).ok_or_else(||anyhow::anyhow!("usage: verify_performance_v1 measured-evidence.json; requires real reference-phone data"))?;
    let e: gate::Evidence = serde_json::from_slice(&std::fs::read(path)?)?;
    gate::validate(&e)?;
    println!("Measured records meet v1 numerical thresholds. Raw trace attribution must also be verified; this command does not collect data.");
    Ok(())
}
