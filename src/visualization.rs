use crate::db::TableData;
use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// 可视化组件，用于渲染表格数据的图形化展示
pub struct Visualization {
    /// 缩放级别 (1.0 = 100%)
    pub zoom: f32,
    /// 视图偏移量
    pub offset: Vec2,
    /// 是否正在拖拽
    dragging: bool,
    /// 拖拽起始位置
    drag_start: Option<Pos2>,
}

impl Visualization {
    pub fn new() -> Self {
        log::info!("创建可视化组件");
        Self {
            zoom: 1.0,
            offset: Vec2::ZERO,
            dragging: false,
            drag_start: None,
        }
    }

    /// 渲染可视化内容
    pub fn render(&mut self, ui: &mut egui::Ui, data: Option<&TableData>) {
        let available_size = ui.available_size();

        // 创建绘图区域
        let (response, painter) =
            ui.allocate_painter(available_size, egui::Sense::click_and_drag());
        let rect = response.rect;

        // 处理滚轮缩放
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                let zoom_delta = scroll_delta * 0.001;
                let old_zoom = self.zoom;
                self.zoom = (self.zoom + zoom_delta).clamp(0.1, 10.0);
                log::debug!("缩放级别变化: {:.2} -> {:.2}", old_zoom, self.zoom);
            }
        }

        // 处理拖拽平移
        if response.dragged() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if let Some(drag_start) = self.drag_start {
                    let delta = pointer_pos - drag_start;
                    self.offset += delta;
                    self.drag_start = Some(pointer_pos);
                    log::trace!("拖拽偏移: {:?}", self.offset);
                } else {
                    self.drag_start = Some(pointer_pos);
                }
                self.dragging = true;
            }
        } else {
            self.drag_start = None;
            if self.dragging {
                self.dragging = false;
                log::debug!("拖拽结束，当前偏移: {:?}", self.offset);
            }
        }

        // 绘制背景
        painter.rect_filled(rect, 0.0, Color32::from_gray(245));

        // 绘制网格
        self.draw_grid(&painter, rect);

        // 如果有数据，绘制可视化
        if let Some(table_data) = data {
            self.draw_data_visualization(&painter, rect, table_data);
        } else {
            // 显示提示信息
            self.draw_placeholder(&painter, rect);
        }

        // 显示控制信息
        self.draw_controls(&painter, rect);
    }

    /// 绘制网格背景
    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let grid_spacing = 50.0 * self.zoom;
        let color = Color32::from_gray(230);
        let stroke = Stroke::new(1.0, color);

        // 计算网格起始点（考虑偏移）
        let start_x = (rect.left() - self.offset.x % grid_spacing).floor();
        let start_y = (rect.top() - self.offset.y % grid_spacing).floor();

        // 绘制垂直线
        let mut x = start_x;
        while x < rect.right() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                stroke,
            );
            x += grid_spacing;
        }

        // 绘制水平线
        let mut y = start_y;
        while y < rect.bottom() {
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                stroke,
            );
            y += grid_spacing;
        }
    }

    /// 绘制数据可视化
    fn draw_data_visualization(&self, painter: &egui::Painter, rect: Rect, data: &TableData) {
        log::trace!(
            "绘制可视化: {} 列, {} 行",
            data.columns.len(),
            data.rows.len()
        );

        // 计算绘图区域中心
        let _center = rect.center();

        // 分析数据类型并选择合适的可视化方式
        if data.rows.is_empty() {
            return;
        }

        // 检测数值列
        let numeric_columns = self.detect_numeric_columns(data);

        if numeric_columns.len() >= 2 {
            // 如果有至少两个数值列，绘制散点图
            self.draw_scatter_plot(painter, rect, data, &numeric_columns);
        } else if numeric_columns.len() == 1 {
            // 如果有一个数值列，绘制柱状图
            self.draw_bar_chart(painter, rect, data, numeric_columns[0]);
        } else {
            // 否则显示数据摘要
            self.draw_data_summary(painter, rect, data);
        }
    }

    /// 检测数值列
    fn detect_numeric_columns(&self, data: &TableData) -> Vec<usize> {
        let mut numeric_cols = Vec::new();

        for (col_idx, _col_name) in data.columns.iter().enumerate() {
            let mut numeric_count = 0;
            let sample_size = data.rows.len().min(10);

            for row in data.rows.iter().take(sample_size) {
                if col_idx < row.len() {
                    if row[col_idx].parse::<f64>().is_ok() {
                        numeric_count += 1;
                    }
                }
            }

            // 如果超过70%的样本可以解析为数字，认为是数值列
            if numeric_count as f32 / sample_size as f32 > 0.7 {
                numeric_cols.push(col_idx);
            }
        }

        numeric_cols
    }

    /// 绘制散点图
    fn draw_scatter_plot(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        data: &TableData,
        numeric_columns: &[usize],
    ) {
        if numeric_columns.len() < 2 {
            return;
        }

        let x_col = numeric_columns[0];
        let y_col = numeric_columns[1];

        // 收集数据点
        let mut points: Vec<(f64, f64)> = Vec::new();
        for row in &data.rows {
            if x_col < row.len() && y_col < row.len() {
                if let (Ok(x), Ok(y)) = (row[x_col].parse::<f64>(), row[y_col].parse::<f64>()) {
                    points.push((x, y));
                }
            }
        }

        if points.is_empty() {
            return;
        }

        // 计算数据范围
        let x_min = points.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
        let x_max = points
            .iter()
            .map(|(x, _)| *x)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_min = points.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
        let y_max = points
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::NEG_INFINITY, f64::max);

        let x_range = (x_max - x_min).max(1.0);
        let y_range = (y_max - y_min).max(1.0);

        // 绘图区域（留出边距）
        let margin = 60.0;
        let plot_rect = Rect::from_min_max(
            Pos2::new(rect.left() + margin, rect.top() + margin),
            Pos2::new(rect.right() - margin, rect.bottom() - margin),
        );

        // 绘制坐标轴
        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.right_bottom()],
            Stroke::new(2.0, Color32::BLACK),
        );
        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.left_top()],
            Stroke::new(2.0, Color32::BLACK),
        );

        // 绘制轴标签
        painter.text(
            Pos2::new(plot_rect.center().x, rect.bottom() - 20.0),
            egui::Align2::CENTER_CENTER,
            &data.columns[x_col],
            egui::FontId::proportional(14.0),
            Color32::BLACK,
        );

        painter.text(
            Pos2::new(rect.left() + 20.0, plot_rect.center().y),
            egui::Align2::CENTER_CENTER,
            &data.columns[y_col],
            egui::FontId::proportional(14.0),
            Color32::BLACK,
        );

        // 保存点数用于显示
        let points_count = points.len();

        // 绘制数据点
        for (x, y) in &points {
            let screen_x =
                plot_rect.left() + ((x - x_min) / x_range * plot_rect.width() as f64) as f32;
            let screen_y =
                plot_rect.bottom() - ((y - y_min) / y_range * plot_rect.height() as f64) as f32;

            let pos = Pos2::new(screen_x, screen_y);
            let point_pos = Pos2::new(
                pos.x * self.zoom + self.offset.x,
                pos.y * self.zoom + self.offset.y,
            );

            painter.circle_filled(point_pos, 4.0 * self.zoom, Color32::from_rgb(70, 130, 180));
        }

        // 显示数据范围
        let info_text = format!(
            "X: [{:.2}, {:.2}]  Y: [{:.2}, {:.2}]  点数: {}",
            x_min, x_max, y_min, y_max, points_count
        );
        painter.text(
            Pos2::new(rect.center().x, rect.top() + 20.0),
            egui::Align2::CENTER_CENTER,
            info_text,
            egui::FontId::proportional(12.0),
            Color32::DARK_GRAY,
        );
    }

    /// 绘制柱状图
    fn draw_bar_chart(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        data: &TableData,
        col_idx: usize,
    ) {
        let mut values: Vec<f64> = Vec::new();
        for row in &data.rows {
            if col_idx < row.len() {
                if let Ok(val) = row[col_idx].parse::<f64>() {
                    values.push(val);
                }
            }
        }

        if values.is_empty() {
            return;
        }

        let max_val = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let min_val = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let range = (max_val - min_val).max(1.0);

        let margin = 60.0;
        let plot_rect = Rect::from_min_max(
            Pos2::new(rect.left() + margin, rect.top() + margin),
            Pos2::new(rect.right() - margin, rect.bottom() - margin),
        );

        let bar_width = (plot_rect.width() / values.len() as f32 * 0.8).min(50.0);
        let spacing = plot_rect.width() / values.len() as f32;

        for (i, &value) in values.iter().enumerate() {
            let x = plot_rect.left() + i as f32 * spacing + spacing / 2.0;
            let height = ((value - min_val) / range * plot_rect.height() as f64) as f32;
            let y = plot_rect.bottom() - height;

            let bar_rect = Rect::from_min_max(
                Pos2::new(x - bar_width / 2.0, y),
                Pos2::new(x + bar_width / 2.0, plot_rect.bottom()),
            );

            // 应用缩放和偏移
            let transformed_rect = Rect::from_min_max(
                Pos2::new(
                    bar_rect.min.x * self.zoom + self.offset.x,
                    bar_rect.min.y * self.zoom + self.offset.y,
                ),
                Pos2::new(
                    bar_rect.max.x * self.zoom + self.offset.x,
                    bar_rect.max.y * self.zoom + self.offset.y,
                ),
            );

            painter.rect_filled(transformed_rect, 0.0, Color32::from_rgb(100, 150, 200));
        }

        // 标题
        painter.text(
            Pos2::new(rect.center().x, rect.top() + 20.0),
            egui::Align2::CENTER_CENTER,
            format!("{} - 柱状图", data.columns[col_idx]),
            egui::FontId::proportional(14.0),
            Color32::BLACK,
        );
    }

    /// 绘制数据摘要
    fn draw_data_summary(&self, painter: &egui::Painter, rect: Rect, data: &TableData) {
        let center = rect.center();

        let summary = format!(
            "数据摘要\n\n列数: {}\n行数: {}\n\n提示: 需要数值类型的列才能生成图表",
            data.columns.len(),
            data.rows.len()
        );

        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            summary,
            egui::FontId::proportional(16.0),
            Color32::DARK_GRAY,
        );
    }

    /// 绘制占位符
    fn draw_placeholder(&self, painter: &egui::Painter, rect: Rect) {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "选择数据表以显示可视化",
            egui::FontId::proportional(18.0),
            Color32::GRAY,
        );
    }

    /// 绘制控制信息
    fn draw_controls(&self, painter: &egui::Painter, rect: Rect) {
        let controls_text = format!(
            "缩放: {:.0}%  |  鼠标滚轮缩放  |  拖拽移动",
            self.zoom * 100.0
        );

        painter.text(
            Pos2::new(rect.right() - 10.0, rect.bottom() - 10.0),
            egui::Align2::RIGHT_BOTTOM,
            controls_text,
            egui::FontId::proportional(11.0),
            Color32::from_gray(100),
        );
    }

    /// 重置视图
    pub fn reset_view(&mut self) {
        log::info!("重置可视化视图");
        self.zoom = 1.0;
        self.offset = Vec2::ZERO;
    }
}

impl Default for Visualization {
    fn default() -> Self {
        Self::new()
    }
}
