use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use eframe::egui;

use crate::{
    media::{PreviewFrame, SubtitleBurnOptions, media_duration, stream_subtitle_preview_frames},
    models::Segment,
};

const PREVIEW_FRAME_WIDTH: usize = 640;
const PREVIEW_FRAME_HEIGHT: usize = 360;
const PREVIEW_FPS: f64 = 3.0;
const PREVIEW_CACHE_BUDGET_BYTES: usize = 192 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreviewSource {
    video_path: PathBuf,
    srt_path: PathBuf,
}

#[derive(Clone, Debug)]
struct CachedFrame {
    index: usize,
    time: f64,
    frame: PreviewFrame,
}

#[derive(Default)]
struct RenderCache {
    generation: u64,
    pending: bool,
    complete: bool,
    key: Option<String>,
    frames: Vec<CachedFrame>,
    error: Option<String>,
}

pub(crate) struct VideoPreview {
    source: Option<PreviewSource>,
    duration: Option<f64>,
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
        let preview_fps = self
            .duration
            .map(adaptive_preview_fps)
            .unwrap_or(PREVIEW_FPS);
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
        let generation = cache.generation;
        let cache_handle = Arc::clone(&self.cache);
        let ctx = ctx.clone();
        drop(cache);

        thread::spawn(move || {
            let max_frames = preview_max_frames();
            let result = stream_subtitle_preview_frames(
                &source.video_path,
                &source.srt_path,
                PREVIEW_FRAME_WIDTH,
                PREVIEW_FRAME_HEIGHT,
                preview_fps,
                max_frames,
                &options,
                |index, rgb| {
                    let mut cache = cache_handle.lock().expect("preview cache lock");
                    if cache.generation != generation {
                        return false;
                    }
                    cache.frames.push(CachedFrame {
                        index,
                        time: index as f64 / preview_fps,
                        frame: PreviewFrame {
                            width: PREVIEW_FRAME_WIDTH,
                            height: PREVIEW_FRAME_HEIGHT,
                            rgb,
                        },
                    });
                    if index == 0 || index % 6 == 0 {
                        ctx.request_repaint();
                    }
                    true
                },
            );

            let mut cache = cache_handle.lock().expect("preview cache lock");
            if cache.generation == generation {
                cache.pending = false;
                cache.complete = result.is_ok();
                if let Err(error) = result {
                    cache.error = Some(error.to_string());
                }
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
        cache
            .frames
            .iter()
            .min_by(|left, right| {
                let left_distance = (left.time - self.current_time).abs();
                let right_distance = (right.time - self.current_time).abs();
                left_distance.total_cmp(&right_distance)
            })
            .cloned()
    }

    fn reset_cache(&mut self) {
        let mut cache = self.cache.lock().expect("preview cache lock");
        cache.generation = cache.generation.wrapping_add(1);
        cache.pending = false;
        cache.complete = false;
        cache.key = None;
        cache.frames.clear();
        cache.error = None;
    }

    fn clamp_time(&self, time: f64) -> f64 {
        let upper = self.duration.unwrap_or(f64::MAX);
        time.max(0.0).min(upper)
    }
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

fn preview_max_frames() -> usize {
    let frame_bytes = PREVIEW_FRAME_WIDTH * PREVIEW_FRAME_HEIGHT * 3;
    (PREVIEW_CACHE_BUDGET_BYTES / frame_bytes).max(1)
}

fn adaptive_preview_fps(duration: f64) -> f64 {
    if duration <= 0.0 {
        return PREVIEW_FPS;
    }
    PREVIEW_FPS.min(preview_max_frames() as f64 / duration.max(1.0))
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
