use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
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

    let title_fg = crate::theme::parse_color(&app.theme.ui.title_fg).unwrap_or(Color::Cyan);
    let title = Paragraph::new("NVIS - Universal Profiler Viewer")
        .style(Style::default().fg(title_fg).bold())
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    let input_text = app.file_input.value();
    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Profiler File Path (Press Enter to load)")
            .border_style(if app.focus == Focus::FileInput {
                crate::theme::parse_color(&app.theme.ui.border_focus).map_or(Style::default(), |c| Style::default().fg(c))
            } else {
                Style::default()
            }),
    );
    f.render_widget(input, chunks[1]);

    if app.focus == Focus::FileInput {
        let cursor_pos = app.file_input.cursor();
        f.set_cursor_position((chunks[1].x + cursor_pos as u16 + 1, chunks[1].y + 1));
    }

    let error_fg = crate::theme::parse_color(&app.theme.ui.error_fg).unwrap_or(Color::Red);
    let hint_fg = crate::theme::parse_color(&app.theme.ui.hint_fg).unwrap_or(Color::Gray);
    let message = if let Some(ref err) = app.error_message {
        Paragraph::new(err.as_str())
            .style(Style::default().fg(error_fg))
            .alignment(Alignment::Center)
    } else {
        Paragraph::new("Enter the path to a profiler data file (nsys .sqlite, ncu .csv, torch .json)\nCtrl+A/E: start/end  Ctrl+U/K: delete line/till end  Ctrl+W: del word  Esc: quit")
            .style(Style::default().fg(hint_fg))
            .alignment(Alignment::Center)
    };
    f.render_widget(message, chunks[2]);
}

fn category_style(cat: &ViewCategory, theme: &crate::theme::Theme) -> Style {
    let color = match cat {
        ViewCategory::Timeline => crate::theme::parse_color(&theme.ui.title_fg),
        ViewCategory::Statistics => crate::theme::parse_color("Magenta"),
        ViewCategory::Metrics => crate::theme::parse_color("Green"),
        ViewCategory::Metadata => crate::theme::parse_color(&theme.ui.title_fg),
        ViewCategory::RawData => crate::theme::parse_color("White"),
    };
    color.map_or(Style::default(), |c| Style::default().fg(c))
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

    // Fullscreen mode: only render the focused panel
    if let Some(ref fs_focus) = app.fullscreen {
        match fs_focus {
            Focus::ViewList => draw_view_list(f, app, area),
            Focus::Chart => draw_chart(f, app, area),
            Focus::DataTable => draw_data_table(f, app, area),
            Focus::SqlInput => draw_sql_input(f, app, area),
            _ => {}
        }
        if app.help_bar_visible {
            draw_help_bar(f, area, app);
        }
        return;
    }

    let is_timeline = app.views
        .get(app.selected_view_index)
        .map_or(false, |v| v.category == ViewCategory::Timeline);
    let is_metadata = app.views
        .get(app.selected_view_index)
        .map_or(false, |v| v.category == ViewCategory::Metadata);

    let (left, _right, right_chunks) = layout_chunks(area, is_timeline, is_metadata, app.sql_mode);

    draw_view_list(f, app, left);

    if app.sql_mode {
        draw_sql_input(f, app, right_chunks[0]);
        draw_data_table(f, app, right_chunks[1]);
    } else if is_metadata {
        draw_data_table(f, app, right_chunks[0]);
    } else {
        draw_chart(f, app, right_chunks[0]);
        draw_data_table(f, app, right_chunks[1]);
    }

    if app.help_bar_visible {
        draw_help_bar(f, area, app);
    }
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
                let fg = crate::theme::parse_color(&app.theme.ui.header_fg);
                let modi = crate::theme::parse_modifier(&app.theme.ui.header_modifier);
                fg.map_or(Style::default(), |c| Style::default().fg(c).add_modifier(modi))
            } else {
                category_style(&view.category, &app.theme)
            };
            ListItem::new(label).style(style)
        })
        .collect();

    let border_focus = crate::theme::parse_color(&app.theme.ui.border_focus);
    let border_unfocus = crate::theme::parse_color(&app.theme.ui.border_unfocus);
    let focus_color = if app.focus == Focus::ViewList { border_focus } else { border_unfocus };
    let hl_bg = crate::theme::parse_color(&app.theme.ui.list_highlight_bg);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Views")
                .title_alignment(Alignment::Center)
                .border_style(focus_color.map_or(Style::default(), |c| Style::default().fg(c)))
        )
        .highlight_style(hl_bg.map_or(Style::default(), |c| Style::default().bg(c)));

    f.render_widget(list, area);
}

fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    if let (Some(ref viz), Some(ref renderer)) = (&app.prepared_viz, &app.active_renderer) {
        renderer.draw(f, area, viz, app.focus == Focus::Chart, &app.theme);
        return;
    }

    let border_focus = crate::theme::parse_color(&app.theme.ui.border_focus);
    let border_unfocus = crate::theme::parse_color(&app.theme.ui.border_unfocus);
    let focus_color = if app.focus == Focus::Chart { border_focus } else { border_unfocus };
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Visualization")
        .title_alignment(Alignment::Center)
        .border_style(focus_color.map_or(Style::default(), |c| Style::default().fg(c)));
    let paragraph = Paragraph::new("No data selected").block(block);
    f.render_widget(paragraph, area);
}

/// Compute per-column widths for the data table.
/// `original_data` is the data AFTER all transformations (null removal, grid/block merge)
/// but BEFORE hiding columns.
/// `hidden` is the set of hidden column names.
/// Returns (visible_col_names, col_widths, col_x_offsets) for the viewport.
pub fn compute_table_layout(
    data: &crate::core::types::ProfilerData,
    area_width: u16,
    hscroll: usize,
    hidden: &std::collections::HashSet<String>,
) -> (Vec<String>, Vec<usize>, Vec<usize>) {
    use ratatui::layout::{Constraint, Flex, Layout, Rect};

    let all_headers: Vec<String> = data.schema.iter().map(|s| s.name.clone()).collect();

    // Build the list of visible column names (preserving original order)
    let visible_names: Vec<String> = all_headers.iter()
        .filter(|h| !hidden.contains(h.as_str()))
        .cloned()
        .collect();
    let vis_total = visible_names.len();

    if vis_total == 0 {
        return (vec![], vec![], vec![]);
    }

    // Scan data to find max content width per original column (sample up to 200 rows)
    let sample_end = data.row_count.min(200);
    let mut natural: Vec<usize> = all_headers.iter().map(|h| h.chars().count()).collect();
    for col_idx in 0..data.columns.len().min(natural.len()) {
        let mut max_len = natural[col_idx];
        for row_idx in 0..sample_end {
            if let Some(val) = data.columns[col_idx].get(row_idx) {
                let len = match val {
                    crate::core::types::ColumnValue::Null => 4,
                    crate::core::types::ColumnValue::Integer(i) => {
                        let digits = if *i == 0 { 1 } else { (*i as f64).abs().log10().floor() as usize + 1 };
                        if *i < 0 { digits + 1 } else { digits }
                    }
                    crate::core::types::ColumnValue::Float(f) => {
                        format!("{:.2}", f).chars().count()
                    }
                    crate::core::types::ColumnValue::Text(s) => s.chars().count(),
                };
                if len > max_len { max_len = len; }
            }
        }
        natural[col_idx] = max_len;
    }

    // Compute effective width for each visible column:
    // max content width + absorb any hidden columns to its right (before next visible)
    let mut vis_widths: Vec<usize> = Vec::with_capacity(vis_total);
    for (vis_pos, vis_name) in visible_names.iter().enumerate() {
        let col_i = all_headers.iter().position(|h| h == vis_name).unwrap_or(0);
        let mut w = natural[col_i];
        let next_vis_col = if vis_pos + 1 < vis_total {
            all_headers.iter().position(|h| h == &visible_names[vis_pos + 1]).unwrap_or(all_headers.len())
        } else {
            all_headers.len()
        };
        for j in (col_i + 1)..next_vis_col {
            if hidden.contains(&all_headers[j]) {
                w += natural[j];
            }
        }
        vis_widths.push(w);
    }

    // Apply hscroll: skip visible columns from the left
    let start = hscroll.min(vis_total);
    let names = &visible_names[start..];
    let widths_slice = &vis_widths[start..];
    let vis_count = names.len();

    if vis_count == 0 {
        return (vec![], vec![], vec![]);
    }

    let usable = area_width.saturating_sub(2); // inside borders
    let total_needed: usize = widths_slice.iter().sum::<usize>() + (vis_count - 1); // + spacing
    let fits = total_needed <= usable as usize;

    let constraints: Vec<Constraint> = if fits {
        // Fits: use Min so columns expand to fill available space
        widths_slice.iter().map(|&w| Constraint::Min(w as u16)).collect()
    } else {
        // Overflows: use Length so columns stay at natural width (get clipped)
        widths_slice.iter().map(|&w| Constraint::Length(w as u16)).collect()
    };

    let columns_area = Rect::new(0, 0, usable, 1);
    let rects = Layout::horizontal(constraints)
        .flex(if fits { Flex::SpaceBetween } else { Flex::Start })
        .spacing(1)
        .split(columns_area);

    let widths: Vec<usize> = rects.iter().map(|r| r.width as usize).collect();
    let offsets: Vec<usize> = rects.iter().map(|r| r.x as usize).collect();

    let names = names.to_vec();
    (names, widths, offsets)
}

