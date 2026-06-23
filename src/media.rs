use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};

#[derive(Clone, Debug)]
pub(crate) struct PreviewFrame {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) rgb: Vec<u8>,
}

pub(crate) fn extract_audio(video_path: &Path, audio_path: &Path) -> Result<()> {
    let input = path_arg(video_path);
    let output = path_arg(audio_path);
    run_ffmpeg(&ffmpeg_args(&[
        "-i", &input, "-vn", "-map", "0:a:0", "-c:a", "aac", "-b:a", "128k", &output,
    ]))
}

#[derive(Clone, Debug)]
pub(crate) struct SubtitleBurnOptions {
    pub(crate) font_family: Option<String>,
    pub(crate) font_size: u32,
    pub(crate) fonts_dir: Option<PathBuf>,
}

impl Default for SubtitleBurnOptions {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 24,
            fonts_dir: None,
        }
    }
}

pub(crate) fn burn_subtitles(
    video_path: &Path,
    srt_path: &Path,
    output_path: &Path,
    options: SubtitleBurnOptions,
) -> Result<()> {
    let subtitle_filter = subtitle_filter(srt_path, &options);
    let input = path_arg(video_path);
    let output = path_arg(output_path);
    let mut errors = Vec::new();

    for (label, args) in burn_subtitle_commands(&input, &subtitle_filter, &output) {
        match run_ffmpeg(&args) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(format!("{label}: {error}")),
        }
    }

    Err(anyhow!("添加字幕到视频失败：{}", errors.join("\n")))
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

pub(crate) fn media_duration(media_path: &Path) -> Result<Option<f64>> {
    let ffmpeg =
        find_ffmpeg().ok_or_else(|| anyhow!("未找到 ffmpeg，请安装 ffmpeg，或设置 FFMPEG_PATH"))?;
    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(path_arg(media_path))
        .output()
        .context("读取媒体时长失败")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(parse_duration(&stderr))
}

pub(crate) fn render_subtitle_preview_frame(
    video_path: &Path,
    srt_path: &Path,
    time_seconds: f64,
    width: usize,
    height: usize,
    options: &SubtitleBurnOptions,
) -> Result<PreviewFrame> {
    let ffmpeg =
        find_ffmpeg().ok_or_else(|| anyhow!("未找到 ffmpeg，请安装 ffmpeg，或设置 FFMPEG_PATH"))?;
    let input = path_arg(video_path);
    let subtitle_filter = subtitle_filter(srt_path, options);
    let filter = format!(
        "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=0x0f171d,{subtitle_filter}"
    );
    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-ss")
        .arg(format!("{:.3}", time_seconds.max(0.0)))
        .arg("-i")
        .arg(input)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(filter)
        .arg("-f")
        .arg("rawvideo")
        .arg("-pix_fmt")
        .arg("rgb24")
        .arg("pipe:1")
        .output()
        .context("渲染预览帧失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("预览帧渲染失败：{}", stderr.trim()));
    }

    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| anyhow!("预览帧尺寸过大"))?;
    if output.stdout.len() != expected_len {
        return Err(anyhow!(
            "预览帧尺寸异常：预期 {} 字节，实际 {} 字节",
            expected_len,
            output.stdout.len()
        ));
    }

    Ok(PreviewFrame {
        width,
        height,
        rgb: output.stdout,
    })
}

fn burn_subtitle_commands(
    input: &str,
    subtitle_filter: &str,
    output: &str,
) -> Vec<(&'static str, Vec<String>)> {
    let mut commands = Vec::new();
    if cfg!(target_os = "macos") {
        commands.push((
            "VideoToolbox H.264 高码率编码",
            subtitle_burn_args(
                input,
                subtitle_filter,
                &["-c:v", "h264_videotoolbox", "-b:v", "32M", "-allow_sw", "1"],
                output,
            ),
        ));
    }

    commands.push((
        "libx264 高质量编码",
        subtitle_burn_args(
            input,
            subtitle_filter,
            &["-c:v", "libx264", "-preset", "slow", "-crf", "16"],
            output,
        ),
    ));

    if !cfg!(target_os = "macos") {
        commands.push((
            "VideoToolbox H.264 高码率编码",
            subtitle_burn_args(
                input,
                subtitle_filter,
                &["-c:v", "h264_videotoolbox", "-b:v", "32M", "-allow_sw", "1"],
                output,
            ),
        ));
    }

    commands.push((
        "MPEG-4 最高质量兜底编码",
        subtitle_burn_args(
            input,
            subtitle_filter,
            &["-c:v", "mpeg4", "-q:v", "1"],
            output,
        ),
    ));

    commands
}

