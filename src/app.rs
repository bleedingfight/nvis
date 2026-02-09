use crate::db::{NsysDatabase, TableData};
use crate::timeline::Timeline;
use crate::visualization::Visualization;
use eframe::egui;
use log::LevelFilter;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn to_level_filter(self) -> LevelFilter {
        match self {
            LogLevel::Off => LevelFilter::Off,
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Off => "关闭",
            LogLevel::Error => "错误",
            LogLevel::Warn => "警告",
            LogLevel::Info => "信息",
            LogLevel::Debug => "调试",
            LogLevel::Trace => "追踪",
        }
    }

    fn all_levels() -> Vec<LogLevel> {
        vec![
            LogLevel::Off,
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ]
    }
}

pub struct NsysViewerApp {
    db: NsysDatabase,
    timeline: Timeline,
    visualization: Visualization,
    selected_table: Option<String>,
    table_data: Option<TableData>,
    current_page: usize,
    page_size: usize,
    error_message: Option<String>,
    file_path: Option<PathBuf>,
    log_level: LogLevel,
    show_log_settings: bool,
    // 面板显示状态
    show_table_panel: bool,
    show_visualization_panel: bool,
    table_panel_height_ratio: f32, // 表格面板高度比例 (0.0 - 1.0)
}

impl Default for NsysViewerApp {
    fn default() -> Self {
        log::info!("初始化 NsysViewerApp");
        Self {
            db: NsysDatabase::new(),
            timeline: Timeline::new(),
            visualization: Visualization::new(),
            selected_table: None,
            table_data: None,
            current_page: 0,
            page_size: 100,
            error_message: None,
            file_path: None,
            log_level: LogLevel::Debug,
            show_log_settings: false,
            show_table_panel: true,
            show_visualization_panel: true,
            table_panel_height_ratio: 0.5, // 默认各占一半
        }
    }
}

