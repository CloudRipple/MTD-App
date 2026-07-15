use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;

use crate::{
    app::MtdApp,
    app_settings::RecentProject,
    config::{APP_NAME, MODELS},
    job::update_job,
    media_types::{AUDIO_EXTENSIONS, VIDEO_EXTENSIONS, supported_extensions},
    models::{JobSnapshot, PreviewMode, Segment, SubtitleExportFormat},
    platform::open_path,
    theme::{
        ACCENT, ACCENT_DARK, ACCENT_SOFT, BORDER, DANGER, FAINT, INK, MUTED, SURFACE,
        WINDOW_CORNER_RADIUS, panel_frame,
    },
    video_preview::{VideoPreview, fallback_duration},
};

const PREVIEW_CHILD_VERTICAL_INSET: f32 = 24.0;
const PREVIEW_BORDER_RESERVE: f32 = 2.0;
const INLINE_FONT_ROW_HEIGHT: f32 = 24.0;
const VIDEO_HEADER_HEIGHT: f32 = 30.0;
const VIDEO_BUTTON_WIDTH: f32 = 58.0;
const VIDEO_TIME_WIDTH: f32 = 76.0;
const VIDEO_CONTROLS_HEIGHT: f32 = 34.0;
const VIDEO_PREVIEW_ASPECT: f32 = 16.0 / 9.0;
const OUTPUT_CHIP_HEIGHT: f32 = 26.0;
const OUTPUT_CHIP_GAP: f32 = 12.0;
const REVIEW_SPLITTER_HEIGHT: f32 = 14.0;
const MIN_MEDIA_REVIEW_HEIGHT: f32 = 230.0;
const MIN_SUBTITLE_REVIEW_HEIGHT: f32 = 180.0;
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_HEIGHT: f32 = 38.0;
#[cfg(not(target_os = "macos"))]
const TITLE_MENU_WIDTH: f32 = 132.0;
#[cfg(not(target_os = "macos"))]
const TITLE_BUTTON_WIDTH: f32 = 46.0;
#[cfg(not(target_os = "macos"))]
const TITLE_CONTROLS_WIDTH: f32 = TITLE_BUTTON_WIDTH * 3.0;

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Copy)]
enum TitleBarIcon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl MtdApp {
    pub(crate) fn render_header(&mut self, ui: &mut egui::Ui) {
        #[cfg(target_os = "macos")]
        self.render_content_header(ui);

        #[cfg(not(target_os = "macos"))]
        self.render_custom_title_bar(ui);
    }

