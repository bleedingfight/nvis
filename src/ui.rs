use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, AppState, CloudDialogType, Focus};
use crate::core::types::{ViewCategory, truncate_str};
use crate::source::browser::{format_size, BrowserNode};

pub fn draw(f: &mut Frame, app: &App) {
    match app.state {
        AppState::FileSelection => draw_file_selection(f, app),
        AppState::ViewBrowser => draw_view_browser(f, app),
        AppState::StatsView => draw_stats_view(f, app),
        AppState::Summary => draw_summary(f, app),
    }
}

fn draw_file_selection(f: &mut Frame, app: &App) {
    let area = f.area();

    // ── Bottom: optional download bar + help bar ──
    let dl_active = app.download_state.is_some();
    let bottom = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if dl_active {
            vec![Constraint::Min(0), Constraint::Length(2), Constraint::Length(1)]
        } else {
            vec![Constraint::Min(0), Constraint::Length(1)]
        })
        .split(area);
    let main_area = bottom[0];

    // ── Vertical split: browser tree (top) + URI input (bottom, 3 rows) ──
    let panels = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(main_area);

    draw_browser_tree(f, app, panels[0]);
    draw_uri_input(f, app, panels[1]);

    if app.cloud_dialog_active {
        draw_cloud_dialog(f, app, main_area);
    }

    if dl_active {
        draw_download_bar(f, app, bottom[1]);
    }

    if app.help_bar_visible {
        draw_help_bar(f, bottom[bottom.len() - 1], app);
    }
}

/// Render a download progress bar (label, bytes/total, percent, elapsed, rate).
fn draw_download_bar(f: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::Gauge;
    let Some(d) = app.download_state.as_ref() else { return };
    let done = app.downloaded_bytes;
    let elapsed = d.started.elapsed().as_secs_f64();
    let rate = if elapsed > 0.0 { done as f64 / elapsed } else { 0.0 };
    let (pct, ratio) = match d.total {
        Some(t) if t > 0 => {
            let p = (done as f64 / t as f64).clamp(0.0, 1.0);
            (p * 100.0, p)
        }
        _ => (0.0, 0.0),
    };

    let title = match d.total {
        Some(t) => format!(
            " ⬇ {}  {}/{}  {:.0}%  {:.0}s  {}/s ",
            d.label,
            format_size(done),
            format_size(t),
            pct,
            elapsed,
            format_size(rate as u64),
        ),
        None => format!(
            " ⬇ {}  {}  {:.0}s  {}/s ",
            d.label,
            format_size(done),
            elapsed,
            format_size(rate as u64),
        ),
    };

    let gauge = Gauge::default()
        .block(ratatui::widgets::Block::default().title(title))
        .ratio(ratio)
        .gauge_style(ratatui::style::Style::default().fg(Color::Cyan))
        .label(format!("{:.0}%", pct));
    f.render_widget(gauge, area);
}

