use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tui_input::Input;

use crate::core::backend::ProfilerBackend;
use crate::core::registry::Registry;
use crate::core::session::ProfilerSession;
use crate::core::types::{ProfilerData, ViewCategory, ViewDescriptor};
use crate::theme::Theme;
use crate::viz::renderer::VizRenderer;
use crate::viz::types::{PreparedVisualization, TimelineViewport, VizData};
use crate::viz::timeline;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    FileSelection,
    ViewBrowser,
    StatsView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    FileInput,
    ViewList,
    DataTable,
    Chart,
    StatsTable,
    SqlInput,
    SqlResult,
}

pub struct App {
    pub state: AppState,
    pub focus: Focus,
    pub file_input: Input,
    pub error_message: Option<String>,

    // Profiler backend infrastructure
    pub registry: Registry,
    pub backend: Option<Arc<dyn ProfilerBackend>>,
    pub session: Option<Box<dyn ProfilerSession>>,

    // Theme
    pub theme: Theme,
    pub theme_name: String,

    // View navigation
    pub views: Vec<ViewDescriptor>,
    pub selected_view_index: usize,
    pub view_data: Option<ProfilerData>,

    // UI scroll state
    pub table_scroll: usize,
    pub table_hscroll: usize,
    pub chart_scroll: usize,

    // Stats sub-view
    pub stats_data: Option<ProfilerData>,
    pub stats_scroll: usize,

    // Cached rendering
    pub prepared_viz: Option<PreparedVisualization>,
    pub active_renderer: Option<Arc<dyn VizRenderer>>,
    pub cached_display_data: Option<ProfilerData>,

    // Timeline interactive state
    pub timeline_viewport: Option<TimelineViewport>,

    // SQL console state
    pub sql_mode: bool,
    pub sql_input: Input,
    pub sql_result: Option<ProfilerData>,
    pub sql_error: Option<String>,

    // Fullscreen state
    pub fullscreen: Option<Focus>,

    // Help bar visibility
    pub help_bar_visible: bool,

    // Table interactive state
    pub hidden_col_names: HashSet<String>,
    pub col_highlight: Option<String>,
    pub row_highlight: Option<usize>,
    pub last_click_at: Option<(u16, u16, Instant)>,
}

fn prepared_stream_count(viz: &Option<PreparedVisualization>) -> usize {
    viz.as_ref()
        .and_then(|v| match &v.data {
            VizData::Timeline(p) => Some(p.stream_ids.len()),
            _ => None,
        })
        .unwrap_or(0)
}

impl App {
    pub fn new(registry: Registry) -> Self {
        Self {
            state: AppState::FileSelection,
            focus: Focus::FileInput,
            file_input: Input::default(),
            error_message: None,
            registry,
            backend: None,
            session: None,
            theme: Theme::default(),
            theme_name: "default".to_string(),
            views: Vec::new(),
            selected_view_index: 0,
            view_data: None,
            table_scroll: 0,
            table_hscroll: 0,
            chart_scroll: 0,
            stats_data: None,
            stats_scroll: 0,
            prepared_viz: None,
            active_renderer: None,
            cached_display_data: None,
            timeline_viewport: None,
            sql_mode: false,
            sql_input: Input::default(),
            sql_result: None,
            sql_error: None,
            fullscreen: None,
            help_bar_visible: true,
            hidden_col_names: HashSet::new(),
            col_highlight: None,
            row_highlight: None,
            last_click_at: None,
        }
    }

    pub fn load_database(&mut self, path: &std::path::Path) -> Result<()> {
        if !path.exists() {
            self.error_message = Some("File does not exist".to_string());
            return Ok(());
        }

        let backend = match self.registry.detect_backend(path) {
            Ok(b) => b,
            Err(e) => {
                self.error_message = Some(format!("Unsupported file: {}", e));
                return Ok(());
            }
        };

        self.backend = Some(backend.clone());

        match backend.open(path) {
            Ok(session) => {
                self.session = Some(session);
                let session_ref = self.session.as_ref().unwrap().as_ref();
                match backend.list_views(session_ref) {
                    Ok(views) => {
                        self.views = views;
                        self.state = AppState::ViewBrowser;
                        self.focus = Focus::ViewList;
                        self.error_message = None;
                        if !self.views.is_empty() {
                            self.selected_view_index = 0;
                            self.load_view_data()?;
                        }
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to list views: {}", e));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to open file: {}", e));
            }
        }

        Ok(())
    }

