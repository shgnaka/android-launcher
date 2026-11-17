use crate::model::AppEntry;
use anyhow::Result;
use std::{path::Path, sync::Arc};
#[derive(Default)]
pub struct AppCatalog {
    apps: Arc<Vec<AppEntry>>,
    revision: Option<String>,
}
impl AppCatalog {
    pub fn invalidate(&mut self) {
        self.revision = None;
    }
    pub fn load(path: &Path) -> Result<Self> {
        #[derive(serde::Deserialize)]
        struct Stored {
            version: u8,
            revision: String,
            apps: Vec<AppEntry>,
        }
        anyhow::ensure!(
            std::fs::metadata(path)?.len() <= 8 * 1024 * 1024,
            "アプリキャッシュが大きすぎます"
        );
        let data: Stored = serde_json::from_slice(&std::fs::read(path)?)?;
        anyhow::ensure!(
            data.version == 1 && data.apps.len() <= 50_000,
            "アプリキャッシュの形式が不正です"
        );
        let mut ids = std::collections::HashSet::new();
        anyhow::ensure!(
            data.apps
                .iter()
                .all(|app| !app.id.is_empty() && ids.insert(&app.id)),
            "起動先の識別子が不正です"
        );
        Ok(Self {
            apps: Arc::new(data.apps),
            revision: Some(data.revision),
        })
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        #[derive(serde::Serialize)]
        struct Stored<'a> {
            version: u8,
            revision: &'a str,
            apps: &'a [AppEntry],
        }
        let revision = self
            .revision
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("未取得の一覧は保存できません"))?;
        let bytes = serde_json::to_vec(&Stored {
            version: 1,
            revision,
            apps: &self.apps,
        })?;
        crate::storage::atomic_write(path, &bytes)
    }
    pub fn apps(&self) -> Arc<Vec<AppEntry>> {
        self.apps.clone()
    }
    pub fn refresh(
        &mut self,
        revision: &str,
        source: impl FnOnce() -> Result<Vec<AppEntry>>,
    ) -> Result<bool> {
        if self.revision.as_deref() == Some(revision) {
            return Ok(false);
        }
        let apps = source()?;
        self.apps = Arc::new(apps);
        self.revision = Some(revision.into());
        Ok(true)
    }
}
