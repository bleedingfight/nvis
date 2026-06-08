use anyhow::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use ratatui::text::{Line, Span};
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
pub const MIN_LABEL_COLS: u16 = 12;

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
        .or_else(|| lowered.iter().position(|c| c == "kernel"))
        .or_else(|| {
            lowered
                .iter()
                .position(|c| c.contains("name") && !c.contains("id"))
        });

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

        let duration = if let Some(ei) = end_col {
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

        let lane_idx_map: HashMap<i64, usize> = prepared
            .stream_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        // Draw lane labels
        for (i, stream_id) in prepared.stream_ids.iter().enumerate() {
            let label = format!(" S{:>5}", stream_id);
            let y = inner.y + TIME_AXIS_ROWS + (i as u16) * LANE_HEIGHT_ROWS;
            f.render_widget(
                Paragraph::new(label).style(Style::default().fg(Color::DarkGray)),
                Rect::new(inner.x, y, LANE_LABEL_COLS, LANE_HEIGHT_ROWS),
            );
        }

        // Draw time axis (relative to view start)
        let num_ticks = (pa.width / 16).max(1) as usize;
        for i in 0..=num_ticks {
            let offset_ns =
                viewport.view_width_ns * (i as f64 / num_ticks as f64);
            let col = ns_to_col(offset_ns, 0.0, viewport.view_width_ns, pa.width);
            if col >= 0.0 && (col as u16) < pa.width {
                let x = pa.x + col as u16;
                let label = format_duration(offset_ns);
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

        // Build per-lane row data using Paragraph/Spans
        let view_start = viewport.view_start_ns;
        let view_width = viewport.view_width_ns;
        let plot_w = pa.width as usize;

        // Lane background colors for visual distinction
        let lane_bg_colors: Vec<Color> = (0..num_lanes)
            .map(|i| {
                if i % 2 == 0 {
                    Color::Rgb(30, 30, 42)
                } else {
                    Color::Rgb(38, 38, 50)
                }
            })
            .collect();

        // Pre-compute visible events and their column positions
        struct EventDraw {
            start_col: usize,
            width_cols: usize,
            color: Color,
            label: Option<String>,
            is_hovered: bool,
            is_selected: bool,
        }

        impl Clone for EventDraw {
            fn clone(&self) -> Self {
                Self {
                    start_col: self.start_col,
                    width_cols: self.width_cols,
                    color: self.color,
                    label: self.label.clone(),
                    is_hovered: self.is_hovered,
                    is_selected: self.is_selected,
                }
            }
        }

        let mut lane_events: Vec<Vec<EventDraw>> = vec![Vec::new(); num_lanes];

        for (idx, event) in prepared.events.iter().enumerate() {
            let event_end = event.start + event.duration;
            if event_end < view_start || event.start > view_start + view_width {
                continue;
            }

            let lane_y = *lane_idx_map.get(&event.stream_id).unwrap_or(&0);

            let start_col_f = ns_to_col(event.start, view_start, view_width, pa.width);
            let end_col_f = ns_to_col(event_end, view_start, view_width, pa.width);

            // Minimum 1 column width so narrow events are always visible
            let width_cols = ((end_col_f - start_col_f).ceil() as usize).max(1);
            let start_col = start_col_f.max(0.0) as usize;

            if start_col >= plot_w {
                continue;
            }

            let clipped_width = width_cols.min(plot_w - start_col);

            let color = match event.event_type {
                TimelineEventType::Kernel => kernel_color(&event.name),
                TimelineEventType::Memcpy => Color::Magenta,
                TimelineEventType::Memset => Color::Yellow,
                _ => Color::White,
            };

            let is_hovered = viewport.hovered == Some(idx);
            let is_selected = viewport.selection.map_or(false, |(s, e)| {
                event.start < e && event_end > s
            });

            // Label for events wide enough to fit text
            let label = if clipped_width >= MIN_LABEL_COLS as usize {
                let dur_text = format_duration(event.duration);
                let text = format!("{} {}", event.name, dur_text);
                Some(crate::core::types::truncate_str(&text, clipped_width - 2))
            } else {
                None
            };

            lane_events[lane_y].push(EventDraw {
                start_col,
                width_cols: clipped_width,
                color: if is_hovered { Color::White } else { color },
                label,
                is_hovered,
                is_selected,
            });
        }

        // Build spans for each lane row
        for (lane_idx, events) in lane_events.iter().enumerate() {
            let y_top = pa.y + (lane_idx as u16) * LANE_HEIGHT_ROWS;
            let y_bot = y_top + 1;

            // Build a char buffer and style buffer for the row
            // We'll use Line with Spans
            let mut spans_top: Vec<Span> = Vec::new();
            let mut spans_bot: Vec<Span> = Vec::new();

            // Background: fill with spaces in dark gray
            let mut col = 0usize;

            // Selection background (if any) covers the entire lane
            let sel_cols: Option<(usize, usize)> = viewport.selection.map(|(s, e)| {
                let sc = ns_to_col(s, view_start, view_width, pa.width).max(0.0) as usize;
                let ec = (ns_to_col(e, view_start, view_width, pa.width).ceil() as usize).min(plot_w);
                (sc, ec)
            });

            // Sort events by start_col for sequential span building
            let mut sorted_events: Vec<&EventDraw> = events.iter().collect();
            sorted_events.sort_by_key(|e| e.start_col);

            for event in &sorted_events {
                // Fill gap before this event
                if event.start_col > col {
                    let gap_len = event.start_col - col;
                    let lane_bg = lane_bg_colors[lane_idx];
                    let gap_bg = if let Some((sc, ec)) = sel_cols {
                        if col < ec && event.start_col > sc {
                            Color::Rgb(60, 60, 80)
                        } else {
                            lane_bg
                        }
                    } else {
                        lane_bg
                    };
                    spans_top.push(Span::styled(" ".repeat(gap_len), Style::default().bg(gap_bg)));
                    spans_bot.push(Span::styled(" ".repeat(gap_len), Style::default().bg(gap_bg)));
                    col = event.start_col; // advances col past the gap; overwritten after event draw
                }

                // Draw event block
                let event_style = Style::default().fg(event.color);
                let event_style_bg = if event.is_selected {
                    Style::default().fg(event.color).bg(Color::Rgb(60, 60, 80))
                } else {
                    event_style
                };

                if let Some(ref label) = event.label {
                    // Wide enough to show text: render as colored text on colored bg
                    let text_style = if event.is_hovered {
                        Style::default().fg(Color::Black).bg(Color::White)
                    } else if event.is_selected {
                        Style::default().fg(Color::White).bg(Color::Rgb(60, 60, 80))
                    } else {
                        Style::default().fg(Color::Black).bg(event.color)
                    };
                    let padded = format!(" {} ", label);
                    let text = if padded.len() > event.width_cols {
                        crate::core::types::truncate_str(&padded, event.width_cols)
                    } else {
                        let mut s = padded;
                        // Pad to fill the block width
                        while s.len() < event.width_cols {
                            s.push(' ');
                        }
                        s
                    };
                    spans_top.push(Span::styled(text.clone(), text_style));
                    // Bottom row: all filled blocks
                    spans_bot.push(Span::styled(
                        "█".repeat(event.width_cols),
                        event_style_bg,
                    ));
                } else {
                    // Narrow event: two rows of filled blocks
                    spans_top.push(Span::styled("█".repeat(event.width_cols), event_style_bg));
                    spans_bot.push(Span::styled("█".repeat(event.width_cols), event_style_bg));
                }

                col = event.start_col + event.width_cols;
            }

            // Fill remaining gap after last event
            if col < plot_w {
                let gap_len = plot_w - col;
                let lane_bg = lane_bg_colors[lane_idx];
                spans_top.push(Span::styled(" ".repeat(gap_len), Style::default().bg(lane_bg)));
                spans_bot.push(Span::styled(" ".repeat(gap_len), Style::default().bg(lane_bg)));
            }

            // Render the two rows for this lane
            f.render_widget(
                Paragraph::new(Line::from(spans_top)),
                Rect::new(pa.x, y_top, pa.width, 1),
            );
            f.render_widget(
                Paragraph::new(Line::from(spans_bot)),
                Rect::new(pa.x, y_bot, pa.width, 1),
            );

            // Draw lane separator line
            let sep_y = y_top + LANE_HEIGHT_ROWS;
            if sep_y < pa.y + pa.height && lane_idx + 1 < num_lanes {
                let sep_line: Line = vec![Span::styled(
                    "─".repeat(plot_w),
                    Style::default().fg(Color::DarkGray),
                )]
                .into();
                f.render_widget(
                    Paragraph::new(sep_line),
                    Rect::new(pa.x, sep_y, pa.width, 1),
                );
            }
        }

        // Draw hover tooltip near the hovered event
        if let Some(hovered_idx) = viewport.hovered {
            if let Some(event) = prepared.events.get(hovered_idx) {
                let dur_text = format_duration(event.duration);
                let tooltip_text = crate::core::types::truncate_str(
                    &format!("{} | {}", event.name, dur_text),
                    50,
                );
                let tooltip_w = (tooltip_text.len() as u16 + 2).min(inner.width);
                let lane = *lane_idx_map.get(&event.stream_id).unwrap_or(&0);
                let event_col = ns_to_col(event.start, view_start, view_width, pa.width) as u16;
                // Position tooltip to the right of the event start, on the row above the lane
                let tooltip_x = (pa.x + event_col + 2).min(inner.x + inner.width.saturating_sub(tooltip_w + 2));
                let tooltip_y = if lane == 0 { inner.y + TIME_AXIS_ROWS } else { pa.y + (lane as u16) * LANE_HEIGHT_ROWS - 1 };
                let tooltip_area = Rect {
                    x: tooltip_x,
                    y: tooltip_y,
                    width: tooltip_w,
                    height: 1,
                };
                f.render_widget(
                    Paragraph::new(format!(" {} ", tooltip_text))
                        .style(Style::default().fg(Color::White).bg(Color::DarkGray)),
                    tooltip_area,
                );
            }
        }

        // Draw bottom info bar
        {
            let mut info_spans: Vec<Span> = Vec::new();

            // Use time-range overlap (exact, no column rounding)
            let mut overlapping: Vec<usize> = Vec::new();
            if let Some((sel_start, sel_end)) = viewport.selection {
                for (i, e) in prepared.events.iter().enumerate() {
                    if e.start < sel_end && e.start + e.duration > sel_start {
                        overlapping.push(i);
                    }
                }
            }

            // Sort: hovered first, then shortest duration first
            overlapping.sort_by(|a, b| {
                let a_h = viewport.hovered == Some(*a);
                let b_h = viewport.hovered == Some(*b);
                b_h.cmp(&a_h).then_with(|| {
                    prepared.events[*a].duration.partial_cmp(&prepared.events[*b].duration).unwrap_or(std::cmp::Ordering::Equal)
                })
            });

            let max_shown = 5;
            let mut shown = 0usize;
            let mut overflow = 0usize;

            for idx in overlapping {
                if shown >= max_shown {
                    overflow += 1;
                    continue;
                }
                if shown > 0 {
                    info_spans.push(Span::styled(" ", Style::default().fg(Color::Yellow).bg(Color::DarkGray)));
                }
                shown += 1;
                let e = &prepared.events[idx];
                let color = match e.event_type {
                    TimelineEventType::Kernel => kernel_color(&e.name),
                    TimelineEventType::Memcpy => Color::Magenta,
                    TimelineEventType::Memset => Color::Yellow,
                    _ => Color::White,
                };
                let name = crate::core::types::truncate_str(&e.name, 15);
                let dur = format_duration(e.duration);
                info_spans.push(Span::styled(
                    format!(" {} {} ", name, dur),
                    Style::default().fg(Color::Black).bg(color),
                ));
            }

            if overflow > 0 {
                info_spans.push(Span::styled(
                    format!(" +{}more ", overflow),
                    Style::default().fg(Color::Yellow).bg(Color::DarkGray),
                ));
            }

            if shown == 0 {
                if let Some(hi) = viewport.hovered {
                    if let Some(e) = prepared.events.get(hi) {
                        let color = match e.event_type {
                            TimelineEventType::Kernel => kernel_color(&e.name),
                            TimelineEventType::Memcpy => Color::Magenta,
                            TimelineEventType::Memset => Color::Yellow,
                            _ => Color::White,
                        };
                        let name = crate::core::types::truncate_str(&e.name, 30);
                        let dur = format_duration(e.duration);
                        info_spans.push(Span::styled(
                            format!(" {} {} ", name, dur),
                            Style::default().fg(Color::Black).bg(color),
                        ));
                    }
                } else if let Some((s, e)) = viewport.selection {
                    let us = (e - s) / 1000.0;
                    let txt = if us >= 1000.0 { format!("Selected: {:.1}ms", us / 1000.0) } else { format!("Selected: {:.1}us", us) };
                    info_spans.push(Span::styled(txt, Style::default().fg(Color::Yellow).bg(Color::DarkGray)));
                }
            }

            if !info_spans.is_empty() {
                let info_w = inner.width.saturating_sub(2);
                let info_area = Rect {
                    x: inner.x + 1,
                    y: inner.y + inner.height.saturating_sub(2),
                    width: info_w,
                    height: 1,
                };
                f.render_widget(Paragraph::new(Line::from(info_spans)), info_area);
            }
        }
    }
}

fn format_duration(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.1}ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.1}us", ns / 1e3)
    } else {
        format!("{:.0}ns", ns)
    }
}
