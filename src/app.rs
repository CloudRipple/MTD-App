use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eframe::egui;

use crate::{
    config::DEFAULT_MODEL,
    media::burn_subtitles,
    models::{JobSnapshot, PreviewMode},
    pipeline::run_job,
    platform::default_output_dir,
    secret_store::{self, ApiKeyStorage},
    subtitles::{render_srt, write_srt, write_vtt},
    theme::CANVAS,
};

pub(crate) struct MtdApp {
    pub(crate) video_path: Option<PathBuf>,
    pub(crate) output_dir: PathBuf,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) include_speaker: bool,
    pub(crate) remember_api_key: bool,
    pub(crate) saved_api_key: Option<String>,
    pub(crate) api_key_store_message: Option<String>,
    pub(crate) api_key_store_error: bool,
    pub(crate) model_picker_open: bool,
    pub(crate) preview_mode: PreviewMode,
    pub(crate) speaker_names: BTreeMap<String, String>,
    pub(crate) time_edits: BTreeMap<usize, (String, String)>,
    pub(crate) job: Arc<Mutex<JobSnapshot>>,
    pub(crate) running: bool,
    pub(crate) burning: bool,
}

impl Default for MtdApp {
    fn default() -> Self {
        let (api_key, remember_api_key, saved_api_key, api_key_store_message, api_key_store_error) =
            match secret_store::load_api_key() {
                Ok(Some(api_key)) => (
                    api_key.clone(),
                    true,
                    Some(api_key),
                    Some("已载入本机保存的 API Key".to_owned()),
                    false,
                ),
                Ok(None) => (String::new(), false, None, None, false),
                Err(error) => (
                    String::new(),
                    false,
                    None,
                    Some(format!("读取保存的 API Key 失败：{error}")),
                    true,
                ),
            };
        Self {
            video_path: None,
            output_dir: default_output_dir(),
            api_key,
            model: DEFAULT_MODEL.to_owned(),
            max_tokens: 48_000,
            include_speaker: true,
            remember_api_key,
            saved_api_key,
            api_key_store_message,
            api_key_store_error,
            model_picker_open: false,
            preview_mode: PreviewMode::Rendered,
            speaker_names: BTreeMap::new(),
            time_edits: BTreeMap::new(),
            job: Arc::new(Mutex::new(JobSnapshot::default())),
            running: false,
            burning: false,
        }
    }
}