fn subtitle_burn_args(
    input: &str,
    subtitle_filter: &str,
    encoder_args: &[&str],
    output: &str,
) -> Vec<String> {
    let mut args = ffmpeg_args(&[
        "-i",
        input,
        "-map",
        "0:v:0",
        "-map",
        "0:a?",
        "-vf",
        subtitle_filter,
    ]);
    args.extend(encoder_args.iter().map(|arg| (*arg).to_owned()));
    args.extend(ffmpeg_args(&[
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
        "-c:a",
        "copy",
        output,
    ]));
    args
}

fn run_ffmpeg(args: &[String]) -> Result<()> {
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

fn ffmpeg_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
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

fn parse_duration(stderr: &str) -> Option<f64> {
    let marker = "Duration:";
    let start = stderr.find(marker)? + marker.len();
    let value = stderr[start..].split(',').next()?.trim();
    parse_duration_value(value)
}

fn parse_duration_value(value: &str) -> Option<f64> {
    let parts = value.split(':').collect::<Vec<_>>();
    let [hours, minutes, seconds] = parts.as_slice() else {
        return None;
    };
    let hours = hours.trim().parse::<f64>().ok()?;
    let minutes = minutes.trim().parse::<f64>().ok()?;
    let seconds = seconds.trim().parse::<f64>().ok()?;
    let duration = hours * 3600.0 + minutes * 60.0 + seconds;
    duration.is_finite().then_some(duration.max(0.0))
}

fn subtitle_filter(srt_path: &Path, options: &SubtitleBurnOptions) -> String {
    let mut filter = format!("subtitles='{}'", escape_subtitle_filter_path(srt_path));
    if let Some(fonts_dir) = options.fonts_dir.as_deref() {
        filter.push_str(&format!(
            ":fontsdir='{}'",
            escape_subtitle_filter_path(fonts_dir)
        ));
    }
    let mut force_style = Vec::new();
    if let Some(font_family) = options
        .font_family
        .as_deref()
        .map(str::trim)
        .filter(|font| !font.is_empty())
    {
        force_style.push(format!(
            "FontName={}",
            escape_subtitle_force_style(font_family)
        ));
    }
    let font_size = options.font_size.clamp(12, 96);
    force_style.push(format!("FontSize={font_size}"));
    if !force_style.is_empty() {
        filter.push_str(&format!(":force_style='{}'", force_style.join(",")));
    }
    filter
}

fn escape_subtitle_filter_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

fn escape_subtitle_force_style(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace(',', "\\,")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_burn_commands_prefer_high_quality_video_encoding() {
        let commands = burn_subtitle_commands("input.mp4", "subtitles='captions.srt'", "out.mp4");

        assert!(commands.len() >= 3);

        let has_libx264 = commands.iter().any(|(_, args)| {
            has_arg_pair(args, "-c:v", "libx264")
                && has_arg_pair(args, "-crf", "16")
                && has_arg_pair(args, "-preset", "slow")
        });
        let has_platform_h264 = commands.iter().any(|(_, args)| {
            has_arg_pair(args, "-c:v", "h264_videotoolbox")
                && has_arg_pair(args, "-b:v", "32M")
                && has_arg_pair(args, "-allow_sw", "1")
        });
        let has_fallback = commands.iter().any(|(_, args)| {
            has_arg_pair(args, "-c:v", "mpeg4") && has_arg_pair(args, "-q:v", "1")
        });
        let copies_audio = commands
            .iter()
            .all(|(_, args)| has_arg_pair(args, "-c:a", "copy"));

        assert!(has_libx264);
        assert!(has_platform_h264);
        assert!(has_fallback);
        assert!(copies_audio);
    }

    #[test]
    fn subtitle_filter_can_force_selected_font() {
        let options = SubtitleBurnOptions {
            font_family: Some("HarmonyOS Sans SC".to_owned()),
            font_size: 28,
            fonts_dir: Some(PathBuf::from("/tmp/fonts")),
        };

        let filter = subtitle_filter(Path::new("/tmp/captions.srt"), &options);

        assert_eq!(
            filter,
            "subtitles='/tmp/captions.srt':fontsdir='/tmp/fonts':force_style='FontName=HarmonyOS Sans SC,FontSize=28'"
        );
    }

    #[test]
    fn subtitle_filter_can_force_font_size_without_font_family() {
        let options = SubtitleBurnOptions {
            font_family: None,
            font_size: 18,
            fonts_dir: None,
        };

        let filter = subtitle_filter(Path::new("/tmp/captions.srt"), &options);

        assert_eq!(
            filter,
            "subtitles='/tmp/captions.srt':force_style='FontSize=18'"
        );
    }

    #[test]
    fn parses_ffmpeg_duration_line() {
        let stderr = "Input #0, mov,mp4,m4a,3gp,3g2,mj2\n  Duration: 00:01:02.540, start: 0.000000, bitrate: 1000 kb/s";

        assert_eq!(parse_duration(stderr), Some(62.54));
    }

    fn has_arg_pair(args: &[String], key: &str, value: &str) -> bool {
        args.windows(2)
            .any(|pair| pair[0] == key && pair[1] == value)
    }
}
