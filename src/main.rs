#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use eframe::egui;
use reqwest::blocking::{Client, multipart};
use serde_json::{Value, json};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

const BASE_URL: &str = "https://studio.mosi.cn";
const DEFAULT_MODEL: &str = "moss-transcribe-diarize";
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const POLL_TIMEOUT: Duration = Duration::from_secs(1300);
const HARMONYOS_FONT_REGULAR: &str = "HarmonyOS_Sans_SC_Regular.ttf";
const SURFACE: egui::Color32 = egui::Color32::from_rgb(252, 253, 253);
const CANVAS: egui::Color32 = egui::Color32::from_rgb(242, 245, 246);
const BORDER: egui::Color32 = egui::Color32::from_rgb(218, 226, 231);
const INK: egui::Color32 = egui::Color32::from_rgb(24, 34, 43);
const MUTED: egui::Color32 = egui::Color32::from_rgb(91, 104, 115);
const FAINT: egui::Color32 = egui::Color32::from_rgb(136, 149, 160);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(30, 132, 118);
const ACCENT_DARK: egui::Color32 = egui::Color32::from_rgb(17, 99, 90);
const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(224, 243, 240);
const DANGER: egui::Color32 = egui::Color32::from_rgb(176, 57, 54);

const MODELS: [&str; 4] = [
    "moss-transcribe-diarize",
    "moss-transcribe-diarize-20260325",
    "moss-transcribe-diarize-20260203",
    "moss-transcribe-diarize-20260101",
];

#[derive(Clone, Debug)]
struct Segment {
    start: f64,
    end: f64,
    speaker: String,
    text: String,
}

#[derive(Clone, Debug)]
struct JobSnapshot {
    status: String,
    progress: f32,
    task_id: String,
    file_id: String,
    usage: String,
    preview: String,
    output_dir: Option<PathBuf>,
    done: bool,
    error: Option<String>,
}

impl Default for JobSnapshot {
    fn default() -> Self {
        Self {
            status: "等待选择视频".to_owned(),
            progress: 0.0,
            task_id: "-".to_owned(),
            file_id: "-".to_owned(),
            usage: "-".to_owned(),
            preview: "生成后在这里预览 SRT。".to_owned(),
            output_dir: None,
            done: false,
            error: None,
        }
    }
}

struct MtdApp {
    video_path: Option<PathBuf>,
    output_dir: PathBuf,
    api_key: String,
    model: String,
    max_tokens: u32,
    include_speaker: bool,
    burn_in: bool,
    job: Arc<Mutex<JobSnapshot>>,
    running: bool,
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

                render_app_header(ui);

                ui.add_space(10.0);

