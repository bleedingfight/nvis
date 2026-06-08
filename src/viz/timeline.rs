use anyhow::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    symbols::Marker,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Rectangle};
use std::collections::HashMap;

use crate::core::types::{ColumnValue, ProfilerData, ViewCategory, ViewDescriptor};
use crate::viz::renderer::VizRenderer;
use crate::viz::types::{
    PreparedVisualization, TimelineEvent, TimelineEventType, TimelinePrepared, TimelineViewport,
    VizData,
};

// --- Shared geometry constants and functions ---

pub const LANE_LABEL_COLS: u16 = 10;
pub const TIME_AXIS_ROWS: u16 = 2;
pub const LANE_HEIGHT_ROWS: u16 = 2;
pub const MIN_PLOT_WIDTH: u16 = 20;
pub const MIN_PLOT_HEIGHT: u16 = 4;

pub fn plot_area(inner: Rect) -> Rect {
    Rect {
        x: inner.x + LANE_LABEL_COLS,
        y: inner.y + TIME_AXIS_ROWS,
        width: inner.width.saturating_sub(LANE_LABEL_COLS),
        height: inner.height.saturating_sub(TIME_AXIS_ROWS),
    }
}

pub fn col_to_ns(col: f64, view_start_ns: f64, view_width_ns: f64, plot_width: u16) -> f64 {
    view_start_ns + (col / plot_width as f64) * view_width_ns
}

pub fn ns_to_col(ns: f64, view_start_ns: f64, view_width_ns: f64, plot_width: u16) -> f64 {
    ((ns - view_start_ns) / view_width_ns) * plot_width as f64
}

pub fn row_to_lane(row: u16, plot_y: u16, num_lanes: usize) -> Option<usize> {
    if row < plot_y {
        return None;
    }
    let lane = (row - plot_y) as usize / LANE_HEIGHT_ROWS as usize;
    if lane < num_lanes {
        Some(lane)
    } else {
        None
    }
}

pub fn lane_to_row(lane_idx: usize, plot_y: u16) -> u16 {
    plot_y + (lane_idx as u16) * LANE_HEIGHT_ROWS
}

pub fn format_ns(ns: f64) -> String {
    if ns >= 1e9 {
        format!("{:.1}s", ns / 1e9)
    } else if ns >= 1e6 {
        format!("{:.1}ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.1}us", ns / 1e3)
    } else {
        format!("{:.0}ns", ns)
    }
}

pub fn kernel_color(name: &str) -> Color {
    let palette = [
        Color::Cyan,
        Color::Green,
        Color::Blue,
        Color::Red,
        Color::Magenta,
        Color::Yellow,
        Color::Rgb(255, 165, 0),
        Color::Rgb(0, 206, 209),
    ];
    let hash = name.chars().fold(0u32, |acc, c| acc.wrapping_add(c as u32));
    palette[hash as usize % palette.len()]
}

// --- Data generation ---

fn generate_timeline_data(data: &ProfilerData) -> Vec<TimelineEvent> {
    let lowered: Vec<String> = data.schema.iter().map(|s| s.name.to_lowercase()).collect();

    let name_col = lowered
        .iter()
        .position(|c| c == "name")
        .or_else(|| {
            lowered
                .iter()
                .position(|c| c.contains("name") && !c.contains("id"))
        })
        .or_else(|| lowered.iter().position(|c| c.contains("kernel")));

    let start_col = lowered.iter().position(|c| c == "start");
    let end_col = lowered.iter().position(|c| c == "end");
    let stream_col = lowered.iter().position(|c| c == "streamid");
    let bytes_col = lowered.iter().position(|c| c == "bytes");
    let copy_kind_col = lowered.iter().position(|c| c.contains("copykind"));

    let is_memcpy = copy_kind_col.is_some() || bytes_col.is_some();
    let is_memset = lowered
        .iter()
        .any(|c| c.contains("memkind") && !is_memcpy);

    if name_col.is_none() || start_col.is_none() {
        return Vec::new();
    }

    let name_idx = name_col.unwrap();
    let start_idx = start_col.unwrap();

    let mut events = Vec::new();

    for row_idx in 0..data.row_count {
        let name = match data.columns[name_idx].get(row_idx) {
            Some(ColumnValue::Text(s)) => {
                if s.is_empty() && is_memcpy {
                    if let Some(bi) = bytes_col {
                        if let Some(ColumnValue::Integer(bytes)) = data.columns[bi].get(row_idx) {
                            format!("Memcpy {}", format_bytes(*bytes))
                        } else {
                            "Memcpy".to_string()
                        }
                    } else {
                        "Memcpy".to_string()
                    }
                } else {
                    s.clone()
                }
            }
            Some(ColumnValue::Integer(i)) => i.to_string(),
            _ => continue,
        };

        let start = match data.columns[start_idx].get(row_idx) {
            Some(ColumnValue::Float(f)) => *f,
            Some(ColumnValue::Integer(i)) => *i as f64,
            _ => continue,
        };

        let duration = if let (Some(_si), Some(ei)) = (Some(start_idx), end_col) {
            let end_val = match data.columns[ei].get(row_idx) {
                Some(ColumnValue::Float(f)) => *f,
                Some(ColumnValue::Integer(i)) => *i as f64,
                _ => start + 1.0,
            };
            (end_val - start).max(0.0)
        } else {
            1.0
        };

        let stream_id = stream_col
            .and_then(|si| match data.columns[si].get(row_idx) {
                Some(ColumnValue::Integer(i)) => Some(*i),
                _ => None,
            })
            .unwrap_or(0);

        let event_type = if is_memcpy {
            TimelineEventType::Memcpy
        } else if is_memset {
            TimelineEventType::Memset
        } else {
            TimelineEventType::Kernel
        };

        events.push(TimelineEvent {
            name,
            start,
            duration,
            stream_id,
            event_type,
        });
    }

    events.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    events
}

