use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use eframe::egui;

use crate::{
    media::{
        PreviewFrame, SubtitleBurnOptions, find_ffmpeg, media_duration, path_arg,
        stream_subtitle_preview_frames, video_frame_rate,
    },
    models::Segment,
    platform::hide_command_window,
};

const PREVIEW_FRAME_WIDTH: usize = 640;
const PREVIEW_FRAME_HEIGHT: usize = 360;
const FALLBACK_PREVIEW_FPS: f64 = 30.0;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreviewSource {
    video_path: PathBuf,
    srt_path: PathBuf,
}

#[derive(Clone, Debug)]
struct CachedFrameLocation {
    index: usize,
    time: f64,
    offset: u64,
}

#[derive(Default)]
struct RenderCache {
    generation: u64,
    pending: bool,
    complete: bool,
    key: Option<String>,
    file_path: Option<PathBuf>,
    frames: Vec<CachedFrameLocation>,
    error: Option<String>,
}

#[derive(Default)]
struct MetadataProbe {
    generation: u64,
    pending: bool,
    duration: Option<f64>,
    frame_rate: Option<f64>,
}

pub(crate) struct VideoPreview {
    source: Option<PreviewSource>,
    duration: Option<f64>,
    frame_rate: f64,
    metadata_generation: u64,
    metadata: Arc<Mutex<MetadataProbe>>,
    current_time: f64,
    playing: bool,
    last_tick: Option<Instant>,
    cache: Arc<Mutex<RenderCache>>,
    texture: Option<egui::TextureHandle>,
    texture_frame_index: Option<usize>,
    texture_time: Option<f64>,
    audio: AudioPreview,
}

impl Default for VideoPreview {
    fn default() -> Self {
        Self {
            source: None,
            duration: None,
            frame_rate: FALLBACK_PREVIEW_FPS,
            metadata_generation: 0,
            metadata: Arc::new(Mutex::new(MetadataProbe::default())),
            current_time: 0.0,
            playing: false,
            last_tick: None,
            cache: Arc::new(Mutex::new(RenderCache::default())),
            texture: None,
            texture_frame_index: None,
            texture_time: None,
            audio: AudioPreview::default(),
        }
    }
}

impl VideoPreview {
    pub(crate) fn current_time(&self) -> f64 {
        self.current_time
    }

    pub(crate) fn duration(&self) -> Option<f64> {
        self.duration
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing
    }

    pub(crate) fn has_texture(&self) -> bool {
        self.texture.is_some()
    }

    pub(crate) fn texture(&self) -> Option<&egui::TextureHandle> {
        self.texture.as_ref()
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.cache.lock().expect("preview cache lock").pending
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.cache.lock().expect("preview cache lock").error.clone()
    }

    pub(crate) fn last_audio_error(&self) -> Option<String> {
        self.audio.last_error()
    }

    pub(crate) fn prepare(
        &mut self,
        ctx: &egui::Context,
        video_path: &Path,
        srt_path: &Path,
        _segments: &[Segment],
    ) {
        let next_source = PreviewSource {
            video_path: video_path.to_path_buf(),
            srt_path: srt_path.to_path_buf(),
        };
        if self.source.as_ref() == Some(&next_source) {
            return;
        }

        self.source = Some(next_source);
        self.duration = None;
        self.frame_rate = FALLBACK_PREVIEW_FPS;
        self.start_metadata_probe(ctx, video_path);
        self.current_time = 0.0;
        self.playing = false;
        self.last_tick = None;
        self.texture = None;
        self.texture_frame_index = None;
        self.texture_time = None;
        self.audio.stop();
        self.reset_cache();
    }

    pub(crate) fn reset(&mut self) {
        self.source = None;
        self.duration = None;
        self.frame_rate = FALLBACK_PREVIEW_FPS;
        self.metadata_generation = self.metadata_generation.wrapping_add(1);
        self.current_time = 0.0;
        self.playing = false;
        self.last_tick = None;
        self.texture = None;
        self.texture_frame_index = None;
        self.texture_time = None;
        self.audio.stop();
        self.reset_cache();
    }