    #[cfg(target_os = "macos")]
    fn render_content_header(&mut self, ui: &mut egui::Ui) {
        let (_, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 32.0),
            egui::Sense::click_and_drag(),
        );
        if response.double_clicked() {
            crate::native_menu::zoom_main_window();
        } else if response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn render_custom_title_bar(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), TITLE_BAR_HEIGHT),
            egui::Sense::hover(),
        );
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0_f32, BORDER),
        );

        let menu_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(14.0, 0.0),
            egui::vec2(TITLE_MENU_WIDTH, rect.height()),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(menu_rect), |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.render_file_menu(ui);
            });
        });

        let controls_min = egui::pos2(rect.max.x - TITLE_CONTROLS_WIDTH, rect.min.y);
        let drag_rect = egui::Rect::from_min_max(
            egui::pos2(menu_rect.max.x, rect.min.y),
            egui::pos2(controls_min.x, rect.max.y),
        );
        let drag_response = ui.interact(
            drag_rect,
            ui.id().with("custom_title_bar_drag"),
            egui::Sense::click_and_drag(),
        );
        let maximized = ui
            .ctx()
            .input(|input| input.viewport().maximized.unwrap_or(false));
        if drag_response.double_clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        } else if drag_response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            APP_NAME,
            egui::FontId::proportional(16.0),
            INK,
        );

        let minimize_rect = egui::Rect::from_min_size(
            controls_min,
            egui::vec2(TITLE_BUTTON_WIDTH, TITLE_BAR_HEIGHT),
        );
        let maximize_rect = minimize_rect.translate(egui::vec2(TITLE_BUTTON_WIDTH, 0.0));
        let close_rect = maximize_rect.translate(egui::vec2(TITLE_BUTTON_WIDTH, 0.0));
        if title_bar_button(ui, minimize_rect, "minimize", TitleBarIcon::Minimize).clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let maximize_icon = if maximized {
            TitleBarIcon::Restore
        } else {
            TitleBarIcon::Maximize
        };
        if title_bar_button(ui, maximize_rect, "maximize", maximize_icon).clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        if title_bar_button(ui, close_rect, "close", TitleBarIcon::Close).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn render_file_menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("文件", |ui| {
                if ui.button("打开项目...").clicked() {
                    ui.close();
                    self.open_project_dialog();
                }
                if ui.button("打开输出目录").clicked() {
                    ui.close();
                    let _ = open_path(&self.new_project_root);
                }
            });
        });
    }

    pub(crate) fn render_workspace(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &JobSnapshot,
        height: f32,
    ) {
        panel_frame().show(ui, |ui| {
            ui.set_min_height(review_panel_body_height(height));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("任务").strong().size(16.0).color(INK));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.render_settings_menu(ui);
                });
            });

            ui.add_space(10.0);
            self.render_file_inputs(ui);
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(10.0);
            self.render_status_cluster(ui, snapshot);
            let remaining = ui.available_height();
            if remaining > 150.0 {
                let history_height = (remaining - 20.0).clamp(130.0, 270.0);
                ui.add_space(20.0);
                self.render_recent_projects(ui, history_height);
            }
        });
    }

    fn render_recent_projects(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("历史项目")
                    .size(14.0)
                    .strong()
                    .color(INK),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!self.recent_projects.is_empty(), egui::Button::new("清除"))
                    .clicked()
                {
                    self.clear_recent_projects();
                }
            });
        });
        ui.add_space(6.0);

        if self.recent_projects.is_empty() {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(248, 250, 251))
                .corner_radius(7.0)
                .inner_margin(egui::Margin::symmetric(10, 12))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("生成或打开项目后会显示在这里")
                            .size(12.0)
                            .color(FAINT),
                    );
                });
            return;
        }

        let projects = self.recent_projects.clone();
        let mut selected = None;
        egui::ScrollArea::vertical()
            .id_salt("recent-projects-scroll")
            .max_height((height - 34.0).max(80.0))
            .show(ui, |ui| {
                for project in &projects {
                    let available = project.path.is_file();
                    let response = recent_project_row(ui, project, available);
                    if available && response.clicked() {
                        selected = Some(project.path.clone());
                    }
                    ui.add_space(5.0);
                }
            });
        if let Some(path) = selected {
            self.load_project_file(path);
        }
    }

    fn render_file_inputs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            inline_config_label(ui, "输入媒体");
            let text = self
                .video_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| "尚未选择媒体文件".to_owned());
            let pill_width = path_pill_width(ui, 76.0);
            path_pill(ui, &text, self.video_path.is_some(), pill_width);
            if ui
                .add_sized([76.0, 32.0], egui::Button::new("选择"))
                .clicked()
            {
                let media_extensions = supported_extensions();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("媒体文件", &media_extensions)
                    .add_filter("视频", VIDEO_EXTENSIONS)
                    .add_filter("音频", AUDIO_EXTENSIONS)
                    .pick_file()
                {
                    self.video_path = Some(path);
                    update_job(&self.job, "已选择媒体，可以开始生成字幕", 0.0, None);
                }
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            inline_config_label(ui, "输出目录");
            let output_text = self.new_project_root.display().to_string();
            let pill_width = path_pill_width(ui, 76.0);
            path_pill(ui, &output_text, true, pill_width);
            if ui
                .add_sized([76.0, 32.0], egui::Button::new("更改"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.new_project_root = path;
                    self.save_current_settings();
                    ui.ctx().request_repaint();
                }
            }
        });

        ui.add_space(7.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            inline_field_label(ui, "字幕字体");
            let response = inline_font_size_control(ui, &mut self.subtitle_font_size_text);
            self.commit_subtitle_font_size(response.changed(), response.lost_focus());
        });
        self.render_font_selector(ui);

        ui.add_space(7.0);
        let hint = if self.api_key.trim().is_empty() {
            "转写设置里填写 API Key 后即可开始。"
        } else {
            "配置已就绪，可以开始生成字幕。"
        };
        ui.label(egui::RichText::new(hint).size(13.0).color(FAINT));
        if self.api_key.trim().is_empty() {
            ui.add_space(2.0);
            ui.hyperlink_to(
                egui::RichText::new("获取 API Key")
                    .size(13.0)
                    .strong()
                    .color(ACCENT_DARK),
                "https://studio.mosi.cn/account/api-keys",
            );
        }
        if self.settings_store_error {
            if let Some(message) = &self.settings_store_message {
                ui.add_space(2.0);
                ui.label(egui::RichText::new(message).size(12.0).color(DANGER));
            }
        }
    }

    fn render_font_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let text = self
                .selected_subtitle_font
                .as_deref()
                .filter(|font| !font.trim().is_empty())
                .unwrap_or("系统默认");
            let pill_width = (ui.available_width() - 124.0).max(160.0);
            font_pill(ui, text, pill_width);

            let response = ui.add_sized([92.0, 32.0], egui::Button::new("选择"));
            egui::Popup::from_toggle_button_response(&response)
                .width(360.0)
                .gap(8.0)
                .frame(settings_popup_frame())
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(360.0);
                    ui.label(
                        egui::RichText::new("添加字幕到视频时使用")
                            .size(13.0)
                            .strong()
                            .color(INK),
                    );
                    ui.label(
                        egui::RichText::new("从本机字体和随包字体中选择")
                            .size(12.0)
                            .color(FAINT),
                    );

                    ui.add_space(10.0);
                    if font_option(ui, "系统默认", self.selected_subtitle_font.is_none()).clicked()
                    {
                        self.selected_subtitle_font = None;
                        self.video_preview.invalidate();
                        self.save_current_settings();
                        ui.close();
                    }

                    ui.add_space(4.0);
                    let font_names = self
                        .subtitle_fonts
                        .iter()
                        .map(|font| font.family.clone())
                        .collect::<Vec<_>>();
                    egui::ScrollArea::vertical()
                        .id_salt("subtitle-fonts-scroll")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for font_name in font_names {
                                let selected =
                                    self.selected_subtitle_font.as_deref() == Some(&font_name);
                                if font_option(ui, &font_name, selected).clicked() {
                                    self.selected_subtitle_font = Some(font_name);
                                    self.video_preview.invalidate();
                                    self.save_current_settings();
                                    ui.close();
                                }
                            }
                        });
                });
        });
    }

    fn commit_subtitle_font_size(&mut self, changed: bool, finished: bool) {
        let trimmed = self.subtitle_font_size_text.trim().to_owned();
        if changed {
            if let Ok(size) = trimmed.parse::<u32>() {
                if (12..=96).contains(&size) {
                    self.subtitle_font_size = size;
                    self.video_preview.invalidate();
                    self.save_current_settings();
                }
            }
        }
        if finished {
            let size = trimmed
                .parse::<u32>()
                .unwrap_or(self.subtitle_font_size)
                .clamp(12, 96);
            self.subtitle_font_size = size;
            self.subtitle_font_size_text = size.to_string();
            self.video_preview.invalidate();
            self.save_current_settings();
        }
    }

    fn render_settings_menu(&mut self, ui: &mut egui::Ui) {
        let response = ui.add(
            egui::Button::new(
                egui::RichText::new("转写设置")
                    .size(13.0)
                    .strong()
                    .color(ACCENT_DARK),
            )
            .min_size(egui::vec2(96.0, 32.0))
            .fill(ACCENT_SOFT)
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(190, 226, 221),
            ))
            .corner_radius(8.0),
        );

        egui::Popup::from_toggle_button_response(&response)
            .width(420.0)
            .gap(8.0)
            .frame(settings_popup_frame())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(420.0);
                self.render_settings_panel(ui);
            });
    }

    fn render_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("转写设置")
                            .size(16.0)
                            .strong()
                            .color(INK),
                    );
                    ui.label(
                        egui::RichText::new("连接 MOSS 并控制字幕输出方式")
                            .size(12.0)
                            .color(FAINT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    soft_badge(
                        ui,
                        if self.api_key.trim().is_empty() {
                            "未配置"
                        } else {
                            "可开始"
                        },
                    );
                });
            });

            ui.add_space(12.0);
            let api_key_response = api_key_field(ui, &mut self.api_key);
            self.render_api_key_storage_controls(ui, api_key_response.changed());

            ui.add_space(10.0);
            let model_response = setting_row_button(
                ui,
                "模型",
                compact_model_name(&self.model),
                "选择用于异步转写的模型版本",
            );
            if model_response.clicked() {
                self.model_picker_open = !self.model_picker_open;
            }
            if self.model_picker_open {
                ui.add_space(8.0);
                setting_block(ui, |ui| {
                    field_label(ui, "模型版本");
                    ui.add_space(6.0);
                    for model in MODELS {
                        if model_option(ui, model, self.model == model).clicked() {
                            self.model = model.to_owned();
                            self.model_picker_open = false;
                            self.save_current_settings();
                        }
                    }
                });
            }

            ui.add_space(10.0);
            setting_block(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        field_label(ui, "最大 token");
                        ui.label(
                            egui::RichText::new("控制单次转写结果的最大长度")
                                .size(12.0)
                                .color(FAINT),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let response = ui.add(
                            egui::DragValue::new(&mut self.max_tokens)
                                .range(1_000..=96_000)
                                .speed(1000),
                        );
                        if response.changed() {
                            self.save_current_settings();
                        }
                    });
                });
            });

            ui.add_space(10.0);
            setting_block(ui, |ui| {
                if ui
                    .checkbox(&mut self.include_speaker, "保留说话人")
                    .changed()
                {
                    self.save_current_settings();
                }
            });
        });
    }

    fn render_start_button(&mut self, ui: &mut egui::Ui) {
        let can_start = self.can_start();
        let button_text = if self.running {
            "生成中"
        } else {
            "开始生成"
        };
        let button = egui::Button::new(egui::RichText::new(button_text).strong().color(
            if can_start {
                egui::Color32::WHITE
            } else {
                FAINT
            },
        ))
        .min_size(egui::vec2(112.0, 32.0))
        .fill(if can_start {
            ACCENT
        } else {
            egui::Color32::from_rgb(236, 241, 243)
        })
        .stroke(egui::Stroke::new(
            1.0_f32,
            if can_start {
                ACCENT
            } else {
                egui::Color32::from_rgb(226, 233, 236)
            },
        ))
        .corner_radius(8.0);

        if ui.add_enabled(can_start, button).clicked() {
            self.start_job();
            ui.ctx().request_repaint();
        }
    }

    fn render_burn_button(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        let can_burn = self.can_burn(snapshot);
        let button_text = if self.burning {
            "添加中"
        } else {
            "添加字幕到视频"
        };
        let button = egui::Button::new(
            egui::RichText::new(button_text)
                .strong()
                .color(if can_burn { ACCENT_DARK } else { FAINT }),
        )
        .min_size(egui::vec2(126.0, 32.0))
        .fill(if can_burn {
            ACCENT_SOFT
        } else {
            egui::Color32::from_rgb(236, 241, 243)
        })
        .stroke(egui::Stroke::new(
            1.0_f32,
            if can_burn {
                egui::Color32::from_rgb(190, 226, 221)
            } else {
                egui::Color32::from_rgb(226, 233, 236)
            },
        ))
        .corner_radius(8.0);

        if ui.add_enabled(can_burn, button).clicked() {
            self.burn_video();
        }
    }

    fn render_status_cluster(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        ui.horizontal(|ui| {
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                percent_badge(ui, snapshot.progress, snapshot.error.is_some());
            });
        });

        let status_color = if snapshot.error.is_some() {
            DANGER
        } else {
            INK
        };
        ui.add_space(7.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 56.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&snapshot.status)
                            .size(17.0)
                            .strong()
                            .color(status_color),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(stage_message(snapshot.progress, snapshot.done))
                            .size(13.0)
                            .color(FAINT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.render_start_button(ui);
                });
            },
        );

        ui.add_space(8.0);
        progress_track(ui, snapshot.progress, snapshot.error.is_some());

        ui.add_space(10.0);
        detail_grid(ui, snapshot);

        let (output_path, open_label) = match snapshot.output_dir.as_ref() {
            Some(project_dir) => (project_dir, "打开项目目录"),
            None => (&self.new_project_root, "打开输出目录"),
        };
        ui.add_space(8.0);
        if ui
            .add_sized([ui.available_width(), 32.0], egui::Button::new(open_label))
            .clicked()
        {
            let _ = open_path(output_path);
        }
        if let Some(error) = &snapshot.error {
            ui.add_space(8.0);
            error_box(ui, error);
        }
    }

    pub(crate) fn render_review_area(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        let panel_size = ui.available_size();
        let (panel_rect, _) = ui.allocate_exact_size(panel_size, egui::Sense::hover());
        ui.painter().rect(
            panel_rect,
            egui::CornerRadius::same(9),
            SURFACE,
            egui::Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );
        let content_rect = panel_rect.shrink(14.0);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                let available_height = ui.available_height();
                let (media_height, subtitle_height) =
                    review_section_heights(available_height, self.review_split_ratio);

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), media_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.render_media_review(ui, snapshot, media_height),
                );

                let splitter = review_splitter(ui);
                if splitter.dragged() {
                    let (content_height, media_min, media_max) =
                        review_media_height_bounds(available_height);
                    let pointer_delta = ui.ctx().input(|input| input.pointer.delta().y);
                    self.review_split_ratio = ((media_height + pointer_delta)
                        .clamp(media_min, media_max)
                        / content_height)
                        .clamp(0.0, 1.0);
                    ui.ctx().request_repaint();
                }

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), subtitle_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.render_subtitle_review(ui, snapshot, subtitle_height),
                );
            },
        );
    }

    fn render_media_review(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot, height: f32) {
        let has_media = snapshot.input_media_path.is_some() || snapshot.input_video_path.is_some();
        let has_video = snapshot.input_video_path.is_some();
        let label = if self.video_preview.is_playing() {
            "播放中"
        } else if self.video_preview.is_pending() {
            "预渲染"
        } else if !snapshot.segments.is_empty() {
            "可检查"
        } else {
            "待预览"
        };
        media_review_header(
            ui,
            if !has_media {
                "媒体预览"
            } else if has_video {
                "视频预览"
            } else {
                "音频预览"
            },
            label,
        );
        ui.add_space(6.0);

        let media_path = snapshot
            .input_video_path
            .as_deref()
            .or(snapshot.input_media_path.as_deref());
        let surface_height = (height - VIDEO_HEADER_HEIGHT - 6.0).max(120.0);
        let (Some(media_path), Some(srt_path)) = (media_path, snapshot.srt_path.as_deref()) else {
            media_empty_surface(ui, "选择媒体并生成字幕后可在这里播放检查", surface_height);
            return;
        };
        if snapshot.segments.is_empty() {
            media_empty_surface(ui, "生成字幕后可在这里播放检查", surface_height);
            return;
        }

        self.video_preview.prepare(
            ui.ctx(),
            media_path,
            srt_path,
            has_video,
            &snapshot.segments,
        );
        let fallback_duration = fallback_duration(&snapshot.segments);
        self.video_preview.update_playback(fallback_duration);
        if self.video_preview.is_playing() {
            ui.ctx().request_repaint();
        }
        self.video_preview
            .ensure_cache(ui.ctx(), self.subtitle_burn_options());
        self.video_preview.sync_frame(ui.ctx());

        let duration = self.video_preview.duration().unwrap_or(fallback_duration);
        let max_video_height =
            (height - VIDEO_HEADER_HEIGHT - VIDEO_CONTROLS_HEIGHT - 12.0).max(120.0);
        let (surface_width, surface_height) =
            fitted_video_surface_size(ui.available_width(), max_video_height);
        media_surface(
            ui,
            &self.video_preview,
            surface_width,
            surface_height,
            has_video,
        );
        ui.add_space(6.0);
        video_controls(ui, &mut self.video_preview, duration);
    }

    fn render_subtitle_review(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot, height: f32) {
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
                let speaker_labels = unique_speaker_labels(&snapshot.segments);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let has_preview = has_subtitle_preview(&snapshot.preview);
                    self.render_burn_button(ui, snapshot);
                    ui.add_space(8.0);
                    if ui
                        .add_enabled(has_preview, egui::Button::new("复制字幕"))
                        .clicked()
                    {
                        ui.ctx().copy_text(snapshot.preview.clone());
                    }
                    ui.add_space(8.0);
                    self.render_export_menu(ui, snapshot);
                    ui.add_space(8.0);
                    if has_preview && !speaker_labels.is_empty() {
                        self.render_speaker_editor(ui, &speaker_labels);
                        ui.add_space(8.0);
                    }
                    preview_mode_switch(ui, &mut self.preview_mode);
                });
            },
        );
        ui.add_space(8.0);
        let content_height = (height - 36.0).max(0.0);

        if has_subtitle_preview(&snapshot.preview) {
            match self.preview_mode {
                PreviewMode::Raw => raw_preview(ui, &snapshot.preview, content_height),
                PreviewMode::Rendered => {
                    self.render_rendered_preview(ui, &snapshot.segments, content_height);
                }
            }
        } else {
            empty_preview(ui, content_height);
        }
    }

    fn render_speaker_editor(&mut self, ui: &mut egui::Ui, speakers: &[String]) {
        let response = ui.add(
            egui::Button::new(
                egui::RichText::new("说话人")
                    .size(12.0)
                    .strong()
                    .color(ACCENT_DARK),
            )
            .min_size(egui::vec2(70.0, 26.0))
            .fill(ACCENT_SOFT)
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(190, 226, 221),
            ))
            .corner_radius(13.0),
        );

        egui::Popup::from_toggle_button_response(&response)
            .width(360.0)
            .gap(8.0)
            .frame(settings_popup_frame())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(360.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("说话人命名")
                                .size(15.0)
                                .strong()
                                .color(INK),
                        );
                        ui.label(
                            egui::RichText::new("将 S01 等标签替换为真实姓名")
                                .size(12.0)
                                .color(FAINT),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        soft_badge(ui, &format!("{} 位", speakers.len()));
                    });
                });

                ui.add_space(10.0);
                for speaker in speakers {
                    speaker_name_row(ui, speaker, &mut self.speaker_names);
                    ui.add_space(6.0);
                }

                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let has_any_name = self
                        .speaker_names
                        .values()
                        .any(|name| !name.trim().is_empty());
                    if ui
                        .add_enabled(has_any_name, primary_small_button("应用到字幕"))
                        .clicked()
                    {
                        self.apply_speaker_names();
                    }
                });
            });
    }

    fn render_export_menu(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        let can_export = self.can_export_subtitles(snapshot);
        let response = ui.add_enabled(can_export, egui::Button::new("导出字幕"));

        egui::Popup::from_toggle_button_response(&response)
            .width(220.0)
            .gap(8.0)
            .frame(settings_popup_frame())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(220.0);
                ui.label(
                    egui::RichText::new("通用字幕文件")
                        .size(14.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new("导出当前已编辑的字幕内容")
                        .size(12.0)
                        .color(FAINT),
                );
                ui.add_space(8.0);
                for format in SubtitleExportFormat::ALL {
                    if export_format_row(ui, format).clicked() {
                        self.export_subtitle_file(format);
                    }
                    ui.add_space(6.0);
                }
            });
    }

    fn render_api_key_storage_controls(&mut self, ui: &mut egui::Ui, key_changed: bool) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let remember_changed = ui
                .checkbox(&mut self.remember_api_key, "记住 API Key")
                .changed();
            if remember_changed {
                if self.remember_api_key {
                    self.save_api_key_to_store();
                } else {
                    self.forget_saved_api_key();
                }
            }

            let trimmed_key = self.api_key.trim();
            let has_unsaved_key = self.saved_api_key.as_deref() != Some(trimmed_key);
            let can_save = self.remember_api_key && !trimmed_key.is_empty() && has_unsaved_key;
            if ui
                .add_enabled(can_save, egui::Button::new("保存"))
                .clicked()
            {
                self.save_api_key_to_store();
            }

            if ui
                .add_enabled(self.saved_api_key.is_some(), egui::Button::new("忘记"))
                .clicked()
            {
                self.forget_saved_api_key();
            }
        });

        if key_changed && self.remember_api_key && self.saved_api_key.is_some() {
            self.api_key_store_message = Some("API Key 已修改，点击保存后下次打开生效".to_owned());
            self.api_key_store_error = false;
        }

        let message = self
            .api_key_store_message
            .as_deref()
            .unwrap_or("未记住时仅保存在当前运行内存中");
        let color = if self.api_key_store_error {
            DANGER
        } else if self.remember_api_key {
            ACCENT_DARK
        } else {
            FAINT
        };
        ui.label(egui::RichText::new(message).size(12.0).color(color));
    }

    fn render_rendered_preview(
        &mut self,
        ui: &mut egui::Ui,
        segments: &[Segment],
        content_height: f32,
    ) {
        if segments.is_empty() {
            empty_structured_preview(ui, content_height);
            return;
        }

        let speaker_names = self.speaker_names.clone();
        let mut edits = Vec::new();
        let current_time = self.video_preview.current_time();
        let active_index = snapshot_active_segment_index(segments, current_time);
        let follow_playback = self.video_preview.is_playing();
        egui::ScrollArea::vertical()
            .id_salt("rendered-subtitles-scroll")
            .max_height(content_height)
            .show(ui, |ui| {
                ui.set_min_height(content_height);
                for (index, segment) in segments.iter().enumerate() {
                    if let Some((start, end, speaker, text, clear_time_errors)) = segment_row(
                        ui,
                        index,
                        index + 1,
                        segment,
                        &speaker_names,
                        &mut self.time_edits,
                        active_index == Some(index),
                        follow_playback,
                    ) {
                        edits.push((index, start, end, speaker, text, clear_time_errors));
                    }
                    if index + 1 < segments.len() {
                        ui.add_space(6.0);
                    }
                }
            });

        for (index, start, end, speaker, text, clear_time_errors) in edits {
            self.update_segment(index, start, end, speaker, text, clear_time_errors);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn title_bar_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id_source: &'static str,
    icon: TitleBarIcon,
) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(id_source), egui::Sense::click());
    let close = matches!(icon, TitleBarIcon::Close);
    let fill = if response.hovered() {
        if close {
            DANGER
        } else {
            egui::Color32::from_rgb(226, 233, 237)
        }
    } else {
        egui::Color32::TRANSPARENT
    };
    let color = if close && response.hovered() {
        egui::Color32::WHITE
    } else {
        MUTED
    };
    let (fill_rect, corner_radius) = if close {
        (
            rect,
            egui::CornerRadius {
                ne: WINDOW_CORNER_RADIUS,
                ..egui::CornerRadius::ZERO
            },
        )
    } else {
        (
            rect.shrink2(egui::vec2(4.0, 5.0)),
            egui::CornerRadius::same(7),
        )
    };
    ui.painter().rect_filled(fill_rect, corner_radius, fill);
    paint_title_bar_icon(ui.painter(), rect, icon, color);
    response
}