    pub fn is_inputting(&self) -> bool {
        self.state == AppState::FileSelection && self.focus == Focus::FileInput
    }

    pub fn is_sql_inputting(&self) -> bool {
        self.state == AppState::ViewBrowser && self.focus == Focus::SqlInput
    }

    pub fn current_view_is_timeline(&self) -> bool {
        self.views
            .get(self.selected_view_index)
            .map_or(false, |v| v.category == ViewCategory::Timeline)
    }

    fn current_view_is_metadata(&self) -> bool {
        self.views
            .get(self.selected_view_index)
            .map_or(false, |v| v.category == ViewCategory::Metadata)
    }

    pub fn open_sql_mode(&mut self) {
        self.sql_mode = true;
        self.focus = Focus::SqlInput;
        self.invalidate_display_cache();
    }

    pub fn close_sql_mode(&mut self) {
        self.sql_mode = false;
        self.sql_result = None;
        self.sql_error = None;
        self.table_scroll = 0;
        self.table_hscroll = 0;
        self.focus = Focus::ViewList;
        self.invalidate_display_cache();
    }

    pub fn toggle_fullscreen(&mut self) {
        if self.fullscreen.is_some() {
            self.fullscreen = None;
        } else if self.state == AppState::ViewBrowser {
            match self.focus {
                Focus::ViewList | Focus::Chart | Focus::DataTable | Focus::SqlInput => {
                    self.fullscreen = Some(self.focus.clone());
                }
                _ => {}
            }
        } else if self.state == AppState::StatsView {
            match self.focus {
                Focus::Chart | Focus::StatsTable => {
                    self.fullscreen = Some(self.focus.clone());
                }
                _ => {}
            }
        }
    }

    pub fn display_data(&self) -> Option<&ProfilerData> {
        if self.cached_display_data.is_some() {
            return self.cached_display_data.as_ref();
        }
        None // caller should call ensure_display_cache first
    }

    pub fn ensure_display_cache(&mut self) {
        if self.cached_display_data.is_some() {
            return;
        }
        let raw = if self.sql_mode {
            self.sql_result.as_ref().or(self.view_data.as_ref())
        } else {
            self.view_data.as_ref()
        };
        if let Some(d) = raw {
            self.cached_display_data = Some(
                d.without_all_null_columns()
                 .without_redundant_name_columns()
                 .with_merged_grid_block()
            );
        }
    }

    pub fn invalidate_display_cache(&mut self) {
        self.cached_display_data = None;
    }

    pub fn execute_current_sql(&mut self) {
        let sql = self.sql_input.value().to_string();
        if sql.trim().is_empty() {
            return;
        }
        if let (Some(ref backend), Some(ref session)) = (&self.backend, &self.session) {
            match backend.execute_sql(session.as_ref(), &sql) {
                Ok(data) => {
                    self.sql_result = Some(data);
                    self.sql_error = None;
                    self.table_scroll = 0;
                    self.invalidate_display_cache();
                }
                Err(e) => {
                    self.sql_error = Some(format!("{}", e));
                }
            }
        }
    }

    pub fn on_up(&mut self) {
        match self.focus {
            Focus::ViewList => {
                if self.views.is_empty() { return; }
                if self.selected_view_index > 0 {
                    self.selected_view_index -= 1;
                } else {
                    self.selected_view_index = self.views.len() - 1;
                }
                let _ = self.load_view_data();
            }
            Focus::DataTable => {
                if let Some(data) = self.cached_display_data.as_ref() {
                    if data.row_count == 0 { return; }
                    self.table_scroll = if self.table_scroll > 0 { self.table_scroll - 1 } else { data.row_count - 1 };
                }
            }
            _ => {}
        }
    }

    pub fn on_down(&mut self) {
        match self.focus {
            Focus::ViewList => {
                if self.views.is_empty() { return; }
                if self.selected_view_index + 1 < self.views.len() {
                    self.selected_view_index += 1;
                } else {
                    self.selected_view_index = 0;
                }
                let _ = self.load_view_data();
            }
            Focus::DataTable => {
                if let Some(data) = self.cached_display_data.as_ref() {
                    if data.row_count == 0 { return; }
                    self.table_scroll = if self.table_scroll + 1 < data.row_count { self.table_scroll + 1 } else { 0 };
                }
            }
            _ => {}
        }
    }

