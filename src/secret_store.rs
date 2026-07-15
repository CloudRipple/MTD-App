use std::{
    fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
use std::{env, process::Command};

use anyhow::{Context, Result, anyhow};
use serde_json::json;

use crate::app_data::{app_data_file_for_read, ensure_app_data_dir, set_private_file_permissions};

const CREDENTIALS_FILE: &str = "credentials.json";
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "cn.moss.subtitle-workbench.moss-api-key";
#[cfg(target_os = "macos")]
const LEGACY_KEYCHAIN_SERVICE: &str = "cn.mtd.subtitle-app.moss-api-key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApiKeyStorage {
    #[cfg(target_os = "macos")]
    Keychain,
    HiddenFile,
}

impl ApiKeyStorage {
    pub(crate) fn label(self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            Self::Keychain => "macOS Keychain",
            Self::HiddenFile => "~/.moss-subtitle-workbench/credentials.json",
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
        let _ = delete_keychain_api_key(KEYCHAIN_SERVICE);
        let _ = delete_keychain_api_key(LEGACY_KEYCHAIN_SERVICE);
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

fn save_file_api_key_to(dir: &Path, api_key: &str) -> Result<()> {
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

#[cfg(target_os = "macos")]
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

fn write_private_file(path: &Path, bytes: Vec<u8>) -> Result<()> {
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
    app_data_file_for_read(CREDENTIALS_FILE)
}

fn ensure_app_dir() -> Result<PathBuf> {
    ensure_app_data_dir()
}

#[cfg(target_os = "macos")]
fn load_keychain_api_key() -> Result<Option<String>> {
    if let Some(api_key) = load_keychain_api_key_from(KEYCHAIN_SERVICE)? {
        return Ok(Some(api_key));
    }
    let Some(api_key) = load_keychain_api_key_from(LEGACY_KEYCHAIN_SERVICE)? else {
        return Ok(None);
    };
    if save_keychain_api_key_to(KEYCHAIN_SERVICE, &api_key).is_ok() {
        let _ = delete_keychain_api_key(LEGACY_KEYCHAIN_SERVICE);
    }
    Ok(Some(api_key))
}

#[cfg(target_os = "macos")]
fn load_keychain_api_key_from(service: &str) -> Result<Option<String>> {
    let output = Command::new("security")
        .arg("find-generic-password")
        .arg("-a")
        .arg(keychain_account())
        .arg("-s")
        .arg(service)
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
    save_keychain_api_key_to(KEYCHAIN_SERVICE, api_key)
}

#[cfg(target_os = "macos")]
fn save_keychain_api_key_to(service: &str, api_key: &str) -> Result<()> {
    let output = Command::new("security")
        .arg("add-generic-password")
        .arg("-U")
        .arg("-a")
        .arg(keychain_account())
        .arg("-s")
        .arg(service)
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
fn delete_keychain_api_key(service: &str) -> Result<()> {
    let output = Command::new("security")
        .arg("delete-generic-password")
        .arg("-a")
        .arg(keychain_account())
        .arg("-s")
        .arg(service)
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
    use std::env;

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
