use crate::db::FunctionCall;
use eframe::egui;

pub struct Timeline {
    pub view_start: f64,
    pub view_end: f64,
    pub selection_start: Option<f64>,
    pub selection_end: Option<f64>,
    dragging: bool,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            view_start: 0.0,
            view_end: 1.0,
            selection_start: None,
            selection_end: None,
            dragging: false,
        }
    }

    pub fn set_range(&mut self, start: f64, end: f64) {
        self.view_start = start;
        self.view_end = end;
    }

    pub fn render(&mut self, ui: &mut egui::Ui, calls: &[&FunctionCall]) {
        let available_height = ui.available_height();
        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width(), available_height),
            egui::Sense::click_and_drag(),
        );

        let rect = response.rect;

        // 背景
        painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));

        if self.view_end <= self.view_start {
            return;
        }

        let time_range = self.view_end - self.view_start;

        // 绘制时间刻度
        self.draw_time_axis(&painter, rect, time_range);

        // 绘制函数调用
        let row_height = 25.0;
        let mut hovered_call: Option<&FunctionCall> = None;

        for call in calls {
            if call.end_time < self.view_start || call.start_time > self.view_end {
                continue;
            }

            let x_start = rect.left()
                + ((call.start_time - self.view_start) / time_range * rect.width() as f64) as f32;
            let x_end = rect.left()
                + ((call.end_time - self.view_start) / time_range * rect.width() as f64) as f32;
            let y_top = rect.top() + 30.0 + (call.depth as f32 * row_height);

            let call_rect = egui::Rect::from_min_max(
                egui::pos2(x_start.max(rect.left()), y_top),
                egui::pos2(x_end.min(rect.right()), y_top + row_height - 2.0),
            );

            if call_rect.width() < 0.5 {
                continue;
            }

            // 根据深度选择颜色
            let hue = (call.depth as f32 * 0.1) % 1.0;
            let color = egui::Color32::from_rgb(
                (255.0 * (1.0 - hue)) as u8,
                (255.0 * hue * 0.8) as u8,
                150,
            );

            painter.rect_filled(call_rect, 2.0, color);
            painter.rect_stroke(
                call_rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
            );

            // 如果有足够的空间，显示函数名
            if call_rect.width() > 50.0 {
                let text = if call.name.len() > 30 {
                    format!("{}...", &call.name[..27])
                } else {
                    call.name.clone()
                };

                painter.text(
                    egui::pos2(call_rect.left() + 4.0, call_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(11.0),
                    egui::Color32::WHITE,
                );
            }

            // 检查是否悬停
            if let Some(hover_pos) = response.hover_pos() {
                if call_rect.contains(hover_pos) {
                    hovered_call = Some(call);
                }
            }
        }

        // 显示工具提示（在循环外）
        if let Some(call) = hovered_call {
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("timeline_tooltip"),
                |ui| {
                    ui.label(format!("函数: {}", call.name));
                    ui.label(format!("开始: {:.3} ms", call.start_time / 1_000_000.0));
                    ui.label(format!("结束: {:.3} ms", call.end_time / 1_000_000.0));
                    ui.label(format!("耗时: {:.3} ms", call.duration / 1_000_000.0));
                    ui.label(format!("深度: {}", call.depth));
                },
            );
        }

        // 处理鼠标交互 - 拖动选择时间范围
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let time =
                    self.view_start + ((pos.x - rect.left()) / rect.width()) as f64 * time_range;
                self.selection_start = Some(time);
                self.selection_end = None;
                self.dragging = true;
            }
        }

        if response.dragged() && self.dragging {
            if let Some(pos) = response.interact_pointer_pos() {
                let time =
                    self.view_start + ((pos.x - rect.left()) / rect.width()) as f64 * time_range;
                self.selection_end = Some(time);
            }
        }

        if response.drag_stopped() {
            self.dragging = false;
        }

        // 绘制选择区域
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let x_start =
                rect.left() + ((start - self.view_start) / time_range * rect.width() as f64) as f32;
            let x_end =
                rect.left() + ((end - self.view_start) / time_range * rect.width() as f64) as f32;

            let selection_rect = egui::Rect::from_min_max(
                egui::pos2(x_start.min(x_end), rect.top()),
                egui::pos2(x_start.max(x_end), rect.bottom()),
            );

            painter.rect_filled(
                selection_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(100, 150, 255, 50),
            );
            painter.rect_stroke(
                selection_rect,
                0.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
            );
        }

        // 滚轮缩放
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                if let Some(hover_pos) = response.hover_pos() {
                    let hover_time = self.view_start
                        + ((hover_pos.x - rect.left()) / rect.width()) as f64 * time_range;
                    let zoom_factor = if scroll > 0.0 { 0.9 } else { 1.1 };

                    let new_range = time_range * zoom_factor as f64;
                    let left_ratio = (hover_time - self.view_start) / time_range;

                    self.view_start = hover_time - new_range * left_ratio;
                    self.view_end = self.view_start + new_range;
                }
            }
        }
    }

    fn draw_time_axis(&self, painter: &egui::Painter, rect: egui::Rect, time_range: f64) {
        let num_ticks = 10;
        let tick_interval = time_range / num_ticks as f64;

        for i in 0..=num_ticks {
            let time = self.view_start + tick_interval * i as f64;
            let x = rect.left() + (i as f32 / num_ticks as f32) * rect.width();

            // 刻度线
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + 5.0)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(150)),
            );

            // 时间标签 (转换为毫秒)
            let label = format!("{:.2}ms", time / 1_000_000.0);
            painter.text(
                egui::pos2(x, rect.top() + 15.0),
                egui::Align2::CENTER_TOP,
                label,
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(200),
            );
        }
    }

    pub fn get_selected_duration(&self) -> Option<f64> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            Some((end - start).abs())
        } else {
            None
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
