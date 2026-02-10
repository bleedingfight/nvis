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

    // 使用文本形式展示箱线图统计信息
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut y = inner.y + 1;
    let max_items = ((inner.height - 2) / 4) as usize;

    for boxplot in boxplots.iter().take(max_items) {
        if y + 3 >= inner.y + inner.height {
            break;
        }

        // 函数名
        let name_line = format!(
            "● {}",
            if boxplot.label.len() > 30 {
                format!("{}...", &boxplot.label[..27])
            } else {
                boxplot.label.clone()
            }
        );
        let name_paragraph =
            Paragraph::new(name_line).style(Style::default().fg(Color::Cyan).bold());
        f.render_widget(
            name_paragraph,
            Rect::new(inner.x + 1, y, inner.width - 2, 1),
        );
        y += 1;

        // 统计信息: count, mean
        let stats_line = format!(
            "  Count: {}  Mean: {:.2}μs  Median: {:.2}μs",
            boxplot.count, boxplot.mean, boxplot.median
        );
        let stats_paragraph = Paragraph::new(stats_line).style(Style::default().fg(Color::White));
        f.render_widget(
            stats_paragraph,
            Rect::new(inner.x + 1, y, inner.width - 2, 1),
        );
        y += 1;

        // 箱线图可视化
        let range = boxplot.max - boxplot.min;
        let width = (inner.width - 20) as f64;

        if range > 0.0 {
            let _min_pos = 0;
            let q1_pos = ((boxplot.q1 - boxplot.min) / range * width) as u16;
            let median_pos = ((boxplot.median - boxplot.min) / range * width) as u16;
            let q3_pos = ((boxplot.q3 - boxplot.min) / range * width) as u16;
            let max_pos = width as u16;

            // 绘制箱线图
            let mut chart_line = String::from("  ");
            chart_line.push_str(&format!("{:.1}", boxplot.min));
            chart_line.push_str(" ├");

            for i in 0..max_pos {
                if i == q1_pos || i == q3_pos {
                    chart_line.push('┤');
                } else if i == median_pos {
                    chart_line.push('┼');
                } else if i > q1_pos && i < q3_pos {
                    chart_line.push('█');
                } else {
                    chart_line.push('─');
                }
            }

            chart_line.push('┤');
            chart_line.push_str(&format!(" {:.1}", boxplot.max));

            let chart_paragraph =
                Paragraph::new(chart_line).style(Style::default().fg(Color::Green));
            f.render_widget(
                chart_paragraph,
                Rect::new(inner.x + 1, y, inner.width - 2, 1),
            );
        }

        y += 2;
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
        " [q]Quit [↑↓]Navigate [←→]Focus [Tab]Switch [Enter]Select [Mouse]Click/Scroll ";
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