    pub fn on_left(&mut self) {
        if self.state == AppState::ViewBrowser {
            self.focus = Focus::ViewList;
        }
    }

    pub fn on_right(&mut self) {
        if self.state == AppState::ViewBrowser && !self.views.is_empty() {
            if self.current_view_is_metadata() {
                self.focus = Focus::DataTable;
            } else {
                self.focus = Focus::DataTable;
            }
        }
    }

    pub fn on_table_hscroll_left(&mut self) {
        self.table_hscroll = self.table_hscroll.saturating_sub(4);
    }

    pub fn on_table_hscroll_right(&mut self) {
        self.table_hscroll = self.table_hscroll.saturating_add(4);
    }

    pub fn on_tab(&mut self) {
        // Tab always exits fullscreen first
        if self.fullscreen.is_some() {
            self.fullscreen = None;
            return;
        }
        if self.state == AppState::ViewBrowser {
            if self.sql_mode {
                self.focus = match self.focus {
                    Focus::ViewList => Focus::SqlInput,
                    Focus::SqlInput => Focus::DataTable,
                    Focus::DataTable => Focus::ViewList,
                    _ => Focus::ViewList,
                };
            } else {
                let is_metadata = self.current_view_is_metadata();
                if is_metadata {
                    self.focus = match self.focus {
                        Focus::ViewList => Focus::DataTable,
                        Focus::DataTable => Focus::ViewList,
                        _ => Focus::ViewList,
                    };
                } else {
                    self.focus = match self.focus {
                        Focus::ViewList => Focus::Chart,
                        Focus::Chart => Focus::DataTable,
                        Focus::DataTable => Focus::ViewList,
                        _ => Focus::ViewList,
                    };
                }
            }
        } else if self.state == AppState::StatsView {
            self.focus = match self.focus {
                Focus::Chart => Focus::StatsTable,
                Focus::StatsTable => Focus::Chart,
                _ => Focus::Chart,
            };
        }
    }

    pub fn on_enter(&mut self) -> Result<()> {
        if self.state == AppState::FileSelection {
            let path_str = self.file_input.value();
            if !path_str.is_empty() {
                self.load_database(&PathBuf::from(path_str))?;
            }
        } else if self.is_sql_inputting() {
            self.execute_current_sql();
        }
        Ok(())
    }

