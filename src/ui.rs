use eframe::egui;

use crate::{
    app::MtdApp,
    config::MODELS,
    job::update_job,
    models::JobSnapshot,
    platform::open_path,
    theme::{
        ACCENT, ACCENT_DARK, ACCENT_SOFT, BORDER, DANGER, FAINT, INK, MUTED, panel_frame,
        preview_frame,
    },
};

impl MtdApp {
    pub(crate) fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            #[cfg(target_os = "macos")]
            ui.add_space(60.0);

            ui.vertical(|ui| {
                ui.heading(
                    egui::RichText::new("视频字幕工作台")
                        .size(24.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new("本地分离音频，调用 MOSS 转写，生成可编辑字幕文件")
                        .size(13.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                soft_badge(ui, "本地音频处理");
            });
        });
    }

    pub(crate) fn render_workspace(&mut self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
        panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("任务").strong().size(16.0).color(INK));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.render_start_button(ui);
                    ui.add_space(10.0);
                    self.render_settings_menu(ui);
                });
            });

            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                let available = ui.available_width();
                let input_width = (available * 0.52).clamp(420.0, 620.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(input_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.render_file_inputs(ui),
                );
                ui.add_space(14.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.render_status_cluster(ui, snapshot),
                );
            });

            ui.add_space(12.0);
            self.render_pipeline_horizontal(ui, snapshot.progress);
        });
    }

    fn render_file_inputs(&mut self, ui: &mut egui::Ui) {
        field_label(ui, "输入视频");
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
                .add_sized([92.0, 32.0], egui::Button::new("选择"))
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
                .add_sized([92.0, 32.0], egui::Button::new("更改"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.output_dir = path;
                }
            }
        });

        ui.add_space(8.0);
        let hint = if self.api_key.trim().is_empty() {
            "转写设置里填写 API Key 后即可开始。"
        } else {
            "配置已就绪，可以开始生成字幕。"
        };
        ui.label(egui::RichText::new(hint).size(13.0).color(FAINT));
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
                1.0,
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
            api_key_field(ui, &mut self.api_key);

            ui.add_space(10.0);
            let model_response = setting_row_button(
                ui,
                "模型",
                compact_model_name(&self.model),
                "选择用于异步转写的模型版本",
            );
            egui::Popup::from_toggle_button_response(&model_response)
                .width(336.0)
                .gap(6.0)
                .frame(settings_popup_frame())
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(336.0);
                    field_label(ui, "模型版本");
                    ui.add_space(6.0);
                    for model in MODELS {
                        if model_option(ui, model, self.model == model).clicked() {
                            self.model = model.to_owned();
                            ui.close();
                        }
                    }
                });

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
                        ui.add(
                            egui::DragValue::new(&mut self.max_tokens)
                                .range(1_000..=96_000)
                                .speed(1000),
                        );
                    });
                });
            });

            ui.add_space(10.0);
            setting_block(ui, |ui| {
                ui.checkbox(&mut self.include_speaker, "保留说话人");
                ui.add_space(2.0);
                ui.checkbox(&mut self.burn_in, "烧录到视频");
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
        });

        if ui.add_enabled(can_start, button).clicked() {
            self.start_job();
        }
    }

    fn render_status_cluster(&self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
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

        ui.add_space(7.0);
        let status_color = if snapshot.error.is_some() {
            DANGER
        } else {
            INK
        };
        ui.label(
            egui::RichText::new(&snapshot.status)
                .size(17.0)
                .strong()
                .color(status_color),
        );

        ui.add_space(8.0);
        progress_track(ui, snapshot.progress, snapshot.error.is_some());

        ui.add_space(10.0);
        detail_grid(ui, snapshot);

        if let Some(path) = &snapshot.output_dir {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("输出").size(13.0).color(MUTED));
                if ui.button("打开输出目录").clicked() {
                    let _ = open_path(path);
                }
            });
        }
        if let Some(error) = &snapshot.error {
            ui.add_space(8.0);
            error_box(ui, error);
        }
    }

    fn render_pipeline_horizontal(&self, ui: &mut egui::Ui, progress: f32) {
        let steps = [
            ("分离音频", 12.0),
            ("上传音频", 28.0),
            ("转写处理", 58.0),
            ("生成字幕", 82.0),
        ];
        ui.columns(4, |columns| {
            for (index, column) in columns.iter_mut().enumerate() {
                let (label, threshold) = steps[index];
                stage_chip(column, index + 1, label, progress >= threshold);
            }
        });
    }

    pub(crate) fn render_preview(&self, ui: &mut egui::Ui, snapshot: &JobSnapshot) {
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
            ui.set_min_width((ui.available_width() - 104.0).max(160.0));
            ui.label(egui::RichText::new(text).color(color));
        });
}