                ui.horizontal_top(|ui| {
                    let available = ui.available_width();
                    let form_width = (available * 0.60).clamp(500.0, 700.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(form_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.render_form(ui),
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.render_status(ui, &snapshot),
                    );
                });

                ui.add_space(12.0);
                self.render_preview(ui, &snapshot);
            });

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl MtdApp {
    fn render_form(&mut self, ui: &mut egui::Ui) {
        panel_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new("视频与转写设置")
                    .strong()
                    .size(16.0)
                    .color(INK),
            );
            ui.add_space(8.0);

            ui.horizontal_top(|ui| {
                let column_gap = 18.0;
                let left_width = (ui.available_width() * 0.48).max(280.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(left_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.render_file_picker(ui),
                );
                ui.add_space(column_gap);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.render_api_settings(ui),
                );
            });

            ui.add_space(12.0);
            let can_start =
                !self.running && self.video_path.is_some() && !self.api_key.trim().is_empty();
            let button_text = if self.running {
                "正在生成字幕"
            } else {
                "开始生成字幕"
            };
            let button = egui::Button::new(egui::RichText::new(button_text).strong().color(
                if can_start {
                    egui::Color32::WHITE
                } else {
                    FAINT
                },
            ))
            .min_size(egui::vec2(148.0, 34.0))
            .fill(if can_start {
                ACCENT
            } else {
                egui::Color32::from_rgb(236, 241, 243)
            });

            ui.horizontal(|ui| {
                if ui.add_enabled(can_start, button).clicked() {
                    self.start_job();
                }

                if !can_start && !self.running {
                    ui.label(
                        egui::RichText::new("选择视频并填写 API Key 后即可开始。")
                            .size(13.0)
                            .color(FAINT),
                    );
                }
            });
        });
    }

    fn render_api_settings(&mut self, ui: &mut egui::Ui) {
        field_label(ui, "MOSS API Key");
        ui.add_sized(
            [ui.available_width(), 34.0],
            egui::TextEdit::singleline(&mut self.api_key)
                .password(true)
                .hint_text("Bearer key 不需要包含 Bearer"),
        );

        ui.add_space(6.0);
        field_label(ui, "模型");
        egui::ComboBox::from_id_salt("model")
            .width(ui.available_width())
            .selected_text(&self.model)
            .show_ui(ui, |ui| {
                for model in MODELS {
                    ui.selectable_value(&mut self.model, model.to_owned(), model);
                }
            });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            field_label(ui, "最大 token");
            ui.add(
                egui::DragValue::new(&mut self.max_tokens)
                    .range(1_000..=96_000)
                    .speed(1000),
            );
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.include_speaker, "保留说话人");
            ui.checkbox(&mut self.burn_in, "烧录到视频");
        });
    }

    fn render_file_picker(&mut self, ui: &mut egui::Ui) {
        field_label(ui, "视频文件");
        ui.horizontal(|ui| {
            let text = self
                .video_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| "尚未选择视频".to_owned());
            path_pill(ui, &text, self.video_path.is_some());
            if ui
                .add_sized([96.0, 32.0], egui::Button::new("选择视频"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Video", &["mp4", "mov", "mkv", "webm", "m4v", "avi"])
                    .pick_file()
                {
                    self.video_path = Some(path);
                    update_job(&self.job, "已选择视频，可以开始生成字幕", 0.0, None);
                }
            }
        });

        ui.add_space(8.0);
        field_label(ui, "输出目录");
        ui.horizontal(|ui| {
            let output_text = self.output_dir.display().to_string();
            path_pill(ui, &output_text, true);
            if ui
                .add_sized([112.0, 32.0], egui::Button::new("选择目录"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.output_dir = path;
                }
            }
        });
    }

    fn render_status(&self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("任务状态")
                        .strong()
                        .size(16.0)
                        .color(INK),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if snapshot.error.is_some() {
                        "需要处理"
                    } else if snapshot.done {
                        "已完成"
                    } else if self.running {
                        "运行中"
                    } else {
                        "待开始"
                    };
                    status_badge(
                        ui,
                        label,
                        snapshot.error.is_some(),
                        snapshot.done || self.running,
                    );
                });
            });

            ui.add_space(12.0);
            let status_color = if snapshot.error.is_some() {
                DANGER
            } else {
                INK
            };
            ui.label(
                egui::RichText::new(&snapshot.status)
                    .size(18.0)
                    .strong()
                    .color(status_color),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::ProgressBar::new(snapshot.progress / 100.0)
                        .fill(if snapshot.error.is_some() {
                            DANGER
                        } else {
                            ACCENT
                        })
                        .desired_width((ui.available_width() - 56.0).max(120.0)),
                );
                ui.label(
                    egui::RichText::new(format!("{:.0}%", snapshot.progress))
                        .monospace()
                        .strong()
                        .color(if snapshot.error.is_some() {
                            DANGER
                        } else {
                            ACCENT_DARK
                        }),
                );
            });

            ui.add_space(14.0);
            render_pipeline(ui, snapshot.progress);

            ui.add_space(12.0);
            detail(ui, "任务 ID", &snapshot.task_id);
            detail(ui, "文件 ID", &snapshot.file_id);
            detail(ui, "Token 用量", &snapshot.usage);

            if let Some(path) = &snapshot.output_dir {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(egui::RichText::new("输出目录").strong().color(INK));
                ui.monospace(path.display().to_string());
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new("打开输出目录"),
                    )
                    .clicked()
                {
                    let _ = open_path(path);
                }
            }
            if let Some(error) = &snapshot.error {
                ui.add_space(10.0);
                error_box(ui, error);
            }
        });
    }

    fn render_preview(&self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        preview_frame().show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 28.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new("字幕预览")
                            .strong()
                            .size(16.0)
                            .color(INK),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let has_preview = has_subtitle_preview(&snapshot.preview);
                        if ui
                            .add_enabled(has_preview, egui::Button::new("复制字幕"))
                            .clicked()
                        {
                            ui.ctx().copy_text(snapshot.preview.clone());
                        }
                    });
                },
            );
            ui.add_space(6.0);

            if has_subtitle_preview(&snapshot.preview) {
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        let mut preview = snapshot.preview.clone();
                        ui.add(
                            egui::TextEdit::multiline(&mut preview)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12)
                                .interactive(false),
                        );
                    });
            } else {
                empty_preview(ui);
            }
        });
    }

    fn start_job(&mut self) {
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
}

fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(18, 14))
}

fn preview_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(14, 12))
}

fn render_app_header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        #[cfg(target_os = "macos")]
        ui.add_space(60.0);

        ui.vertical(|ui| {
            ui.heading(
                egui::RichText::new("视频字幕工作台")
                    .size(26.0)
                    .strong()
                    .color(INK),
            );
            ui.label(
                egui::RichText::new("本地分离音频，调用 MOSS 转写，生成可编辑字幕文件")
                    .size(14.0)
                    .color(MUTED),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            egui::Frame::NONE
                .fill(ACCENT_SOFT)
                .corner_radius(999.0)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("本地音频处理")
                            .color(ACCENT_DARK)
                            .strong()
                            .size(13.0),
                    );
                });
        });
    });
}

#[cfg(target_os = "macos")]
fn app_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_app_id("cn.mtd.subtitle-app")
        .with_title("MTD 字幕工作台")
        .with_fullsize_content_view(true)
        .with_title_shown(false)
        .with_titlebar_shown(false)
        .with_movable_by_background(true)
        .with_inner_size([1040.0, 760.0])
        .with_min_inner_size([860.0, 620.0])
}

#[cfg(not(target_os = "macos"))]
fn app_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_app_id("cn.mtd.subtitle-app")
        .with_title("MTD 字幕工作台")
        .with_inner_size([1040.0, 760.0])
        .with_min_inner_size([860.0, 620.0])
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(13.0).strong().color(MUTED));
}

fn path_pill(ui: &mut egui::Ui, text: &str, selected: bool) {
    let fill = if selected {
        egui::Color32::from_rgb(246, 249, 250)
    } else {
        egui::Color32::from_rgb(241, 245, 247)
    };
    let color = if selected { INK } else { FAINT };
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 120.0).max(160.0));
            ui.label(egui::RichText::new(text).color(color));
        });
}

fn status_badge(ui: &mut egui::Ui, label: &str, is_error: bool, is_active: bool) {
    let (fill, text) = if is_error {
        (egui::Color32::from_rgb(252, 235, 233), DANGER)
    } else if is_active {
        (ACCENT_SOFT, ACCENT_DARK)
    } else {
        (egui::Color32::from_rgb(238, 242, 244), MUTED)
    };
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(12.0).strong().color(text));
        });
}

fn render_pipeline(ui: &mut egui::Ui, progress: f32) {
    let steps = [
        ("分离音频", 12.0),
        ("上传音频", 28.0),
        ("转写处理", 58.0),
        ("生成字幕", 82.0),
    ];
    for (index, (label, threshold)) in steps.iter().enumerate() {
        let active = progress >= *threshold;
        ui.horizontal(|ui| {
            let number_fill = if active {
                ACCENT
            } else {
                egui::Color32::from_rgb(232, 238, 241)
            };
            let number_text = if active { egui::Color32::WHITE } else { FAINT };
            egui::Frame::NONE
                .fill(number_fill)
                .corner_radius(999.0)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new((index + 1).to_string())
                            .size(12.0)
                            .strong()
                            .color(number_text),
                    );
                });
            ui.label(egui::RichText::new(*label).size(13.0).color(if active {
                INK
            } else {
                FAINT
            }));
        });
    }
}

