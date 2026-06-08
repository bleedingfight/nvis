use anyhow::Result;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::collections::HashMap;

use crate::core::types::{ColumnValue, ProfilerData, ViewCategory, ViewDescriptor, truncate_str};
use crate::viz::renderer::VizRenderer;
use crate::viz::types::{BoxPlotData, PreparedVisualization, VizData};

pub struct BoxPlotRenderer;

fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    let n = sorted_values.len();
    if n == 0 {
        return 0.0;
    }
    let idx = (p / 100.0) * (n - 1) as f64;
    let lower = idx.floor() as usize;
    let upper = idx.ceil() as usize;
    let weight = idx - lower as f64;
    if lower == upper {
        sorted_values[lower]
    } else if upper < n {
        sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
    } else {
        sorted_values[lower]
    }
}

fn generate_boxplot_data(data: &ProfilerData) -> Vec<BoxPlotData> {
    let lowered: Vec<String> = data.schema.iter().map(|s| s.name.to_lowercase()).collect();
    let mut name_col: Option<usize> = None;
    for exact in ["name", "function", "api", "apiname"] {
        if let Some(idx) = lowered.iter().position(|c| c == exact) {
            name_col = Some(idx);
            break;
        }
    }
    if name_col.is_none() {
        name_col = lowered.iter().position(|c| {
            (c.contains("function") || c.contains("api") || c.contains("name")) && !c.contains("id")
        });
    }

    if name_col.is_none() {
        return Vec::new();
    }

    let name_idx = name_col.unwrap();

    let duration_col = data.schema.iter().position(|s| {
        let cl = s.name.to_lowercase();
        cl == "duration"
            || cl.contains("duration")
            || cl.contains("elapsed")
            || (cl == "time")
            || (cl.contains("time") && !cl.contains("start") && !cl.contains("end"))
    });

    let start_col = data
        .schema
        .iter()
        .position(|s| s.name.to_lowercase().contains("start"));

    let end_col = data
        .schema
        .iter()
        .position(|s| s.name.to_lowercase().contains("end"));

    if duration_col.is_none() && (start_col.is_none() || end_col.is_none()) {
        return Vec::new();
    }

    let mut groups: HashMap<String, Vec<f64>> = HashMap::new();

    for row_idx in 0..data.row_count {
        let name_val = match &data.columns[name_idx].get(row_idx) {
            Some(ColumnValue::Text(s)) => s.clone(),
            Some(ColumnValue::Integer(i)) => i.to_string(),
            _ => continue,
        };

        let duration_ns = if let Some(dur_idx) = duration_col {
            match data.columns[dur_idx].get(row_idx) {
                Some(ColumnValue::Float(f)) => Some(*f),
                Some(ColumnValue::Integer(i)) => Some(*i as f64),
                _ => None,
            }
        } else if let (Some(si), Some(ei)) = (start_col, end_col) {
            let start = match data.columns[si].get(row_idx) {
                Some(ColumnValue::Float(f)) => Some(*f),
                Some(ColumnValue::Integer(i)) => Some(*i as f64),
                _ => None,
            };
            let end = match data.columns[ei].get(row_idx) {
                Some(ColumnValue::Float(f)) => Some(*f),
                Some(ColumnValue::Integer(i)) => Some(*i as f64),
                _ => None,
            };
            match (start, end) {
                (Some(s), Some(e)) => Some(e - s),
                _ => None,
            }
        } else {
            None
        };

        if let Some(dur_value) = duration_ns {
            if dur_value >= 0.0 {
                groups.entry(name_val).or_default().push(dur_value);
            }
        }
    }

    let mut result: Vec<BoxPlotData> = groups
        .into_iter()
        .map(|(label, mut values)| {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let count = values.len();
            let min = values[0];
            let max = values[count - 1];
            let mean = values.iter().sum::<f64>() / count as f64;
            let q1 = percentile(&values, 25.0);
            let median = percentile(&values, 50.0);
            let q3 = percentile(&values, 75.0);
            BoxPlotData {
                label,
                min,
                max,
                median,
                q1,
                q3,
                mean,
                count,
            }
        })
        .collect();

    result.sort_by(|a, b| b.mean.partial_cmp(&a.mean).unwrap());
    result.truncate(10);
    result
}