fn settings_popup_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(253, 254, 254))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(204, 216, 222),
        ))
        .corner_radius(12.0)
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 22,
            spread: 0,
            color: egui::Color32::from_black_alpha(36),
        })
        .inner_margin(egui::Margin::symmetric(14, 14))
}

fn setting_block(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(247, 250, 251))
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(9.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, add_contents);
}

fn api_key_field(ui: &mut egui::Ui, api_key: &mut String) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(246, 250, 250))
        .stroke(egui::Stroke::new(1.0, BORDER))
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
                        egui::RichText::new("仅保存在当前运行内存中，用于上传音频和查询任务")
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
                    1.0,
                    egui::Color32::from_rgb(208, 221, 226),
                ))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(9, 7))
                .show(ui, |ui| {
                    ui.set_min_height(30.0);
                    ui.set_max_height(30.0);
                    ui.horizontal_centered(|ui| {
                        key_prefix(ui);
                        ui.add_space(6.0);
                        ui.add_sized(
                            [ui.available_width(), 24.0],
                            egui::TextEdit::singleline(api_key)
                                .password(true)
                                .frame(false)
                                .hint_text("粘贴 API Key，不需要包含 Bearer"),
                        );
                    });
                });
        });
}

fn key_prefix(ui: &mut egui::Ui) {
    egui::Frame::NONE
        .fill(ACCENT_SOFT)
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(190, 226, 221),
        ))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("KEY")
                    .monospace()
                    .size(11.0)
                    .strong()
                    .color(ACCENT_DARK),
            );
        });
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
        egui::Stroke::new(1.0, BORDER),
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
        .stroke(egui::Stroke::new(1.0, BORDER))
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
    egui::Grid::new("job_details")
        .num_columns(3)
        .spacing(egui::vec2(14.0, 4.0))
        .show(ui, |ui| {
            detail_cell(ui, "任务 ID", &snapshot.task_id);
            detail_cell(ui, "文件 ID", &snapshot.file_id);
            detail_cell(ui, "Token", &snapshot.usage);
            ui.end_row();
        });
}

fn detail_cell(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(FAINT));
        ui.label(
            egui::RichText::new(value)
                .monospace()
                .size(12.0)
                .color(if value == "-" { FAINT } else { MUTED }),
        );
    });
}

fn stage_chip(ui: &mut egui::Ui, number: usize, label: &str, active: bool) {
    let fill = if active {
        ACCENT_SOFT
    } else {
        egui::Color32::from_rgb(247, 250, 251)
    };
    let stroke = if active {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(184, 222, 216))
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(stroke)
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                number_dot(ui, number, active);
                ui.label(
                    egui::RichText::new(label)
                        .size(13.0)
                        .strong()
                        .color(if active { INK } else { FAINT }),
                );
            });
        });
}

fn number_dot(ui: &mut egui::Ui, number: usize, active: bool) {
    let fill = if active {
        ACCENT
    } else {
        egui::Color32::from_rgb(232, 238, 241)
    };
    let text = if active { egui::Color32::WHITE } else { FAINT };
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(number.to_string())
                    .size(12.0)
                    .strong()
                    .color(text),
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
