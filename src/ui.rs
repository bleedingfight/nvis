use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, AppState, Focus};
use crate::core::types::{ViewCategory, truncate_str};

pub fn draw(f: &mut Frame, app: &App) {
    match app.state {
        AppState::FileSelection => draw_file_selection(f, app),
        AppState::ViewBrowser => draw_view_browser(f, app),
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

    let title = Paragraph::new("NVIS - Universal Profiler Viewer")
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    let input_text = app.file_input.value();
    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Profiler File Path (Press Enter to load)")
            .border_style(if app.focus == Focus::FileInput {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            }),
    );
    f.render_widget(input, chunks[1]);

    if app.focus == Focus::FileInput {
        let cursor_pos = app.file_input.cursor();
        f.set_cursor_position((chunks[1].x + cursor_pos as u16 + 1, chunks[1].y + 1));
    }

    let message = if let Some(ref err) = app.error_message {
        Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
    } else {
        Paragraph::new("Enter the path to a profiler data file (nsys .sqlite, ncu .csv, torch .json)\nCtrl+A/E: start/end  Ctrl+U/K: delete line/till end  Ctrl+W: del word  Esc: quit")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
    };
    f.render_widget(message, chunks[2]);
}

fn category_style(cat: &ViewCategory) -> Style {
    match cat {
        ViewCategory::Timeline => Style::default().fg(Color::Cyan),
        ViewCategory::Statistics => Style::default().fg(Color::Magenta),
        ViewCategory::Metrics => Style::default().fg(Color::Green),
        ViewCategory::Metadata => Style::default().fg(Color::Yellow),
        ViewCategory::RawData => Style::default().fg(Color::White),
    }
}

fn category_label(cat: &ViewCategory) -> &str {
    match cat {
        ViewCategory::Timeline => "⏱",
        ViewCategory::Statistics => "📊",
        ViewCategory::Metrics => "📈",
        ViewCategory::Metadata => "ℹ",
        ViewCategory::RawData => "📋",
    }
}

fn draw_view_browser(f: &mut Frame, app: &App) {
    let area = f.area();

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);

    draw_view_list(f, app, main_chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[1]);

    draw_chart(f, app, right_chunks[0]);
    draw_data_table(f, app, right_chunks[1]);

    draw_help_bar(f, area);
}

fn draw_view_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .views
        .iter()
        .enumerate()
        .map(|(i, view)| {
            let label = format!(
                "{} {}",
                category_label(&view.category),
                view.display_name
            );
            let style = if i == app.selected_view_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                category_style(&view.category)
            };
            ListItem::new(label).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Views")
                .border_style(if app.focus == Focus::ViewList {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(list, area);
}

fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    if let (Some(ref viz), Some(ref renderer)) = (&app.prepared_viz, &app.active_renderer) {
        renderer.draw(f, area, viz, app.focus == Focus::Chart);
        return;
    }

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

fn draw_data_table(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Data")
        .border_style(if app.focus == Focus::DataTable {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if let Some(ref data) = app.view_data {
        let available_height = area.height.saturating_sub(3) as usize;
        let start_row = app.table_scroll;
        let end_row = (start_row + available_height).min(data.row_count);

        let (headers, rows_str) = data.to_row_strings();

        let header_cells = headers.iter().map(|c| Cell::from(c.as_str()));
        let header = Row::new(header_cells)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .height(1);

        let rows: Vec<Row> = rows_str[start_row..end_row]
            .iter()
            .map(|row| {
                let cells = row.iter().map(|c| {
                    let content = if c.chars().count() > 20 {
                        truncate_str(c, 17)
                    } else {
                        c.clone()
                    };
                    Cell::from(content)
                });
                Row::new(cells).height(1)
            })
            .collect();

        let col_count = headers.len().max(1);
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
        let paragraph = Paragraph::new("Select a view to see data").block(block);
        f.render_widget(paragraph, area);
    }
}

fn draw_stats_view(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Top: chart visualization
    draw_chart(f, app, chunks[0]);

    // Bottom: stats table
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Aggregated Statistics")
        .border_style(if app.focus == Focus::StatsTable {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if let Some(ref data) = app.stats_data {
        let available_height = chunks[1].height.saturating_sub(3) as usize;
        let start = app.stats_scroll;
        let end = (start + available_height).min(data.row_count);

        let (headers, rows_str) = data.to_row_strings();

        let header_cells = headers.iter().map(|c| Cell::from(c.as_str()));
        let header = Row::new(header_cells)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .height(1);

        let rows: Vec<Row> = rows_str[start..end]
            .iter()
            .map(|row| {
                let cells = row.iter().map(|c| {
                    let content = if c.chars().count() > 28 {
                        truncate_str(c, 25)
                    } else {
                        c.clone()
                    };
                    Cell::from(content)
                });
                Row::new(cells).height(1)
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
    } else {
        let paragraph = Paragraph::new("No statistics data available").block(block);
        f.render_widget(paragraph, chunks[1]);
    }

    draw_help_bar(f, area);
}

pub fn compute_chart_area(area: Rect) -> Rect {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[1]);

    right_chunks[0]
}

fn draw_help_bar(f: &mut Frame, area: Rect) {
    let help_text =
        " [q/Esc]Quit [↑↓]Nav [←→/h/l]Pan [Tab]Switch [Enter]Select [s]Stats [b]Back [+/-]Zoom [r]Reset [Drag]Select ";
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
