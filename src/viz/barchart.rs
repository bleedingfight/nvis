use anyhow::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{BarChart, Block, Borders, Paragraph},
    Frame,
};

use crate::core::types::{ColumnValue, ProfilerData, ViewCategory, ViewDescriptor};
use crate::viz::renderer::VizRenderer;
use crate::viz::types::{PreparedVisualization, VizData};

pub struct BarChartRenderer;

fn generate_bar_chart_data(data: &ProfilerData, scroll: usize) -> Vec<(String, f64)> {
    if data.row_count == 0 {
        return Vec::new();
    }

    let mut numeric_col = None;
    for (col_idx, _col_name) in data.schema.iter().enumerate() {
        let has_float = data.columns[col_idx]
            .iter()
            .any(|v| matches!(v, ColumnValue::Float(_) | ColumnValue::Integer(_)));
        if has_float {
            numeric_col = Some(col_idx);
            break;
        }
    }

    if let Some(col_idx) = numeric_col {
        let start = scroll.min(data.row_count.saturating_sub(1));
        let end = (start + 10).min(data.row_count);

        if start >= end {
            return Vec::new();
        }

        data.columns[col_idx][start..end]
            .iter()
            .enumerate()
            .filter_map(|(i, v)| match v {
                ColumnValue::Float(f) => Some((format!("R{}", start + i), f.abs())),
                ColumnValue::Integer(n) => Some((format!("R{}", start + i), (*n as f64).abs())),
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn draw_bar_chart(f: &mut Frame, area: Rect, title: &str, labels: &[String], values: &[f64], focused: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if values.is_empty() {
        let paragraph = Paragraph::new("No numeric data available").block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let data: Vec<(&str, u64)> = labels
        .iter()
        .zip(values.iter())
        .map(|(l, v)| (l.as_str(), *v as u64))
        .collect();

    let barchart = BarChart::default()
        .block(block)
        .bar_width(5)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::Green))
        .value_style(Style::default().fg(Color::White).bg(Color::Green))
        .data(&data);

    f.render_widget(barchart, area);
}

impl VizRenderer for BarChartRenderer {
    fn id(&self) -> &str {
        "barchart"
    }

    fn can_render(&self, _data: &ProfilerData, view: &ViewDescriptor) -> bool {
        view.category == ViewCategory::RawData || view.category == ViewCategory::Metrics
    }

    fn prepare(
        &self,
        data: &ProfilerData,
        view: &ViewDescriptor,
        scroll: usize,
    ) -> Result<PreparedVisualization> {
        let items = generate_bar_chart_data(data, scroll);
        let (labels, values): (Vec<String>, Vec<f64>) = items.into_iter().unzip();
        Ok(PreparedVisualization {
            title: format!("{} - Data", view.display_name),
            data: VizData::Bars { labels, values },
            viewport: None,
        })
    }

    fn draw(&self, f: &mut Frame, area: Rect, viz: &PreparedVisualization, focused: bool) {
        if let VizData::Bars { labels, values } = &viz.data {
            draw_bar_chart(f, area, &viz.title, labels, values, focused);
        }
    }
}
