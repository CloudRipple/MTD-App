use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eframe::egui;

use crate::{
    config::DEFAULT_MODEL, models::JobSnapshot, pipeline::run_job, platform::default_output_dir,
    theme::CANVAS,
};

pub(crate) struct MtdApp {
    pub(crate) video_path: Option<PathBuf>,
    pub(crate) output_dir: PathBuf,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) include_speaker: bool,
    pub(crate) burn_in: bool,
    pub(crate) job: Arc<Mutex<JobSnapshot>>,
    pub(crate) running: bool,
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
            burn_in: false,
            job: Arc::new(Mutex::new(JobSnapshot::default())),
            running: false,
        }
    }
}

impl eframe::App for MtdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let snapshot = self.job.lock().expect("job lock").clone();
        if snapshot.done {
            self.running = false;
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
        let burn_in = self.burn_in;
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
                burn_in,
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
}