fn empty_preview(ui: &mut egui::Ui) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(247, 250, 251))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(226, 233, 236),
        ))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_height(112.0);
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("等待生成字幕")
                        .size(17.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new(
                        "选择视频并填写 API Key 后，会在这里显示可复制的 SRT 预览。",
                    )
                    .color(MUTED),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 320.0).max(0.0) / 2.0);
                    output_chip(ui, "SRT");
                    output_chip(ui, "VTT");
                    output_chip(ui, "TXT");
                    output_chip(ui, "JSON");
                });
            });
        });
}

fn install_app_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.interact_size = egui::vec2(44.0, 36.0);
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = SURFACE;
    style.visuals.faint_bg_color = egui::Color32::from_rgb(238, 243, 245);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(247, 250, 251);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(246, 249, 250);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = ACCENT_SOFT;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(204, 232, 228);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT_DARK);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.hyperlink_color = ACCENT_DARK;
    ctx.set_style(style);
}

fn detail(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(13.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(value)
                    .monospace()
                    .size(13.0)
                    .color(if value == "-" { FAINT } else { INK }),
            );
        });
    });
}

fn output_chip(ui: &mut egui::Ui, label: &str) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(239, 245, 247))
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .size(12.0)
                    .strong()
                    .color(MUTED),
            );
        });
}

fn error_box(ui: &mut egui::Ui, error: &str) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(253, 241, 239))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(239, 196, 191),
        ))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("任务失败")
                    .strong()
                    .size(13.0)
                    .color(DANGER),
            );
            ui.label(egui::RichText::new(error).size(13.0).color(DANGER));
        });
}

fn has_subtitle_preview(preview: &str) -> bool {
    preview.contains("-->") || preview.lines().take(3).any(|line| line.trim() == "WEBVTT")
}

fn update_job(job: &Arc<Mutex<JobSnapshot>>, status: &str, progress: f32, preview: Option<String>) {
    let mut state = job.lock().expect("job lock");
    state.status = status.to_owned();
    state.progress = progress;
    if let Some(preview) = preview {
        state.preview = preview;
    }
}

