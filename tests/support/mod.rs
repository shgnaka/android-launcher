#![allow(dead_code)]
use quiet_launcher::model::AppEntry;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "quiet-v1-{}-{}-{name}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    pub fn settings(&self) -> PathBuf {
        self.0.join("settings.json")
    }
    pub fn write(&self, bytes: &[u8]) -> PathBuf {
        let p = self.settings();
        std::fs::write(&p, bytes).unwrap();
        p
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub fn app(id: &str, label: &str) -> AppEntry {
    AppEntry {
        id: id.into(),
        label: label.into(),
    }
}
pub fn apps() -> Vec<AppEntry> {
    vec![
        app("example.camera/Main", "Camera"),
        app("example.camera/Tools", "Camera Tools"),
        app("example.other/Main", "Camera"),
        app("example.notes/Main", "メモ"),
    ]
}
pub fn ids(apps: Vec<&AppEntry>) -> Vec<String> {
    apps.into_iter().map(|a| a.id.clone()).collect()
}
pub const BASIC: &str = r#"{"favorites":[],"hidden_packages":[],"widget_ids":[],"font_percent":100,"show_weather":false,"show_usage":false,"show_media":false,"dark":false}"#;
#[cfg(feature = "ui")]
pub mod screen;

#[cfg(feature = "ui")]
pub mod controlled;
#[cfg(feature = "ui")]
pub mod gated;