#[cfg(not(target_os = "macos"))]
fn paint_title_bar_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    icon: TitleBarIcon,
    color: egui::Color32,
) {
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 14.0));
    let center = icon_rect.center();
    let stroke_width = 1.45;
    let stroke = egui::Stroke::new(stroke_width, color);
    match icon {
        TitleBarIcon::Minimize => {
            let bar = egui::Rect::from_center_size(
                center + egui::vec2(0.0, 3.5),
                egui::vec2(11.0, stroke_width),
            );
            painter.rect_filled(bar, 1.0, color);
        }
        TitleBarIcon::Maximize => {
            let box_rect = egui::Rect::from_center_size(center, egui::vec2(10.5, 10.5));
            painter.rect_stroke(box_rect, 2.0, stroke, egui::StrokeKind::Inside);
        }
        TitleBarIcon::Restore => {
            let back =
                egui::Rect::from_center_size(center + egui::vec2(2.0, -2.0), egui::vec2(9.0, 9.0));
            let front =
                egui::Rect::from_center_size(center + egui::vec2(-2.0, 2.0), egui::vec2(9.0, 9.0));
            painter.rect_stroke(back, 2.0, stroke, egui::StrokeKind::Inside);
            painter.rect_stroke(front, 2.0, stroke, egui::StrokeKind::Inside);
        }
        TitleBarIcon::Close => {
            rounded_line(
                painter,
                center + egui::vec2(-5.0, -5.0),
                center + egui::vec2(5.0, 5.0),
                stroke_width,
                color,
            );
            rounded_line(
                painter,
                center + egui::vec2(5.0, -5.0),
                center + egui::vec2(-5.0, 5.0),
                stroke_width,
                color,
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn rounded_line(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    width: f32,
    color: egui::Color32,
) {
    painter.line_segment([start, end], egui::Stroke::new(width, color));
    let radius = width * 0.5;
    painter.circle_filled(start, radius, color);
    painter.circle_filled(end, radius, color);
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(13.0).strong().color(MUTED));
}

fn inline_field_label(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(56.0, INLINE_FONT_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| field_label(ui, text),
    );
}

fn inline_config_label(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(60.0, 32.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| field_label(ui, text),
    );
}

fn path_pill_width(ui: &egui::Ui, trailing_button_width: f32) -> f32 {
    (ui.available_width() - trailing_button_width - ui.spacing().item_spacing.x).max(160.0)
}

fn path_pill(ui: &mut egui::Ui, text: &str, selected: bool, width: f32) {
    let fill = if selected {
        egui::Color32::from_rgb(246, 249, 250)
    } else {
        egui::Color32::from_rgb(241, 245, 247)
    };
    let color = if selected { INK } else { FAINT };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::hover());
    ui.painter().rect(
        rect,
        7.0,
        fill,
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );

    let inner_rect = rect.shrink2(egui::vec2(10.0, 7.0));
    ui.painter().with_clip_rect(inner_rect).text(
        egui::pos2(inner_rect.left(), inner_rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        color,
    );
    response.on_hover_text(text);
}

fn font_pill(ui: &mut egui::Ui, text: &str, width: f32) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(246, 249, 250))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width((width - 20.0).max(120.0));
            ui.label(egui::RichText::new(text).color(INK));
        });
}

