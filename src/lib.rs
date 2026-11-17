pub mod catalog;
pub mod config;
pub mod home;
pub mod media;
pub mod model;
pub mod platform;
#[cfg(feature = "ui")]
mod recovery;
pub mod runtime;
pub mod search;
mod storage;
pub mod usage;
pub mod weather;

#[cfg(feature = "ui")]
mod media_ui;
#[cfg(feature = "ui")]
pub mod mobile;
#[cfg(feature = "ui")]
pub mod ui;

#[cfg(all(target_os = "android", feature = "ui"))]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    use std::sync::Arc;
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    let path = app
        .internal_data_path()
        .expect("Android internal data directory")
        .join("settings.json");
    let scheduler_app = app.clone();
    let scheduler: platform::android::MainThread = Arc::new(move |task| {
        scheduler_app.run_on_java_main_thread(Box::new(task));
    });
    let insets_app = app.clone();
    let insets: platform::android::WindowInsets = Arc::new(move || {
        let Some(window) = insets_app.native_window() else {
            return [0.0; 4];
        };
        let rect = insets_app.content_rect();
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return [0.0; 4];
        }
        [
            rect.left.max(0) as f32,
            rect.top.max(0) as f32,
            (window.width() - rect.right).max(0) as f32,
            (window.height() - rect.bottom).max(0) as f32,
        ]
    });
    // AndroidApp owns these references for the lifetime of this entry point.
    let platform = unsafe {
        platform::android::AndroidPlatform::new(
            app.vm_as_ptr(),
            app.activity_as_ptr(),
            scheduler,
            insets,
        )
    };
    let platform = match platform {
        Ok(platform) => Arc::new(platform),
        Err(error) => {
            log::error!("Android initialization: {error:#}");
            return;
        }
    };
    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "Quiet",
        options,
        Box::new(move |cc| Ok(Box::new(ui::Launcher::new(cc, platform, path)))),
    ) {
        log::error!("Renderer: {error}");
    }
}