fn draw_browser_tree(f: &mut Frame, app: &App, area: Rect) {
    let border_focus = crate::theme::parse_color(&app.theme.ui.border_focus);
    let border_unfocus = crate::theme::parse_color(&app.theme.ui.border_unfocus);
    let hl_bg = crate::theme::parse_color(&app.theme.browser.highlight_bg)
        .or_else(|| crate::theme::parse_color(&app.theme.ui.row_highlight_bg));
    let hl_fg = crate::theme::parse_color(&app.theme.browser.highlight_fg);
    let cat_fg = crate::theme::parse_color(&app.theme.browser.category_fg);
    let dir_fg = crate::theme::parse_color(&app.theme.browser.dir_fg);
    let file_fg = crate::theme::parse_color(&app.theme.browser.file_fg);

    let border_style = if app.focus == Focus::BrowserTree {
        border_focus.map_or(Style::default(), |c| Style::default().fg(c))
    } else {
        border_unfocus.map_or(Style::default(), |c| Style::default().fg(c))
    };

    let items: Vec<ListItem> = app
        .browser_nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let indent = "  ".repeat(node.depth());
            let (expander, icon, label, size_str) = match node {
                BrowserNode::Category { .. } => {
                    let exp = if node.is_expandable() {
                        if app.browser_expanded.contains(&node.id()) { "v " } else { "> " }
                    } else { "  " };
                    (exp, " ", node.display_label().to_string(), String::new())
                }
                BrowserNode::LocalDir { .. } | BrowserNode::LocalParent { .. } => {
                    let exp = if app.browser_expanded.contains(&node.id()) { "v " } else { "> " };
                    (exp, " ", node.display_label().to_string(), String::new())
                }
                BrowserNode::Bucket { .. } | BrowserNode::Prefix { .. } | BrowserNode::WebDavDir { .. } | BrowserNode::SshDir { .. } | BrowserNode::CloudConn { .. } => {
                    let exp = if app.browser_expanded.contains(&node.id()) { "v " } else { "> " };
                    (exp, " ", node.display_label().to_string(), String::new())
                }
                BrowserNode::NewConn { .. } => {
                    ("  ", "+ ", node.display_label().to_string(), String::new())
                }
                BrowserNode::File { name, size, uri, .. } => {
                    // Show a download marker on the node currently downloading.
                    let dl = app.download_state.as_ref().filter(|d| d.source_uri == *uri);
                    let prefix = if let Some(d) = dl {
                        match d.total {
                            Some(t) if t > 0 => format!("⬇{:.0}% ", app.downloaded_bytes as f64 / t as f64 * 100.0),
                            _ => format!("⬇{} ", crate::source::browser::format_size(app.downloaded_bytes)),
                        }
                    } else {
                        String::new()
                    };
                    ("  ", "  ", format!("{}{}", prefix, name), format_size(*size))
                }
            };
            let line = format!("{}{}{}{}", indent, expander, icon, label);
            let line = if size_str.is_empty() { line } else { format!("{:<50} {:>10}", line, size_str) };
            let style = if i == app.browser_selected {
                let mut s = Style::default().bold();
                if let Some(bg) = hl_bg { s = s.bg(bg); }
                if let Some(fg) = hl_fg { s = s.fg(fg); }
                s
            } else {
                match node {
                    BrowserNode::Category { .. } => cat_fg.map_or(Style::default(), |c| Style::default().fg(c)),
                    BrowserNode::LocalDir { .. } | BrowserNode::LocalParent { .. } => dir_fg.map_or(Style::default(), |c| Style::default().fg(c)),
                    BrowserNode::Bucket { .. } | BrowserNode::Prefix { .. } | BrowserNode::WebDavDir { .. } | BrowserNode::SshDir { .. } | BrowserNode::CloudConn { .. } => Style::default().fg(Color::Cyan),
                    BrowserNode::NewConn { .. } => Style::default().fg(Color::DarkGray),
                    BrowserNode::File { .. } => file_fg.map_or(Style::default().fg(Color::Gray), |c| Style::default().fg(c)),
                }
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Browser")
                .title_alignment(Alignment::Center)
                .border_style(border_style),
        );

    // Scroll state
    let mut state = ListState::default();
    state.select(Some(app.browser_selected));
    // Ensure visible by adjusting scroll
    if app.browser_selected < app.browser_scroll {
        // scroll up handled externally
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_uri_input(f: &mut Frame, app: &App, area: Rect) {
    let border_focus = crate::theme::parse_color(&app.theme.ui.border_focus);
    let border_unfocus = crate::theme::parse_color(&app.theme.ui.border_unfocus);

    let border_style = if app.focus == Focus::FileInput {
        border_focus.map_or(Style::default(), |c| Style::default().fg(c))
    } else {
        border_unfocus.map_or(Style::default(), |c| Style::default().fg(c))
    };

    let input_text = app.file_input.value();
    let widget = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Path / URI (Enter to load) ")
            .border_style(border_style),
    );
    f.render_widget(widget, area);

    if app.focus == Focus::FileInput {
        let cursor_pos = app.file_input.cursor();
        f.set_cursor_position((area.x + cursor_pos as u16 + 1, area.y + 1));
    }
}

fn draw_cloud_dialog(f: &mut Frame, app: &App, area: Rect) {
    // Clear the underlying content
    f.render_widget(Clear, area);

    // Compute dialog dimensions
    let dialog_width = (area.width * 3 / 5).min(64);
    let field_count = app.cloud_dialog_fields.len() as u16;
    // borders(2) + top pad(1) + fields + button(1) + footer(1)
    let dialog_height = 2 + 1 + field_count + 1 + 1;
    let dialog_height = dialog_height.min(area.height.saturating_sub(2));

    let dialog_area = Rect {
        x: area.x + (area.width.saturating_sub(dialog_width)) / 2,
        y: area.y + (area.height.saturating_sub(dialog_height)) / 2,
        width: dialog_width,
        height: dialog_height,
    };

    // Title
    let title = match &app.cloud_dialog_type {
        CloudDialogType::Ssh => " SSH Connection ",
        CloudDialogType::WebDav => " WebDAV Connection ",
        CloudDialogType::S3 => " S3 Connection ",
        CloudDialogType::Ks3 => " KS3 Connection ",
    };

    // Border
    let border_fg = crate::theme::parse_color(&app.theme.ui.border_focus).unwrap_or(Color::Yellow);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(border_fg));
    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    // Layout: top padding + one row per field + Connect button + footer
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(1)]; // top pad
    for _ in 0..field_count {
        constraints.push(Constraint::Length(1)); // label + input on same row
    }
    constraints.push(Constraint::Length(1)); // Connect button
    constraints.push(Constraint::Min(1)); // remaining → footer hint

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let hl_bg = crate::theme::parse_color(&app.theme.ui.row_highlight_bg).unwrap_or(Color::DarkGray);
    let dim_fg = crate::theme::parse_color(&app.theme.ui.border_unfocus).unwrap_or(Color::DarkGray);
    let label_width: u16 = app
        .cloud_dialog_fields
        .iter()
        .map(|f| f.label.chars().count() as u16 + 2) // " X:" → label len + 2
        .max()
        .unwrap_or(10);

    let dialog_focused = app.focus == Focus::CloudDialog;
    let connect_idx = app.cloud_dialog_connect_index();

    for (i, field) in app.cloud_dialog_fields.iter().enumerate() {
        let focused = i == app.cloud_dialog_selected && dialog_focused;
        let row_idx = 1 + i;

        // Horizontal split: label (fixed) | input (rest)
        let row_area = chunks[row_idx];
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(label_width),
                Constraint::Min(0),
            ])
            .split(row_area);

        // Label portion
        let label_style = if focused {
            Style::default().bold().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };
        f.render_widget(
            Paragraph::new(format!(" {}:", field.label)).style(label_style),
            h_chunks[0],
        );

        // Input portion — dark background marks the input area, no border line
        let input_area = h_chunks[1];
        let input_bg = if focused { hl_bg } else { Color::Rgb(30, 30, 40) };
        let input_fg = if focused { Color::White } else { Color::Gray };
        f.render_widget(
            Paragraph::new(" ".repeat(input_area.width as usize))
                .style(Style::default().bg(input_bg)),
            input_area,
        );

        // Masked value for secret fields, plain otherwise.
        let val = field.display_value();
        f.render_widget(
            Paragraph::new(val).style(Style::default().fg(input_fg).bg(input_bg)),
            input_area,
        );

        // Cursor (offset is char-based; mask has one glyph per char)
        if focused {
            let cx = field.input.cursor() as u16;
            f.set_cursor_position((input_area.x + cx, input_area.y));
        }
    }

    // Connect button row (after the fields)
    let button_idx = 1 + field_count as usize;
    let button_area = chunks[button_idx];
    let button_selected = app.cloud_dialog_selected == connect_idx && dialog_focused;
    let button_style = if button_selected {
        Style::default().bg(Color::Yellow).fg(Color::Black).bold()
    } else {
        Style::default().fg(border_fg)
    };
    f.render_widget(
        Paragraph::new(" < Connect > ").style(button_style).alignment(Alignment::Center),
        button_area,
    );

    // Footer hint
    let footer_area = chunks[chunks.len() - 1];
    f.render_widget(
        Paragraph::new(" [Tab/↑↓] Navigate  [Enter] Connect  [Esc] Cancel ")
            .style(Style::default().fg(dim_fg))
            .alignment(Alignment::Center),
        Rect {
            y: footer_area.y,
            ..footer_area
        },
    );
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
            Focus::SummaryChart => draw_summary(f, app),
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

    if let Some(ref err) = app.error_message {
        let error_fg = crate::theme::parse_color(&app.theme.ui.error_fg).unwrap_or(Color::Red);
        let err_area = Rect::new(area.x, area.height.saturating_sub(2), area.width, 1);
        f.render_widget(
            Paragraph::new(err.as_str()).style(Style::default().fg(error_fg)),
            err_area,
        );
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
        " [q/Esc]Quit [↑↓]Nav [←→]Pan [Shift+←→]H-Scroll [Tab]Switch [f]Full [/]SQL [d]HideCol [u]Unhide [s]Stats [b]Back [+/-]Zoom [r]Reset [Ctrl+T]Theme [m]Summary "
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

fn draw_summary(f: &mut Frame, app: &App) {
    use crate::viz::summary_data::{LANE_LABEL_COLS, TIME_AXIS_ROWS, MIN_PLOT_WIDTH, MIN_PLOT_HEIGHT, ns_to_col, format_duration, SummaryEvent, SummaryEventType};
    use ratatui::text::{Line, Span};

    let area = f.area();
    let focused = app.focus == Focus::SummaryChart;

    let border_color = if focused {
        crate::theme::parse_color(&app.theme.ui.border_focus)
    } else {
        crate::theme::parse_color(&app.theme.ui.border_unfocus)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Summary Timeline ")
        .title_alignment(Alignment::Center)
        .border_style(border_color.map_or(Style::default(), |c| Style::default().fg(c)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prep = match app.summary_prepared {
        Some(ref p) => p,
        None => return,
    };
    let vp = match app.summary_viewport {
        Some(ref v) => v,
        None => return,
    };

    if prep.events.is_empty() {
        f.render_widget(Paragraph::new("No events found"), inner);
        return;
    }

    // Layout: left label panel (fixed) | right chart panel (zoomable)
    let label_panel = Rect {
        x: inner.x,
        y: inner.y + TIME_AXIS_ROWS,
        width: LANE_LABEL_COLS,
        height: inner.height.saturating_sub(TIME_AXIS_ROWS),
    };
    let chart_panel = Rect {
        x: inner.x + LANE_LABEL_COLS,
        y: inner.y,
        width: inner.width.saturating_sub(LANE_LABEL_COLS),
        height: inner.height,
    };
    let pa = Rect {
        x: chart_panel.x,
        y: chart_panel.y + TIME_AXIS_ROWS,
        width: chart_panel.width,
        height: chart_panel.height.saturating_sub(TIME_AXIS_ROWS),
    };

    if pa.width < MIN_PLOT_WIDTH || pa.height < MIN_PLOT_HEIGHT {
        f.render_widget(Paragraph::new("Area too small"), inner);
        return;
    }

    let gpu_bg = crate::theme::parse_color(&app.theme.summary.gpu_bg).unwrap_or(Color::Rgb(20, 25, 45));
    let cpu_bg = crate::theme::parse_color(&app.theme.summary.cpu_bg).unwrap_or(Color::Rgb(45, 20, 20));
    let label_fg = crate::theme::parse_color(&app.theme.summary.lane_label_fg).unwrap_or(Color::Gray);
    let label_bg = gpu_bg;
    let time_fg = crate::theme::parse_color(&app.theme.timeline.time_axis_fg).unwrap_or(Color::Gray);
    let sep_fg = label_fg;
    let ht_fg = crate::theme::parse_color(&app.theme.timeline.hover_text_fg).unwrap_or(Color::Black);
    let ht_bg = crate::theme::parse_color(&app.theme.timeline.hover_text_bg).unwrap_or(Color::White);
    let _sel_bg = crate::theme::parse_color(&app.theme.timeline.selection_bg).unwrap_or(Color::Rgb(60, 60, 80));
    let kc: Vec<Color> = app.theme.timeline.kernel_colors.iter().filter_map(|s| crate::theme::parse_color(s)).collect();
    let mc = crate::theme::parse_color(&app.theme.timeline.memcpy_color).unwrap_or(Color::Magenta);
    let ms = crate::theme::parse_color(&app.theme.timeline.memset_color).unwrap_or(Color::Yellow);
    let rc = crate::theme::parse_color(&app.theme.summary.runtime_color).unwrap_or(Color::Cyan);

    // --- Left panel: lane labels (fixed) ---
    f.render_widget(
        Paragraph::new(" ".repeat(LANE_LABEL_COLS as usize)).style(Style::default().bg(label_bg)),
        label_panel,
    );

    for (i, lane_key) in prep.gpu_lanes.iter().chain(prep.cpu_lanes.iter()).enumerate() {
        let lane_y = pa.y + i as u16;
        if lane_y >= inner.y + inner.height { break; }
        let bg = if i < prep.gpu_lane_count() { gpu_bg } else { cpu_bg };
        let label_text = prep.lane_labels.get(lane_key).map(|s| s.as_str()).unwrap_or(lane_key);
        let label = format!(" {:<13}", label_text);
        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(label_fg).bg(bg)),
            Rect::new(label_panel.x, lane_y, LANE_LABEL_COLS, 1),
        );
    }

    // --- Vertical separator ---
    let sep_x = inner.x + LANE_LABEL_COLS;
    for row in (inner.y + TIME_AXIS_ROWS)..(inner.y + inner.height) {
        f.render_widget(
            Paragraph::new("\u{2502}").style(Style::default().fg(sep_fg)),
            Rect::new(sep_x, row, 1, 1),
        );
    }

    // --- Right panel: time axis + lanes ---
    // Time axis
    let num_ticks = (pa.width / 16).max(1) as usize;
    for i in 0..=num_ticks {
        let offset_ns = vp.view_width_ns * (i as f64 / num_ticks as f64);
        let col = ns_to_col(offset_ns, 0.0, vp.view_width_ns, pa.width);
        if col >= 0.0 && (col as u16) < pa.width {
            let x = pa.x + col as u16;
            let label = format_duration(offset_ns);
            f.render_widget(
                Paragraph::new(label.clone()).style(Style::default().fg(time_fg)),
                Rect::new(x.saturating_sub((label.len() / 2) as u16), chart_panel.y, label.len() as u16 + 1, 1),
            );
            f.render_widget(
               Paragraph::new("|").style(Style::default().fg(sep_fg)),
                Rect::new(x, chart_panel.y + 1, 1, 1),
            );
        }
    }

    // GPU/CPU separator
    if prep.gpu_lane_count() > 0 && prep.cpu_lanes.len() > 0 {
        let sep_row = pa.y + prep.gpu_lane_count() as u16;
        if sep_row < pa.y + pa.height {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled("\u{2500}".repeat(pa.width as usize), Style::default().fg(sep_fg)))),
                Rect::new(pa.x, sep_row, pa.width, 1),
            );
        }
    }

    // Draw events per lane
    let plot_w = pa.width as usize;
    let view_start = vp.view_start_ns;
    let view_width = vp.view_width_ns;

    for lane_idx in 0..prep.total_lanes() {
        let y = pa.y + lane_idx as u16;
        if y >= pa.y + pa.height { break; }
        let lane_bg = if lane_idx < prep.gpu_lane_count() { gpu_bg } else { cpu_bg };

        // Collect visible events for this lane
        let lane_key = prep.gpu_lanes.iter().chain(prep.cpu_lanes.iter()).nth(lane_idx).unwrap();

        let mut visible: Vec<&SummaryEvent> = prep.events.iter()
            .filter(|e| e.lane_key == *lane_key && e.end > view_start && e.start < view_start + view_width)
            .collect();
        visible.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

        // Build per-column style buffer for this lane.
        // Start with lane background, then paint events on top (short events last for visibility).
        visible.sort_by(|a, b| {
            let dur_a = a.end - a.start;
            let dur_b = b.end - b.start;
            dur_b.partial_cmp(&dur_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut cell_styles: Vec<Style> = vec![Style::default().bg(lane_bg); plot_w];
        let mut cell_chars: Vec<char> = vec![' '; plot_w];

        for event in visible.iter() {
            let global_idx = prep.events.iter().position(|e| std::ptr::eq(e, *event)).unwrap_or(0);
            let raw_start = ns_to_col(event.start, view_start, view_width, pa.width);
            let raw_end = ns_to_col(event.end, view_start, view_width, pa.width);
            let start_col = (raw_start.max(0.0) as usize).min(plot_w);
            let end_col = ((raw_end.ceil() as usize).max(start_col + 1)).min(plot_w);
            let clipped_width = end_col - start_col;
            if clipped_width == 0 { continue; }

            let color = match event.event_type {
                SummaryEventType::Kernel => crate::viz::timeline::kernel_color(&event.name, &kc),
                SummaryEventType::Memcpy => mc,
                SummaryEventType::Memset => ms,
                SummaryEventType::Runtime => rc,
            };

            let is_hovered = vp.hovered == Some(global_idx);
            let block_bg = if is_hovered { ht_bg } else { color };
            let block_fg = ht_fg;

            // For narrow events (1-3 cols), just paint solid background
            if clipped_width < 4 {
                for c in start_col..end_col {
                    cell_styles[c] = Style::default().bg(block_bg);
                }
            } else {
                // For wider events, paint label centered
                let label = crate::core::types::truncate_str(&event.name, clipped_width.saturating_sub(2));
                let padded = format!(" {} ", label);
                let text = if padded.len() > clipped_width {
                    crate::core::types::truncate_str(&padded, clipped_width).to_string()
                } else {
                    let mut s = padded;
                    while s.len() < clipped_width { s.push(' '); }
                    s
                };
                for (i, ch) in text.chars().enumerate() {
                    let c = start_col + i;
                    if c < plot_w {
                        cell_styles[c] = Style::default().fg(block_fg).bg(block_bg);
                        cell_chars[c] = ch;
                    }
                }
            }
        }

        // Convert cell buffer to spans
        let mut lane_spans: Vec<Span> = Vec::new();
        let mut i = 0;
        while i < plot_w {
            let style = cell_styles[i];
            let start = i;
            while i < plot_w && cell_styles[i] == style { i += 1; }
            let _len = i - start;
            let text: String = cell_chars[start..i].iter().collect();
            lane_spans.push(Span::styled(text, style));
        }

        f.render_widget(Paragraph::new(Line::from(lane_spans)), Rect::new(pa.x, y, pa.width, 1));
    }

    // Hover tooltip
    if let Some(hi) = vp.hovered {
        if let Some(event) = prep.events.get(hi) {
            let dur = format_duration(event.end - event.start);
            let tooltip = crate::core::types::truncate_str(&format!("{} | {}", event.name, dur), 50);
            let tw = (tooltip.len() as u16 + 2).min(chart_panel.width);
            let lane = *prep.lane_map.get(&event.lane_key).unwrap_or(&0);
            let ecol = ns_to_col(event.start, view_start, view_width, pa.width) as u16;
            let tx = (pa.x + ecol + 2).min(chart_panel.x + chart_panel.width.saturating_sub(tw + 2));
            let ty = if lane == 0 { chart_panel.y + TIME_AXIS_ROWS } else { pa.y + lane as u16 - 1 };
            f.render_widget(
                Paragraph::new(format!(" {} ", tooltip)).style(Style::default().fg(ht_fg).bg(crate::theme::parse_color(&app.theme.timeline.info_bar_bg).unwrap_or(Color::DarkGray))),
                Rect::new(tx, ty, tw, 1),
            );
        }
    }

    // Info bar
    let mut info_spans: Vec<Span> = Vec::new();
    let ib_fg = crate::theme::parse_color(&app.theme.timeline.info_bar_fg).unwrap_or(Color::White);
    let ib_bg = crate::theme::parse_color(&app.theme.timeline.info_bar_bg).unwrap_or(Color::DarkGray);

    if let Some(hi) = vp.hovered {
        if let Some(event) = prep.events.get(hi) {
            let color = match event.event_type {
                SummaryEventType::Kernel => crate::viz::timeline::kernel_color(&event.name, &kc),
                SummaryEventType::Memcpy => mc,
                SummaryEventType::Memset => ms,
                SummaryEventType::Runtime => rc,
            };
            let name = crate::core::types::truncate_str(&event.name, 30);
            let dur = format_duration(event.end - event.start);
            info_spans.push(Span::styled(format!(" {} {} ", name, dur), Style::default().fg(ht_fg).bg(color)));
        }
    } else if let Some((s, e)) = vp.selection {
        let us = (e - s) / 1000.0;
        let txt = if us >= 1000.0 { format!("Selected: {:.1}ms", us / 1000.0) } else { format!("Selected: {:.1}us", us) };
        info_spans.push(Span::styled(txt, Style::default().fg(ib_fg).bg(ib_bg)));
    }

    if !info_spans.is_empty() {
        let info_area = Rect {
            x: chart_panel.x + 1,
            y: inner.y + inner.height.saturating_sub(2),
            width: chart_panel.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(info_spans)), info_area);
    }
}
