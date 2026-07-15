use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::config::APP_NAME;

pub(crate) struct EmbeddedFile {
    pub(crate) name: &'static str,
    pub(crate) bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

static FFMPEG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static UI_FONT_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

pub(crate) fn ui_font_bytes() -> Option<&'static [u8]> {
    UI_FONT_BYTES
}

pub(crate) fn bundled_ffmpeg_path() -> Option<PathBuf> {
    FFMPEG_PATH.get_or_init(extract_ffmpeg_runtime).clone()
}

pub(crate) fn bundled_ui_font_path() -> Option<PathBuf> {
    UI_FONT_PATH.get_or_init(extract_ui_font).clone()
}

fn extract_ffmpeg_runtime() -> Option<PathBuf> {
    if FFMPEG_FILES.is_empty() {
        return None;
    }

    let dir = runtime_root()?.join("ffmpeg").join(FFMPEG_FINGERPRINT);
    fs::create_dir_all(&dir).ok()?;
    for file in FFMPEG_FILES {
        write_embedded_file(&dir.join(file.name), file.bytes).ok()?;
    }

    let executable_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let path = dir.join(executable_name);
    path.exists().then_some(path)
}

fn extract_ui_font() -> Option<PathBuf> {
    let bytes = UI_FONT_BYTES?;
    let dir = runtime_root()?.join("fonts").join(UI_FONT_FINGERPRINT);
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join(UI_FONT_FILE_NAME);
    write_embedded_file(&path, bytes).ok()?;
    Some(path)
}

fn write_embedded_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() == bytes.len() as u64 {
            return Ok(());
        }
    }

    let temp_path = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(temp_path, path)?;
    Ok(())
}

fn runtime_root() -> Option<PathBuf> {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return Some(PathBuf::from(local_app_data).join(APP_NAME));
    }
    env::temp_dir()
        .canonicalize()
        .ok()
        .map(|path| path.join(APP_NAME))
}
