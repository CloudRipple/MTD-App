use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::{
    config::DEFAULT_MODEL,
    platform::{default_output_dir, hide_command_window},
};

const APP_DIR: &str = ".mtd-subtitle-app";
const SETTINGS_FILE: &str = "app-settings.json";

#[derive(Clone, Debug)]
pub(crate) struct AppSettings {
    pub(crate) output_dir: PathBuf,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) include_speaker: bool,
    pub(crate) subtitle_font: Option<String>,
    pub(crate) subtitle_font_size: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            model: DEFAULT_MODEL.to_owned(),
            max_tokens: 48_000,
            include_speaker: true,
            subtitle_font: None,
            subtitle_font_size: 24,
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
        output_dir: value
            .get("output_dir")
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_output_dir),
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
    })
}

pub(crate) fn save_app_settings(settings: &AppSettings) -> Result<()> {
    let dir = ensure_app_dir()?;
    let path = dir.join(SETTINGS_FILE);
    let temp_path = dir.join(format!("{SETTINGS_FILE}.tmp"));
    let payload = json!({
        "output_dir": settings.output_dir.display().to_string(),
        "model": settings.model,
        "max_tokens": settings.max_tokens,
        "include_speaker": settings.include_speaker,
        "subtitle_font": settings.subtitle_font,
        "subtitle_font_size": settings.subtitle_font_size,
    });
    write_private_file(&temp_path, serde_json::to_vec_pretty(&payload)?)?;
    fs::rename(&temp_path, &path)
        .with_context(|| format!("保存应用设置失败：{}", path.display()))?;
    set_private_file_permissions(&path)?;
    Ok(())
}

fn settings_path() -> Result<PathBuf> {
    Ok(app_dir()?.join(SETTINGS_FILE))
}

fn ensure_app_dir() -> Result<PathBuf> {
    let dir = app_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("创建配置目录失败：{}", dir.display()))?;
    set_private_dir_permissions(&dir)?;
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("attrib");
        hide_command_window(&mut command);
        let _ = command.arg("+h").arg(&dir).status();
    }
    Ok(dir)
}

fn app_dir() -> Result<PathBuf> {
    home_dir()
        .map(|home| home.join(APP_DIR))
        .ok_or_else(|| anyhow!("无法定位用户目录，不能保存应用设置"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
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

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("设置目录权限失败：{}", path.display()))
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("设置文件权限失败：{}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
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
    }
}
