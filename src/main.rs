fn main() -> eframe::Result {
    let path = std::env::var_os("QUIET_SETTINGS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("quiet-preview/settings.json"));
    eframe::run_native(
        "Quiet — desktop preview",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default().with_inner_size([420.0, 860.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(quiet_launcher::ui::Launcher::new(
                cc,
                std::sync::Arc::new(quiet_launcher::platform::desktop::Desktop),
                path,
            )))
        }),
    )
}
