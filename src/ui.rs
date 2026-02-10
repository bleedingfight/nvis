use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{BarChart, Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, AppState, Focus};

pub fn draw(f: &mut Frame, app: &App) {
    match app.state {
        AppState::FileSelection => draw_file_selection(f, app),
        AppState::TableView => draw_table_view(f, app),
        AppState::StatsView => draw_stats_view(f, app),
    }
}

fn draw_file_selection(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("NSYS SQLite Database Viewer")
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // File input
    let input_text = app.file_input.value();
    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Database File Path (Press Enter to load)")
            .border_style(if app.focus == Focus::FileInput {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            }),
    );
    f.render_widget(input, chunks[1]);

    // Set cursor position
    if app.focus == Focus::FileInput {
        let cursor_pos = app.file_input.cursor();
        f.set_cursor_position((chunks[1].x + cursor_pos as u16 + 1, chunks[1].y + 1));
    }

    // Error message or help
    let message = if let Some(ref err) = app.error_message {
        Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
    } else {
        Paragraph::new("Enter the path to an NSYS SQLite database file\nPress 'q' or 'Esc' to quit")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
    };
    f.render_widget(message, chunks[2]);
}

fn draw_table_view(f: &mut Frame, app: &App) {
    let area = f.area();

    // Main layout: left (tables list) and right (data view)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);

    // Left side: table list
    draw_table_list(f, app, main_chunks[0]);

    // Right side: split into chart (top) and data table (bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[1]);

    draw_chart(f, app, right_chunks[0]);
    draw_data_table(f, app, right_chunks[1]);

    // Draw help bar at the bottom
    draw_help_bar(f, area);
}