fn run_job(
    job: &Arc<Mutex<JobSnapshot>>,
    video_path: PathBuf,
    output_root: PathBuf,
    api_key: String,
    model: String,
    max_tokens: u32,
    include_speaker: bool,
    burn_in: bool,
) -> Result<()> {
    let job_dir = output_root.join(format!("MTD字幕-{}", unix_timestamp()));
    fs::create_dir_all(&job_dir)
        .with_context(|| format!("无法创建输出目录：{}", job_dir.display()))?;

    let input_copy = job_dir.join(safe_filename(
        video_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("video.mp4"),
    ));
    fs::copy(&video_path, &input_copy).with_context(|| "无法复制视频到输出目录")?;

    let audio_path = job_dir.join("audio.m4a");
    let srt_path = job_dir.join("subtitles.srt");
    let vtt_path = job_dir.join("subtitles.vtt");
    let json_path = job_dir.join("transcript.json");
    let text_path = job_dir.join("transcript.txt");
    let subtitled_path = job_dir.join("subtitled.mp4");

    update_job(job, "正在分离音频", 12.0, None);
    extract_audio(&input_copy, &audio_path)?;

    update_job(job, "正在上传音频到 MOSS", 28.0, None);
    let client = Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let upload = upload_audio(&client, &api_key, &audio_path)?;
    let file_id = upload
        .get("file_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("上传响应缺少 file_id"))?
        .to_owned();
    {
        let mut state = job.lock().expect("job lock");
        state.file_id = file_id.clone();
    }

    update_job(job, "正在创建转写任务", 42.0, None);
    let task = create_asr_task(&client, &api_key, &file_id, &model, max_tokens)?;
    let task_id = task
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("创建任务响应缺少 task_id"))?
        .to_owned();
    {
        let mut state = job.lock().expect("job lock");
        state.task_id = task_id.clone();
    }

    update_job(job, "MOSS 正在转写和区分说话人", 58.0, None);
    let result = poll_task(&client, &api_key, &task_id, job)?;
    if result.get("status").and_then(Value::as_str) == Some("FAILED") {
        let message = result
            .get("error_message")
            .and_then(Value::as_str)
            .unwrap_or("转写任务失败");
        return Err(anyhow!(message.to_owned()));
    }

    update_job(job, "正在生成字幕文件", 82.0, None);
    let result_text = result
        .get("result_text")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("转写结果缺少 result_text"))?;
    let transcript: Value =
        serde_json::from_str(result_text).context("无法解析 result_text JSON")?;
    let segments = normalize_segments(&transcript)?;
    if segments.is_empty() {
        return Err(anyhow!("转写成功，但没有可用字幕片段"));
    }

    fs::write(&json_path, serde_json::to_vec_pretty(&transcript)?)?;
    let full_text = transcript
        .get("full_text")
        .and_then(Value::as_str)
        .unwrap_or("");
    fs::write(&text_path, format!("{full_text}\n"))?;
    write_srt(&srt_path, &segments, include_speaker)?;
    write_vtt(&vtt_path, &segments, include_speaker)?;

    if burn_in {
        update_job(job, "正在把字幕烧录回视频", 90.0, None);
        burn_subtitles(&input_copy, &srt_path, &subtitled_path)?;
    }

    let usage = result
        .get("usage")
        .and_then(|usage| usage.get("total_tokens"))
        .map(|value| format!("{value} total"))
        .unwrap_or_else(|| "-".to_owned());
    let preview = fs::read_to_string(&srt_path)?;
    let mut state = job.lock().expect("job lock");
    state.status = "完成".to_owned();
    state.progress = 100.0;
    state.usage = usage;
    state.preview = preview;
    state.output_dir = Some(job_dir);
    state.done = true;
    Ok(())
}

fn upload_audio(client: &Client, api_key: &str, audio_path: &Path) -> Result<Value> {
    let form = multipart::Form::new()
        .file("file", audio_path)
        .with_context(|| format!("无法读取音频文件：{}", audio_path.display()))?;
    let response = client
        .post(format!("{BASE_URL}/api/v1/files/upload"))
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .context("上传音频失败")?
        .error_for_status()
        .context("MOSS 音频上传返回错误")?;
    Ok(response.json()?)
}

fn create_asr_task(
    client: &Client,
    api_key: &str,
    file_id: &str,
    model: &str,
    max_tokens: u32,
) -> Result<Value> {
    let payload = json!({
        "file_id": file_id,
        "model": model,
        "sampling_params": { "max_new_tokens": max_tokens }
    });
    let response = client
        .post(format!("{BASE_URL}/api/v1/asr/tasks"))
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .context("创建转写任务失败")?
        .error_for_status()
        .context("MOSS 创建任务返回错误")?;
    Ok(response.json()?)
}

fn poll_task(
    client: &Client,
    api_key: &str,
    task_id: &str,
    job: &Arc<Mutex<JobSnapshot>>,
) -> Result<Value> {
    let started = Instant::now();
    while started.elapsed() < POLL_TIMEOUT {
        let response = client
            .get(format!("{BASE_URL}/api/v1/asr/tasks/{task_id}"))
            .bearer_auth(api_key)
            .send()
            .context("查询转写任务失败")?
            .error_for_status()
            .context("MOSS 查询任务返回错误")?;
        let result: Value = response.json()?;
        match result.get("status").and_then(Value::as_str) {
            Some("SUCCESS") | Some("FAILED") => return Ok(result),
            Some(status) => {
                let progress = 58.0
                    + (started.elapsed().as_secs_f32() / POLL_TIMEOUT.as_secs_f32() * 20.0)
                        .min(20.0);
                update_job(job, &format!("MOSS 任务状态：{status}"), progress, None);
            }
            None => update_job(job, "MOSS 正在处理", 62.0, None),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(anyhow!("等待转写结果超时，请稍后检查任务状态：{task_id}"))
}

fn normalize_segments(transcript: &Value) -> Result<Vec<Segment>> {
    let segments = transcript
        .get("segments")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("模型结果缺少 segments 字段"))?;

    let mut normalized = Vec::new();
    for segment in segments {
        let text = segment
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            continue;
        }
        let start = seconds_from_value(
            segment
                .get("start_s")
                .or_else(|| segment.get("start"))
                .or_else(|| segment.get("start_ms")),
        )?;
        let mut end = seconds_from_value(
            segment
                .get("end_s")
                .or_else(|| segment.get("end"))
                .or_else(|| segment.get("end_ms")),
        )?;
        if end <= start {
            end = start + 1.0;
        }
        normalized.push(Segment {
            start,
            end,
            speaker: segment
                .get("speaker")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_owned(),
            text: text.to_owned(),
        });
    }
    Ok(normalized)
}