fn inline_font_size_control(ui: &mut egui::Ui, value: &mut String) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(80.0, INLINE_FONT_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let response = compact_font_size_input(ui, value);
            ui.add_sized(
                [16.0, INLINE_FONT_ROW_HEIGHT],
                egui::Label::new(egui::RichText::new("号").size(13.0).strong().color(MUTED)),
            );
            response
        },
    )
    .inner
}

fn compact_font_size_input(ui: &mut egui::Ui, value: &mut String) -> egui::Response {
    let (rect, frame_response) = ui.allocate_exact_size(
        egui::vec2(44.0, INLINE_FONT_ROW_HEIGHT),
        egui::Sense::click(),
    );

    ui.painter().rect(
        rect,
        6.0,
        egui::Color32::from_rgb(252, 254, 254),
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );

    let inner_rect = rect.shrink2(egui::vec2(7.0, 2.0));
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("subtitle-font-size-input")
            .max_rect(inner_rect)
            .layout(egui::Layout::centered_and_justified(
                egui::Direction::LeftToRight,
            )),
    );
    let response = child_ui.add_sized(
        inner_rect.size(),
        egui::TextEdit::singleline(value)
            .font(egui::TextStyle::Button)
            .frame(false)
            .hint_text("24"),
    );
    if frame_response.clicked() {
        response.request_focus();
    }

    frame_response.union(response)
}

fn settings_popup_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(253, 254, 254))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(204, 216, 222),
        ))
        .corner_radius(10.0)
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 8,
            spread: 0,
            color: egui::Color32::from_black_alpha(36),
        })
        .inner_margin(egui::Margin::symmetric(14, 14))
}

fn setting_block(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(247, 250, 251))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(9.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, add_contents);
}