fn draw_data_table(f: &mut Frame, app: &App, area: Rect) {
    let view_name = app.views.get(app.selected_view_index).map(|v| v.display_name.as_str()).unwrap_or("Data");
    let title = if app.sql_mode && app.sql_result.is_some() { "SQL Result" } else { view_name };
    let border_focus = crate::theme::parse_color(&app.theme.ui.border_focus);
    let border_unfocus = crate::theme::parse_color(&app.theme.ui.border_unfocus);
    let focus_color = if app.focus == Focus::DataTable { border_focus } else { border_unfocus };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(Alignment::Center)
        .border_style(focus_color.map_or(Style::default(), |c| Style::default().fg(c)));

    if let Some(data) = app.display_data() {
        let available_height = area.height.saturating_sub(3) as usize;
        let start_row = app.table_scroll;
        let end_row = (start_row + available_height).min(data.row_count);

        let (vis_names, widths, _offsets) = compute_table_layout(data, area.width, app.table_hscroll, &app.hidden_col_names);
        let vis_count = widths.len();
        if vis_count == 0 {
            let paragraph = Paragraph::new("All columns hidden — press u to restore").block(block);
            f.render_widget(paragraph, area);
            return;
        }

        let usable = area.width.saturating_sub(2) as usize;
        let total_needed: usize = widths.iter().sum::<usize>() + vis_count.saturating_sub(1);
        let fits = total_needed <= usable;

        // Build header cells from vis_names
        let col_hl_name = app.col_highlight.as_deref();
        let col_hl_bg = crate::theme::parse_color(&app.theme.ui.col_highlight_bg).unwrap_or(Color::Rgb(0, 80, 80));
        let col_hl_fg = crate::theme::parse_color(&app.theme.ui.col_highlight_fg).unwrap_or(Color::White);
        let row_hl_bg = crate::theme::parse_color(&app.theme.ui.row_highlight_bg).unwrap_or(Color::DarkGray);
        let row_hl_fg = crate::theme::parse_color(&app.theme.ui.row_highlight_fg).unwrap_or(Color::White);
        let hdr_fg = crate::theme::parse_color(&app.theme.ui.header_fg).unwrap_or(Color::Yellow);
        let hdr_mod = crate::theme::parse_modifier(&app.theme.ui.header_modifier);

        let header_cells = vis_names.iter().enumerate().map(|(i, name)| {
            let w = widths[i];
            let content = if name.chars().count() > w { truncate_str(name, w) } else { name.clone() };
            let style = if col_hl_name == Some(name.as_str()) {
                Style::default().bg(col_hl_bg).fg(col_hl_fg).add_modifier(hdr_mod)
            } else {
                Style::default().fg(hdr_fg).add_modifier(hdr_mod)
            };
            Cell::from(content).style(style)
        });
        let header = Row::new(header_cells).height(1);

        let constraints: Vec<Constraint> = if fits {
            widths.iter().map(|&w| Constraint::Min(w as u16)).collect()
        } else {
            widths.iter().map(|&w| Constraint::Length(w as u16)).collect()
        };

        // Build data rows directly from columns — only visible rows
        let mut rows: Vec<Row> = Vec::new();
        for row_idx in start_row..end_row {
            let is_row_hl = app.row_highlight == Some(row_idx);
            let cells = vis_names.iter().enumerate().map(|(vis_i, name)| {
                let is_col_hl = col_hl_name == Some(name.as_str());
                let col_idx = data.schema.iter().position(|s| s.name == *name).unwrap_or(0);
                let val_str = match data.columns.get(col_idx).and_then(|c| c.get(row_idx)) {
                    Some(crate::core::types::ColumnValue::Null) => "NULL".to_string(),
                    Some(crate::core::types::ColumnValue::Integer(i)) => i.to_string(),
                    Some(crate::core::types::ColumnValue::Float(f)) => format!("{:.2}", f),
                    Some(crate::core::types::ColumnValue::Text(s)) => s.clone(),
                    None => "NULL".to_string(),
                };
                let w = widths[vis_i];
                let content = if val_str.chars().count() > w { truncate_str(&val_str, w) } else { val_str };
                let style = if is_row_hl {
                    Style::default().bg(row_hl_bg).fg(row_hl_fg)
                } else if is_col_hl {
                    Style::default().bg(col_hl_bg).fg(col_hl_fg)
                } else {
                    Style::default()
                };
                Cell::from(content).style(style)
            });
            rows.push(Row::new(cells).height(1));
        }

        let table = Table::new(rows, &constraints)
            .header(header)
            .block(block)
            .column_spacing(1)
            .flex(if fits { ratatui::layout::Flex::SpaceBetween } else { ratatui::layout::Flex::Start });

        f.render_widget(table, area);
    } else {
        let paragraph = Paragraph::new(if app.sql_mode { "Execute a SQL query above" } else { "Select a view to see data" }).block(block);
        f.render_widget(paragraph, area);
    }
}

