use serde::{Deserialize, Serialize};

pub type WidgetNames = std::collections::BTreeMap<i32, String>;

/// Native inline search field positioned in physical screen pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchField {
    pub rect: [i32; 4],
    pub text: String,
    pub dark: bool,
    pub focus: bool,
    pub font_percent: u16,
}

pub fn widget_title(id: i32, names: &WidgetNames, resolved: Option<&str>) -> String {
    resolved
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            names
                .get(&id)
                .map(String::as_str)
                .filter(|name| !name.trim().is_empty())
        })
        .map(str::to_owned)
        .unwrap_or_else(|| format!("未連携 ({id})"))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    /// Flattened Android ComponentName. Multiple launcher activities stay distinct.
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct DeviceSnapshot {
    pub battery_percent: Option<u8>,
    pub screen_seconds: Option<u64>,
    pub music_active: bool,
}

#[derive(Clone, Debug)]
pub struct WidgetProvider {
    pub component: String,
    pub label: String,
}

#[derive(Clone, Debug)]
/// Text extracted from another app's Android widget.
pub struct TextWidget {
    pub id: i32,
    pub title: String,
    pub lines: Vec<String>,
    pub pending: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn widget_names_survive_pending_and_failed_content_reads() {
        let names = WidgetNames::from([(7, "カレンダー".into())]);
        assert_eq!(widget_title(7, &names, None), "カレンダー");
        assert_eq!(widget_title(7, &names, Some("")), "カレンダー");
        assert_eq!(widget_title(7, &names, Some("予定表")), "予定表");
        assert_eq!(widget_title(9, &names, None), "未連携 (9)");
    }
}

#[derive(Clone, Copy)]
pub enum MediaCommand {
    Previous,
    PlayPause,
    Next,
}

#[derive(Clone, Copy)]
pub enum SystemSettings {
    Home,
    Usage,
}

/// Identifies one independently controlled external playback session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaSession {
    pub id: String,
    pub package: String,
    pub provider: String,
    pub title: Option<String>,
    pub state: Playback,
    pub previous: bool,
    pub play: bool,
    pub pause: bool,
    pub next: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Playback {
    Playing,
    Paused,
    Buffering,
    Connecting,
    Stopped,
    None,
    Error,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaAction {
    Previous,
    Play,
    Pause,
    Next,
}
