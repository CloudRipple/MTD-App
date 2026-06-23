use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use eframe::egui;

use crate::{
    media::{
        PreviewFrame, SubtitleBurnOptions, media_duration, stream_subtitle_preview_frames,
        video_frame_rate,
    },
    models::Segment,
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

pub(crate) struct VideoPreview {
    source: Option<PreviewSource>,
    duration: Option<f64>,
    frame_rate: f64,
    current_time: f64,
    playing: bool,
    last_tick: Option<Instant>,
    cache: Arc<Mutex<RenderCache>>,
    texture: Option<egui::TextureHandle>,
    texture_frame_index: Option<usize>,
    texture_time: Option<f64>,
}

impl Default for VideoPreview {
    fn default() -> Self {
        Self {
            source: None,
            duration: None,
            frame_rate: FALLBACK_PREVIEW_FPS,
            current_time: 0.0,
            playing: false,
            last_tick: None,
            cache: Arc::new(Mutex::new(RenderCache::default())),
            texture: None,
            texture_frame_index: None,
            texture_time: None,
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

    pub(crate) fn prepare(&mut self, video_path: &Path, srt_path: &Path, segments: &[Segment]) {
        let next_source = PreviewSource {
            video_path: video_path.to_path_buf(),
            srt_path: srt_path.to_path_buf(),
        };
        if self.source.as_ref() == Some(&next_source) {
            return;
        }

        self.source = Some(next_source);
        self.duration = media_duration(video_path).ok().flatten();
        self.frame_rate = video_frame_rate(video_path)
            .ok()
            .flatten()
            .unwrap_or(FALLBACK_PREVIEW_FPS);
        self.current_time = first_subtitle_time(segments);
        self.playing = false;
        self.last_tick = None;
        self.texture = None;
        self.texture_frame_index = None;
        self.texture_time = None;
        self.reset_cache();
    }

    pub(crate) fn reset(&mut self) {
        self.source = None;
        self.duration = None;
        self.frame_rate = FALLBACK_PREVIEW_FPS;
        self.current_time = 0.0;
        self.playing = false;
        self.last_tick = None;
        self.texture = None;
        self.texture_frame_index = None;
        self.texture_time = None;
        self.reset_cache();
    }

    pub(crate) fn invalidate(&mut self) {
        self.texture_frame_index = None;
        self.texture_time = None;
        self.reset_cache();
    }

    pub(crate) fn toggle_playing(&mut self) {
        self.playing = !self.playing;
        self.last_tick = self.playing.then(Instant::now);
    }

    pub(crate) fn pause(&mut self) {
        self.playing = false;
        self.last_tick = None;
    }

    pub(crate) fn seek(&mut self, time: f64) {
        self.current_time = self.clamp_time(time);
        self.last_tick = self.playing.then(Instant::now);
    }

    pub(crate) fn update_playback(&mut self, fallback_duration: f64) {
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

fn first_subtitle_time(segments: &[Segment]) -> f64 {
    segments.first().map(|segment| segment.start).unwrap_or(0.0)
}
