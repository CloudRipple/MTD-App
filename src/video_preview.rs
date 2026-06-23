use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use eframe::egui;

use crate::{
    media::{PreviewFrame, SubtitleBurnOptions, media_duration, render_subtitle_preview_frame},
    models::Segment,
};

const PREVIEW_FRAME_WIDTH: usize = 960;
const PREVIEW_FRAME_HEIGHT: usize = 540;
const FRAME_INTERVAL_SECONDS: f64 = 0.28;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreviewSource {
    video_path: PathBuf,
    srt_path: PathBuf,
}

#[derive(Default)]
struct FrameSlot {
    pending: bool,
    request_id: u64,
    image: Option<PreviewImage>,
    error: Option<String>,
}

struct PreviewImage {
    request_id: u64,
    time: f64,
    frame: PreviewFrame,
}

pub(crate) struct VideoPreview {
    source: Option<PreviewSource>,
    duration: Option<f64>,
    current_time: f64,
    playing: bool,
    last_tick: Option<Instant>,
    frame_slot: Arc<Mutex<FrameSlot>>,
    texture: Option<egui::TextureHandle>,
    texture_request_id: u64,
    texture_time: Option<f64>,
    needs_frame: bool,
}

impl Default for VideoPreview {
    fn default() -> Self {
        Self {
            source: None,
            duration: None,
            current_time: 0.0,
            playing: false,
            last_tick: None,
            frame_slot: Arc::new(Mutex::new(FrameSlot::default())),
            texture: None,
            texture_request_id: 0,
            texture_time: None,
            needs_frame: false,
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
        self.frame_slot.lock().expect("preview frame lock").pending
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.frame_slot
            .lock()
            .expect("preview frame lock")
            .error
            .clone()
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
        self.texture_request_id = 0;
        self.texture_time = None;
        self.needs_frame = true;
        *self.frame_slot.lock().expect("preview frame lock") = FrameSlot::default();
    }

    pub(crate) fn reset(&mut self) {
        self.source = None;
        self.duration = None;
        self.current_time = 0.0;
        self.playing = false;
        self.last_tick = None;
        self.texture = None;
        self.texture_request_id = 0;
        self.texture_time = None;
        self.needs_frame = false;
        *self.frame_slot.lock().expect("preview frame lock") = FrameSlot::default();
    }

    pub(crate) fn invalidate(&mut self) {
        self.texture_time = None;
        self.needs_frame = true;
    }

    pub(crate) fn toggle_playing(&mut self) {
        self.playing = !self.playing;
        self.last_tick = self.playing.then(Instant::now);
        if self.playing {
            self.needs_frame = true;
        }
    }

    pub(crate) fn pause(&mut self) {
        self.playing = false;
        self.last_tick = None;
    }

    pub(crate) fn seek(&mut self, time: f64) {
        self.current_time = self.clamp_time(time);
        self.last_tick = self.playing.then(Instant::now);
        self.needs_frame = true;
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

    pub(crate) fn sync_frame(&mut self, ctx: &egui::Context) {
        let mut slot = self.frame_slot.lock().expect("preview frame lock");
        let Some(image) = slot.image.take() else {
            return;
        };
        let color_image =
            egui::ColorImage::from_rgb([image.frame.width, image.frame.height], &image.frame.rgb);

        if let Some(texture) = self.texture.as_mut() {
            texture.set(color_image, egui::TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture(
                "video-preview-frame",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
        self.texture_request_id = image.request_id;
        self.texture_time = Some(image.time);
    }

    pub(crate) fn maybe_request_frame(
        &mut self,
        ctx: &egui::Context,
        options: SubtitleBurnOptions,
    ) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let needs_due_to_time = self
            .texture_time
            .map(|time| (time - self.current_time).abs() >= FRAME_INTERVAL_SECONDS)
            .unwrap_or(true);
        if !self.needs_frame && !needs_due_to_time {
            return;
        }

        let mut slot = self.frame_slot.lock().expect("preview frame lock");
        if slot.pending {
            return;
        }
        slot.pending = true;
        slot.error = None;
        slot.request_id = slot.request_id.wrapping_add(1);
        let request_id = slot.request_id;
        let frame_slot = Arc::clone(&self.frame_slot);
        let time = self.current_time;
        let ctx = ctx.clone();
        self.needs_frame = false;
        drop(slot);

        thread::spawn(move || {
            let result = render_subtitle_preview_frame(
                &source.video_path,
                &source.srt_path,
                time,
                PREVIEW_FRAME_WIDTH,
                PREVIEW_FRAME_HEIGHT,
                &options,
            );
            let mut slot = frame_slot.lock().expect("preview frame lock");
            match result {
                Ok(frame) => {
                    slot.image = Some(PreviewImage {
                        request_id,
                        time,
                        frame,
                    });
                }
                Err(error) => {
                    slot.error = Some(error.to_string());
                }
            }
            slot.pending = false;
            ctx.request_repaint();
        });
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