fn format_bytes(bytes: i64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1}MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

// --- Renderer ---

pub struct TimelineRenderer;

impl VizRenderer for TimelineRenderer {
    fn id(&self) -> &str {
        "timeline"
    }

    fn can_render(&self, data: &ProfilerData, view: &ViewDescriptor) -> bool {
        if view.category == ViewCategory::Timeline {
            let lowered: Vec<String> = data.schema.iter().map(|s| s.name.to_lowercase()).collect();
            let has_name = lowered
                .iter()
                .any(|c| c.contains("name") || c.contains("kernel"));
            let has_start = lowered.iter().any(|c| c.contains("start"));
            return has_name && has_start;
        }
        false
    }

    fn prepare(
        &self,
        data: &ProfilerData,
        view: &ViewDescriptor,
        _scroll: usize,
    ) -> Result<PreparedVisualization> {
        let events = generate_timeline_data(data);

        if events.is_empty() {
            return Ok(PreparedVisualization {
                title: format!("{} - Timeline", view.display_name),
                data: VizData::Timeline(TimelinePrepared {
                    events: Vec::new(),
                    global_start_ns: 0.0,
                    global_end_ns: 1.0,
                    stream_ids: Vec::new(),
                }),
                viewport: None,
            });
        }

        let global_start_ns = events
            .iter()
            .map(|e| e.start)
            .fold(f64::INFINITY, f64::min);
        let global_end_ns = events
            .iter()
            .map(|e| e.start + e.duration)
            .fold(f64::NEG_INFINITY, f64::max);

        let mut stream_ids: Vec<i64> = events.iter().map(|e| e.stream_id).collect();
        stream_ids.sort();
        stream_ids.dedup();

        Ok(PreparedVisualization {
            title: format!("{} - Timeline", view.display_name),
            data: VizData::Timeline(TimelinePrepared {
                events,
                global_start_ns,
                global_end_ns,
                stream_ids,
            }),
            viewport: None,
        })
    }

    fn draw(&self, f: &mut Frame, area: Rect, viz: &PreparedVisualization, focused: bool) {
        let VizData::Timeline(ref prepared) = viz.data else {
            return;
        };

        let default_viewport = TimelineViewport {
            view_start_ns: prepared.global_start_ns,
            view_width_ns: prepared.global_end_ns - prepared.global_start_ns,
            stream_lanes: prepared.stream_ids.clone(),
            hovered: None,
            selection: None,
            drag_origin: None,
            panning: false,
            pan_anchor_ns: 0.0,
            pan_anchor_col: 0,
        };
        let viewport = viz.viewport.as_ref().unwrap_or(&default_viewport);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(viz.title.as_str())
            .border_style(if focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });

        let inner = block.inner(area);
        f.render_widget(block, area);

        if prepared.events.is_empty() {
            f.render_widget(Paragraph::new("No timeline data available"), inner);
            return;
        }

        let num_lanes = prepared.stream_ids.len();
        let pa = plot_area(inner);

        if pa.width < MIN_PLOT_WIDTH || pa.height < MIN_PLOT_HEIGHT {
            f.render_widget(Paragraph::new("Area too small to draw timeline"), inner);
            return;
        }

        // Draw lane labels
        for (i, stream_id) in prepared.stream_ids.iter().enumerate() {
            let label = format!(" S{:>5}", stream_id);
            let y = inner.y + TIME_AXIS_ROWS + (i as u16) * LANE_HEIGHT_ROWS;
            f.render_widget(
                Paragraph::new(label).style(Style::default().fg(Color::DarkGray)),
                Rect::new(inner.x, y, LANE_LABEL_COLS, LANE_HEIGHT_ROWS),
            );
        }

