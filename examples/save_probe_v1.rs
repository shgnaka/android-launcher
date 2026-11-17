//! Separate process invoking the real settings writer for OS-level failure/crash tests.
use quiet_launcher::config::Settings;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let settings: Settings = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    match settings.save(std::path::Path::new(&args[1])) {
        Ok(()) => println!("SAVE_OK"),
        Err(e) => {
            eprintln!("SAVE_ERR: {e:#}");
            std::process::exit(7);
        }
    }
}
