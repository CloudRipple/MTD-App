use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use serde_json::json;

const APP_DIR: &str = ".mtd-subtitle-app";
const CREDENTIALS_FILE: &str = "credentials.json";
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "cn.mtd.subtitle-app.moss-api-key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApiKeyStorage {
    Keychain,
    HiddenFile,
}

impl ApiKeyStorage {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Keychain => "macOS Keychain",
            Self::HiddenFile => "~/.mtd-subtitle-app/credentials.json",
        }
    }
}

pub(crate) fn load_api_key() -> Result<Option<String>> {
    #[cfg(target_os = "macos")]
    if let Some(api_key) = load_keychain_api_key()? {
        return Ok(Some(api_key));
    }

    load_file_api_key()
}

pub(crate) fn save_api_key(api_key: &str) -> Result<ApiKeyStorage> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(anyhow!("API Key 为空，未保存"));
    }

    #[cfg(target_os = "macos")]
    if save_keychain_api_key(api_key).is_ok() {
        write_storage_marker(ApiKeyStorage::Keychain)?;
        return Ok(ApiKeyStorage::Keychain);
    }

    save_file_api_key(api_key)?;
    Ok(ApiKeyStorage::HiddenFile)
}

pub(crate) fn clear_api_key() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = delete_keychain_api_key();
    }

    let path = credentials_path()?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("删除保存的 API Key 失败：{}", path.display()))
        }
    }
}

fn load_file_api_key() -> Result<Option<String>> {
    let path = credentials_path()?;
    load_file_api_key_from(&path)
}

fn load_file_api_key_from(path: &PathBuf) -> Result<Option<String>> {
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("读取 API Key 文件失败：{}", path.display()));
        }
    };
    let value: serde_json::Value =
        serde_json::from_str(&content).context("解析保存的 API Key 文件失败")?;
    Ok(value
        .get("api_key")
        .and_then(|api_key| api_key.as_str())
        .map(str::trim)
        .filter(|api_key| !api_key.is_empty())
        .map(ToOwned::to_owned))
}

fn save_file_api_key(api_key: &str) -> Result<()> {
    let dir = ensure_app_dir()?;
    save_file_api_key_to(&dir, api_key)
}

fn save_file_api_key_to(dir: &PathBuf, api_key: &str) -> Result<()> {
    let path = dir.join(CREDENTIALS_FILE);
    let temp_path = dir.join(format!("{CREDENTIALS_FILE}.tmp"));
    let payload = json!({
        "api_key": api_key,
        "warning": "This file contains a secret. Keep directory permissions restricted to the current user."
    });
    write_private_file(&temp_path, serde_json::to_vec_pretty(&payload)?)?;
    fs::rename(&temp_path, &path)
        .with_context(|| format!("保存 API Key 文件失败：{}", path.display()))?;
    set_private_file_permissions(&path)?;
    Ok(())
}

fn write_storage_marker(storage: ApiKeyStorage) -> Result<()> {
    let dir = ensure_app_dir()?;
    let path = dir.join("settings.json");
    let temp_path = dir.join("settings.json.tmp");
    let payload = json!({ "api_key_storage": storage.label() });
    write_private_file(&temp_path, serde_json::to_vec_pretty(&payload)?)?;
    fs::rename(&temp_path, &path)
        .with_context(|| format!("保存设置文件失败：{}", path.display()))?;
    set_private_file_permissions(&path)?;
    Ok(())
}

fn write_private_file(path: &PathBuf, bytes: Vec<u8>) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("创建文件失败：{}", path.display()))?;
    set_private_file_permissions(path)?;
    file.write_all(&bytes)
        .with_context(|| format!("写入文件失败：{}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("同步文件失败：{}", path.display()))?;
    Ok(())
}

fn credentials_path() -> Result<PathBuf> {
    Ok(app_dir()?.join(CREDENTIALS_FILE))
}

fn ensure_app_dir() -> Result<PathBuf> {
    let dir = app_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("创建配置目录失败：{}", dir.display()))?;
    set_private_dir_permissions(&dir)?;
    #[cfg(windows)]
    {
        let _ = Command::new("attrib").arg("+h").arg(&dir).status();
    }
    Ok(dir)
}

fn app_dir() -> Result<PathBuf> {
    home_dir()
        .map(|home| home.join(APP_DIR))
        .ok_or_else(|| anyhow!("无法定位用户目录，不能安全保存 API Key"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("设置目录权限失败：{}", path.display()))
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &PathBuf) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("设置文件权限失败：{}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &PathBuf) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn load_keychain_api_key() -> Result<Option<String>> {
    let output = Command::new("security")
        .arg("find-generic-password")
        .arg("-a")
        .arg(keychain_account())
        .arg("-s")
        .arg(KEYCHAIN_SERVICE)
        .arg("-w")
        .output()
        .context("读取 macOS Keychain 失败")?;
    if !output.status.success() {
        return Ok(None);
    }
    let api_key = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok((!api_key.is_empty()).then_some(api_key))
}

#[cfg(target_os = "macos")]
fn save_keychain_api_key(api_key: &str) -> Result<()> {
    let output = Command::new("security")
        .arg("add-generic-password")
        .arg("-U")
        .arg("-a")
        .arg(keychain_account())
        .arg("-s")
        .arg(KEYCHAIN_SERVICE)
        .arg("-w")
        .arg(api_key)
        .output()
        .context("写入 macOS Keychain 失败")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "写入 macOS Keychain 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(target_os = "macos")]
fn delete_keychain_api_key() -> Result<()> {
    let output = Command::new("security")
        .arg("delete-generic-password")
        .arg("-a")
        .arg(keychain_account())
        .arg("-s")
        .arg(KEYCHAIN_SERVICE)
        .output()
        .context("删除 macOS Keychain 项失败")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "删除 macOS Keychain 项失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(target_os = "macos")]
fn keychain_account() -> String {
    env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .unwrap_or_else(|_| "default".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_file_storage_roundtrips_key() {
        let dir = env::temp_dir().join(format!("mtd-secret-store-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        save_file_api_key_to(&dir, "test-secret-key").unwrap();
        let path = dir.join(CREDENTIALS_FILE);
        let loaded = load_file_api_key_from(&path).unwrap();

        assert_eq!(loaded.as_deref(), Some("test-secret-key"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
