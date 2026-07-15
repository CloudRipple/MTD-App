use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};

use crate::config::{APP_DATA_DIR, APP_NAME, LEGACY_APP_DATA_DIR};
#[cfg(windows)]
use crate::platform::hide_command_window;

const MIGRATABLE_FILES: [&str; 3] = ["app-settings.json", "credentials.json", "settings.json"];

pub(crate) fn default_project_root() -> PathBuf {
    home_dir()
        .map(|home| home.join(APP_NAME))
        .or_else(|| env::current_dir().ok().map(|dir| dir.join(APP_NAME)))
        .unwrap_or_else(|| PathBuf::from(APP_NAME))
}

pub(crate) fn app_data_file_for_read(file_name: &str) -> Result<PathBuf> {
    let current = app_data_dir()?.join(file_name);
    if current.exists() {
        return Ok(current);
    }

    let legacy = legacy_app_data_dir()?.join(file_name);
    if !legacy.is_file() {
        return Ok(current);
    }

    if migrate_file(&legacy, &current) {
        remove_legacy_dir_if_empty();
        Ok(current)
    } else {
        Ok(legacy)
    }
}

pub(crate) fn ensure_app_data_dir() -> Result<PathBuf> {
    let current = app_data_dir()?;
    create_private_dir(&current)?;

    let legacy_dir = legacy_app_data_dir()?;
    for file_name in MIGRATABLE_FILES {
        let legacy = legacy_dir.join(file_name);
        let destination = current.join(file_name);
        if legacy.is_file() && !destination.exists() {
            let _ = migrate_file(&legacy, &destination);
        }
    }
    remove_legacy_dir_if_empty();
    Ok(current)
}

pub(crate) fn set_private_file_permissions(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(_path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("设置文件权限失败：{}", _path.display()))?;
    }
    Ok(())
}

fn app_data_dir() -> Result<PathBuf> {
    home_dir()
        .map(|home| home.join(APP_DATA_DIR))
        .ok_or_else(|| anyhow!("无法定位用户目录，不能保存应用数据"))
}

fn legacy_app_data_dir() -> Result<PathBuf> {
    home_dir()
        .map(|home| home.join(LEGACY_APP_DATA_DIR))
        .ok_or_else(|| anyhow!("无法定位用户目录，不能读取旧版应用数据"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("创建配置目录失败：{}", path.display()))?;
    set_private_dir_permissions(path)?;
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("attrib");
        hide_command_window(&mut command);
        let _ = command.arg("+h").arg(path).status();
    }
    Ok(())
}

fn migrate_file(source: &Path, destination: &Path) -> bool {
    let Some(parent) = destination.parent() else {
        return false;
    };
    if create_private_dir(parent).is_err() {
        return false;
    }

    if fs::rename(source, destination).is_ok() {
        let _ = set_private_file_permissions(destination);
        return true;
    }

    if fs::copy(source, destination).is_err() {
        return false;
    }
    if set_private_file_permissions(destination).is_err()
        || fs::File::open(destination)
            .and_then(|file| file.sync_all())
            .is_err()
    {
        let _ = fs::remove_file(destination);
        return false;
    }
    let _ = fs::remove_file(source);
    true
}

fn remove_legacy_dir_if_empty() {
    let Ok(legacy) = legacy_app_data_dir() else {
        return;
    };
    let Ok(mut entries) = fs::read_dir(&legacy) else {
        return;
    };
    if entries.next().is_none() {
        let _ = fs::remove_dir(legacy);
    }
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

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::migrate_file;

    #[test]
    fn migrates_a_legacy_file_without_copying_its_contents_by_hand() {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("moss-app-data-test-{stamp}"));
        let source = root.join("legacy").join("credentials.json");
        let destination = root.join("current").join("credentials.json");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "secret").unwrap();

        assert!(migrate_file(&source, &destination));
        assert_eq!(fs::read_to_string(&destination).unwrap(), "secret");
        assert!(!source.exists());

        let _ = fs::remove_dir_all(root);
    }
}
