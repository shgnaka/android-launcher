use crate::model::*;
use anyhow::Result;

#[cfg(any(target_os = "android", feature = "android-check"))]
pub mod android;
#[cfg(not(target_os = "android"))]
pub mod desktop;

/// Android services and UI are isolated from search, persistence and rendering.
pub trait Platform: Send + Sync {
    fn apps(&self) -> Result<Vec<AppEntry>>;
    fn catalog_revision(&self) -> Result<Option<String>> {
        Ok(None)
    }
    fn snapshot(&self) -> Result<DeviceSnapshot>;
    fn snapshot_range(&self, _last_24h: bool) -> Result<DeviceSnapshot> {
        self.snapshot()
    }
    fn launch(&self, id: &str) -> Result<()>;
    fn settings(&self, settings: SystemSettings) -> Result<()>;
    fn media(&self, command: MediaCommand) -> Result<()>;
    fn media_access_settings(&self) -> Result<()> {
        anyhow::bail!("通知アクセス設定を開けません")
    }
    fn media_sessions(&self) -> Result<Vec<MediaSession>> {
        anyhow::bail!("メディアセッションを取得できません")
    }
    fn control_session(&self, _id: &str, _action: MediaAction) -> Result<()> {
        anyhow::bail!("メディアセッションを操作できません")
    }
    fn take_cancelled_widgets(&self) -> Vec<i32> {
        Vec::new()
    }
    fn widget_providers(&self) -> Result<Vec<WidgetProvider>>;
    fn add_widget(&self, provider: &str) -> Result<i32>;
    fn remove_widget(&self, id: i32) -> Result<()>;
    fn poll_widgets(&self, ids: &[i32], names: &WidgetNames);
    fn widgets(&self) -> Vec<TextWidget>;
    fn try_widgets(&self) -> Option<Vec<TextWidget>> {
        Some(self.widgets())
    }
    /// Physical-pixel safe drawing insets, including Android system bars/cutouts.
    fn window_insets(&self) -> [f32; 4] {
        [0.0; 4]
    }
    fn set_dark_theme(&self, _dark: bool) {}
    fn request_text(&self, key: &str, title: &str, initial: &str) -> Result<()>;
    fn set_search_field(&self, _field: Option<SearchField>) {}
    fn poll_search_field(&self) {}
    fn dismiss_keyboard(&self) {}
    fn keyboard_inset(&self) -> f32 {
        0.0
    }
    fn take_text(&self) -> Option<(String, String)>;
    fn is_android(&self) -> bool {
        false
    }
    fn take_error(&self) -> Option<String> {
        None
    }
}