fn draw_boxplot_chart(
    f: &mut Frame,
    area: Rect,
    title: &str,
    boxplots: &[BoxPlotData],
    focused: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if boxplots.is_empty() {
        let paragraph = Paragraph::new("No statistical data available").block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 24 || inner.height < 10 {
        let paragraph = Paragraph::new("Area too small to draw chart");
        f.render_widget(paragraph, inner);
        return;
    }

    let y_axis_w = 12u16.min(inner.width / 3);
    let x_axis_h = 3u16;
    let plot_left = inner.x + y_axis_w;
    let plot_top = inner.y + 1;
    let plot_right = inner.x + inner.width.saturating_sub(2);
    let plot_bottom = inner.y + inner.height.saturating_sub(1 + x_axis_h);
    if plot_right <= plot_left || plot_bottom <= plot_top {
        return;
    }
    let plot_w = plot_right - plot_left;
    let plot_h = plot_bottom - plot_top;

    let max_items = (plot_w / 10).max(1) as usize;
    let items: Vec<&BoxPlotData> = boxplots.iter().take(max_items).collect();
    if items.is_empty() {
        return;
    }

    let gmin = items.iter().map(|b| b.min).fold(f64::INFINITY, f64::min);
    let gmax = items
        .iter()
        .map(|b| b.max)
        .fold(f64::NEG_INFINITY, f64::max);
    if !(gmax > gmin) {
        return;
    }

    let map_y = |v: f64| -> u16 {
        let ratio = ((v - gmin) / (gmax - gmin)).clamp(0.0, 1.0);
        plot_bottom - (ratio * plot_h as f64) as u16
    };

    for yy in plot_top..=plot_bottom {
        f.render_widget(Paragraph::new("│"), Rect::new(plot_left, yy, 1, 1));
    }
    let tick_n = 4u16;
    let fmt = |v: f64| -> String {
        if v.abs() >= 1e9 {
            format!("{:.1}e9", v / 1e9)
        } else if v.abs() >= 1e6 {
            format!("{:.1}e6", v / 1e6)
        } else if v.abs() >= 1e3 {
            format!("{:.1}e3", v / 1e3)
        } else {
            format!("{:.0}", v)
        }
    };
    for i in 0..=tick_n {
        let t = gmin + (gmax - gmin) * (i as f64) / (tick_n as f64);
        let y = map_y(t);
        f.render_widget(Paragraph::new("┼"), Rect::new(plot_left, y, 1, 1));
        f.render_widget(
            Paragraph::new("─".repeat(3)),
            Rect::new(plot_left + 1, y, 3, 1),
        );
        let label = format!("{:>width$}", fmt(t), width = (y_axis_w - 2) as usize);
        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(Color::Gray)),
            Rect::new(inner.x + 1, y, y_axis_w.saturating_sub(2), 1),
        );
    }
    f.render_widget(
        Paragraph::new("耗时 (ns)").style(Style::default().fg(Color::Gray)),
        Rect::new(
            inner.x + 1,
            plot_top.saturating_sub(1),
            y_axis_w.saturating_sub(2),
            1,
        ),
    );

    f.render_widget(
        Paragraph::new("─".repeat(plot_w as usize)),
        Rect::new(plot_left, plot_bottom, plot_w, 1),
    );

    let slot_w = (plot_w / items.len() as u16).max(10);
    for (i, b) in items.iter().enumerate() {
        let slot_x = plot_left + (i as u16) * slot_w;
        let cx = slot_x + (slot_w / 2);
        let box_w = (slot_w.saturating_sub(4)).min(12).max(6);
        let left = cx.saturating_sub(box_w / 2);

        let min_y = map_y(b.min);
        let q1_y = map_y(b.q1);
        let med_y = map_y(b.median);
        let q3_y = map_y(b.q3);
        let max_y = map_y(b.max);
        let (top_y, bottom_y) = (q3_y.min(q1_y), q3_y.max(q1_y));

        let top_line = "─".repeat(box_w as usize);
        f.render_widget(
            Paragraph::new(top_line.clone()).style(Style::default().fg(Color::Green)),
            Rect::new(left, max_y, box_w, 1),
        );
        f.render_widget(
            Paragraph::new(top_line).style(Style::default().fg(Color::Green)),
            Rect::new(left, min_y, box_w, 1),
        );
        for yy in (top_y + 1)..max_y {
            f.render_widget(Paragraph::new("│"), Rect::new(cx, yy, 1, 1));
        }
        for yy in (min_y + 1)..bottom_y {
            f.render_widget(Paragraph::new("│"), Rect::new(cx, yy, 1, 1));
        }
        for yy in top_y..=bottom_y {
            let line = if yy == top_y && yy == bottom_y {
                format!("┌{}┐", "─".repeat((box_w - 2) as usize))
            } else if yy == top_y {
                format!("┌{}┐", "─".repeat((box_w - 2) as usize))
            } else if yy == bottom_y {
                format!("└{}┘", "─".repeat((box_w - 2) as usize))
            } else {
                format!("│{}│", " ".repeat((box_w - 2) as usize))
            };
            f.render_widget(
                Paragraph::new(line).style(Style::default().fg(Color::Green)),
                Rect::new(left, yy, box_w, 1),
            );
        }
        let med_len = (box_w - 2).max(1);
        f.render_widget(
            Paragraph::new("─".repeat(med_len as usize))
                .style(Style::default().fg(Color::Yellow)),
            Rect::new(left + 1, med_y, med_len, 1),
        );

        f.render_widget(Paragraph::new("┬"), Rect::new(cx, plot_bottom, 1, 1));
        let label = if b.label.chars().count() > (slot_w as usize - 2) {
            truncate_str(&b.label, (slot_w as usize - 5).max(3))
        } else {
            b.label.clone()
        };
        let lbl = Paragraph::new(label).alignment(Alignment::Center);
        f.render_widget(lbl, Rect::new(slot_x, plot_bottom + 1, slot_w, 1));
    }
}