fn api_key_field(ui: &mut egui::Ui, api_key: &mut String) -> egui::Response {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(246, 250, 250))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("MOSS API Key")
                            .size(13.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.label(
                        egui::RichText::new("默认仅保存在运行内存中，可选择记住到本机")
                            .size(12.0)
                            .color(FAINT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    tiny_status_chip(
                        ui,
                        if api_key.trim().is_empty() {
                            "必填"
                        } else {
                            "已填写"
                        },
                        !api_key.trim().is_empty(),
                    );
                });
            });

            ui.add_space(9.0);
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(252, 254, 254))
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgb(208, 221, 226),
                ))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(9, 7))
                .show(ui, |ui| {
                    ui.set_min_height(30.0);
                    ui.set_max_height(30.0);
                    ui.add_sized(
                        [ui.available_width(), 24.0],
                        egui::TextEdit::singleline(api_key)
                            .password(true)
                            .frame(false)
                            .hint_text("粘贴 API Key，不需要包含 Bearer"),
                    )
                })
                .inner
        })
        .inner
}

fn tiny_status_chip(ui: &mut egui::Ui, label: &str, ready: bool) {
    let (fill, text) = if ready {
        (ACCENT_SOFT, ACCENT_DARK)
    } else {
        (
            egui::Color32::from_rgb(240, 244, 246),
            egui::Color32::from_rgb(116, 130, 141),
        )
    };
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(12.0).strong().color(text));
        });
}

fn primary_small_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(
        egui::RichText::new(label)
            .size(12.0)
            .strong()
            .color(egui::Color32::WHITE),
    )
    .min_size(egui::vec2(84.0, 28.0))
    .fill(ACCENT)
    .stroke(egui::Stroke::new(1.0_f32, ACCENT))
    .corner_radius(8.0)
}

fn setting_row_button(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    description: &str,
) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 58.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if response.hovered() {
        ACCENT_SOFT
    } else {
        egui::Color32::from_rgb(247, 250, 251)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(9),
        fill,
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Outside,
    );

    let left = rect.left() + 12.0;
    let center_y = rect.center().y;
    ui.painter().text(
        egui::pos2(left, center_y - 9.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        MUTED,
    );
    ui.painter().text(
        egui::pos2(left, center_y + 10.0),
        egui::Align2::LEFT_CENTER,
        description,
        egui::FontId::proportional(12.0),
        FAINT,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 34.0, center_y),
        egui::Align2::RIGHT_CENTER,
        value,
        egui::FontId::proportional(13.0),
        INK,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 12.0, center_y),
        egui::Align2::RIGHT_CENTER,
        "›",
        egui::FontId::proportional(18.0),
        MUTED,
    );
    response
}

fn model_option(ui: &mut egui::Ui, model: &str, selected: bool) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 38.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if selected {
        ACCENT_SOFT
    } else if response.hovered() {
        egui::Color32::from_rgb(247, 250, 251)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), fill);
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        model,
        egui::FontId::proportional(13.0),
        if selected { ACCENT_DARK } else { INK },
    );
    if selected {
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            "已选",
            egui::FontId::proportional(12.0),
            ACCENT_DARK,
        );
    }
    response
}

fn font_option(ui: &mut egui::Ui, font: &str, selected: bool) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 34.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if selected {
        ACCENT_SOFT
    } else if response.hovered() {
        egui::Color32::from_rgb(247, 250, 251)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), fill);
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        font,
        egui::FontId::proportional(13.0),
        if selected { ACCENT_DARK } else { INK },
    );
    if selected {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            "已选",
            egui::FontId::proportional(12.0),
            ACCENT_DARK,
        );
    }
    response
}

fn export_format_row(ui: &mut egui::Ui, format: SubtitleExportFormat) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if response.hovered() {
        ACCENT_SOFT
    } else {
        egui::Color32::from_rgb(247, 250, 251)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        fill,
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format.label(),
        egui::FontId::monospace(13.0),
        ACCENT_DARK,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!(".{}", format.extension()),
        egui::FontId::proportional(12.0),
        MUTED,
    );
    response
}

fn soft_badge(ui: &mut egui::Ui, label: &str) {
    egui::Frame::NONE
        .fill(ACCENT_SOFT)
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .color(ACCENT_DARK)
                    .strong()
                    .size(13.0),
            );
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

fn percent_badge(ui: &mut egui::Ui, progress: f32, is_error: bool) {
    let text = if is_error { DANGER } else { INK };
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(240, 244, 246))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("{:.0}%", progress))
                    .monospace()
                    .strong()
                    .color(text),
            );
        });
}

fn progress_track(ui: &mut egui::Ui, progress: f32, is_error: bool) {
    let width = ui.available_width();
    let height = 8.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let radius = egui::CornerRadius::same(4);
    ui.painter()
        .rect_filled(rect, radius, egui::Color32::from_rgb(232, 238, 241));

    let fill_width = (rect.width() * (progress / 100.0).clamp(0.0, 1.0)).max(if progress > 0.0 {
        height
    } else {
        0.0
    });
    if fill_width > 0.0 {
        let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, height));
        ui.painter()
            .rect_filled(fill_rect, radius, if is_error { DANGER } else { ACCENT });
    }
}

fn detail_grid(ui: &mut egui::Ui, snapshot: &JobSnapshot) {
    let full_width = ui.available_width();
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(248, 250, 251))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width((full_width - 20.0).max(0.0));
            let spacing = 12.0;
            let separator_width = 1.0;
            let content_width = ui.available_width() - spacing * 4.0 - separator_width * 2.0;
            let cell_width = (content_width / 3.0).max(88.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                detail_cell(ui, "任务 ID", &snapshot.task_id, cell_width);
                detail_separator(ui);
                detail_cell(ui, "文件 ID", &snapshot.file_id, cell_width);
                detail_separator(ui);
                detail_cell(ui, "Token", &snapshot.usage, cell_width);
            });
        });
}

fn detail_cell(ui: &mut egui::Ui, label: &str, value: &str, width: f32) {
    let display_value = compact_detail_value(value);
    ui.allocate_ui_with_layout(
        egui::vec2(width, 36.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.label(egui::RichText::new(label).size(11.0).color(FAINT));
            ui.label(
                egui::RichText::new(display_value)
                    .monospace()
                    .size(12.0)
                    .color(if value == "-" { FAINT } else { MUTED }),
            );
        },
    );
}

fn detail_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 34.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
}

fn recent_project_row(
    ui: &mut egui::Ui,
    project: &RecentProject,
    available: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 46.0),
        if available {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let fill = if response.hovered() && available {
        ACCENT_SOFT
    } else {
        egui::Color32::from_rgb(248, 250, 251)
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(7), fill);

    let name = recent_project_name(&project.path);
    let primary = if available { INK } else { FAINT };
    let status_width = recent_status_width(&project.status);
    let status_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - status_width * 0.5 - 10.0, rect.center().y),
        egui::vec2(status_width, 24.0),
    );
    let name_clip =
        egui::Rect::from_min_max(rect.min, egui::pos2(status_rect.left() - 8.0, rect.max.y));
    ui.painter().with_clip_rect(name_clip).text(
        egui::pos2(rect.left() + 10.0, rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(12.5),
        primary,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        if available {
            recent_project_time(project.opened_at)
        } else {
            "文件已移动".to_owned()
        },
        egui::FontId::proportional(11.0),
        FAINT,
    );
    let status_ready = project.status == "转写完成";
    ui.painter().rect_filled(
        status_rect,
        egui::CornerRadius::same(12),
        if status_ready {
            ACCENT_SOFT
        } else {
            egui::Color32::from_rgb(240, 244, 246)
        },
    );
    ui.painter().text(
        status_rect.center(),
        egui::Align2::CENTER_CENTER,
        &project.status,
        egui::FontId::proportional(10.5),
        if status_ready { ACCENT_DARK } else { MUTED },
    );
    response.on_hover_text(project.path.display().to_string())
}

fn recent_status_width(status: &str) -> f32 {
    let text_width = status
        .chars()
        .map(|character| if character.is_ascii() { 6.0 } else { 11.0 })
        .sum::<f32>();
    (text_width + 18.0).clamp(58.0, 86.0)
}

fn recent_project_name(path: &std::path::Path) -> String {
    path.parent()
        .and_then(std::path::Path::file_name)
        .or_else(|| path.file_stem())
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("未命名项目")
        .to_owned()
}

