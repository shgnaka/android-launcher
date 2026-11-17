//! Explicit reset: archive damaged inputs before writing a new complete configuration.
use crate::config::Settings;
use anyhow::{Context, Result};
use std::{fs::OpenOptions, io::Write, path::Path};

fn archive(path: &Path, suffix: &str) -> Result<()> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for index in 0.. {
        let extension = if index == 0 {
            suffix.to_owned()
        } else {
            format!("{suffix}.{index}")
        };
        let destination = path.with_extension(extension);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(mut file) => {
                file.write_all(&bytes)?;
                file.sync_all()?;
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}

pub(crate) fn reset(path: &Path) -> Result<Settings> {
    let _guard = crate::storage::WRITE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("保存処理を利用できません"))?;
    archive(path, "json.corrupt").context("元の設定を退避できません")?;
    let backup = path.with_extension("json.bak");
    archive(&backup, "json.bak.corrupt").context("バックアップを退避できません")?;
    let settings = Settings::default();
    let data = serde_json::to_vec_pretty(&settings)?;
    crate::storage::atomic_write(&backup, &data)?;
    crate::storage::atomic_write(path, &data)?;
    Ok(settings)
}