impl VizRenderer for BoxPlotRenderer {
    fn id(&self) -> &str {
        "boxplot"
    }

    fn can_render(&self, data: &ProfilerData, view: &ViewDescriptor) -> bool {
        if view.category == ViewCategory::Statistics {
            let lowered: Vec<String> = data.schema.iter().map(|s| s.name.to_lowercase()).collect();
            let has_name = lowered.iter().any(|c| {
                c.contains("function")
                    || c.contains("api")
                    || c.contains("name") && !c.contains("id")
            });
            let has_duration = lowered.iter().any(|c| {
                c.contains("duration")
                    || c.contains("elapsed")
                    || (c.contains("time") && !c.contains("start") && !c.contains("end"))
                    || c.contains("start") && lowered.iter().any(|d| d.contains("end"))
            });
            return has_name && has_duration;
        }
        false
    }

    fn prepare(
        &self,
        data: &ProfilerData,
        view: &ViewDescriptor,
        _scroll: usize,
    ) -> Result<PreparedVisualization> {
        let boxplots = generate_boxplot_data(data);
        Ok(PreparedVisualization {
            title: format!("{} - API Call Statistics", view.display_name),
            data: VizData::BoxPlots(boxplots),
            viewport: None,
        })
    }

    fn draw(&self, f: &mut Frame, area: Rect, viz: &PreparedVisualization, focused: bool) {
        if let VizData::BoxPlots(ref boxplots) = &viz.data {
            draw_boxplot_chart(f, area, &viz.title, boxplots, focused);
        }
    }
}