fn recent_project_time(opened_at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(opened_at);
    let elapsed = now.saturating_sub(opened_at);
    if elapsed < 60 {
        "刚刚".to_owned()
    } else if elapsed < 3_600 {
        format!("{} 分钟前", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{} 小时前", elapsed / 3_600)
    } else {
        format!("{} 天前", elapsed / 86_400)
    }
}

fn compact_detail_value(value: &str) -> String {
    let value = value.trim();
    let chars: Vec<char> = value.chars().collect();
    if chars.len() > 18 {
        let start: String = chars.iter().take(8).collect();
        let end: String = chars.iter().skip(chars.len().saturating_sub(6)).collect();
        format!("{start}...{end}")
    } else {
        value.to_owned()
    }
}

fn stage_message(progress: f32, done: bool) -> &'static str {
    if done {
        "阶段：生成字幕完成"
    } else if progress >= 82.0 {
        "阶段：生成字幕"
    } else if progress >= 58.0 {
        "阶段：转写处理"
    } else if progress >= 28.0 {
        "阶段：上传音频"
    } else if progress >= 12.0 {
        "阶段：准备音频"
    } else {
        "阶段：等待媒体处理"
    }
}

fn error_box(ui: &mut egui::Ui, error: &str) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(253, 241, 239))
        .stroke(egui::Stroke::new(
            1.0_f32,
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

fn empty_preview(ui: &mut egui::Ui, content_height: f32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), content_height.max(0.0)),
        egui::Sense::hover(),
    );
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgb(249, 251, 252),
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(226, 233, 236)),
        egui::StrokeKind::Inside,
    );
    let content_rect = rect.shrink2(egui::vec2(14.0, 12.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            let body_height = content_rect.height().max(0.0);
            ui.vertical_centered(|ui| {
                ui.add_space(((body_height - 88.0) * 0.5).clamp(0.0, 80.0));
                ui.label(
                    egui::RichText::new("等待生成字幕")
                        .size(16.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new(
                        "选择音频或视频并填写 API Key 后，会在这里显示可复制的 SRT 预览。",
                    )
                    .color(MUTED),
                );
                ui.add_space(8.0);
                output_chip_row(ui, &["SRT", "VTT", "TXT", "JSON"]);
            });
        },
    );
}

fn review_panel_body_height(panel_height: f32) -> f32 {
    (panel_height - 28.0 - PREVIEW_BORDER_RESERVE).max(160.0)
}

fn review_media_height_bounds(available_height: f32) -> (f32, f32, f32) {
    let content_height = (available_height - REVIEW_SPLITTER_HEIGHT).max(1.0);
    let media_min = MIN_MEDIA_REVIEW_HEIGHT.min(content_height * 0.5);
    let subtitle_min = MIN_SUBTITLE_REVIEW_HEIGHT.min((content_height - media_min).max(0.0));
    let media_max = (content_height - subtitle_min).max(media_min);
    (content_height, media_min, media_max)
}

fn review_section_heights(available_height: f32, ratio: f32) -> (f32, f32) {
    let (content_height, media_min, media_max) = review_media_height_bounds(available_height);
    let media_height = (content_height * ratio).clamp(media_min, media_max);
    (media_height, content_height - media_height)
}

fn review_splitter(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), REVIEW_SPLITTER_HEIGHT),
        egui::Sense::drag(),
    );
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let color = if response.hovered() || response.dragged() {
        ACCENT
    } else {
        BORDER
    };
    let width = if response.dragged() { 2.0_f32 } else { 1.0_f32 };
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ],
        egui::Stroke::new(width, color),
    );
    response
}

fn media_empty_surface(ui: &mut egui::Ui, message: &str, height: f32) {
    let height = height.max(120.0);
    let (width, height) = fitted_video_surface_size(ui.available_width(), height);
    let row_width = ui.available_width().max(width);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(row_width, height), egui::Sense::hover());
    let rect = egui::Rect::from_center_size(rect.center(), egui::vec2(width, height));
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(9),
        egui::Color32::from_rgb(18, 27, 34),
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(30, 43, 51)),
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        message,
        egui::FontId::proportional(15.0),
        egui::Color32::from_rgb(190, 203, 211),
    );
}

fn media_review_header(ui: &mut egui::Ui, title: &str, badge: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), VIDEO_HEADER_HEIGHT),
        egui::Sense::hover(),
    );
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(16.0),
        INK,
    );

    let badge_width = badge_text_width(badge) + 24.0;
    let badge_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - badge_width * 0.5, rect.center().y),
        egui::vec2(badge_width, 30.0),
    );
    ui.painter().rect(
        badge_rect,
        egui::CornerRadius::same(15),
        ACCENT_SOFT,
        egui::Stroke::NONE,
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        badge,
        egui::FontId::proportional(13.0),
        ACCENT_DARK,
    );
}

fn badge_text_width(text: &str) -> f32 {
    text.chars()
        .map(|character| if character.is_ascii() { 8.0 } else { 13.0 })
        .sum::<f32>()
        .max(42.0)
}

fn fitted_video_surface_size(max_width: f32, max_height: f32) -> (f32, f32) {
    let max_width = max_width.max(1.0);
    let max_height = max_height.max(1.0);
    let height_from_width = max_width / VIDEO_PREVIEW_ASPECT;
    if height_from_width <= max_height {
        (max_width, height_from_width)
    } else {
        (max_height * VIDEO_PREVIEW_ASPECT, max_height)
    }
}

fn media_surface(
    ui: &mut egui::Ui,
    preview: &VideoPreview,
    width: f32,
    height: f32,
    has_video: bool,
) {
    let row_width = ui.available_width().max(width);
    let (row_rect, _) = ui.allocate_exact_size(egui::vec2(row_width, height), egui::Sense::hover());
    let rect = egui::Rect::from_center_size(row_rect.center(), egui::vec2(width, height));
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(9),
        egui::Color32::from_rgb(15, 23, 29),
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(30, 43, 51)),
        egui::StrokeKind::Outside,
    );

    if has_video && let Some(texture) = preview.texture() {
        let image_rect = fit_texture_rect(rect, texture.size_vec2());
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if has_video {
                "正在准备预览"
            } else {
                "音频播放"
            },
            egui::FontId::proportional(15.0),
            egui::Color32::from_rgb(190, 203, 211),
        );
    }

    if has_video && preview.is_pending() {
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 12.0, rect.top() + 12.0),
            egui::vec2(74.0, 26.0),
        );
        ui.painter().rect(
            badge_rect,
            egui::CornerRadius::same(13),
            egui::Color32::from_rgba_premultiplied(18, 27, 34, 210),
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(49, 67, 77)),
            egui::StrokeKind::Outside,
        );
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            "预渲染",
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(214, 226, 232),
        );
    }

    if let Some(error) = preview.last_error().or_else(|| preview.last_audio_error()) {
        let message = compact_error(&error);
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 18.0),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(255, 198, 190),
        );
    }
}

fn video_controls(ui: &mut egui::Ui, preview: &mut VideoPreview, duration: f64) {
    let row_width = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(row_width, VIDEO_CONTROLS_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let label = if preview.is_playing() {
                "暂停"
            } else {
                "播放"
            };
            if ui
                .add_sized([VIDEO_BUTTON_WIDTH, 30.0], egui::Button::new(label))
                .clicked()
            {
                preview.toggle_playing();
            }

            time_text(
                ui,
                display_time(preview.current_time()),
                VIDEO_TIME_WIDTH,
                egui::Align2::RIGHT_CENTER,
                MUTED,
            );

            let mut time = preview.current_time();
            let scrubber_width =
                (row_width - VIDEO_BUTTON_WIDTH - VIDEO_TIME_WIDTH * 2.0 - 8.0 * 3.0).max(120.0);
            if video_scrubber(ui, &mut time, duration, scrubber_width).changed() {
                preview.seek(time);
            }

            time_text(
                ui,
                display_time(duration),
                VIDEO_TIME_WIDTH,
                egui::Align2::LEFT_CENTER,
                FAINT,
            );
        },
    );
}

fn time_text(
    ui: &mut egui::Ui,
    text: String,
    width: f32,
    align: egui::Align2,
    color: egui::Color32,
) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, VIDEO_CONTROLS_HEIGHT),
        egui::Sense::hover(),
    );
    let x = match align {
        egui::Align2::RIGHT_CENTER => rect.right(),
        egui::Align2::LEFT_CENTER => rect.left(),
        _ => rect.center().x,
    };
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        align,
        text,
        egui::FontId::monospace(12.0),
        color,
    );
}

