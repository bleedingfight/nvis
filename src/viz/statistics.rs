use anyhow::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::core::types::{ColumnValue, ProfilerData, ViewCategory, ViewDescriptor};
use crate::viz::renderer::VizRenderer;
use crate::viz::types::{PreparedVisualization, VizData};

pub struct StatisticsRenderer;

fn generate_statistics_text(view_name: &str, data: &ProfilerData) -> String {
    let mut text = format!("View: {}\n", view_name);
    text.push_str(&format!("Columns: {}\n", data.schema.len()));
    text.push_str(&format!("Rows: {}\n\n", data.row_count));

    for (col_idx, col_schema) in data.schema.iter().enumerate() {
        let values: Vec<f64> = data.columns[col_idx]
            .iter()
            .filter_map(|v| match v {
                ColumnValue::Float(f) => Some(*f),
                ColumnValue::Integer(i) => Some(*i as f64),
                _ => None,
            })
            .collect();

        if !values.is_empty() && values.len() > data.row_count / 2 {
            let sum: f64 = values.iter().sum();
            let mean = sum / values.len() as f64;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            text.push_str(&format!("{}:\n", col_schema.name));
            text.push_str(&format!("  Mean: {:.2}\n", mean));
            text.push_str(&format!("  Min: {:.2}\n", min));
            text.push_str(&format!("  Max: {:.2}\n\n", max));
        }
    }

    text
}

fn draw_statistics_text(f: &mut Frame, area: Rect, title: &str, text: &str, focused: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));
    f.render_widget(paragraph, area);
}

impl VizRenderer for StatisticsRenderer {
    fn id(&self) -> &str {
        "statistics"
    }

    fn can_render(&self, _data: &ProfilerData, view: &ViewDescriptor) -> bool {
        view.category == ViewCategory::Metadata || view.category == ViewCategory::Statistics
    }

    fn prepare(
        &self,
        data: &ProfilerData,
        view: &ViewDescriptor,
        _scroll: usize,
    ) -> Result<PreparedVisualization> {
        let text = generate_statistics_text(&view.display_name, data);
        Ok(PreparedVisualization {
            title: format!("{} - Statistics", view.display_name),
            data: VizData::StatsText(text),
            viewport: None,
        })
    }

    fn draw(&self, f: &mut Frame, area: Rect, viz: &PreparedVisualization, focused: bool) {
        if let VizData::StatsText(ref text) = &viz.data {
            draw_statistics_text(f, area, &viz.title, text, focused);
        }
    }
}
