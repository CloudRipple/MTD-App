use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eframe::egui;

use crate::{
    config::DEFAULT_MODEL, media::burn_subtitles, models::JobSnapshot, pipeline::run_job,
    platform::default_output_dir, theme::CANVAS,
};

pub(crate) struct MtdApp {
    pub(crate) video_path: Option<PathBuf>,
    pub(crate) output_dir: PathBuf,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) include_speaker: bool,
    pub(crate) model_picker_open: bool,
    pub(crate) job: Arc<Mutex<JobSnapshot>>,
    pub(crate) running: bool,
    pub(crate) burning: bool,
}

impl Default for MtdApp {
    fn default() -> Self {
        Self {
            video_path: None,
            output_dir: default_output_dir(),
            api_key: String::new(),
            model: DEFAULT_MODEL.to_owned(),
            max_tokens: 48_000,
            include_speaker: true,
            model_picker_open: false,
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
        if self.burning && !snapshot.status.contains("正在烧录") {
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
        {
            let mut state = job.lock().expect("job lock");
            *state = JobSnapshot {
                status: "正在准备任务".to_owned(),
                progress: 4.0,
                preview: "等待字幕生成。".to_owned(),
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
            state.status = "正在烧录到视频".to_owned();
            state.error = None;
        }

        thread::spawn(move || {
            let result = burn_subtitles(&input_video_path, &srt_path, &subtitled_path);
            let mut state = job.lock().expect("job lock");
            match result {
                Ok(()) => {
                    state.status = "烧录完成".to_owned();
                    state.output_dir = subtitled_path.parent().map(|path| path.to_path_buf());
                    state.subtitled_path = Some(subtitled_path);
                    state.done = true;
                    state.error = None;
                }
                Err(error) => {
                    state.status = "烧录失败".to_owned();
                    state.done = true;
                    state.error = Some(error.to_string());
                }
            }
        });
    }
}