fn video_scrubber(ui: &mut egui::Ui, time: &mut f64, duration: f64, width: f32) -> egui::Response {
    let duration = duration.max(0.1);
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::click_and_drag());

    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let progress = ((pointer.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
        *time = duration * progress as f64;
        response.mark_changed();
    }

    let progress = (*time / duration).clamp(0.0, 1.0) as f32;
    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 4.0));
    let filled_rect = egui::Rect::from_min_max(
        track_rect.left_top(),
        egui::pos2(
            track_rect.left() + track_rect.width() * progress,
            track_rect.bottom(),
        ),
    );
    let handle_center = egui::pos2(
        track_rect.left() + track_rect.width() * progress,
        track_rect.center().y,
    );

    ui.painter().rect(
        track_rect,
        egui::CornerRadius::same(2),
        egui::Color32::from_rgb(224, 229, 232),
        egui::Stroke::NONE,
        egui::StrokeKind::Outside,
    );
    if filled_rect.width() > 0.5 {
        ui.painter().rect(
            filled_rect,
            egui::CornerRadius::same(2),
            ACCENT,
            egui::Stroke::NONE,
            egui::StrokeKind::Outside,
        );
    }
    ui.painter().circle(
        handle_center,
        6.0,
        egui::Color32::from_rgb(247, 250, 251),
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(75, 88, 98)),
    );

    response
}

fn fit_texture_rect(container: egui::Rect, image_size: egui::Vec2) -> egui::Rect {
    let image_aspect = if image_size.y > 0.0 {
        image_size.x / image_size.y
    } else {
        16.0 / 9.0
    };
    let container_aspect = container.width() / container.height().max(1.0);
    let size = if container_aspect > image_aspect {
        egui::vec2(container.height() * image_aspect, container.height())
    } else {
        egui::vec2(container.width(), container.width() / image_aspect)
    };
    egui::Rect::from_center_size(container.center(), size)
}

fn compact_error(error: &str) -> String {
    let trimmed = error.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() > 64 {
        format!("{}...", chars.iter().take(64).collect::<String>())
    } else {
        trimmed.to_owned()
    }
}

fn output_chip_row(ui: &mut egui::Ui, labels: &[&str]) {
    let total_width = labels
        .iter()
        .map(|label| output_chip_width(label))
        .sum::<f32>()
        + OUTPUT_CHIP_GAP * labels.len().saturating_sub(1) as f32;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = OUTPUT_CHIP_GAP;
        ui.add_space((ui.available_width() - total_width).max(0.0) * 0.5);
        for label in labels {
            output_chip(ui, label);
        }
    });
}

fn output_chip(ui: &mut egui::Ui, label: &str) {
    let size = egui::vec2(output_chip_width(label), OUTPUT_CHIP_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same((OUTPUT_CHIP_HEIGHT * 0.5) as u8),
        egui::Color32::from_rgb(239, 245, 247),
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::monospace(12.0),
        MUTED,
    );
}

fn output_chip_width(label: &str) -> f32 {
    let text_width = label
        .chars()
        .map(|character| if character.is_ascii() { 7.5 } else { 12.0 })
        .sum::<f32>();
    (text_width + 28.0).max(48.0)
}

fn preview_mode_switch(ui: &mut egui::Ui, mode: &mut PreviewMode) {
    let size = egui::vec2(88.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            *mode = if pos.x < rect.center().x {
                PreviewMode::Raw
            } else {
                PreviewMode::Rendered
            };
        }
    }

    let radius = egui::CornerRadius::same(13);
    ui.painter().rect(
        rect,
        radius,
        egui::Color32::from_rgb(239, 245, 247),
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Outside,
    );

    let selected_rect = match mode {
        PreviewMode::Raw => {
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.center().x, rect.max.y))
        }
        PreviewMode::Rendered => {
            egui::Rect::from_min_max(egui::pos2(rect.center().x, rect.min.y), rect.max)
        }
    }
    .shrink(2.0);
    ui.painter().rect(
        selected_rect,
        egui::CornerRadius::same(11),
        ACCENT_SOFT,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(190, 226, 221)),
        egui::StrokeKind::Outside,
    );

    let raw_color = if *mode == PreviewMode::Raw {
        ACCENT_DARK
    } else {
        MUTED
    };
    let rendered_color = if *mode == PreviewMode::Rendered {
        ACCENT_DARK
    } else {
        MUTED
    };
    ui.painter().text(
        egui::pos2(rect.left() + rect.width() * 0.25, rect.center().y),
        egui::Align2::CENTER_CENTER,
        "Raw",
        egui::FontId::proportional(11.0),
        raw_color,
    );
    ui.painter().text(
        egui::pos2(rect.left() + rect.width() * 0.75, rect.center().y),
        egui::Align2::CENTER_CENTER,
        "渲染",
        egui::FontId::proportional(11.0),
        rendered_color,
    );
}

fn raw_preview(ui: &mut egui::Ui, preview: &str, content_height: f32) {
    egui::ScrollArea::vertical()
        .id_salt("raw-subtitles-scroll")
        .max_height(content_height)
        .show(ui, |ui| {
            ui.set_min_height(content_height);
            let mut preview = preview.to_owned();
            let rows = ((content_height / 18.0).floor() as usize).max(1);
            ui.add(
                egui::TextEdit::multiline(&mut preview)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(rows)
                    .interactive(false),
            );
        });
}

fn segment_row(
    ui: &mut egui::Ui,
    segment_index: usize,
    index: usize,
    segment: &Segment,
    speaker_names: &BTreeMap<String, String>,
    time_edits: &mut BTreeMap<usize, (String, String)>,
    active: bool,
    follow_playback: bool,
) -> Option<(f64, f64, String, String, bool)> {
    let mut speaker = display_speaker(&segment.speaker, speaker_names);
    let mut text = segment.text.clone();
    let time_draft = time_edits.entry(segment_index).or_insert_with(|| {
        (
            initial_time_text(
                segment.raw_start.as_deref(),
                segment.start_valid,
                segment.start,
            ),
            initial_time_text(segment.raw_end.as_deref(), segment.end_valid, segment.end),
        )
    });
    let mut start_text = time_draft.0.clone();
    let mut end_text = time_draft.1.clone();
    let mut time_changed = false;
    let mut content_changed = false;
    let start_parse = parse_edit_time(&start_text);
    let end_parse = parse_edit_time(&end_text);
    let unchanged_invalid_start = !segment.start_valid
        && segment
            .raw_start
            .as_deref()
            .is_some_and(|raw| start_text.trim() == raw);
    let unchanged_invalid_end = !segment.end_valid
        && segment
            .raw_end
            .as_deref()
            .is_some_and(|raw| end_text.trim() == raw);
    let range_invalid = start_parse
        .zip(end_parse)
        .is_some_and(|(start, end)| end <= start);
    let start_invalid = unchanged_invalid_start || start_parse.is_none() || range_invalid;
    let end_invalid = unchanged_invalid_end || end_parse.is_none() || range_invalid;

    let row = egui::Frame::NONE
        .fill(if active {
            egui::Color32::from_rgb(235, 247, 245)
        } else {
            egui::Color32::from_rgb(247, 250, 251)
        })
        .stroke(egui::Stroke::new(
            1.0_f32,
            if active {
                egui::Color32::from_rgb(166, 215, 207)
            } else {
                egui::Color32::from_rgb(226, 233, 236)
            },
        ))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width((ui.available_width() - 24.0).max(0.0));
            ui.set_min_height(50.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add_sized(
                    [30.0, 24.0],
                    egui::Label::new(
                        egui::RichText::new(format!("{index:02}"))
                            .monospace()
                            .size(12.0)
                            .strong()
                            .color(FAINT),
                    ),
                );
                time_changed |= editable_time_field(ui, &mut start_text, start_invalid).changed();
                time_separator(ui);
                time_changed |= editable_time_field(ui, &mut end_text, end_invalid).changed();
                content_changed |= editable_speaker_field(ui, &mut speaker).changed();
                ui.add_space(6.0);
                content_changed |= editable_subtitle_field(ui, &mut text).changed();
            });
        });
    if active && follow_playback {
        ui.scroll_to_rect(row.response.rect, Some(egui::Align::Center));
    }

    if time_changed {
        *time_draft = (start_text.clone(), end_text.clone());
    }

    let unchanged_invalid_start = !segment.start_valid
        && segment
            .raw_start
            .as_deref()
            .is_some_and(|raw| start_text.trim() == raw);
    let unchanged_invalid_end = !segment.end_valid
        && segment
            .raw_end
            .as_deref()
            .is_some_and(|raw| end_text.trim() == raw);
    let parsed_time = parse_edit_time(&start_text).zip(parse_edit_time(&end_text));
    let valid_time = parsed_time
        .filter(|(start, end)| *end > *start && !unchanged_invalid_start && !unchanged_invalid_end)
        .map(|(start, end)| (start, end));

    if let Some((start, end)) = valid_time {
        if time_changed || content_changed {
            return Some((start, end, speaker, text, true));
        }
    } else if content_changed {
        return Some((segment.start, segment.end, speaker, text, false));
    }

    None
}

