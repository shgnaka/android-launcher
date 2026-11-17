use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SCHEMA_VERSION: u32 = 1;

fn legacy_schema_version() -> u32 {
    0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 0 is the unversioned legacy format; saves always emit [`SCHEMA_VERSION`].
    #[serde(default = "legacy_schema_version")]
    pub schema_version: u32,
    pub font_percent: u16,
    pub usage_last_24h: bool,
    pub hidden_packages: Vec<String>,
    pub search_readings: std::collections::BTreeMap<String, String>,
    pub favorites: Vec<String>,
    pub widget_ids: Vec<i32>,
    pub widget_names: crate::model::WidgetNames,
    pub weather_location: Option<crate::weather::Location>,
    pub show_weather: bool,
    pub show_usage: bool,
    pub show_media: bool,
    pub dark: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            font_percent: 100,
            usage_last_24h: false,
            hidden_packages: vec![],
            search_readings: Default::default(),
            favorites: vec![],
            widget_ids: vec![],
            widget_names: Default::default(),
            weather_location: None,
            show_weather: true,
            show_usage: true,
            show_media: true,
            dark: false,
        }
    }
}

fn is_component_id(id: &str) -> bool {
    id.split_once('/').is_some_and(|(package, activity)| {
        !package.is_empty() && !activity.is_empty() && !activity.contains('/')
    })
}

fn dedupe<T: Clone + PartialEq>(items: &mut Vec<T>) {
    let mut unique = Vec::with_capacity(items.len());
    for item in items.iter() {
        if !unique.contains(item) {
            unique.push(item.clone());
        }
    }
    *items = unique;
}

fn check_schema_version(value: &serde_json::Value) -> Result<()> {
    let Some(object) = value.as_object() else {
        bail!("設定ファイルの形式が不正です");
    };
    if let Some(version) = object.get("schema_version") {
        let version = version
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("設定形式の版が不正です"))?;
        if version > SCHEMA_VERSION as u64 {
            bail!("未対応の設定形式です（schema_version: {version}）");
        }
    }
    Ok(())
}