impl eframe::App for NsysViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 处理文件拖放
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(path) = &file.path {
                        log::info!("检测到拖放文件: {}", path.display());
                        self.load_database(path.clone());
                    }
                }
            }
        });

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("NSYS Profile Viewer");
                ui.separator();

                if let Some(path) = &self.file_path {
                    ui.label(format!("文件: {}", path.display()));
                } else {
                    ui.label("拖放 SQLite 文件到窗口");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 日志设置按钮
                    if ui.button("⚙ 日志设置").clicked() {
                        self.show_log_settings = !self.show_log_settings;
                        log::debug!("切换日志设置面板: {}", self.show_log_settings);
                    }
                });
            });
        });

        // 日志设置窗口
        if self.show_log_settings {
            egui::Window::new("日志设置")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("日志等级");
                    ui.separator();

                    let current_level = self.log_level;

                    for level in LogLevel::all_levels() {
                        let is_selected = level == current_level;
                        if ui.selectable_label(is_selected, level.as_str()).clicked() {
                            self.log_level = level;
                            log::set_max_level(level.to_level_filter());
                            log::warn!("日志等级已更改为: {:?}", level);
                        }
                    }

                    ui.separator();
                    ui.label("说明:");
                    ui.label("• 关闭: 不记录任何日志");
                    ui.label("• 错误: 仅记录错误信息");
                    ui.label("• 警告: 记录警告和错误");
                    ui.label("• 信息: 记录一般信息");
                    ui.label("• 调试: 记录调试信息(推荐)");
                    ui.label("• 追踪: 记录详细追踪信息");

                    ui.separator();
                    if ui.button("关闭").clicked() {
                        self.show_log_settings = false;
                    }
                });
        }

        // 底部状态栏
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, format!("错误: {}", err));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(duration) = self.timeline.get_selected_duration() {
                        ui.label(format!("选中时间范围: {:.3} ms", duration / 1_000_000.0));
                        if ui.button("清除选择").clicked() {
                            log::debug!("清除时间线选择");
                            self.timeline.clear_selection();
                        }
                    }

                    ui.label(format!("函数调用: {}", self.db.function_calls.len()));
                    ui.label(format!("数据表: {}", self.db.tables.len()));
                    ui.label(format!("日志: {}", self.log_level.as_str()));
                });
            });
        });

        // 用于收集需要执行的操作
        let mut table_to_load: Option<String> = None;
        let mut should_reload = false;

        egui::SidePanel::left("left_panel")
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.heading("数据表");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.db.tables.is_empty() {
                        ui.colored_label(egui::Color32::GRAY, "暂无数据");
                    } else {
                        for table in &self.db.tables {
                            let is_selected = self.selected_table.as_ref() == Some(&table.name);

                            if ui
                                .selectable_label(
                                    is_selected,
                                    format!("{} ({})", table.name, table.row_count),
                                )
                                .clicked()
                            {
                                log::info!("用户选择了数据表: {}", table.name);
                                table_to_load = Some(table.name.clone());
                            }
                        }
                    }
                });

                ui.separator();

                if ui.button("重新加载").clicked() {
                    log::info!("用户点击重新加载按钮");
                    should_reload = true;
                }
            });

        // 处理侧边栏的操作
        if let Some(table_name) = table_to_load {
            log::debug!("开始加载表数据: {}", table_name);
            self.selected_table = Some(table_name);
            self.load_table_data();
        }

        if should_reload {
            if let Some(path) = self.file_path.clone() {
                log::info!("重新加载数据库文件: {}", path.display());
                self.load_database(path);
            }
        }

        // 收集UI操作
        let mut should_clear_selection = false;
        let mut next_page: Option<usize> = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            // 如果选中了表,显示分割面板;否则显示时间线
            if let Some(table_name) = &self.selected_table {
                let table_name_str = table_name.clone();

                // 顶部控制栏
                ui.horizontal(|ui| {
                    ui.heading(format!("表: {}", table_name_str));
                    ui.separator();

                    // 面板切换按钮
                    if ui
                        .button(if self.show_table_panel {
                            "✓ 表格"
                        } else {
                            "表格"
                        })
                        .clicked()
                    {
                        self.show_table_panel = !self.show_table_panel;
                        log::debug!("切换表格面板: {}", self.show_table_panel);
                    }

                    if ui
                        .button(if self.show_visualization_panel {
                            "✓ 可视化"
                        } else {
                            "可视化"
                        })
                        .clicked()
                    {
                        self.show_visualization_panel = !self.show_visualization_panel;
                        log::debug!("切换可视化面板: {}", self.show_visualization_panel);
                    }

                    ui.separator();

                    if ui.button("返回时间线").clicked() {
                        log::info!("用户点击返回时间线");
                        should_clear_selection = true;
                    }
                });
                ui.separator();

                // 根据面板显示状态渲染内容
                let both_panels = self.show_table_panel && self.show_visualization_panel;

                if both_panels {
                    // 两个面板都显示，使用分割布局
                    self.render_split_panels(ui, &table_name_str, &mut next_page);
                } else if self.show_table_panel {
                    // 只显示表格面板
                    self.render_table_only(ui, &table_name_str, &mut next_page);
                } else if self.show_visualization_panel {
                    // 只显示可视化面板
                    self.render_visualization_only(ui);
                } else {
                    // 两个都不显示
                    ui.vertical_centered(|ui| {
                        ui.add_space(200.0);
                        ui.heading("请至少启用一个面板");
                        ui.label("点击上方的「表格」或「可视化」按钮");
                    });
                }
            } else {
                // 显示时间线
                ui.heading("时间线");
                ui.separator();

                if self.db.function_calls.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(200.0);
                        ui.heading("没有函数调用数据");
                        ui.label("请拖放一个包含性能分析数据的 SQLite 文件");
                    });
                } else {
                    // 获取可见范围内的调用
                    let calls: Vec<_> = self
                        .db
                        .get_calls_in_range(self.timeline.view_start, self.timeline.view_end);

                    self.timeline.render(ui, &calls);
                }
            }
        });

        // 处理中央面板的操作
        if should_clear_selection {
            self.selected_table = None;
            self.table_data = None;
        }

        if let Some(page) = next_page {
            self.load_page(page);
        }
    }
}

impl NsysViewerApp {
    fn load_database(&mut self, path: PathBuf) {
        log::info!("开始加载数据库: {}", path.display());
        self.error_message = None;

        match self.db.load_file(&path) {
            Ok(()) => {
                log::info!("数据库加载成功");
                log::debug!("找到 {} 个数据表", self.db.tables.len());
                log::debug!("找到 {} 个函数调用", self.db.function_calls.len());

                self.file_path = Some(path);

                // 设置时间线的初始范围
                if let Some((min_time, max_time)) = self.db.get_time_range() {
                    log::debug!("时间范围: {} 到 {}", min_time, max_time);
                    self.timeline.set_range(min_time, max_time);
                    self.timeline.clear_selection();
                }

                if self.db.function_calls.is_empty() {
                    log::warn!("未找到函数调用数据");
                    self.error_message =
                        Some("未找到函数调用数据，请确保这是一个有效的 NSYS 输出文件".to_string());
                }
            }
            Err(e) => {
                log::error!("加载数据库失败: {}", e);
                self.error_message = Some(format!("加载数据库失败: {}", e));
            }
        }
    }

    fn load_table_data(&mut self) {
        if let Some(table_name) = &self.selected_table {
            log::info!("加载表数据: {}, 页大小: {}", table_name, self.page_size);
            self.current_page = 0;
            match self.db.get_table_data(table_name, self.page_size, 0) {
                Ok(data) => {
                    log::debug!(
                        "表数据加载成功: {} 列, {} 行",
                        data.columns.len(),
                        data.rows.len()
                    );
                    self.table_data = Some(data);
                }
                Err(e) => {
                    log::error!("加载表数据失败: {}", e);
                    self.error_message = Some(format!("加载表数据失败: {}", e));
                    self.table_data = None;
                }
            }
        }
    }

