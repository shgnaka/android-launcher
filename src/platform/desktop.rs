use super::*;

/// Desktop preview never presents invented device readings as real data.
pub struct Desktop;
impl Platform for Desktop {
    fn apps(&self) -> Result<Vec<AppEntry>> {
        Ok([
            ("preview/phone", "電話"),
            ("preview/messages", "メッセージ"),
            ("preview/camera", "カメラ"),
            ("preview/calendar", "カレンダー"),
            ("preview/settings", "設定"),
            ("preview/browser", "Browser"),
        ]
        .into_iter()
        .map(|(id, label)| AppEntry {
            id: id.into(),
            label: label.into(),
        })
        .collect())
    }
    fn snapshot(&self) -> Result<DeviceSnapshot> {
        Ok(DeviceSnapshot::default())
    }
    fn launch(&self, _: &str) -> Result<()> {
        anyhow::bail!("アプリの起動は Android 実機で利用できます")
    }
    fn settings(&self, _: SystemSettings) -> Result<()> {
        anyhow::bail!("Android 実機で設定してください")
    }
    fn media(&self, _: MediaCommand) -> Result<()> {
        anyhow::bail!("メディア操作は Android 実機で利用できます")
    }
    fn widget_providers(&self) -> Result<Vec<WidgetProvider>> {
        Ok(vec![])
    }
    fn add_widget(&self, _: &str) -> Result<i32> {
        anyhow::bail!("Android 実機で利用できます")
    }
    fn remove_widget(&self, _: i32) -> Result<()> {
        Ok(())
    }
    fn poll_widgets(&self, _: &[i32], _: &WidgetNames) {}
    fn widgets(&self) -> Vec<TextWidget> {
        vec![]
    }
    fn request_text(&self, _: &str, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    fn take_text(&self) -> Option<(String, String)> {
        None
    }
}
