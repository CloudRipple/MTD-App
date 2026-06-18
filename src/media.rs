use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};

pub(crate) fn extract_audio(video_path: &Path, audio_path: &Path) -> Result<()> {
    run_ffmpeg(&[
        "-i",
        path_arg(video_path).as_str(),
        "-vn",
        "-map",
        "0:a:0",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        path_arg(audio_path).as_str(),
    ])
}

pub(crate) fn burn_subtitles(video_path: &Path, srt_path: &Path, output_path: &Path) -> Result<()> {
    let subtitle_filter = format!("subtitles='{}'", escape_subtitle_filter_path(srt_path));
    run_ffmpeg(&[
        "-i",
        path_arg(video_path).as_str(),
        "-vf",
        subtitle_filter.as_str(),
        "-c:a",
        "copy",
        path_arg(output_path).as_str(),
    ])
}

pub(crate) fn has_video_stream(media_path: &Path) -> Result<bool> {
    let ffmpeg =
        find_ffmpeg().ok_or_else(|| anyhow!("未找到 ffmpeg，请安装 ffmpeg，或设置 FFMPEG_PATH"))?;
    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(path_arg(media_path))
        .output()
        .context("读取媒体信息失败")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let has_stream_metadata = stderr.lines().any(|line| line.contains("Stream #"));
    if !has_stream_metadata {
        let message = stderr.trim();
        return Err(anyhow!(
            "无法识别媒体流信息{}",
            if message.is_empty() {
                String::new()
            } else {
                format!("：{message}")
            }
        ));
    }
    Ok(stderr
        .lines()
        .any(|line| line.contains("Stream #") && line.contains("Video:")))
}

fn run_ffmpeg(args: &[&str]) -> Result<()> {
    let ffmpeg =
        find_ffmpeg().ok_or_else(|| anyhow!("未找到 ffmpeg，请安装 ffmpeg，或设置 FFMPEG_PATH"))?;
    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-y")
        .args(args)
        .output()
        .context("运行 ffmpeg 失败")?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("ffmpeg 失败：{}", stderr.trim()))
    }
}

fn find_ffmpeg() -> Option<PathBuf> {
    let executable_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(path) = env::var("FFMPEG_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(executable_name));
            candidates.push(parent.join("ffmpeg").join(executable_name));
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.join("Resources").join(executable_name));
                candidates.push(
                    grandparent
                        .join("Resources")
                        .join("ffmpeg")
                        .join(executable_name),
                );
            }
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join(executable_name));
        candidates.push(
            current_dir
                .join("vendor")
                .join("ffmpeg")
                .join(executable_name),
        );
    }
    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    find_in_path(executable_name)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(name))
        .find(|candidate| candidate.exists())
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn escape_subtitle_filter_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}