    pub fn on_char(&mut self, c: char) {
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::InsertChar(c));
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::InsertChar(c));
        }
    }

    pub fn on_backspace(&mut self) {
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::DeletePrevChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::DeletePrevChar);
        }
    }

    pub fn on_delete(&mut self) {
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::DeleteNextChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::DeleteNextChar);
        }
    }

    pub fn on_input_left(&mut self) {
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::GoToPrevChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::GoToPrevChar);
        }
    }

    pub fn on_input_right(&mut self) {
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::GoToNextChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::GoToNextChar);
        }
    }

    pub fn on_input_ctrl_a(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToStart);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToStart);
        }
    }

    pub fn on_input_ctrl_e(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToEnd);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToEnd);
        }
    }

    pub fn on_input_ctrl_u(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeleteLine);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::DeleteLine);
        }
    }

    pub fn on_input_ctrl_k(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeleteTillEnd);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::DeleteTillEnd);
        }
    }

    pub fn on_input_ctrl_w(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeletePrevWord);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::DeletePrevWord);
        }
    }

    pub fn on_input_alt_b(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToPrevWord);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToPrevWord);
        }
    }

    pub fn on_input_alt_f(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToNextWord);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToNextWord);
        }
    }

    pub fn on_hide_col(&mut self) {
        if let Some(ref name) = self.col_highlight.take() {
            self.hidden_col_names.insert(name.clone());
            self.invalidate_display_cache();
        }
    }

    pub fn on_unhide_all_cols(&mut self) {
        self.hidden_col_names.clear();
        self.invalidate_display_cache();
        self.col_highlight = None;
    }

    pub fn cycle_theme(&mut self) {
        let themes = crate::theme::discover_themes();
        if themes.is_empty() {
            return;
        }
        let current_idx = themes.iter().position(|t| t == &self.theme_name).unwrap_or(0);
        let next_idx = (current_idx + 1) % themes.len();
        let next_name = themes[next_idx].clone();
        match crate::theme::load_theme(&next_name) {
            Ok(t) => {
                self.theme = t;
                self.theme_name = next_name;
            }
            Err(_) => {}
        }
    }

    pub fn on_paste(&mut self, text: &str) {
        if self.is_inputting() {
            for c in text.chars() {
                self.file_input.handle(tui_input::InputRequest::InsertChar(c));
            }
        } else if self.is_sql_inputting() {
            for c in text.chars() {
                self.sql_input.handle(tui_input::InputRequest::InsertChar(c));
            }
        }
    }

    pub fn on_home(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input.handle(tui_input::InputRequest::GoToStart);
        }
    }

    pub fn on_end(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input.handle(tui_input::InputRequest::GoToEnd);
        }
    }

    pub fn on_scroll_down(&mut self) {
        match self.focus {
            Focus::ViewList => self.on_down(),
            Focus::DataTable => {
                if let Some(data) = self.cached_display_data.as_ref() {
                    if data.row_count == 0 { return; }
                    self.table_scroll = if self.table_scroll + 1 < data.row_count { self.table_scroll + 1 } else { 0 };
                }
            }
            Focus::Chart => {
                self.chart_scroll = self.chart_scroll.saturating_add(1);
                self.refresh_visualization();
            }
            Focus::StatsTable => {
                if let Some(ref data) = self.stats_data {
                    if data.row_count == 0 { return; }
                    self.stats_scroll = if self.stats_scroll + 1 < data.row_count { self.stats_scroll + 1 } else { 0 };
                }
            }
            _ => {}
        }
    }

    pub fn on_scroll_up(&mut self) {
        match self.focus {
            Focus::ViewList => self.on_up(),
            Focus::DataTable => {
                if let Some(data) = self.cached_display_data.as_ref() {
                    if data.row_count == 0 { return; }
                    self.table_scroll = if self.table_scroll > 0 { self.table_scroll - 1 } else { data.row_count - 1 };
                }
            }
            Focus::Chart => {
                self.chart_scroll = self.chart_scroll.saturating_sub(1);
                self.refresh_visualization();
            }
            Focus::StatsTable => {
                if let Some(ref data) = self.stats_data {
                    if data.row_count == 0 { return; }
                    self.stats_scroll = if self.stats_scroll > 0 { self.stats_scroll - 1 } else { data.row_count - 1 };
                }
            }
            _ => {}
        }
    }

    pub fn on_mouse_click(&mut self, x: u16, y: u16, area: ratatui::layout::Rect) -> Result<()> {
        if self.state == AppState::ViewBrowser {
            let is_tl = self.current_view_is_timeline();
            let is_md = self.current_view_is_metadata();
            let (left, _, right_chunks) = crate::ui::layout_chunks(area, is_tl, is_md, self.sql_mode);

            if x < left.x + left.width {
                self.focus = Focus::ViewList;
            } else if self.sql_mode {
                if y < right_chunks[0].y + right_chunks[0].height {
                    self.focus = Focus::SqlInput;
                } else {
                    self.focus = Focus::DataTable;
                }
            } else if is_md {
                self.focus = Focus::DataTable;
            } else if y < right_chunks[0].y + right_chunks[0].height {
                self.focus = Focus::Chart;
            } else {
                self.focus = Focus::DataTable;
            }
        }
        Ok(())
    }

    pub fn enter_stats_view(&mut self) -> Result<()> {
        if self.sql_mode {
            self.close_sql_mode();
        }
        if let (Some(ref backend), Some(ref session), Some(view)) =
            (&self.backend, &self.session, self.views.get(self.selected_view_index))
        {
            match backend.get_stats(session.as_ref(), &view.id) {
                Ok(Some(stats)) => {
                    self.stats_data = Some(stats);
                    self.stats_scroll = 0;

                    // Try to find a renderer for the stats visualization
                    if let Some(ref stats_data) = self.stats_data {
                        self.active_renderer = self.registry.find_renderer(stats_data, view);
                        if let Some(ref renderer) = self.active_renderer {
                            match renderer.prepare(stats_data, view, 0) {
                                Ok(viz) => self.prepared_viz = Some(viz),
                                Err(e) => self.error_message = Some(format!("Viz error: {}", e)),
                            }
                        }
                    }

                    self.state = AppState::StatsView;
                    self.focus = Focus::Chart;
                    self.error_message = None;
                }
                Ok(None) => {
                    self.error_message = Some("No statistics available for this view".to_string());
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to compute stats: {}", e));
                }
            }
        }
        Ok(())
    }

    pub fn leave_stats_view(&mut self) {
        if self.state == AppState::StatsView {
            self.state = AppState::ViewBrowser;
            self.focus = Focus::ViewList;
            self.stats_data = None;
            self.prepared_viz = None;
            self.active_renderer = None;
        }
    }

    fn load_view_data(&mut self) -> Result<()> {
        if let (Some(ref backend), Some(ref session), Some(view)) =
            (&self.backend, &self.session, self.views.get(self.selected_view_index))
        {
            match backend.get_view_data(session.as_ref(), &view.id) {
                Ok(data) => {
                    self.view_data = Some(data);
                    self.table_scroll = 0;
                    self.table_hscroll = 0;
                    self.hidden_col_names.clear();
                    self.col_highlight = None;
                    self.row_highlight = None;
                    self.invalidate_display_cache();
                    self.chart_scroll = 0;
                    self.error_message = None;
                    self.sql_result = None;
                    self.sql_error = None;
                    self.refresh_visualization();

                    // Initialize timeline viewport if this is a timeline view
                    if let Some(ref viz) = self.prepared_viz {
                        if let VizData::Timeline(ref prepared) = viz.data {
                            self.timeline_viewport = Some(TimelineViewport {
                                view_start_ns: prepared.global_start_ns,
                                view_width_ns: prepared.global_end_ns - prepared.global_start_ns,
                                stream_lanes: prepared.stream_ids.clone(),
                                hovered: None,
                                selection: None,
                                drag_origin: None,
                                panning: false,
                                pan_anchor_ns: 0.0,
                                pan_anchor_col: 0,
                            });
                            self.update_timeline_viewport();
                        } else {
                            self.timeline_viewport = None;
                        }
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load view: {}", e));
                }
            }
        }
        Ok(())
    }

    fn refresh_visualization(&mut self) {
        if let (Some(ref data), Some(view)) =
            (&self.view_data, self.views.get(self.selected_view_index))
        {
            self.active_renderer = self.registry.find_renderer(data, view);
            if let Some(ref renderer) = self.active_renderer {
                if let Ok(viz) = renderer.prepare(data, view, self.chart_scroll) {
                    self.prepared_viz = Some(viz);
                    return;
                }
            }
        }
        self.prepared_viz = None;
    }

    fn update_timeline_viewport(&mut self) {
        if let (Some(ref mut viz), Some(ref vp)) = (&mut self.prepared_viz, &self.timeline_viewport)
        {
            viz.viewport = Some(vp.clone());
        }
    }

    pub fn on_mouse_down(
        &mut self,
        button: crossterm::event::MouseButton,
        col: u16,
        row: u16,
        full_area: ratatui::layout::Rect,
    ) {
        let is_timeline = self.current_view_is_timeline();
        let is_metadata = self.current_view_is_metadata();
        let (left, _, right_chunks) = crate::ui::layout_chunks(full_area, is_timeline, is_metadata, self.sql_mode);
        let chart_area = right_chunks[0];

        // Compute data table area
        let data_area = if self.fullscreen == Some(Focus::DataTable) {
            full_area
        } else if self.sql_mode {
            right_chunks[1]
        } else if is_metadata {
            right_chunks[0]
        } else {
            right_chunks[1]
        };

        // Determine which region was clicked and set focus
        if col < left.x + left.width {
            self.focus = Focus::ViewList;
            return;
        }

        if self.sql_mode {
            if row < right_chunks[0].y + right_chunks[0].height {
                self.focus = Focus::SqlInput;
            } else {
                self.focus = Focus::DataTable;
            }
        } else if is_metadata {
            self.focus = Focus::DataTable;
        } else if row < chart_area.y + chart_area.height {
            self.focus = Focus::Chart;
        } else {
            self.focus = Focus::DataTable;
        }

        // Data table interactions
        if self.focus == Focus::DataTable && button == crossterm::event::MouseButton::Left {
            let header_y = data_area.y + 1; // inside top border
            let separator_y = header_y + 1; // header separator line
            let data_start_y = separator_y + 1; // first data row
            let data_end_y = data_area.y + data_area.height - 2; // inside bottom border

            if row == header_y {
                // Click on header — determine column using shared layout logic
                if let Some(data) = self.cached_display_data.as_ref() {
                    let (vis_names, widths, offsets) = crate::ui::compute_table_layout(data, data_area.width, self.table_hscroll, &self.hidden_col_names);
                    let rel_x = col.saturating_sub(data_area.x + 1) as usize;
                    for (vis_i, &off) in offsets.iter().enumerate() {
                        if rel_x >= off && rel_x < off + widths[vis_i] {
                            let name = vis_names[vis_i].clone();
                            self.col_highlight = if self.col_highlight.as_deref() == Some(&name) {
                                None
                            } else {
                                Some(name)
                            };
                            break;
                        }
                    }
                }
            } else if row >= data_start_y && row <= data_end_y {
                // Click in data area — check for double-click
                let data_row = self.table_scroll + (row - data_start_y) as usize;
                if let Some((prev_col, prev_row, prev_time)) = self.last_click_at {
                    if prev_col == col && prev_row == row && prev_time.elapsed() < Duration::from_millis(400) {
                        self.row_highlight = if self.row_highlight == Some(data_row) {
                            None
                        } else {
                            Some(data_row)
                        };
                        self.last_click_at = None;
                        return;
                    }
                }
                self.last_click_at = Some((col, row, Instant::now()));
            }
        }

        // Only start timeline drag if focus is Chart, not in SQL mode, and click is within the plot area
        if self.focus == Focus::Chart && !self.sql_mode {
            let Some(ref mut viewport) = self.timeline_viewport else {
                return;
            };
            let inner = chart_area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
            let pa = timeline::plot_area(inner);
            let stream_count = prepared_stream_count(&self.prepared_viz);
            let in_plot = col >= pa.x && col < pa.x + pa.width
                && row >= pa.y && row < pa.y + (stream_count as u16) * timeline::LANE_HEIGHT_ROWS;

            if in_plot {
                match button {
                    crossterm::event::MouseButton::Left => {
                        viewport.drag_origin = Some((col, row));
                        viewport.selection = None;
                    }
                    crossterm::event::MouseButton::Right | crossterm::event::MouseButton::Middle => {
                        viewport.panning = true;
                        viewport.pan_anchor_ns = viewport.view_start_ns;
                        viewport.pan_anchor_col = col;
                    }
                }
            }
        }
    }

    pub fn on_mouse_up(
        &mut self,
        button: crossterm::event::MouseButton,
        col: u16,
        row: u16,
        full_area: ratatui::layout::Rect,
    ) {
        if self.focus != Focus::Chart {
            return;
        }

        match button {
            crossterm::event::MouseButton::Left => {
                let click_result = self.timeline_viewport.as_ref().and_then(|vp| {
                    if let Some((origin_col, _)) = vp.drag_origin {
                        let dx = (col as i16 - origin_col as i16).unsigned_abs();
                        if dx <= 2 && vp.selection.is_none() {
                            return self.prepared_viz.as_ref().and_then(|viz| {
                                if let VizData::Timeline(ref prepared) = viz.data {
                                    let is_tl = self.current_view_is_timeline();
                                    let is_md = self.current_view_is_metadata();
                                    let (_, _, right_chunks) = crate::ui::layout_chunks(full_area, is_tl, is_md, self.sql_mode);
                                    let chart_area = right_chunks[0];
                                    let inner = chart_area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
                                    let pa = timeline::plot_area(inner);
                                    if col < pa.x || col >= pa.x + pa.width || row < pa.y { return None; }
                                    let mouse_int = (col.saturating_sub(pa.x)) as usize;
                                    let lane = timeline::row_to_lane(row, pa.y, prepared.stream_ids.len())?;
                                    let target_stream = prepared.stream_ids[lane];
                                    let mut best: Option<(usize, f64, f64, usize)> = None; // (idx, start, end, distance)
                                    for (i, e) in prepared.events.iter().enumerate() {
                                        if e.stream_id != target_stream { continue; }
                                        let sc_f = timeline::ns_to_col(e.start, vp.view_start_ns, vp.view_width_ns, pa.width);
                                        let ec_f = timeline::ns_to_col(e.start + e.duration, vp.view_start_ns, vp.view_width_ns, pa.width);
                                        let sc = sc_f.max(0.0) as usize;
                                        let w = ((ec_f - sc_f).ceil() as usize).max(1);
                                        let dist = if mouse_int >= sc && mouse_int < sc + w {
                                            0
                                        } else if mouse_int >= sc.saturating_sub(1) && mouse_int < sc + w + 1 {
                                            1
                                        } else {
                                            continue;
                                        };
                                        if best.is_none() || dist < best.as_ref().unwrap().3 {
                                            best = Some((i, e.start, e.start + e.duration, dist));
                                        }
                                    }
                                    best.map(|(i, s, e, _)| (i, s, e))
                                } else {
                                    None
                                }
                            });
                        }
                    }
                    None
                });

                if let Some((idx, start_ns, end_ns)) = click_result {
                    if let Some(ref mut viewport) = self.timeline_viewport {
                        viewport.selection = Some((start_ns, end_ns));
                        viewport.hovered = Some(idx);
                        self.update_timeline_viewport();
                    }
                }
                if let Some(ref mut viewport) = self.timeline_viewport {
                    viewport.drag_origin = None;
                }
            }
            crossterm::event::MouseButton::Right | crossterm::event::MouseButton::Middle => {
                if let Some(ref mut viewport) = self.timeline_viewport {
                    viewport.panning = false;
                }
            }
        }
    }

    pub fn on_mouse_drag(
        &mut self,
        button: crossterm::event::MouseButton,
        col: u16,
        row: u16,
        full_area: ratatui::layout::Rect,
    ) {
        if self.focus != Focus::Chart {
            return;
        }

        let is_tl = self.current_view_is_timeline();
        let is_md = self.current_view_is_metadata();
        let (_, _, right_chunks) = crate::ui::layout_chunks(full_area, is_tl, is_md, self.sql_mode);
        let chart_area = right_chunks[0];
        let inner = chart_area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
        let pa = timeline::plot_area(inner);

        let Some(ref mut viewport) = self.timeline_viewport else {
            return;
        };

        if button == crossterm::event::MouseButton::Left {
            if let Some((origin_col, _)) = viewport.drag_origin {
                let start_ns = timeline::col_to_ns(
                    origin_col.saturating_sub(pa.x) as f64,
                    viewport.view_start_ns,
                    viewport.view_width_ns,
                    pa.width,
                );
                let end_ns = timeline::col_to_ns(
                    col.saturating_sub(pa.x) as f64,
                    viewport.view_start_ns,
                    viewport.view_width_ns,
                    pa.width,
                );
                viewport.selection = Some((start_ns.min(end_ns), start_ns.max(end_ns)));

                // Update hovered event during drag (with ±1 col tolerance)
                if let Some(ref viz) = self.prepared_viz {
                    if let VizData::Timeline(ref prepared) = viz.data {
                        let stream_count = prepared.stream_ids.len() as u16;
                        if col >= pa.x && col < pa.x + pa.width
                            && row >= pa.y && row < pa.y + stream_count * timeline::LANE_HEIGHT_ROWS
                        {
                            let mouse_int = (col - pa.x) as usize;
                            let lane = timeline::row_to_lane(row, pa.y, prepared.stream_ids.len());
                            if let Some(lane_idx) = lane {
                                let target_stream = prepared.stream_ids[lane_idx];
                                let mut best: Option<(usize, usize)> = None;
                                for (i, e) in prepared.events.iter().enumerate() {
                                    if e.stream_id != target_stream { continue; }
                                    let start_col_f = timeline::ns_to_col(
                                        e.start, viewport.view_start_ns, viewport.view_width_ns, pa.width,
                                    );
                                    let end_col_f = timeline::ns_to_col(
                                        e.start + e.duration, viewport.view_start_ns, viewport.view_width_ns, pa.width,
                                    );
                                    let start_col = start_col_f.max(0.0) as usize;
                                    let width = ((end_col_f - start_col_f).ceil() as usize).max(1);
                                    let dist = if mouse_int >= start_col && mouse_int < start_col + width {
                                        0
                                    } else if mouse_int >= start_col.saturating_sub(1) && mouse_int < start_col + width + 1 {
                                        1
                                    } else {
                                        continue;
                                    };
                                    if best.is_none() || dist < best.unwrap().1 {
                                        best = Some((i, dist));
                                    }
                                }
                                viewport.hovered = best.map(|(i, _)| i);
                            } else {
                                viewport.hovered = None;
                            }
                        } else {
                            viewport.hovered = None;
                        }
                    }
                }
                self.update_timeline_viewport();
            }
        } else if viewport.panning {
            let dx_cols = col as i16 - viewport.pan_anchor_col as i16;
            let ns_per_col = viewport.view_width_ns / pa.width as f64;
            viewport.view_start_ns = viewport.pan_anchor_ns - dx_cols as f64 * ns_per_col;
            self.update_timeline_viewport();
        }
    }

    pub fn on_mouse_move(&mut self, col: u16, row: u16, full_area: ratatui::layout::Rect) {
        if self.focus != Focus::Chart {
            return;
        }
        let Some(ref viz) = self.prepared_viz else {
            return;
        };
        let VizData::Timeline(ref prepared) = viz.data else {
            return;
        };

        let is_tl = self.current_view_is_timeline();
        let is_md = self.current_view_is_metadata();
        let (_, _, right_chunks) = crate::ui::layout_chunks(full_area, is_tl, is_md, self.sql_mode);
        let chart_area = right_chunks[0];
        let inner = chart_area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
        let pa = timeline::plot_area(inner);

        let Some(ref mut viewport) = self.timeline_viewport else {
            return;
        };

        if col >= pa.x
            && col < pa.x + pa.width
            && row >= pa.y
            && row < pa.y + (prepared.stream_ids.len() as u16) * timeline::LANE_HEIGHT_ROWS
        {
            let mouse_int = (col - pa.x) as usize;
            let lane = timeline::row_to_lane(row, pa.y, prepared.stream_ids.len());
            if let Some(lane_idx) = lane {
                let target_stream = prepared.stream_ids[lane_idx];
                let mut best: Option<(usize, usize)> = None;
                for (i, e) in prepared.events.iter().enumerate() {
                    if e.stream_id != target_stream { continue; }
                    let start_col_f = timeline::ns_to_col(
                        e.start, viewport.view_start_ns, viewport.view_width_ns, pa.width,
                    );
                    let end_col_f = timeline::ns_to_col(
                        e.start + e.duration, viewport.view_start_ns, viewport.view_width_ns, pa.width,
                    );
                    let start_col = start_col_f.max(0.0) as usize;
                    let width = ((end_col_f - start_col_f).ceil() as usize).max(1);
                    let dist = if mouse_int >= start_col && mouse_int < start_col + width {
                        0
                    } else if mouse_int >= start_col.saturating_sub(1) && mouse_int < start_col + width + 1 {
                        1
                    } else {
                        continue;
                    };
                    if best.is_none() || dist < best.unwrap().1 {
                        best = Some((i, dist));
                    }
                }
                viewport.hovered = best.map(|(i, _)| i);
            } else {
                viewport.hovered = None;
            }
        } else {
            viewport.hovered = None;
        }
        self.update_timeline_viewport();
    }

    pub fn on_timeline_zoom_in(&mut self) {
        if let Some(ref mut vp) = self.timeline_viewport {
            let center = vp.view_start_ns + vp.view_width_ns / 2.0;
            vp.view_width_ns *= 0.5;
            vp.view_start_ns = center - vp.view_width_ns / 2.0;
            self.update_timeline_viewport();
        }
    }

    pub fn on_timeline_zoom_out(&mut self) {
        if let Some(ref mut vp) = self.timeline_viewport {
            let center = vp.view_start_ns + vp.view_width_ns / 2.0;
            vp.view_width_ns *= 2.0;
            vp.view_start_ns = center - vp.view_width_ns / 2.0;
            self.update_timeline_viewport();
        }
    }

    pub fn on_timeline_pan_left(&mut self) {
        if let Some(ref mut vp) = self.timeline_viewport {
            let step = vp.view_width_ns * 0.1;
            vp.view_start_ns -= step;
            self.update_timeline_viewport();
        }
    }

    pub fn on_timeline_pan_right(&mut self) {
        if let Some(ref mut vp) = self.timeline_viewport {
            let step = vp.view_width_ns * 0.1;
            vp.view_start_ns += step;
            self.update_timeline_viewport();
        }
    }

    pub fn on_timeline_reset_view(&mut self) {
        if let Some(ref viz) = &self.prepared_viz {
            if let VizData::Timeline(ref prepared) = viz.data {
                if let Some(ref mut vp) = self.timeline_viewport {
                    vp.view_start_ns = prepared.global_start_ns;
                    vp.view_width_ns = prepared.global_end_ns - prepared.global_start_ns;
                    vp.selection = None;
                    vp.hovered = None;
                    self.update_timeline_viewport();
                }
            }
        }
    }
}