    fn load_page(&mut self, page: usize) {
        if let Some(table_name) = &self.selected_table {
            let offset = page * self.page_size;
            log::debug!("加载第 {} 页，偏移量: {}", page + 1, offset);
            match self.db.get_table_data(table_name, self.page_size, offset) {
                Ok(data) => {
                    log::trace!("页面数据加载成功: {} 行", data.rows.len());
                    self.current_page = page;
                    self.table_data = Some(data);
                }
                Err(e) => {
                    log::error!("加载页面失败: {}", e);
                    self.error_message = Some(format!("加载页面失败: {}", e));
                }
            }
        }
    }

    /// 渲染分割面板（表格 + 可视化）
    fn render_split_panels(
        &mut self,
        ui: &mut egui::Ui,
        table_name: &str,
        next_page: &mut Option<usize>,
    ) {
        let available_height = ui.available_height();

        // 表格面板（上半部分）
        egui::TopBottomPanel::top("table_panel")
            .resizable(true)
            .default_height(available_height * self.table_panel_height_ratio)
            .height_range(100.0..=available_height - 100.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("📊 表格数据");
                    ui.separator();
                    if ui.button("🔄 重置视图").clicked() {
                        self.table_panel_height_ratio = 0.5;
                        log::debug!("重置面板比例为 50:50");
                    }
                });
                ui.separator();

                if let Some(data) = &self.table_data {
                    self.render_table_with_pagination(ui, table_name, data, next_page);
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.spinner();
                        ui.label("正在加载表数据...");
                    });
                }
            });

        // 可视化面板（下半部分）
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("📈 数据可视化");
                ui.separator();
                if ui.button("🔄 重置缩放").clicked() {
                    self.visualization.reset_view();
                }
            });
            ui.separator();

            self.visualization.render(ui, self.table_data.as_ref());
        });
    }

    /// 仅渲染表格面板
    fn render_table_only(
        &mut self,
        ui: &mut egui::Ui,
        table_name: &str,
        next_page: &mut Option<usize>,
    ) {
        ui.strong("📊 表格数据");
        ui.separator();

        if let Some(data) = &self.table_data {
            self.render_table_with_pagination(ui, table_name, data, next_page);
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(200.0);
                ui.spinner();
                ui.label("正在加载表数据...");
            });
        }
    }

    /// 仅渲染可视化面板
    fn render_visualization_only(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("📈 数据可视化");
            ui.separator();
            if ui.button("🔄 重置缩放").clicked() {
                self.visualization.reset_view();
            }
        });
        ui.separator();

        self.visualization.render(ui, self.table_data.as_ref());
    }

    /// 渲染表格及分页控制
    fn render_table_with_pagination(
        &self,
        ui: &mut egui::Ui,
        table_name: &str,
        data: &TableData,
        next_page: &mut Option<usize>,
    ) {
        // 分页控制
        ui.horizontal(|ui| {
            ui.label(format!("页码: {}", self.current_page + 1));
            ui.separator();

            if ui.button("⏮ 上一页").clicked() && self.current_page > 0 {
                log::debug!("用户点击上一页，当前页: {}", self.current_page);
                *next_page = Some(self.current_page - 1);
            }

            if ui.button("下一页 ⏭").clicked() {
                log::debug!("用户点击下一页，当前页: {}", self.current_page);
                *next_page = Some(self.current_page + 1);
            }

            ui.separator();
            ui.label(format!("每页显示: {} 行", self.page_size));

            // 获取总行数
            if let Some(table_info) = self.db.tables.iter().find(|t| t.name == table_name) {
                let total_pages = (table_info.row_count + self.page_size - 1) / self.page_size;
                ui.label(format!("总页数: {}", total_pages));
                ui.label(format!("总行数: {}", table_info.row_count));
            }
        });

        ui.separator();

        // 表格展示
        Self::render_table_ui(ui, data);
    }

    fn render_table_ui(ui: &mut egui::Ui, data: &TableData) {
        // 表格展示
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                use egui_extras::{Column, TableBuilder};

                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .min_scrolled_height(0.0);

                // 动态添加列
                let mut table_builder = table;
                for _ in &data.columns {
                    table_builder = table_builder.column(Column::auto().at_least(100.0));
                }

                table_builder
                    .header(20.0, |mut header| {
                        for col_name in &data.columns {
                            header.col(|ui| {
                                ui.strong(col_name);
                            });
                        }
                    })
                    .body(|mut body| {
                        for row in &data.rows {
                            body.row(18.0, |mut table_row| {
                                for cell in row {
                                    table_row.col(|ui| {
                                        ui.label(cell);
                                    });
                                }
                            });
                        }
                    });
            });
    }
}
