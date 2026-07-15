use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use eframe::egui;

use crate::{
    app_settings::{self, AppSettings, RecentProject},
    config::{DEFAULT_MODEL, MODELS},
    fonts::{self, SubtitleFont},
    media::{SubtitleBurnOptions, burn_subtitles},
    models::{JobSnapshot, PreviewMode, SubtitleExportFormat},
    native_menu,
    pipeline::run_job,
    project::{load_project, save_project},
    secret_store::{self, ApiKeyStorage},
    subtitles::{
        render_srt, render_srt_preview, render_subtitle_json, render_txt, render_vtt, write_srt,
        write_vtt,
    },
    theme::{BORDER, CANVAS, WINDOW_CORNER_RADIUS},
    video_preview::VideoPreview,
};

#[cfg(not(target_os = "macos"))]
const RESIZE_HANDLE_SIZE: f32 = 6.0;
#[cfg(not(target_os = "macos"))]
const RESIZE_CORNER_SIZE: f32 = 16.0;

struct SettingsSaveResult {
    generation: u64,
    result: Result<(), String>,
}

pub(crate) struct MtdApp {
    pub(crate) video_path: Option<PathBuf>,
    pub(crate) new_project_root: PathBuf,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) include_speaker: bool,
    pub(crate) subtitle_fonts: Vec<SubtitleFont>,
    pub(crate) selected_subtitle_font: Option<String>,
    pub(crate) subtitle_font_size: u32,
    pub(crate) subtitle_font_size_text: String,
    pub(crate) settings_store_message: Option<String>,
    pub(crate) settings_store_error: bool,
    settings_save_generation: Arc<AtomicU64>,
    settings_save_result: Arc<Mutex<Option<SettingsSaveResult>>>,
    settings_save_write_lock: Arc<Mutex<()>>,
    pub(crate) remember_api_key: bool,
    pub(crate) saved_api_key: Option<String>,
    pub(crate) api_key_store_message: Option<String>,
    pub(crate) api_key_store_error: bool,
    pub(crate) model_picker_open: bool,
    pub(crate) preview_mode: PreviewMode,
    pub(crate) review_split_ratio: f32,
    pub(crate) video_preview: VideoPreview,
    pub(crate) speaker_names: BTreeMap<String, String>,
    pub(crate) time_edits: BTreeMap<usize, (String, String)>,
    pub(crate) job: Arc<Mutex<JobSnapshot>>,
    pub(crate) running: bool,
    pub(crate) burning: bool,
    pub(crate) recent_projects: Vec<RecentProject>,
    last_registered_project: Option<PathBuf>,
}

impl Default for MtdApp {
    fn default() -> Self {
        let app_settings = match app_settings::load_app_settings() {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!("读取应用设置失败：{error}");
                AppSettings::default()
            }
        };
        let subtitle_fonts = fonts::discover_subtitle_fonts();
        let selected_subtitle_font =
            choose_subtitle_font(&subtitle_fonts, app_settings.subtitle_font.as_deref());
        let subtitle_font_size = app_settings.subtitle_font_size;
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
            new_project_root: app_settings.new_project_root,
            api_key,
            model: valid_model_or_default(&app_settings.model),
            max_tokens: app_settings.max_tokens,
            include_speaker: app_settings.include_speaker,
            subtitle_fonts,
            selected_subtitle_font,
            subtitle_font_size,
            subtitle_font_size_text: subtitle_font_size.to_string(),
            settings_store_message: None,
            settings_store_error: false,
            settings_save_generation: Arc::new(AtomicU64::new(0)),
            settings_save_result: Arc::new(Mutex::new(None)),
            settings_save_write_lock: Arc::new(Mutex::new(())),
            remember_api_key,
            saved_api_key,
            api_key_store_message,
            api_key_store_error,
            model_picker_open: false,
            preview_mode: PreviewMode::Rendered,
            review_split_ratio: 0.47,
            video_preview: VideoPreview::default(),
            speaker_names: BTreeMap::new(),
            time_edits: BTreeMap::new(),
            job: Arc::new(Mutex::new(JobSnapshot::default())),
            running: false,
            burning: false,
            recent_projects: app_settings.recent_projects,
            last_registered_project: None,
        }
    }
}

