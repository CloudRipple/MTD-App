use std::path::Path;

pub(crate) const AUDIO_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "aac", "flac", "ogg", "mpeg", "m4a", "mp4", "webm", "pcm",
];
pub(crate) const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv", "webm", "m4v", "avi"];

const DIRECT_AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "aac", "flac", "ogg", "m4a", "pcm"];

pub(crate) fn supported_extensions() -> Vec<&'static str> {
    let mut extensions = Vec::new();
    for extension in VIDEO_EXTENSIONS
        .iter()
        .chain(AUDIO_EXTENSIONS.iter())
        .copied()
    {
        if !extensions.contains(&extension) {
            extensions.push(extension);
        }
    }
    extensions
}

pub(crate) fn is_direct_audio_path(path: &Path) -> bool {
    extension(path)
        .map(|extension| DIRECT_AUDIO_EXTENSIONS.contains(&extension.as_str()))
        .unwrap_or(false)
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::trim)
        .map(|extension| extension.trim_start_matches('.'))
        .filter(|extension| !extension.is_empty())
        .map(|extension| extension.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_direct_audio_path, supported_extensions};

    #[test]
    fn keeps_requested_audio_formats_in_picker() {
        let extensions = supported_extensions();
        for extension in [
            "wav", "mp3", "aac", "flac", "ogg", "mpeg", "m4a", "mp4", "webm", "pcm",
        ] {
            assert!(extensions.contains(&extension));
        }
    }

    #[test]
    fn treats_pcm_as_direct_audio_without_probe() {
        assert!(is_direct_audio_path(Path::new("voice.PCM")));
    }
}
