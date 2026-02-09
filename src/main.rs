mod app;
mod db;
mod i18n;
mod timeline;
mod visualization;

use eframe::egui;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

fn main() -> Result<(), eframe::Error> {
    // 初始化日志系统
    init_logger();

    log::info!("应用程序启动");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "NSYS Profile Viewer",
        options,
        Box::new(|cc| {
            // 设置中文字体支持
            setup_custom_fonts(&cc.egui_ctx);
            log::info!("字体配置完成");
            Ok(Box::new(app::NsysViewerApp::default()))
        }),
    )
}

/// 初始化日志系统
fn init_logger() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .filter(None, LevelFilter::Debug) // 默认日志等级为 Debug
        .parse_default_env() // 支持通过 RUST_LOG 环境变量设置日志等级
        .init();
}

/// 配置自定义字体以支持中文显示（跨平台方案）
fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 加载内嵌的 Noto Sans SC 字体（支持简体中文）
    // 该字体文件会被编译到二进制文件中，确保跨平台一致性
    fonts.font_data.insert(
        "noto_sans_sc".to_owned(),
        egui::FontData::from_static(include_bytes!("../fonts/NotoSansSC-Regular.otf")),
    );

    // 将中文字体添加到 Proportional 字体族的最前面
    // 这样中文字符会优先使用 Noto Sans SC，而英文字符会回退到默认字体
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "noto_sans_sc".to_owned());

    // 同样添加到 Monospace 字体族
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "noto_sans_sc".to_owned());

    ctx.set_fonts(fonts);
}