fn draw_table_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .tables
        .iter()
        .enumerate()
        .map(|(i, table)| {
            let style = if i == app.selected_table_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(table.as_str()).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Tables")
                .border_style(if app.focus == Focus::TableList {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(list, area);
}

fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    if let Some(ref data) = app.table_data {
        if let Some(ref table_name) = app.selected_table {
            let viz =
                crate::visualization::generate_visualization(table_name, data, app.chart_scroll);

            match viz.data {
                crate::visualization::ChartData::BoxPlots(ref boxplots) => {
                    draw_boxplot_chart(f, area, &viz.title, boxplots, app.focus == Focus::Chart);
                }
                crate::visualization::ChartData::Timeline(ref events) => {
                    draw_timeline_chart(f, area, &viz.title, events, app.focus == Focus::Chart);
                }
                crate::visualization::ChartData::Stats(ref text) => {
                    draw_statistics_text(f, area, &viz.title, text, app.focus == Focus::Chart);
                }
                crate::visualization::ChartData::Bars(ref bars) => {
                    draw_bar_chart(f, area, &viz.title, bars, app.focus == Focus::Chart);
                }
            }
            return;
        }
    }

    // Fallback
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Visualization")
        .border_style(if app.focus == Focus::Chart {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let paragraph = Paragraph::new("No data selected").block(block);
    f.render_widget(paragraph, area);
}

fn draw_bar_chart(f: &mut Frame, area: Rect, title: &str, data: &[(&str, u64)], focused: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if data.is_empty() {
        let paragraph = Paragraph::new("No numeric data available").block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let barchart = BarChart::default()
        .block(block)
        .bar_width(5)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::Green))
        .value_style(Style::default().fg(Color::White).bg(Color::Green))
        .data(data);

    f.render_widget(barchart, area);
}

fn draw_boxplot_chart(
    f: &mut Frame,
    area: Rect,
    title: &str,
    boxplots: &[crate::visualization::BoxPlotData],
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

    // 绘制带坐标轴的箱线图：横轴为函数名（等距分布），纵轴为耗时(ns)
    let inner = block.inner(area);
    f.render_widget(block, area);

    // 布局：左侧给 y 轴与刻度，底部给 x 轴与标签
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

    // 限制函数数量，确保每个函数至少有 10 列宽度
    let max_items = (plot_w / 10).max(1) as usize;
    let items = boxplots.iter().take(max_items).collect::<Vec<_>>();
    if items.is_empty() {
        return;
    }

    // 全局 y 范围
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

    // Y 轴与刻度
    for yy in plot_top..=plot_bottom {
        f.render_widget(Paragraph::new("│"), Rect::new(plot_left, yy, 1, 1));
    }
    let tick_n = 4u16; // 5 档
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
    // Y 轴单位
    f.render_widget(
        Paragraph::new("耗时 (ns)").style(Style::default().fg(Color::Gray)),
        Rect::new(
            inner.x + 1,
            plot_top.saturating_sub(1),
            y_axis_w.saturating_sub(2),
            1,
        ),
    );

    // X 轴
    f.render_widget(
        Paragraph::new("─".repeat(plot_w as usize)),
        Rect::new(plot_left, plot_bottom, plot_w, 1),
    );

    // 为每个函数绘制箱线图
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

        // whiskers
        let top_line = "─".repeat(box_w as usize);
        f.render_widget(
            Paragraph::new(top_line.clone()).style(Style::default().fg(Color::Green)),
            Rect::new(left, max_y, box_w, 1),
        );
        f.render_widget(
            Paragraph::new(top_line).style(Style::default().fg(Color::Green)),
            Rect::new(left, min_y, box_w, 1),
        );
        // stems
        for yy in (top_y + 1)..max_y {
            f.render_widget(Paragraph::new("│"), Rect::new(cx, yy, 1, 1));
        }
        for yy in (min_y + 1)..bottom_y {
            f.render_widget(Paragraph::new("│"), Rect::new(cx, yy, 1, 1));
        }
        // box
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
        // median
        let med_len = (box_w - 2).max(1);
        f.render_widget(
            Paragraph::new("─".repeat(med_len as usize)).style(Style::default().fg(Color::Yellow)),
            Rect::new(left + 1, med_y, med_len, 1),
        );

        // X 轴刻度与函数名标签
        f.render_widget(Paragraph::new("┬"), Rect::new(cx, plot_bottom, 1, 1));
        let label = if b.label.len() > (slot_w as usize - 2) {
            let keep = (slot_w as usize - 5).max(3);
            format!("{}...", &b.label[..keep])
        } else {
            b.label.clone()
        };
        let lbl = Paragraph::new(label).alignment(Alignment::Center);
        f.render_widget(lbl, Rect::new(slot_x, plot_bottom + 1, slot_w, 1));
    }
}

fn draw_timeline_chart(
    f: &mut Frame,
    area: Rect,
    title: &str,
    events: &[crate::visualization::TimelineEvent],
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

    if events.is_empty() {
        let paragraph = Paragraph::new("No timeline data available").block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 找到时间范围
    let min_start = events.iter().map(|e| e.start).fold(f64::INFINITY, f64::min);
    let max_end = events
        .iter()
        .map(|e| e.start + e.duration)
        .fold(f64::NEG_INFINITY, f64::max);
    let time_range = max_end - min_start;

    if time_range <= 0.0 {
        return;
    }

    let mut y = inner.y + 1;
    let width = (inner.width - 25) as f64;

    for event in events.iter().take((inner.height - 2) as usize) {
        if y >= inner.y + inner.height - 1 {
            break;
        }

        // 事件名称
        let name = if event.name.len() > 18 {
            format!("{:.15}...", event.name)
        } else {
            format!("{:<18}", event.name)
        };

        // 计算位置
        let start_pos = ((event.start - min_start) / time_range * width) as u16;
        let duration_width = ((event.duration / time_range * width).max(1.0)) as u16;

        // 绘制时间线
        let mut line = String::new();
        line.push_str(&name);
        line.push_str(" │");

        for i in 0..width as u16 {
            if i >= start_pos && i < start_pos + duration_width {
                line.push('█');
            } else {
                line.push(' ');
            }
        }

        line.push_str(&format!("│ {:.1}μs", event.duration));

        let paragraph = Paragraph::new(line).style(Style::default().fg(Color::Cyan));
        f.render_widget(paragraph, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;
    }
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

fn draw_data_table(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Data")
        .border_style(if app.focus == Focus::DataTable {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if let Some(ref data) = app.table_data {
        // Calculate visible rows
        let available_height = area.height.saturating_sub(3) as usize; // minus borders and header
        let start_row = app.table_scroll;
        let end_row = (start_row + available_height).min(data.rows.len());

        // Create header
        let header_cells = data.columns.iter().map(|c| Cell::from(c.as_str()));
        let header = Row::new(header_cells)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .height(1);

        // Create rows
        let rows: Vec<Row> = data.rows[start_row..end_row]
            .iter()
            .map(|row| {
                let cells = row.iter().map(|c| {
                    let content = if c.len() > 20 {
                        format!("{}...", &c[..17])
                    } else {
                        c.clone()
                    };
                    Cell::from(content)
                });
                Row::new(cells).height(1)
            })
            .collect();

        // Calculate column widths
        let col_count = data.columns.len().max(1);
        let col_width = 100 / col_count as u16;
        let widths: Vec<Constraint> = (0..col_count)
            .map(|_| Constraint::Length(col_width))
            .collect();

        let table = Table::new(rows, &widths)
            .header(header)
            .block(block)
            .column_spacing(1);

        f.render_widget(table, area);
    } else {
        let paragraph = Paragraph::new("Select a table to view data").block(block);
        f.render_widget(paragraph, area);
    }
}

fn draw_help_bar(f: &mut Frame, area: Rect) {
    let help_text =
        " [q]Quit [↑↓]Navigate [←→]Focus [Tab]Switch [Enter]Select [s]Stats [b]Back [Mouse]Click/Scroll ";
    let help = Paragraph::new(help_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .alignment(Alignment::Center);

    let help_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };

    f.render_widget(help, help_area);
}

fn draw_stats_view(f: &mut Frame, app: &App) {
    let area = f.area();

    // Vertical split: top chart (box plot), bottom stats table
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Top: use runtime data to generate a boxplot chart across CUDA API functions
    if let Some(ref rt) = app.stats_runtime_data {
        let viz = crate::visualization::Visualization {
            _viz_type: crate::visualization::VisualizationType::BoxPlot,
            data: crate::visualization::ChartData::BoxPlots(
                crate::visualization::generate_boxplot_data(rt),
            ),
            title: "CUDA API 调用耗时箱线图 (ns)".to_string(),
        };

        match viz.data {
            crate::visualization::ChartData::BoxPlots(ref boxplots) => {
                draw_boxplot_chart(
                    f,
                    chunks[0],
                    &viz.title,
                    boxplots,
                    app.focus == Focus::Chart,
                );
            }
            _ => {}
        }
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("CUDA API 调用耗时箱线图 (ns)")
            .border_style(if app.focus == Focus::Chart {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        let paragraph = Paragraph::new("No runtime data").block(block);
        f.render_widget(paragraph, chunks[0]);
    }

    // Bottom: Stats aggregates table
    let block = Block::default()
        .borders(Borders::ALL)
        .title("CUDA API 聚合统计（单位：ns）")
        .border_style(if app.focus == Focus::StatsTable {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let header = Row::new([
        Cell::from("Function"),
        Cell::from("Calls"),
        Cell::from("Total"),
        Cell::from("Mean"),
        Cell::from("P50"),
        Cell::from("P95"),
        Cell::from("P99"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let start = app.stats_scroll;
    let end = (start + 100).min(app.stats_rows.len());
    let rows: Vec<Row> = app.stats_rows[start..end]
        .iter()
        .map(|r| {
            Row::new([
                Cell::from(if r.name.len() > 28 {
                    format!("{}...", &r.name[..25])
                } else {
                    r.name.clone()
                }),
                Cell::from(r.calls.to_string()),
                Cell::from(format!("{:.2}", r.total_us * 1000.0)),
                Cell::from(format!("{:.2}", r.mean_us * 1000.0)),
                Cell::from(format!("{:.2}", r.p50_us * 1000.0)),
                Cell::from(format!("{:.2}", r.p95_us * 1000.0)),
                Cell::from(format!("{:.2}", r.p99_us * 1000.0)),
            ])
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(34),
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, &widths)
        .header(header)
        .block(block)
        .column_spacing(1);
    f.render_widget(table, chunks[1]);

    draw_help_bar(f, area);
}