impl eframe::App for MtdApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if cfg!(target_os = "macos") {
            CANVAS.to_normalized_gamma_f32()
        } else {
            egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_settings_save_result();
        let snapshot = self.job.lock().expect("job lock").clone();
        if snapshot.done {
            self.running = false;
            if let Some(path) = snapshot.project_path.as_ref()
                && self.last_registered_project.as_ref() != Some(path)
            {
                self.remember_recent_project(path.clone(), project_status(&snapshot));
            }
        }
        if self.burning && !snapshot.status.contains("正在添加字幕") {
            self.burning = false;
        }
        native_menu::install_file_menu();
        native_menu::adjust_window_controls();
        if native_menu::take_open_project_request() {
            self.open_project_dialog();
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::O)) {
            self.open_project_dialog();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let shell_rect = ui.max_rect().shrink(0.5);
                ui.painter().rect_filled(
                    shell_rect,
                    egui::CornerRadius::same(WINDOW_CORNER_RADIUS),
                    CANVAS,
                );
                ui.painter().rect_stroke(
                    shell_rect,
                    egui::CornerRadius::same(WINDOW_CORNER_RADIUS),
                    egui::Stroke::new(1.0_f32, BORDER),
                    egui::StrokeKind::Inside,
                );

                #[cfg(not(target_os = "macos"))]
                render_resize_handles(ui, shell_rect);

                ui.scope_builder(egui::UiBuilder::new().max_rect(shell_rect), |ui| {
                    self.render_header(ui);
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(22, 16))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(14.0, 12.0);
                            let height = ui.available_height();
                            ui.horizontal_top(|ui| {
                                let sidebar_width = 380.0_f32.min(ui.available_width() * 0.34);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(sidebar_width, height),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| self.render_workspace(ui, &snapshot, height),
                                );
                                ui.add_space(12.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), height),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| self.render_review_area(ui, &snapshot),
                                );
                            });
                        });
                });
            });

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if self.burning {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if self.video_preview.is_playing() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn render_resize_handles(ui: &mut egui::Ui, rect: egui::Rect) {
    let maximized = ui
        .ctx()
        .input(|input| input.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }

    let left = rect.left();
    let right = rect.right();
    let top = rect.top();
    let bottom = rect.bottom();
    let edge = RESIZE_HANDLE_SIZE;
    let corner = RESIZE_CORNER_SIZE;
    let handles = [
        (
            "resize_n",
            egui::Rect::from_min_max(
                egui::pos2(left + corner, top),
                egui::pos2(right - corner, top + edge),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeNorth,
        ),
        (
            "resize_s",
            egui::Rect::from_min_max(
                egui::pos2(left + corner, bottom - edge),
                egui::pos2(right - corner, bottom),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeSouth,
        ),
        (
            "resize_e",
            egui::Rect::from_min_max(
                egui::pos2(right - edge, top + corner),
                egui::pos2(right, bottom - corner),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeEast,
        ),
        (
            "resize_w",
            egui::Rect::from_min_max(
                egui::pos2(left, top + corner),
                egui::pos2(left + edge, bottom - corner),
            ),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeWest,
        ),
        (
            "resize_ne",
            egui::Rect::from_min_max(
                egui::pos2(right - corner, top),
                egui::pos2(right, top + corner),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNorthEast,
        ),
        (
            "resize_nw",
            egui::Rect::from_min_max(
                egui::pos2(left, top),
                egui::pos2(left + corner, top + corner),
            ),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNorthWest,
        ),
        (
            "resize_se",
            egui::Rect::from_min_max(
                egui::pos2(right - corner, bottom - corner),
                egui::pos2(right, bottom),
            ),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeSouthEast,
        ),
        (
            "resize_sw",
            egui::Rect::from_min_max(
                egui::pos2(left, bottom - corner),
                egui::pos2(left + corner, bottom),
            ),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeSouthWest,
        ),
    ];

    for (id, handle_rect, direction, cursor) in handles {
        let response = ui
            .interact(handle_rect, ui.id().with(id), egui::Sense::click_and_drag())
            .on_hover_cursor(cursor);
        if response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

impl MtdApp {
    pub(crate) fn open_project_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("打开 MOSS-Subtitle-Workbench 项目")
            .add_filter("MOSS-Subtitle-Workbench 项目", &["json"])
            .pick_file()
        else {
            return;
        };
        self.load_project_file(path);
    }

    pub(crate) fn load_project_file(&mut self, path: PathBuf) {
        match load_project(&path) {
            Ok(snapshot) => {
                self.running = false;
                self.burning = false;
                self.video_preview.reset();
                self.speaker_names.clear();
                self.time_edits.clear();
                self.video_path = snapshot
                    .input_media_path
                    .clone()
                    .or_else(|| snapshot.input_video_path.clone());
                let status = project_status(&snapshot);
                *self.job.lock().expect("job lock") = snapshot;
                self.remember_recent_project(path, status);
            }
            Err(error) => {
                let mut state = self.job.lock().expect("job lock");
                state.status = "打开项目失败".to_owned();
                state.progress = 100.0;
                state.done = true;
                state.error = Some(error.to_string());
            }
        }
    }

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

    pub(crate) fn save_current_settings(&mut self) {
        let settings = self.current_settings();
        let generation = self.settings_save_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let generation_handle = Arc::clone(&self.settings_save_generation);
        let result_handle = Arc::clone(&self.settings_save_result);
        let write_lock = Arc::clone(&self.settings_save_write_lock);
        self.settings_store_error = false;

        thread::spawn(move || {
            let _write_guard = write_lock.lock().expect("settings save write lock");
            if generation_handle.load(Ordering::SeqCst) != generation {
                return;
            }
            let result =
                app_settings::save_app_settings(&settings).map_err(|error| error.to_string());

            if generation_handle.load(Ordering::SeqCst) == generation {
                *result_handle.lock().expect("settings save result lock") =
                    Some(SettingsSaveResult { generation, result });
            }
        });
    }

    fn poll_settings_save_result(&mut self) {
        let save_result = self
            .settings_save_result
            .lock()
            .expect("settings save result lock")
            .take();
        let Some(save_result) = save_result else {
            return;
        };
        if save_result.generation != self.settings_save_generation.load(Ordering::SeqCst) {
            return;
        }

        match save_result.result {
            Ok(()) => {
                self.settings_store_message = Some("设置已保存到本机".to_owned());
                self.settings_store_error = false;
            }
            Err(error) => {
                self.settings_store_message = Some(format!("保存设置失败：{error}"));
                self.settings_store_error = true;
            }
        }
    }

    fn current_settings(&self) -> AppSettings {
        AppSettings {
            new_project_root: self.new_project_root.clone(),
            model: self.model.clone(),
            max_tokens: self.max_tokens.clamp(1_000, 96_000),
            include_speaker: self.include_speaker,
            subtitle_font: self.selected_subtitle_font.clone(),
            subtitle_font_size: self.subtitle_font_size.clamp(12, 96),
            recent_projects: self.recent_projects.clone(),
        }
    }

    pub(crate) fn start_job(&mut self) {
        let Some(video_path) = self.video_path.clone() else {
            return;
        };
        let api_key = self.api_key.trim().to_owned();
        let new_project_root = self.new_project_root.clone();
        let model = self.model.clone();
        let max_tokens = self.max_tokens.clamp(1_000, 96_000);
        let include_speaker = self.include_speaker;
        let job = Arc::clone(&self.job);

        self.save_current_settings();
        self.running = true;
        self.last_registered_project = None;
        self.video_preview.reset();
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
                new_project_root,
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

    pub(crate) fn clear_recent_projects(&mut self) {
        self.recent_projects.clear();
        self.last_registered_project = None;
        self.save_current_settings();
    }

    fn remember_recent_project(&mut self, path: PathBuf, status: String) {
        let opened_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let changed = upsert_recent_project(&mut self.recent_projects, &path, status, opened_at);
        self.last_registered_project = Some(path);
        if changed {
            self.save_current_settings();
        }
    }

    pub(crate) fn can_burn(&self, snapshot: &JobSnapshot) -> bool {
        !self.running
            && !self.burning
            && snapshot.done
            && snapshot.error.is_none()
            && !snapshot
                .segments
                .iter()
                .any(|segment| segment.has_invalid_time())
            && snapshot.input_video_path.is_some()
            && snapshot.srt_path.is_some()
            && snapshot.subtitled_path.is_some()
    }

    pub(crate) fn can_export_subtitles(&self, snapshot: &JobSnapshot) -> bool {
        !snapshot.segments.is_empty()
            && !snapshot
                .segments
                .iter()
                .any(|segment| segment.has_invalid_time())
    }

    pub(crate) fn export_subtitle_file(&mut self, format: SubtitleExportFormat) {
        let snapshot = self.job.lock().expect("job lock").clone();
        if !self.can_export_subtitles(&snapshot) {
            self.set_export_error("请先修正字幕时间轴后再导出");
            return;
        }

        let default_name = default_export_file_name(&snapshot, format);
        let mut dialog = rfd::FileDialog::new()
            .set_title("导出通用字幕文件")
            .set_file_name(&default_name)
            .add_filter(format.label(), &[format.extension()]);
        if let Some(output_dir) = snapshot
            .output_dir
            .as_ref()
            .or(Some(&self.new_project_root))
        {
            dialog = dialog.set_directory(output_dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        let path = path_with_extension(path, format.extension());

        let result = render_export_text(&snapshot, format)
            .and_then(|text| fs::write(&path, text).map_err(anyhow::Error::from));
        match result {
            Ok(()) => {
                let mut state = self.job.lock().expect("job lock");
                state.status = format!("已导出 {} 字幕", format.label());
                state.output_dir = path.parent().map(|path| path.to_path_buf());
                state.error = None;
            }
            Err(error) => {
                self.set_export_error(&format!("导出字幕失败：{error}"));
            }
        }
    }

    fn set_export_error(&mut self, message: &str) {
        let mut state = self.job.lock().expect("job lock");
        state.status = "导出失败".to_owned();
        state.error = Some(message.to_owned());
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
        let burn_options = self.subtitle_burn_options();

        self.burning = true;
        {
            let mut state = job.lock().expect("job lock");
            state.status = "正在添加字幕到视频".to_owned();
            state.error = None;
        }

        thread::spawn(move || {
            let result =
                burn_subtitles(&input_video_path, &srt_path, &subtitled_path, burn_options);
            let mut state = job.lock().expect("job lock");
            match result {
                Ok(()) => {
                    state.status = "添加完成".to_owned();
                    state.output_dir = subtitled_path.parent().map(|path| path.to_path_buf());
                    state.subtitled_path = Some(subtitled_path);
                    state.done = true;
                    state.error = None;
                    sync_project_file(&state);
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
        self.video_preview.invalidate();

        sync_subtitle_outputs(&mut state);
        sync_project_file(&state);
    }

    pub(crate) fn update_segment(
        &mut self,
        index: usize,
        start: f64,
        end: f64,
        speaker: String,
        text: String,
        clear_time_errors: bool,
    ) {
        let mut state = self.job.lock().expect("job lock");
        let Some(segment) = state.segments.get_mut(index) else {
            return;
        };

        segment.start = start.max(0.0);
        segment.end = end.max(segment.start + 0.001);
        segment.speaker = speaker.trim().to_owned();
        segment.text = text;
        if clear_time_errors {
            segment.raw_start = None;
            segment.raw_end = None;
            segment.start_valid = true;
            segment.end_valid = true;
        }
        sync_subtitle_outputs(&mut state);
        if !state
            .segments
            .iter()
            .any(|segment| segment.has_invalid_time())
        {
            state.status = "完成".to_owned();
            state.error = None;
        }
        sync_project_file(&state);
        drop(state);
        self.video_preview.invalidate();
    }

    pub(crate) fn subtitle_burn_options(&self) -> SubtitleBurnOptions {
        let font =
            fonts::selected_font(&self.subtitle_fonts, self.selected_subtitle_font.as_deref());
        SubtitleBurnOptions {
            font_family: self.selected_subtitle_font.clone(),
            font_size: self.subtitle_font_size.clamp(12, 96),
            fonts_dir: font.and_then(|font| font.source_dir.clone()),
        }
    }
}

fn project_status(snapshot: &JobSnapshot) -> String {
    if snapshot.error.is_some() {
        "转写失败".to_owned()
    } else if snapshot.done {
        "转写完成".to_owned()
    } else if snapshot.progress > 0.0 {
        "转写处理中".to_owned()
    } else {
        "等待转写".to_owned()
    }
}

fn saved_message(storage: ApiKeyStorage) -> String {
    format!("已保存到{}，下次打开会自动填入", storage.label())
}

fn valid_model_or_default(model: &str) -> String {
    if MODELS.contains(&model) {
        model.to_owned()
    } else {
        DEFAULT_MODEL.to_owned()
    }
}

fn choose_subtitle_font(fonts: &[SubtitleFont], saved: Option<&str>) -> Option<String> {
    if let Some(saved) = saved {
        if fonts.iter().any(|font| font.family == saved) {
            return Some(saved.to_owned());
        }
    }
    fonts
        .iter()
        .find(|font| font.family == "HarmonyOS Sans SC")
        .or_else(|| fonts.first())
        .map(|font| font.family.clone())
}

fn default_export_file_name(snapshot: &JobSnapshot, format: SubtitleExportFormat) -> String {
    snapshot
        .input_media_path
        .as_ref()
        .or(snapshot.input_video_path.as_ref())
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .map(|stem| format!("{stem}.{}", format.extension()))
        .unwrap_or_else(|| format.file_name().to_owned())
}

fn path_with_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    let has_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension));
    if !has_extension {
        path.set_extension(extension);
    }
    path
}

fn render_export_text(
    snapshot: &JobSnapshot,
    format: SubtitleExportFormat,
) -> anyhow::Result<String> {
    match format {
        SubtitleExportFormat::Srt => render_srt(&snapshot.segments, snapshot.include_speaker),
        SubtitleExportFormat::Vtt => render_vtt(&snapshot.segments, snapshot.include_speaker),
        SubtitleExportFormat::Txt => render_txt(&snapshot.segments, snapshot.include_speaker),
        SubtitleExportFormat::Json => {
            render_subtitle_json(&snapshot.segments, snapshot.include_speaker)
        }
    }
}

fn sync_subtitle_outputs(state: &mut JobSnapshot) {
    if state.segments.is_empty() {
        return;
    }

    state.preview = render_srt_preview(&state.segments, state.include_speaker);
    if state
        .segments
        .iter()
        .any(|segment| segment.has_invalid_time())
    {
        return;
    }
    if render_srt(&state.segments, state.include_speaker).is_ok() {
        if let Some(srt_path) = &state.srt_path {
            let _ = write_srt(srt_path, &state.segments, state.include_speaker);
        }
        if let Some(vtt_path) = &state.vtt_path {
            let _ = write_vtt(vtt_path, &state.segments, state.include_speaker);
        }
    }
}

fn sync_project_file(state: &JobSnapshot) {
    if let Some(project_path) = &state.project_path {
        let _ = save_project(project_path, state);
    }
}

fn upsert_recent_project(
    projects: &mut Vec<RecentProject>,
    path: &std::path::Path,
    status: String,
    opened_at: u64,
) -> bool {
    if let Some(project) = projects.iter_mut().find(|project| project.path == path) {
        if project.status == status {
            return false;
        }
        project.status = status;
        return true;
    }

    projects.insert(
        0,
        RecentProject {
            path: path.to_path_buf(),
            opened_at,
            status,
        },
    );
    projects.truncate(6);
    true
}

#[cfg(test)]
mod recent_project_tests {
    use super::upsert_recent_project;
    use crate::app_settings::RecentProject;
    use std::path::PathBuf;

    #[test]
    fn keeps_existing_project_order_and_access_time_stable() {
        let first = PathBuf::from("/projects/first/project.mtd.json");
        let second = PathBuf::from("/projects/second/project.mtd.json");
        let mut projects = vec![
            RecentProject {
                path: first.clone(),
                opened_at: 10,
                status: "转写完成".to_owned(),
            },
            RecentProject {
                path: second.clone(),
                opened_at: 20,
                status: "转写完成".to_owned(),
            },
        ];

        assert!(!upsert_recent_project(
            &mut projects,
            &second,
            "转写完成".to_owned(),
            999,
        ));
        assert_eq!(projects[0].path, first);
        assert_eq!(projects[1].path, second);
        assert_eq!(projects[1].opened_at, 20);
    }

    #[test]
    fn updates_status_in_place_and_only_prepends_new_projects() {
        let existing = PathBuf::from("/projects/existing/project.mtd.json");
        let new_project = PathBuf::from("/projects/new/project.mtd.json");
        let mut projects = vec![RecentProject {
            path: existing.clone(),
            opened_at: 10,
            status: "转写处理中".to_owned(),
        }];

        assert!(upsert_recent_project(
            &mut projects,
            &existing,
            "转写完成".to_owned(),
            500,
        ));
        assert_eq!(projects[0].opened_at, 10);
        assert_eq!(projects[0].status, "转写完成");

        assert!(upsert_recent_project(
            &mut projects,
            &new_project,
            "转写完成".to_owned(),
            600,
        ));
        assert_eq!(projects[0].path, new_project);
        assert_eq!(projects[0].opened_at, 600);
        assert_eq!(projects[1].path, existing);
    }
}