    pub(crate) fn invalidate(&mut self) {
        self.texture_frame_index = None;
        self.texture_time = None;
        self.reset_cache();
    }

    pub(crate) fn toggle_playing(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.playing = true;
            self.last_tick = Some(Instant::now());
            self.start_audio_at_current_time();
        }
    }

    pub(crate) fn pause(&mut self) {
        self.playing = false;
        self.last_tick = None;
        self.audio.stop();
    }

    pub(crate) fn seek(&mut self, time: f64) {
        self.current_time = self.clamp_time(time);
        self.last_tick = self.playing.then(Instant::now);
        if self.playing {
            self.start_audio_at_current_time();
        }
    }

    pub(crate) fn update_playback(&mut self, fallback_duration: f64) {
        self.sync_metadata_probe();
        if !self.playing {
            return;
        }

        let now = Instant::now();
        if let Some(last_tick) = self.last_tick.replace(now) {
            self.current_time += now.duration_since(last_tick).as_secs_f64();
        }

        let end_time = self.duration.unwrap_or(fallback_duration).max(0.1);
        if self.current_time >= end_time {
            self.current_time = end_time;
            self.pause();
        }
    }

    pub(crate) fn ensure_cache(&mut self, ctx: &egui::Context, options: SubtitleBurnOptions) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let key = render_key(&source, &options);
        {
            let cache = self.cache.lock().expect("preview cache lock");
            if cache.pending || cache.key.as_deref() == Some(&key) {
                return;
            }
        }

        let mut cache = self.cache.lock().expect("preview cache lock");
        cache.generation = cache.generation.wrapping_add(1);
        cache.pending = true;
        cache.complete = false;
        cache.key = Some(key);
        cache.frames.clear();
        cache.error = None;
        let cache_path = preview_cache_path(cache.generation);
        cache.file_path = Some(cache_path.clone());
        let generation = cache.generation;
        let cache_handle = Arc::clone(&self.cache);
        let ctx = ctx.clone();
        let frame_rate = self.frame_rate;
        drop(cache);

        thread::spawn(move || {
            let frame_len = preview_frame_len();
            let mut write_error = None;
            let result = File::create(&cache_path)
                .map_err(anyhow::Error::from)
                .and_then(|mut file| {
                    stream_subtitle_preview_frames(
                        &source.video_path,
                        &source.srt_path,
                        PREVIEW_FRAME_WIDTH,
                        PREVIEW_FRAME_HEIGHT,
                        &options,
                        None,
                        |index, rgb| {
                            let offset = (index * frame_len) as u64;
                            if let Err(error) = file.write_all(&rgb) {
                                write_error = Some(error.to_string());
                                return false;
                            }

                            let mut cache = cache_handle.lock().expect("preview cache lock");
                            if cache.generation != generation {
                                return false;
                            }
                            cache.frames.push(CachedFrameLocation {
                                index,
                                time: index as f64 / frame_rate,
                                offset,
                            });
                            if index == 0 || index % frame_rate.max(1.0).round() as usize == 0 {
                                ctx.request_repaint();
                            }
                            true
                        },
                    )
                });

            let result = match write_error {
                Some(error) => Err(anyhow::anyhow!("写入预渲染缓存失败：{error}")),
                None => result,
            };

            let mut cache = cache_handle.lock().expect("preview cache lock");
            if cache.generation == generation {
                cache.pending = false;
                cache.complete = result.is_ok();
                if let Err(error) = result {
                    cache.error = Some(error.to_string());
                }
            } else {
                drop(cache);
                let _ = fs::remove_file(cache_path);
                return;
            }
            ctx.request_repaint();
        });
    }

    pub(crate) fn sync_frame(&mut self, ctx: &egui::Context) {
        let Some(frame) = self.cached_frame_for_current_time() else {
            return;
        };
        if self.texture_frame_index == Some(frame.index) {
            return;
        }

        let color_image =
            egui::ColorImage::from_rgb([frame.frame.width, frame.frame.height], &frame.frame.rgb);
        if let Some(texture) = self.texture.as_mut() {
            texture.set(color_image, egui::TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture(
                "video-preview-frame",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
        self.texture_frame_index = Some(frame.index);
        self.texture_time = Some(frame.time);
    }

    fn cached_frame_for_current_time(&self) -> Option<CachedFrame> {
        let cache = self.cache.lock().expect("preview cache lock");
        let frame = nearest_cached_frame(&cache.frames, self.current_time)?.clone();
        let file_path = cache.file_path.clone()?;
        drop(cache);

        let mut file = File::open(file_path).ok()?;
        let mut rgb = vec![0; preview_frame_len()];
        file.seek(SeekFrom::Start(frame.offset)).ok()?;
        file.read_exact(&mut rgb).ok()?;
        Some(CachedFrame {
            index: frame.index,
            time: frame.time,
            frame: PreviewFrame {
                width: PREVIEW_FRAME_WIDTH,
                height: PREVIEW_FRAME_HEIGHT,
                rgb,
            },
        })
    }

    fn reset_cache(&mut self) {
        let old_cache_path = {
            let mut cache = self.cache.lock().expect("preview cache lock");
            cache.generation = cache.generation.wrapping_add(1);
            cache.pending = false;
            cache.complete = false;
            cache.key = None;
            cache.frames.clear();
            cache.error = None;
            cache.file_path.take()
        };
        if let Some(path) = old_cache_path {
            let _ = fs::remove_file(path);
        }
    }

    fn clamp_time(&self, time: f64) -> f64 {
        let upper = self.duration.unwrap_or(f64::MAX);
        time.max(0.0).min(upper)
    }

    fn start_metadata_probe(&mut self, ctx: &egui::Context, video_path: &Path) {
        self.metadata_generation = self.metadata_generation.wrapping_add(1);
        let generation = self.metadata_generation;
        {
            let mut metadata = self.metadata.lock().expect("preview metadata lock");
            metadata.generation = generation;
            metadata.pending = true;
            metadata.duration = None;
            metadata.frame_rate = None;
        }

        let video_path = video_path.to_path_buf();
        let metadata_handle = Arc::clone(&self.metadata);
        let ctx = ctx.clone();
        thread::spawn(move || {
            let duration = media_duration(&video_path).ok().flatten();
            let frame_rate = video_frame_rate(&video_path).ok().flatten();

            let mut metadata = metadata_handle.lock().expect("preview metadata lock");
            if metadata.generation == generation {
                metadata.pending = false;
                metadata.duration = duration;
                metadata.frame_rate = frame_rate;
            }
            drop(metadata);
            ctx.request_repaint();
        });
    }

    fn sync_metadata_probe(&mut self) {
        let metadata = self.metadata.lock().expect("preview metadata lock");
        if metadata.generation != self.metadata_generation || metadata.pending {
            return;
        }
        self.duration = metadata.duration;
        self.frame_rate = metadata.frame_rate.unwrap_or(FALLBACK_PREVIEW_FPS);
        self.current_time = self.clamp_time(self.current_time);
    }

    fn start_audio_at_current_time(&mut self) {
        let Some(source) = &self.source else {
            return;
        };
        self.audio.start(&source.video_path, self.current_time);
    }
}

#[derive(Default)]
struct AudioPreview {
    stream: Option<rodio::OutputStream>,
    playback: Option<AudioPlayback>,
    error: Option<String>,
}

impl AudioPreview {
    fn last_error(&self) -> Option<String> {
        self.error.clone()
    }

    fn start(&mut self, video_path: &Path, start_time: f64) {
        self.stop();
        self.error = None;
        if let Err(error) = self.start_inner(video_path, start_time) {
            self.error = Some(error.to_string());
        }
    }

    fn stop(&mut self) {
        self.playback = None;
    }

    fn start_inner(&mut self, video_path: &Path, start_time: f64) -> Result<()> {
        if self.stream.is_none() {
            let mut stream = rodio::OutputStreamBuilder::open_default_stream()
                .context("打开音频输出设备失败")?;
            stream.log_on_drop(false);
            self.stream = Some(stream);
        }
        let stream = self.stream.as_ref().expect("audio stream initialized");

        let ffmpeg =
            find_ffmpeg().ok_or_else(|| anyhow::anyhow!("未找到 ffmpeg，无法播放预览音频"))?;
        let mut command = Command::new(ffmpeg);
        hide_command_window(&mut command);
        let mut child = command
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-nostats")
            .arg("-loglevel")
            .arg("error")
            .arg("-ss")
            .arg(format!("{:.3}", start_time.max(0.0)))
            .arg("-i")
            .arg(path_arg(video_path))
            .arg("-map")
            .arg("0:a:0")
            .arg("-vn")
            .arg("-f")
            .arg("f32le")
            .arg("-sample_fmt")
            .arg("flt")
            .arg("-ar")
            .arg("48000")
            .arg("-ac")
            .arg("2")
            .arg("pipe:1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("启动预览音频失败")?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("无法读取预览音频输出"))?;
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.append(FfmpegPcmSource { stdout });
        sink.play();
        self.playback = Some(AudioPlayback { sink, child });
        Ok(())
    }
}

struct AudioPlayback {
    sink: rodio::Sink,
    child: Child,
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        self.sink.stop();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct FfmpegPcmSource {
    stdout: std::process::ChildStdout,
}

impl Iterator for FfmpegPcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = [0_u8; 4];
        self.stdout.read_exact(&mut bytes).ok()?;
        Some(f32::from_le_bytes(bytes))
    }
}

impl rodio::Source for FfmpegPcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        2
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        48_000
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[derive(Clone, Debug)]
struct CachedFrame {
    index: usize,
    time: f64,
    frame: PreviewFrame,
}

fn nearest_cached_frame(
    frames: &[CachedFrameLocation],
    current_time: f64,
) -> Option<&CachedFrameLocation> {
    let insertion = frames.partition_point(|frame| frame.time < current_time);
    match (insertion.checked_sub(1), frames.get(insertion)) {
        (Some(previous), Some(next)) => {
            let previous = &frames[previous];
            let previous_distance = (previous.time - current_time).abs();
            let next_distance = (next.time - current_time).abs();
            if previous_distance <= next_distance {
                Some(previous)
            } else {
                Some(next)
            }
        }
        (Some(previous), None) => frames.get(previous),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    }
}

fn preview_frame_len() -> usize {
    PREVIEW_FRAME_WIDTH * PREVIEW_FRAME_HEIGHT * 3
}

fn preview_cache_path(generation: u64) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mtd-subtitle-preview-{}-{generation}.rgb",
        std::process::id()
    ))
}

pub(crate) fn fallback_duration(segments: &[Segment]) -> f64 {
    segments
        .last()
        .map(|segment| segment.end.max(segment.start + 1.0))
        .unwrap_or(1.0)
}

pub(crate) fn active_segment_at(segments: &[Segment], time: f64) -> Option<&Segment> {
    segments
        .iter()
        .find(|segment| time >= segment.start && time <= segment.end)
        .or_else(|| {
            segments.iter().min_by(|left, right| {
                let left_distance = distance_to_segment(left, time);
                let right_distance = distance_to_segment(right, time);
                left_distance.total_cmp(&right_distance)
            })
        })
}

fn render_key(source: &PreviewSource, options: &SubtitleBurnOptions) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        source.video_path.display(),
        source.srt_path.display(),
        options.font_family.as_deref().unwrap_or(""),
        options.font_size,
        options
            .fonts_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    )
}

fn distance_to_segment(segment: &Segment, time: f64) -> f64 {
    if time < segment.start {
        segment.start - time
    } else if time > segment.end {
        time - segment.end
    } else {
        0.0
    }
}
