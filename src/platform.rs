use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Result;
use eframe::egui;
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

use crate::{config::HARMONYOS_FONT_REGULAR, embedded_assets};

#[cfg(windows)]
pub(crate) fn hide_command_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn hide_command_window(_command: &mut Command) {}

pub(crate) fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: app_viewport(),
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            builder
                .with_activation_policy(ActivationPolicy::Regular)
                .with_activate_ignoring_other_apps(true);
        })),
        run_and_return: false,
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn app_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_app_id("cn.mtd.subtitle-app")
        .with_title("MOSS 字幕工作台")
        .with_fullsize_content_view(true)
        .with_title_shown(false)
        .with_titlebar_shown(false)
        .with_movable_by_background(false)
        .with_inner_size([1440.0, 930.0])
        .with_min_inner_size([1180.0, 820.0])
}

#[cfg(not(target_os = "macos"))]
fn app_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_app_id("cn.mtd.subtitle-app")
        .with_title("MOSS 字幕工作台")
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([1440.0, 930.0])
        .with_min_inner_size([1180.0, 820.0])
}

pub(crate) fn install_app_fonts(ctx: &egui::Context) {
    let font = embedded_assets::ui_font_bytes()
        .map(|bytes| ("HarmonyOS Sans SC".to_owned(), bytes.to_vec()))
        .or_else(|| {
            let (font_name, font_path) = find_ui_font()?;
            let font_bytes = fs::read(&font_path).ok()?;
            Some((font_name, font_bytes))
        });
    let Some((font_name, font_bytes)) = font else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        font_name.clone(),
        egui::FontData::from_owned(font_bytes).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, font_name.clone());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, font_name);
    ctx.set_fonts(fonts);
}

fn find_ui_font() -> Option<(String, PathBuf)> {
    find_harmonyos_font()
        .map(|path| ("HarmonyOS Sans SC".to_owned(), path))
        .or_else(|| find_development_cjk_font().map(|path| ("CJK UI Fallback".to_owned(), path)))
}

fn find_harmonyos_font() -> Option<PathBuf> {
    if let Ok(path) = env::var("HARMONYOS_FONT_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
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

fn find_development_cjk_font() -> Option<PathBuf> {
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.exists())
}

pub(crate) fn open_path(path: &Path) -> Result<()> {
    if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()?;
    } else if cfg!(windows) {
        Command::new("explorer").arg(path).spawn()?;
    } else {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

pub(crate) fn default_output_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}
