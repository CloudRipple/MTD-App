#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod api;
mod app;
mod app_settings;
mod config;
mod embedded_assets;
mod fonts;
mod job;
mod media;
mod media_types;
mod models;
mod native_menu;
mod pipeline;
mod platform;
mod project;
mod secret_store;
mod subtitles;
mod theme;
mod ui;
mod video_preview;

use app::MtdApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "MOSS 字幕工作台",
        platform::native_options(),
        Box::new(|cc| {
            platform::install_app_fonts(&cc.egui_ctx);
            theme::install_app_style(&cc.egui_ctx);
            Ok(Box::new(MtdApp::default()))
        }),
    )
}
