#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod api;
mod app;
mod app_settings;
mod config;
mod fonts;
mod job;
mod media;
mod media_types;
mod models;
mod pipeline;
mod platform;
mod secret_store;
mod subtitles;
mod theme;
mod ui;
mod video_preview;

use app::MtdApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "MTD 字幕工作台",
        platform::native_options(),
        Box::new(|cc| {
            platform::install_app_fonts(&cc.egui_ctx);
            theme::install_app_style(&cc.egui_ctx);
            Ok(Box::new(MtdApp::default()))
        }),
    )
}
