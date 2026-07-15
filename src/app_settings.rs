use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::{
    app_data::{
        app_data_file_for_read, default_project_root, ensure_app_data_dir,
        set_private_file_permissions,
    },
    config::DEFAULT_MODEL,
};

const SETTINGS_FILE: &str = "app-settings.json";

#[derive(Clone, Debug)]
pub(crate) struct AppSettings {
    pub(crate) new_project_root: PathBuf,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) include_speaker: bool,
    pub(crate) subtitle_font: Option<String>,
    pub(crate) subtitle_font_size: u32,
    pub(crate) recent_projects: Vec<RecentProject>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecentProject {
    pub(crate) path: PathBuf,
    pub(crate) opened_at: u64,
    pub(crate) status: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            new_project_root: default_project_root(),
            model: DEFAULT_MODEL.to_owned(),
            max_tokens: 48_000,
            include_speaker: true,
            subtitle_font: None,
            subtitle_font_size: 24,
            recent_projects: Vec::new(),
        }
    }
}

pub(crate) fn load_app_settings() -> Result<AppSettings> {
    let path = settings_path()?;
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppSettings::default());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("读取应用设置失败：{}", path.display()));
        }
    };
    let value: Value = serde_json::from_str(&content).context("解析应用设置失败")?;
    Ok(AppSettings {
        new_project_root: value
            .get("new_project_root")
            .or_else(|| value.get("output_dir"))
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_project_root),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .filter(|model| !model.trim().is_empty())
            .unwrap_or(DEFAULT_MODEL)
            .to_owned(),
        max_tokens: value
            .get("max_tokens")
            .and_then(Value::as_u64)
            .and_then(|tokens| u32::try_from(tokens).ok())
            .unwrap_or(48_000)
            .clamp(1_000, 96_000),
        include_speaker: value
            .get("include_speaker")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        subtitle_font: value
            .get("subtitle_font")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|font| !font.is_empty())
            .map(ToOwned::to_owned),
        subtitle_font_size: value
            .get("subtitle_font_size")
            .and_then(Value::as_u64)
            .and_then(|size| u32::try_from(size).ok())
            .unwrap_or(24)
            .clamp(12, 96),
        recent_projects: value
            .get("recent_projects")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let path = item
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|path| !path.trim().is_empty())?;
                Some(RecentProject {
                    path: PathBuf::from(path),
                    opened_at: item.get("opened_at").and_then(Value::as_u64).unwrap_or(0),
                    status: item
                        .get("status")
                        .and_then(Value::as_str)
                        .filter(|status| !status.trim().is_empty())
                        .unwrap_or("转写完成")
                        .to_owned(),
                })
            })
            .take(6)
            .collect(),
    })
}

pub(crate) fn save_app_settings(settings: &AppSettings) -> Result<()> {
    let dir = ensure_app_dir()?;
    let path = dir.join(SETTINGS_FILE);
    let temp_path = dir.join(format!("{SETTINGS_FILE}.tmp"));
    let payload = json!({
        "new_project_root": settings.new_project_root.display().to_string(),
        "model": settings.model,
        "max_tokens": settings.max_tokens,
        "include_speaker": settings.include_speaker,
        "subtitle_font": settings.subtitle_font,
        "subtitle_font_size": settings.subtitle_font_size,
        "recent_projects": settings.recent_projects.iter().map(|project| json!({
            "path": project.path.display().to_string(),
            "opened_at": project.opened_at,
            "status": project.status,
        })).collect::<Vec<_>>(),
    });
    write_private_file(&temp_path, serde_json::to_vec_pretty(&payload)?)?;
    fs::rename(&temp_path, &path)
        .with_context(|| format!("保存应用设置失败：{}", path.display()))?;
    set_private_file_permissions(&path)?;
    Ok(())
}

fn settings_path() -> Result<PathBuf> {
    app_data_file_for_read(SETTINGS_FILE)
}

fn ensure_app_dir() -> Result<PathBuf> {
    ensure_app_data_dir()
}

fn write_private_file(path: &Path, bytes: Vec<u8>) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("创建设置文件失败：{}", path.display()))?;
    set_private_file_permissions(path)?;
    file.write_all(&bytes)
        .with_context(|| format!("写入设置文件失败：{}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("同步设置文件失败：{}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AppSettings;
    use crate::config::DEFAULT_MODEL;

    #[test]
    fn default_settings_keep_transcription_ready_values() {
        let settings = AppSettings::default();

        assert_eq!(settings.model, DEFAULT_MODEL);
        assert_eq!(settings.max_tokens, 48_000);
        assert!(settings.include_speaker);
        assert!(settings.subtitle_font.is_none());
        assert_eq!(settings.subtitle_font_size, 24);
        assert!(settings.recent_projects.is_empty());
        assert_eq!(
            settings
                .new_project_root
                .file_name()
                .and_then(|name| name.to_str()),
            Some("MOSS-Subtitle-Workbench")
        );
    }
}