        // Draw time axis
        let num_ticks = (pa.width / 16).max(1) as usize;
        for i in 0..=num_ticks {
            let ns =
                viewport.view_start_ns + viewport.view_width_ns * (i as f64 / num_ticks as f64);
            let col = ns_to_col(ns, viewport.view_start_ns, viewport.view_width_ns, pa.width);
            if col >= 0.0 && (col as u16) < pa.width {
                let x = pa.x + col as u16;
                let label = format_ns(ns);
                f.render_widget(
                    Paragraph::new(label.clone()).style(Style::default().fg(Color::Gray)),
                    Rect::new(
                        x.saturating_sub((label.len() / 2) as u16),
                        inner.y,
                        label.len() as u16 + 1,
                        1,
                    ),
                );
                f.render_widget(
                    Paragraph::new("|").style(Style::default().fg(Color::DarkGray)),
                    Rect::new(x, inner.y + 1, 1, 1),
                );
            }
        }

        // Build stream lane index map
        let lane_idx_map: HashMap<i64, usize> = prepared
            .stream_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        // Canvas for the main plot area
        let canvas_area = Rect {
            x: pa.x,
            y: pa.y,
            width: pa.width,
            height: (num_lanes as u16) * LANE_HEIGHT_ROWS,
        };

        let view_start = viewport.view_start_ns;
        let view_width = viewport.view_width_ns;
        let x_bounds = [view_start, view_start + view_width];
        let y_bounds = [0.0, num_lanes as f64];

        let canvas = Canvas::default()
            .marker(Marker::HalfBlock)
            .x_bounds(x_bounds)
            .y_bounds(y_bounds)
            .background_color(Color::Reset)
            .paint(|ctx| {
                // Draw selection highlight
                if let Some((sel_start, sel_end)) = viewport.selection {
                    ctx.draw(&Rectangle {
                        x: sel_start,
                        y: 0.0,
                        width: sel_end - sel_start,
                        height: num_lanes as f64,
                        color: Color::Rgb(60, 60, 80),
                    });
                    ctx.layer();
                }

                // Draw events
                for (idx, event) in prepared.events.iter().enumerate() {
                    let event_end = event.start + event.duration;
                    if event_end < view_start || event.start > view_start + view_width {
                        continue;
                    }

                    let lane_y = *lane_idx_map.get(&event.stream_id).unwrap_or(&0) as f64;
                    let rect_height = 0.85;

                    let color = match event.event_type {
                        TimelineEventType::Kernel => kernel_color(&event.name),
                        TimelineEventType::Memcpy => Color::Magenta,
                        TimelineEventType::Memset => Color::Yellow,
                        _ => Color::White,
                    };

                    let final_color = if viewport.hovered == Some(idx) {
                        Color::White
                    } else {
                        color
                    };

                    // Ensure minimum visible width
                    let min_data_width = view_width / (pa.width as f64 * 2.0);
                    let draw_width = event.duration.max(min_data_width);

                    ctx.draw(&Rectangle {
                        x: event.start,
                        y: lane_y,
                        width: draw_width,
                        height: rect_height,
                        color: final_color,
                    });
                }
                ctx.layer();

                // Draw lane separator lines
                for i in 1..num_lanes {
                    ctx.draw(&CanvasLine {
                        x1: view_start,
                        y1: i as f64,
                        x2: view_start + view_width,
                        y2: i as f64,
                        color: Color::DarkGray,
                    });
                }
            });

        f.render_widget(canvas, canvas_area);

        // Draw hover tooltip
        if let Some(hovered_idx) = viewport.hovered {
            if let Some(event) = prepared.events.get(hovered_idx) {
                let dur_text = if event.duration >= 1e6 {
                    format!("{:.1}ms", event.duration / 1e6)
                } else if event.duration >= 1e3 {
                    format!("{:.1}us", event.duration / 1e3)
                } else {
                    format!("{:.0}ns", event.duration)
                };
                let tooltip = crate::core::types::truncate_str(
                    &format!("{} | {}", event.name, dur_text),
                    50,
                );
                let tooltip_w = (tooltip.len() as u16 + 2).min(inner.width);
                let tooltip_area = Rect {
                    x: inner.x + inner.width.saturating_sub(tooltip_w + 2),
                    y: inner.y + 1,
                    width: tooltip_w,
                    height: 1,
                };
                f.render_widget(
                    Paragraph::new(tooltip)
                        .style(Style::default().fg(Color::White).bg(Color::DarkGray)),
                    tooltip_area,
                );
            }
        }

        // Draw selection duration indicator
        if let Some((start_ns, end_ns)) = viewport.selection {
            let dur_us = (end_ns - start_ns) / 1000.0;
            let sel_text = if dur_us >= 1000.0 {
                format!("Selected: {:.1}ms", dur_us / 1000.0)
            } else {
                format!("Selected: {:.1}us", dur_us)
            };
            let sel_w = (sel_text.len() as u16 + 2).min(inner.width);
            let sel_area = Rect {
                x: inner.x + 1,
                y: inner.y + inner.height.saturating_sub(2),
                width: sel_w,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(sel_text)
                    .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray)),
                sel_area,
            );
        }
    }
}