impl eframe::App for MtdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let snapshot = self.job.lock().expect("job lock").clone();
        if snapshot.done {
            self.running = false;
        }
        if self.burning && !snapshot.status.contains("正在添加字幕") {
            self.burning = false;
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::symmetric(22, 20)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(14.0, 12.0);
                self.render_header(ui);
                ui.add_space(10.0);
                self.render_workspace(ui, &snapshot);
                ui.add_space(12.0);
                self.render_preview(ui, &snapshot);
            });

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if self.burning {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl MtdApp {
    pub(crate) fn save_api_key_to_store(&mut self) {
        let api_key = self.api_key.trim();
        if api_key.is_empty() {
            self.remember_api_key = self.saved_api_key.is_some();
            self.api_key_store_message = Some("API Key 为空，未保存".to_owned());
            self.api_key_store_error = true;
            return;
        }

        match secret_store::save_api_key(api_key) {
            Ok(storage) => {
                self.saved_api_key = Some(api_key.to_owned());
                self.remember_api_key = true;
                self.api_key_store_message = Some(saved_message(storage));
                self.api_key_store_error = false;
            }
            Err(error) => {
                self.api_key_store_message = Some(format!("保存 API Key 失败：{error}"));
                self.api_key_store_error = true;
            }
        }
    }

    pub(crate) fn forget_saved_api_key(&mut self) {
        match secret_store::clear_api_key() {
            Ok(()) => {
                self.saved_api_key = None;
                self.remember_api_key = false;
                self.api_key_store_message = Some("已从本机移除保存的 API Key".to_owned());
                self.api_key_store_error = false;
            }
            Err(error) => {
                self.api_key_store_message = Some(format!("移除保存的 API Key 失败：{error}"));
                self.api_key_store_error = true;
            }
        }
    }

    pub(crate) fn start_job(&mut self) {
        let Some(video_path) = self.video_path.clone() else {
            return;
        };
        let api_key = self.api_key.trim().to_owned();
        let output_dir = self.output_dir.clone();
        let model = self.model.clone();
        let max_tokens = self.max_tokens.clamp(1_000, 96_000);
        let include_speaker = self.include_speaker;
        let job = Arc::clone(&self.job);

        self.running = true;
        self.speaker_names.clear();
        self.time_edits.clear();
        {
            let mut state = job.lock().expect("job lock");
            *state = JobSnapshot {
                status: "正在准备任务".to_owned(),
                progress: 4.0,
                preview: "等待字幕生成。".to_owned(),
                include_speaker,
                ..JobSnapshot::default()
            };
        }

        thread::spawn(move || {
            let result = run_job(
                &job,
                video_path,
                output_dir,
                api_key,
                model,
                max_tokens,
                include_speaker,
            );
            if let Err(error) = result {
                let mut state = job.lock().expect("job lock");
                state.status = "失败".to_owned();
                state.progress = 100.0;
                state.done = true;
                state.error = Some(error.to_string());
            }
        });
    }

    pub(crate) fn can_start(&self) -> bool {
        !self.running && self.video_path.is_some() && !self.api_key.trim().is_empty()
    }

    pub(crate) fn can_burn(&self, snapshot: &JobSnapshot) -> bool {
        !self.running
            && !self.burning
            && snapshot.done
            && snapshot.error.is_none()
            && snapshot.input_video_path.is_some()
            && snapshot.srt_path.is_some()
            && snapshot.subtitled_path.is_some()
    }

    pub(crate) fn burn_video(&mut self) {
        let snapshot = self.job.lock().expect("job lock").clone();
        let (Some(input_video_path), Some(srt_path), Some(subtitled_path)) = (
            snapshot.input_video_path,
            snapshot.srt_path,
            snapshot.subtitled_path,
        ) else {
            return;
        };
        let job = Arc::clone(&self.job);

        self.burning = true;
        {
            let mut state = job.lock().expect("job lock");
            state.status = "正在添加字幕到视频".to_owned();
            state.error = None;
        }

        thread::spawn(move || {
            let result = burn_subtitles(&input_video_path, &srt_path, &subtitled_path);
            let mut state = job.lock().expect("job lock");
            match result {
                Ok(()) => {
                    state.status = "添加完成".to_owned();
                    state.output_dir = subtitled_path.parent().map(|path| path.to_path_buf());
                    state.subtitled_path = Some(subtitled_path);
                    state.done = true;
                    state.error = None;
                }
                Err(error) => {
                    state.status = "添加失败".to_owned();
                    state.done = true;
                    state.error = Some(error.to_string());
                }
            }
        });
    }

    pub(crate) fn apply_speaker_names(&mut self) {
        let mut state = self.job.lock().expect("job lock");
        if state.segments.is_empty() {
            return;
        }

        for segment in &mut state.segments {
            if let Some(name) = self.speaker_names.get(&segment.speaker) {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    segment.speaker = trimmed.to_owned();
                }
            }
        }
        self.speaker_names.clear();

        sync_subtitle_outputs(&mut state);
    }

    pub(crate) fn update_segment(
        &mut self,
        index: usize,
        start: f64,
        end: f64,
        speaker: String,
        text: String,
    ) {
        let mut state = self.job.lock().expect("job lock");
        let Some(segment) = state.segments.get_mut(index) else {
            return;
        };

        segment.start = start.max(0.0);
        segment.end = end.max(segment.start + 0.001);
        segment.speaker = speaker.trim().to_owned();
        segment.text = text;
        sync_subtitle_outputs(&mut state);
    }
}

fn saved_message(storage: ApiKeyStorage) -> String {
    format!("已保存到{}，下次打开会自动填入", storage.label())
}

fn sync_subtitle_outputs(state: &mut JobSnapshot) {
    if state.segments.is_empty() {
        return;
    }

    if let Ok(preview) = render_srt(&state.segments, state.include_speaker) {
        state.preview = preview;
    }
    if let Some(srt_path) = &state.srt_path {
        let _ = write_srt(srt_path, &state.segments, state.include_speaker);
    }
    if let Some(vtt_path) = &state.vtt_path {
        let _ = write_vtt(vtt_path, &state.segments, state.include_speaker);
    }
}
