use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use crate::{config::HARMONYOS_FONT_REGULAR, embedded_assets};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SubtitleFont {
    pub(crate) family: String,
    pub(crate) source_dir: Option<PathBuf>,
}

impl SubtitleFont {
    fn new(family: impl Into<String>, source_dir: Option<PathBuf>) -> Self {
        Self {
            family: family.into(),
            source_dir,
        }
    }
}

pub(crate) fn discover_subtitle_fonts() -> Vec<SubtitleFont> {
    let mut fonts = BTreeMap::new();
    if let Some(path) = find_bundled_harmonyos_font() {
        fonts.insert(
            "HarmonyOS Sans SC".to_owned(),
            SubtitleFont::new("HarmonyOS Sans SC", path.parent().map(Path::to_path_buf)),
        );
    }

    for dir in system_font_dirs() {
        scan_font_dir(&dir, 0, &mut fonts);
    }

    fonts.into_values().collect()
}

pub(crate) fn selected_font<'a>(
    fonts: &'a [SubtitleFont],
    family: Option<&str>,
) -> Option<&'a SubtitleFont> {
    let family = family?.trim();
    fonts.iter().find(|font| font.family == family)
}

fn scan_font_dir(dir: &Path, depth: usize, fonts: &mut BTreeMap<String, SubtitleFont>) {
    if depth > 4 || !dir.exists() {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            scan_font_dir(&path, depth + 1, fonts);
            continue;
        }

        let Some(family) = normalized_font_name(&path) else {
            continue;
        };
        fonts
            .entry(family.clone())
            .or_insert_with(|| SubtitleFont::new(family, path.parent().map(Path::to_path_buf)));
    }
}

fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if cfg!(target_os = "macos") {
        dirs.extend([
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts/Supplemental"),
            PathBuf::from("/Library/Fonts"),
        ]);
        if let Some(home) = home_dir() {
            dirs.push(home.join("Library").join("Fonts"));
        }
    } else if cfg!(windows) {
        if let Some(windir) = env::var_os("WINDIR") {
            dirs.push(PathBuf::from(windir).join("Fonts"));
        }
    } else {
        dirs.extend([
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ]);
        if let Some(home) = home_dir() {
            dirs.push(home.join(".fonts"));
            dirs.push(home.join(".local").join("share").join("fonts"));
        }
    }

    dirs
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn find_bundled_harmonyos_font() -> Option<PathBuf> {
    if let Ok(path) = env::var("HARMONYOS_FONT_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Some(path) = embedded_assets::bundled_ui_font_path() {
        return Some(path);
    }

    let mut candidates = Vec::new();
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join("fonts").join(HARMONYOS_FONT_REGULAR));
            if let Some(contents_dir) = parent.parent() {
                candidates.push(
                    contents_dir
                        .join("Resources")
                        .join("fonts")
                        .join(HARMONYOS_FONT_REGULAR),
                );
            }
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(
            current_dir
                .join("assets")
                .join("fonts")
                .join(HARMONYOS_FONT_REGULAR),
        );
        candidates.push(current_dir.join("fonts").join(HARMONYOS_FONT_REGULAR));
    }

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn normalized_font_name(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(extension.as_str(), "ttf" | "ttc" | "otf" | "otc") {
        return None;
    }

    let stem = path.file_stem()?.to_str()?;
    let mut name = stem.replace(['_', '-'], " ");
    for suffix in [
        " Regular",
        " Bold Italic",
        " Bold",
        " Italic",
        " Oblique",
        " Medium",
        " SemiBold",
        " Semibold",
        " DemiBold",
        " ExtraLight",
        " ExtraBold",
        " Light",
        " Thin",
        " Black",
        " Heavy",
        " Book",
        " Condensed",
    ] {
        if let Some(trimmed) = name.strip_suffix(suffix) {
            name = trimmed.to_owned();
            break;
        }
    }

    let normalized = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalized_font_name;
    use std::path::Path;

    #[test]
    fn normalizes_font_file_names_for_display() {
        assert_eq!(
            normalized_font_name(Path::new("HarmonyOS_Sans_SC_Regular.ttf")).as_deref(),
            Some("HarmonyOS Sans SC")
        );
        assert_eq!(
            normalized_font_name(Path::new("PingFang.ttc")).as_deref(),
            Some("PingFang")
        );
    }
}