impl Settings {
    pub fn recover(path: &Path) -> Result<Self> {
        let _guard = crate::storage::WRITE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("保存処理を利用できません"))?;
        let backup = path.with_extension("json.bak");
        let restored = Self::load_existing(&backup)?;
        let bytes = restored.encoded()?;
        if path.exists() {
            let mut archive = path.with_extension("json.corrupt");
            let mut number = 0;
            while archive.exists() {
                number += 1;
                archive = path.with_extension(format!("json.corrupt.{number}"));
            }
            crate::storage::atomic_write(&archive, &std::fs::read(path)?)?;
        }
        crate::storage::atomic_write(path, &bytes)?;
        Ok(restored)
    }
    pub fn is_hidden(&self, id: &str) -> bool {
        let package = id.split('/').next().unwrap_or(id);
        self.hidden_packages.iter().any(|hidden| hidden == package)
    }
    pub fn set_hidden(&mut self, id: &str, hidden: bool) {
        let package = id.split('/').next().unwrap_or(id);
        if hidden && !package.is_empty() && !self.is_hidden(package) {
            self.hidden_packages.push(package.into());
        }
        if !hidden {
            self.hidden_packages.retain(|item| item != package);
        }
    }
    pub fn visible_favorites(&self) -> Vec<&str> {
        self.favorites
            .iter()
            .filter(|id| !self.is_hidden(id))
            .map(String::as_str)
            .collect()
    }
    pub fn move_visible_favorite(&mut self, id: &str, direction: i32) {
        let visible: Vec<usize> = self
            .favorites
            .iter()
            .enumerate()
            .filter(|(_, id)| !self.is_hidden(id))
            .map(|(i, _)| i)
            .collect();
        if let Some(index) = visible.iter().position(|&i| self.favorites[i] == id) {
            let target = index as i64 + direction.signum() as i64;
            if target >= 0 && (target as usize) < visible.len() {
                self.favorites
                    .swap(visible[index], visible[target as usize]);
            }
        }
    }
    fn normalized(&self) -> Self {
        let mut settings = self.clone();
        dedupe(&mut settings.favorites);
        dedupe(&mut settings.hidden_packages);
        dedupe(&mut settings.widget_ids);
        settings.search_readings = settings
            .search_readings
            .into_iter()
            .filter_map(|(id, reading)| {
                let reading = reading.trim().to_owned();
                (!reading.is_empty()).then_some((id, reading))
            })
            .collect();
        settings
    }
    fn encoded(&self) -> Result<Vec<u8>> {
        let mut settings = self.normalized();
        settings.schema_version = SCHEMA_VERSION;
        settings.validate()?;
        Ok(serde_json::to_vec_pretty(&settings)?)
    }
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.schema_version <= SCHEMA_VERSION,
            "未対応の設定形式です"
        );
        anyhow::ensure!(
            (80..=160).contains(&self.font_percent),
            "文字サイズは80〜160%で指定してください"
        );
        if let Some(location) = &self.weather_location {
            location.validate()?;
        }
        anyhow::ensure!(
            self.hidden_packages
                .iter()
                .all(|id| !id.is_empty() && !id.contains('/')),
            "非表示アプリの識別子が不正です"
        );
        anyhow::ensure!(
            self.favorites.iter().all(|id| is_component_id(id)),
            "ホーム登録の識別子が不正です"
        );
        for (id, reading) in &self.search_readings {
            anyhow::ensure!(is_component_id(id), "検索の読みの識別子が不正です");
            anyhow::ensure!(
                reading.trim().chars().count() <= 128,
                "検索の読みは128文字以内で指定してください"
            );
        }
        Ok(())
    }
    fn load_existing(path: &Path) -> Result<Self> {
        anyhow::ensure!(
            std::fs::metadata(path)?.len() <= 2 * 1024 * 1024,
            "設定ファイルが大きすぎます"
        );
        let bytes = std::fs::read(path)?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).context("設定ファイルを読み込めません")?;
        check_schema_version(&value)?;
        let settings: Self =
            serde_json::from_value(value).context("設定ファイルを読み込めません")?;
        let settings = settings.normalized();
        settings.validate()?;
        Ok(settings)
    }
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
            Ok(_) => Self::load_existing(path),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = self.encoded()?;
        let _guard = crate::storage::WRITE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("保存処理を利用できません"))?;
        let previous = if path.exists() {
            Self::load_existing(path)?;
            std::fs::read(path)?
        } else {
            bytes.clone()
        };
        crate::storage::atomic_write(&path.with_extension("json.bak"), &previous)?;
        crate::storage::atomic_write(path, &bytes).context("設定を保存できません")
    }
    pub fn toggle_favorite(&mut self, id: &str) {
        if let Some(index) = self.favorites.iter().position(|item| item == id) {
            self.favorites.remove(index);
        } else {
            self.favorites.push(id.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_round_trip_and_defaults() {
        let path = std::env::temp_dir().join(format!("quiet-settings-{}.json", std::process::id()));
        let mut settings = Settings::default();
        settings.toggle_favorite("pkg/Activity");
        settings.widget_ids.push(7);
        settings.widget_names.insert(7, "予定表".into());
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.favorites, settings.favorites);
        assert_eq!(loaded.widget_ids, vec![7]);
        assert_eq!(loaded.widget_names[&7], "予定表");
        assert!(serde_json::from_str::<Settings>("{}").unwrap().show_media);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn malformed_settings_are_reported() {
        assert!(serde_json::from_str::<Settings>("not json").is_err());
    }
    #[test]
    fn old_text_notes_are_ignored_without_losing_existing_settings() {
        let settings: Settings = serde_json::from_str(r#"{"favorites":["pkg/Activity"],"widget_ids":[42],"notes":[{"title":"メモ","text":"旧データ"}]}"#).unwrap();
        assert_eq!(settings.favorites, vec!["pkg/Activity"]);
        assert_eq!(settings.widget_ids, vec![42]);
        assert!(settings.widget_names.is_empty());
        let saved = serde_json::to_value(settings).unwrap();
        assert!(saved.get("notes").is_none());
    }
}