fn draw_sql_input(f: &mut Frame, app: &App, area: Rect) {
    let input_text = app.sql_input.value();
    let sql_err_color = crate::theme::parse_color(&app.theme.ui.sql_error_border);
    let border_focus_color = crate::theme::parse_color(&app.theme.ui.border_focus);
    let border_color = if app.sql_error.is_some() {
        sql_err_color.unwrap_or(Color::Red)
    } else if app.focus == Focus::SqlInput {
        border_focus_color.unwrap_or(Color::Yellow)
    } else {
        crate::theme::parse_color(&app.theme.ui.border_unfocus).unwrap_or(Color::White)
    };
    let title = if let Some(ref err) = app.sql_error {
        format!("SQL Error: {}", truncate_str(err, 40))
    } else {
        "SQL (Enter to execute, Esc to close)".to_string()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(input_text).block(block);
    f.render_widget(paragraph, area);

    if app.focus == Focus::SqlInput {
        let cursor_pos = app.sql_input.cursor();
        f.set_cursor_position((area.x + cursor_pos as u16 + 1, area.y + 1));
    }
}

fn draw_stats_view(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    draw_chart(f, app, chunks[0]);

    let view_name = app.views.get(app.selected_view_index).map(|v| v.display_name.as_str()).unwrap_or("Statistics");
    let stats_border_color = if app.focus == Focus::StatsTable {
        crate::theme::parse_color(&app.theme.ui.border_focus)
    } else {
        crate::theme::parse_color(&app.theme.ui.border_unfocus)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(view_name)
        .title_alignment(Alignment::Center)
        .border_style(stats_border_color.map_or(Style::default(), |c| Style::default().fg(c)));

    if let Some(ref data) = app.stats_data {
        let available_height = chunks[1].height.saturating_sub(3) as usize;
        let start = app.stats_scroll;
        let end = (start + available_height).min(data.row_count);

        let col_count = data.schema.len().max(1);
        let usable = chunks[1].width.saturating_sub(2) as usize;

        // Compute column widths from visible data only
        let mut max_widths: Vec<usize> = data.schema.iter().map(|s| s.name.chars().count()).collect();
        for row_idx in start..end {
            for (col_idx, col) in data.columns.iter().enumerate() {
                if col_idx >= max_widths.len() { break; }
                if let Some(val) = col.get(row_idx) {
                    let len = match val {
                        crate::core::types::ColumnValue::Null => 4,
                        crate::core::types::ColumnValue::Integer(i) => i.to_string().chars().count(),
                        crate::core::types::ColumnValue::Float(f) => format!("{:.2}", f).chars().count(),
                        crate::core::types::ColumnValue::Text(s) => s.chars().count(),
                    };
                    max_widths[col_idx] = max_widths[col_idx].max(len);
                }
            }
        }
        let total_needed: usize = max_widths.iter().map(|w| w + 1).sum::<usize>().saturating_sub(1);
        let capped: Vec<u16> = if total_needed <= usable {
            max_widths.iter().map(|w| (*w as u16).max(4)).collect()
        } else {
            let base = (usable / col_count).max(4) as u16;
            let mut widths: Vec<u16> = max_widths.iter().map(|w| (*w as u16).max(4).min(base)).collect();
            let used: usize = widths.iter().map(|w| *w as usize + 1).sum::<usize>().saturating_sub(1);
            if used < usable {
                let extra = (usable - used) / col_count;
                for w in &mut widths { *w += extra as u16; }
            }
            widths
        };

        let hdr_fg = crate::theme::parse_color(&app.theme.ui.header_fg).unwrap_or(Color::Yellow);
        let hdr_mod = crate::theme::parse_modifier(&app.theme.ui.header_modifier);

        let header_cells = data.schema.iter().enumerate().map(|(i, s)| {
            let w = capped[i] as usize;
            Cell::from(if s.name.chars().count() > w { truncate_str(&s.name, w.saturating_sub(1)) } else { s.name.clone() })
        });
        let header = Row::new(header_cells)
            .style(Style::default().fg(hdr_fg).add_modifier(hdr_mod))
            .height(1);

        let rows: Vec<Row> = (start..end)
            .map(|row_idx| {
                let cells = data.columns.iter().enumerate().map(|(col_idx, col)| {
                    let w = capped.get(col_idx).copied().unwrap_or(4) as usize;
                    let val_str = match col.get(row_idx) {
                        Some(crate::core::types::ColumnValue::Null) => "NULL".to_string(),
                        Some(crate::core::types::ColumnValue::Integer(i)) => i.to_string(),
                        Some(crate::core::types::ColumnValue::Float(f)) => format!("{:.2}", f),
                        Some(crate::core::types::ColumnValue::Text(s)) => s.clone(),
                        None => "NULL".to_string(),
                    };
                    let content = if val_str.chars().count() > w { truncate_str(&val_str, w.saturating_sub(1)) } else { val_str };
                    Cell::from(content)
                });
                Row::new(cells).height(1)
            })
            .collect();

        let widths: Vec<Constraint> = capped.iter().map(|w| Constraint::Length(*w)).collect();

        let table = Table::new(rows, &widths)
            .header(header)
            .block(block)
            .column_spacing(1);

        f.render_widget(table, chunks[1]);
    } else {
        let paragraph = Paragraph::new("No statistics data available").block(block);
        f.render_widget(paragraph, chunks[1]);
    }

    if app.help_bar_visible {
        draw_help_bar(f, area, app);
    }
}

/// Compute the main layout chunks shared by draw and mouse handlers.
pub fn layout_chunks(area: Rect, _is_timeline: bool, is_metadata: bool, sql_mode: bool) -> (Rect, Rect, Vec<Rect>) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);

    let right_constraints = if sql_mode {
        vec![Constraint::Length(3), Constraint::Min(0)]
    } else if is_metadata {
        vec![Constraint::Percentage(100)]
    } else {
        vec![
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ]
    };

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(right_constraints)
        .split(main_chunks[1]);

    (main_chunks[0], main_chunks[1], right_chunks.to_vec())
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

fn draw_help_bar(f: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.sql_mode {
        " [q]Quit [↑↓]Scroll [Shift+←→]H-Scroll [Tab]Switch [f]Full [Enter]Exec [Esc]Close [/]SQL "
    } else {
        " [q/Esc]Quit [↑↓]Nav [←→]Pan [Shift+←→]H-Scroll [Tab]Switch [f]Full [/]SQL [d]HideCol [u]Unhide [s]Stats [b]Back [+/-]Zoom [r]Reset [Ctrl+T]Theme "
    };
    let bg = crate::theme::parse_color(&app.theme.ui.help_bar_bg).unwrap_or(Color::DarkGray);
    let fg = crate::theme::parse_color(&app.theme.ui.help_bar_fg).unwrap_or(Color::White);
    let help = Paragraph::new(help_text)
        .style(Style::default().bg(bg).fg(fg))
        .alignment(Alignment::Center);

    let help_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };

    f.render_widget(help, help_area);
}