fn speaker_name_row(ui: &mut egui::Ui, speaker: &str, names: &mut BTreeMap<String, String>) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(247, 250, 251))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                compact_speaker_badge(ui, speaker);
                ui.add_space(6.0);
                let value = names.entry(speaker.to_owned()).or_default();
                ui.add_sized(
                    [ui.available_width(), 24.0],
                    egui::TextEdit::singleline(value)
                        .frame(false)
                        .hint_text("输入真实姓名"),
                );
            });
        });
}

fn editable_time_field(ui: &mut egui::Ui, value: &mut String, invalid: bool) -> egui::Response {
    egui::Frame::NONE
        .fill(if invalid {
            egui::Color32::from_rgb(255, 239, 239)
        } else {
            egui::Color32::from_rgb(252, 254, 254)
        })
        .stroke(egui::Stroke::new(
            1.0_f32,
            if invalid {
                DANGER
            } else {
                egui::Color32::from_rgb(226, 233, 236)
            },
        ))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.add_sized(
                [72.0, 20.0],
                egui::TextEdit::singleline(value)
                    .font(egui::TextStyle::Monospace)
                    .frame(false)
                    .hint_text("00:00.000"),
            )
        })
        .inner
}

fn initial_time_text(raw: Option<&str>, valid: bool, seconds: f64) -> String {
    if valid {
        display_time(seconds)
    } else {
        raw.unwrap_or("").to_owned()
    }
}

fn time_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 30.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.center().x, rect.center().y - 1.0),
        egui::Align2::CENTER_CENTER,
        "–",
        egui::FontId::proportional(13.0),
        FAINT,
    );
}

fn editable_speaker_field(ui: &mut egui::Ui, speaker: &mut String) -> egui::Response {
    let width = speaker_field_width(speaker);
    egui::Frame::NONE
        .fill(ACCENT_SOFT)
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(190, 226, 221),
        ))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.add_sized(
                [width, 20.0],
                egui::TextEdit::singleline(speaker)
                    .frame(false)
                    .hint_text("说话人"),
            )
        })
        .inner
}

fn speaker_field_width(speaker: &str) -> f32 {
    let estimated_text_width = speaker
        .chars()
        .map(|character| if character.is_ascii() { 8.0 } else { 14.0 })
        .sum::<f32>();
    estimated_text_width.clamp(32.0, 112.0)
}

fn editable_subtitle_field(ui: &mut egui::Ui, text: &mut String) -> egui::Response {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(252, 254, 254))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(226, 233, 236),
        ))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.add_sized(
                [ui.available_width(), 22.0],
                egui::TextEdit::singleline(text)
                    .frame(false)
                    .hint_text("字幕内容"),
            )
        })
        .inner
}

fn compact_speaker_badge(ui: &mut egui::Ui, label: &str) {
    egui::Frame::NONE
        .fill(ACCENT_SOFT)
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.set_min_width(40.0);
            ui.label(
                egui::RichText::new(label)
                    .color(ACCENT_DARK)
                    .strong()
                    .size(12.0),
            );
        });
}

fn unique_speaker_labels(segments: &[Segment]) -> Vec<String> {
    let speakers: BTreeSet<String> = segments
        .iter()
        .filter_map(|segment| {
            let speaker = segment.speaker.trim();
            (!speaker.is_empty()).then(|| speaker.to_owned())
        })
        .collect();
    speakers.into_iter().collect()
}

fn display_speaker(speaker: &str, names: &BTreeMap<String, String>) -> String {
    names
        .get(speaker)
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or(speaker)
        .to_owned()
}

fn parse_edit_time(value: &str) -> Option<f64> {
    let normalized = value.trim().replace(',', ".");
    if normalized.is_empty() {
        return None;
    }

    let parts: Vec<&str> = normalized.split(':').collect();
    let seconds = match parts.as_slice() {
        [seconds] => seconds.parse::<f64>().ok()?,
        [minutes, seconds] => minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?,
        [hours, minutes, seconds] => {
            hours.parse::<f64>().ok()? * 3600.0
                + minutes.parse::<f64>().ok()? * 60.0
                + seconds.parse::<f64>().ok()?
        }
        _ => return None,
    };

    seconds.is_finite().then_some(seconds.max(0.0))
}

fn empty_structured_preview(ui: &mut egui::Ui, content_height: f32) {
    let body_height =
        (content_height - PREVIEW_CHILD_VERTICAL_INSET - PREVIEW_BORDER_RESERVE).max(0.0);
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(247, 250, 251))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(226, 233, 236),
        ))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_height(body_height);
            ui.vertical_centered(|ui| {
                ui.add_space(((body_height - 66.0) * 0.5).clamp(0.0, 80.0));
                ui.label(
                    egui::RichText::new("没有可渲染的结构化字幕")
                        .size(16.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new("Raw 内容仍可查看；新生成的字幕会自动解析成分段结构。")
                        .size(13.0)
                        .color(MUTED),
                );
            });
        });
}

fn display_time(seconds: f64) -> String {
    let millis = (seconds.max(0.0) * 1000.0).round() as u64;
    let minutes = millis / 60_000;
    let secs = (millis % 60_000) / 1000;
    let ms = millis % 1000;
    format!("{minutes:02}:{secs:02}.{ms:03}")
}

fn snapshot_active_segment_index(segments: &[Segment], time: f64) -> Option<usize> {
    segments
        .iter()
        .position(|segment| time >= segment.start && time < segment.end)
}

fn has_subtitle_preview(preview: &str) -> bool {
    preview.contains("-->") || preview.lines().take(3).any(|line| line.trim() == "WEBVTT")
}

fn compact_model_name(model: &str) -> &str {
    model
        .strip_prefix("moss-transcribe-diarize")
        .and_then(|suffix| suffix.strip_prefix('-'))
        .filter(|suffix| !suffix.is_empty())
        .unwrap_or("默认")
}

#[cfg(test)]
mod tests {
    use super::{
        REVIEW_SPLITTER_HEIGHT, parse_edit_time, review_media_height_bounds,
        review_section_heights, speaker_field_width,
    };

    #[test]
    fn parses_subtitle_time_editor_values() {
        assert_eq!(parse_edit_time("00:02.540"), Some(2.54));
        assert_eq!(parse_edit_time("01:00:02.500"), Some(3602.5));
        assert_eq!(parse_edit_time("2.5"), Some(2.5));
    }

    #[test]
    fn speaker_field_width_adapts_to_label_length() {
        let short_label = speaker_field_width("S01");
        let full_name = speaker_field_width("张三丰");
        let long_name = speaker_field_width("Very long speaker display name");

        assert!(short_label < full_name);
        assert!(full_name < long_name);
        assert_eq!(long_name, 112.0);
    }

    #[test]
    fn review_sections_always_fit_within_the_panel_height() {
        for available_height in [300.0, 600.0, 1_000.0] {
            let (content_height, media_min, media_max) =
                review_media_height_bounds(available_height);
            let (media_height, subtitle_height) = review_section_heights(available_height, 0.47);

            assert!(
                (media_height + subtitle_height + REVIEW_SPLITTER_HEIGHT - available_height).abs()
                    < f32::EPSILON
            );
            assert!(media_height >= media_min);
            assert!(media_height <= media_max);
            assert!(subtitle_height >= 0.0);
            assert!(content_height >= 0.0);
        }
    }
}