fn seconds_from_value(value: Option<&Value>) -> Result<f64> {
    let Some(value) = value else {
        return Ok(0.0);
    };
    if let Some(number) = value.as_f64() {
        return Ok(number);
    }
    let text = value.as_str().unwrap_or("0");
    let number = text
        .parse::<f64>()
        .with_context(|| format!("时间戳格式无效：{text}"))?;
    if number > 1000.0 && !text.contains('.') {
        Ok(number / 1000.0)
    } else {
        Ok(number)
    }
}

fn write_srt(path: &Path, segments: &[Segment], include_speaker: bool) -> Result<()> {
    let mut file = fs::File::create(path)?;
    for (index, segment) in segments.iter().enumerate() {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("{}: {}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        writeln!(
            file,
            "{}\n{} --> {}\n{}\n",
            index + 1,
            srt_time(segment.start),
            srt_time(segment.end),
            text
        )?;
    }
    Ok(())
}

fn write_vtt(path: &Path, segments: &[Segment], include_speaker: bool) -> Result<()> {
    let mut file = fs::File::create(path)?;
    writeln!(file, "WEBVTT\n")?;
    for segment in segments {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("<v {}>{}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        writeln!(
            file,
            "{} --> {}\n{}\n",
            vtt_time(segment.start),
            vtt_time(segment.end),
            text
        )?;
    }
    Ok(())
}

fn srt_time(seconds: f64) -> String {
    let millis = (seconds.max(0.0) * 1000.0).round() as u64;
    let hours = millis / 3_600_000;
    let minutes = (millis % 3_600_000) / 60_000;
    let secs = (millis % 60_000) / 1000;
    let ms = millis % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02},{ms:03}")
}

fn vtt_time(seconds: f64) -> String {
    srt_time(seconds).replace(',', ".")
}

fn extract_audio(video_path: &Path, audio_path: &Path) -> Result<()> {
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

fn burn_subtitles(video_path: &Path, srt_path: &Path, output_path: &Path) -> Result<()> {
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

fn install_app_fonts(ctx: &egui::Context) {
    let Some((font_name, font_path)) = find_ui_font() else {
        return;
    };
    let Ok(font_bytes) = fs::read(&font_path) else {
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

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn escape_subtitle_filter_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

fn safe_filename(name: &str) -> String {
    let mut output = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    let trimmed = output.trim_matches(['.', '_']);
    if trimmed.is_empty() {
        "video.mp4".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn open_path(path: &Path) -> Result<()> {
    if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()?;
    } else if cfg!(windows) {
        Command::new("explorer").arg(path).spawn()?;
    } else {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

fn default_output_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: app_viewport(),
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            builder
                .with_activation_policy(ActivationPolicy::Regular)
                .with_activate_ignoring_other_apps(true);
        })),
        run_and_return: false,
        ..Default::default()
    };
    eframe::run_native(
        "MTD 字幕工作台",
        options,
        Box::new(|cc| {
            install_app_fonts(&cc.egui_ctx);
            install_app_style(&cc.egui_ctx);
            Ok(Box::new(MtdApp::default()))
        }),
    )
}
