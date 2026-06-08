use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tui_input::Input;

use crate::core::backend::ProfilerBackend;
use crate::core::registry::Registry;
use crate::core::session::ProfilerSession;
use crate::core::types::{ProfilerData, ViewDescriptor};
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

    // View navigation
    pub views: Vec<ViewDescriptor>,
    pub selected_view_index: usize,
    pub view_data: Option<ProfilerData>,

    // UI scroll state
    pub table_scroll: usize,
    pub chart_scroll: usize,

    // Stats sub-view
    pub stats_data: Option<ProfilerData>,
    pub stats_scroll: usize,

    // Cached rendering
    pub prepared_viz: Option<PreparedVisualization>,
    pub active_renderer: Option<Arc<dyn VizRenderer>>,

    // Timeline interactive state
    pub timeline_viewport: Option<TimelineViewport>,
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
            views: Vec::new(),
            selected_view_index: 0,
            view_data: None,
            table_scroll: 0,
            chart_scroll: 0,
            stats_data: None,
            stats_scroll: 0,
            prepared_viz: None,
            active_renderer: None,
            timeline_viewport: None,
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

    pub fn on_up(&mut self) {
        match self.focus {
            Focus::ViewList => {
                if self.selected_view_index > 0 {
                    self.selected_view_index -= 1;
                    let _ = self.load_view_data();
                }
            }
            Focus::DataTable
                if self.table_scroll > 0 => {
                    self.table_scroll -= 1;
                }
            _ => {}
        }
    }

    pub fn on_down(&mut self) {
        match self.focus {
            Focus::ViewList => {
                if self.selected_view_index + 1 < self.views.len() {
                    self.selected_view_index += 1;
                    let _ = self.load_view_data();
                }
            }
            Focus::DataTable => {
                if let Some(ref data) = self.view_data {
                    if self.table_scroll + 1 < data.row_count {
                        self.table_scroll += 1;
                    }
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
            self.focus = Focus::DataTable;
        }
    }

    pub fn on_tab(&mut self) {
        if self.state == AppState::ViewBrowser {
            self.focus = match self.focus {
                Focus::ViewList => Focus::Chart,
                Focus::Chart => Focus::DataTable,
                Focus::DataTable => Focus::ViewList,
                _ => Focus::ViewList,
            };
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
        }
        Ok(())
    }

    pub fn on_char(&mut self, c: char) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input
                .handle(tui_input::InputRequest::InsertChar(c));
        }
    }

    pub fn on_backspace(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input
                .handle(tui_input::InputRequest::DeletePrevChar);
        }
    }

    pub fn on_delete(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input
                .handle(tui_input::InputRequest::DeleteNextChar);
        }
    }

    pub fn on_input_left(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input
                .handle(tui_input::InputRequest::GoToPrevChar);
        }
    }

    pub fn on_input_right(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
            self.file_input
                .handle(tui_input::InputRequest::GoToNextChar);
        }
    }

    pub fn on_input_ctrl_a(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToStart);
        }
    }

    pub fn on_input_ctrl_e(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToEnd);
        }
    }

    pub fn on_input_ctrl_u(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeleteLine);
        }
    }

    pub fn on_input_ctrl_k(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeleteTillEnd);
        }
    }

    pub fn on_input_ctrl_w(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeletePrevWord);
        }
    }

    pub fn on_input_alt_b(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToPrevWord);
        }
    }

    pub fn on_input_alt_f(&mut self) {
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToNextWord);
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
                if let Some(ref data) = self.view_data {
                    if self.table_scroll + 1 < data.row_count {
                        self.table_scroll += 1;
                    }
                }
            }
            Focus::Chart => {
                self.chart_scroll = self.chart_scroll.saturating_add(1);
                self.refresh_visualization();
            }
            Focus::StatsTable => {
                if let Some(ref data) = self.stats_data {
                    if self.stats_scroll + 1 < data.row_count {
                        self.stats_scroll += 1;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn on_scroll_up(&mut self) {
        match self.focus {
            Focus::ViewList => self.on_up(),
            Focus::DataTable => {
                if self.table_scroll > 0 {
                    self.table_scroll -= 1;
                }
            }
            Focus::Chart => {
                self.chart_scroll = self.chart_scroll.saturating_sub(1);
                self.refresh_visualization();
            }
            Focus::StatsTable
                if self.stats_scroll > 0 => {
                    self.stats_scroll -= 1;
                }
            _ => {}
        }
    }

    pub fn on_mouse_click(&mut self, x: u16, y: u16, area: ratatui::layout::Rect) -> Result<()> {
        if self.state == AppState::ViewBrowser {
            let main_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(25),
                    ratatui::layout::Constraint::Percentage(75),
                ])
                .split(area);

            if x < main_chunks[0].x + main_chunks[0].width {
                self.focus = Focus::ViewList;
            } else {
                let right_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(40),
                        ratatui::layout::Constraint::Percentage(60),
                    ])
                    .split(main_chunks[1]);

                if y < right_chunks[0].y + right_chunks[0].height {
                    self.focus = Focus::Chart;
                } else {
                    self.focus = Focus::DataTable;
                }
            }
        }
        Ok(())
    }

    pub fn enter_stats_view(&mut self) -> Result<()> {
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
                    self.chart_scroll = 0;
                    self.error_message = None;
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
        _chart_area: ratatui::layout::Rect,
    ) {
        if self.focus != Focus::Chart {
            return;
        }
        let Some(ref mut viewport) = self.timeline_viewport else {
            return;
        };

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

    pub fn on_mouse_up(&mut self, button: crossterm::event::MouseButton) {
        if self.focus != Focus::Chart {
            return;
        }
        let Some(ref mut viewport) = self.timeline_viewport else {
            return;
        };

        match button {
            crossterm::event::MouseButton::Left => {
                viewport.drag_origin = None;
            }
            crossterm::event::MouseButton::Right | crossterm::event::MouseButton::Middle => {
                viewport.panning = false;
            }
        }
    }

    pub fn on_mouse_drag(
        &mut self,
        button: crossterm::event::MouseButton,
        col: u16,
        _row: u16,
        chart_area: ratatui::layout::Rect,
    ) {
        if self.focus != Focus::Chart {
            return;
        }
        let Some(ref mut viewport) = self.timeline_viewport else {
            return;
        };

        if button == crossterm::event::MouseButton::Left {
            if let Some((origin_col, _origin_row)) = viewport.drag_origin {
                let inner = chart_area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 1,
                });
                let pa = timeline::plot_area(inner);
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
                self.update_timeline_viewport();
            }
        } else if viewport.panning {
            let inner = chart_area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 1,
            });
            let pa = timeline::plot_area(inner);
            let dx_cols = col as i16 - viewport.pan_anchor_col as i16;
            let ns_per_col = viewport.view_width_ns / pa.width as f64;
            viewport.view_start_ns = viewport.pan_anchor_ns - dx_cols as f64 * ns_per_col;
            self.update_timeline_viewport();
        }
    }

    pub fn on_mouse_move(&mut self, col: u16, row: u16, chart_area: ratatui::layout::Rect) {
        if self.focus != Focus::Chart {
            return;
        }
        let Some(ref viz) = self.prepared_viz else {
            return;
        };
        let VizData::Timeline(ref prepared) = viz.data else {
            return;
        };
        let Some(ref mut viewport) = self.timeline_viewport else {
            return;
        };

        let inner = chart_area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 1,
        });
        let pa = timeline::plot_area(inner);

        if col >= pa.x
            && col < pa.x + pa.width
            && row >= pa.y
            && row < pa.y + (prepared.stream_ids.len() as u16) * timeline::LANE_HEIGHT_ROWS
        {
            let ns = timeline::col_to_ns(
                (col - pa.x) as f64,
                viewport.view_start_ns,
                viewport.view_width_ns,
                pa.width,
            );
            let lane = timeline::row_to_lane(row, pa.y, prepared.stream_ids.len());
            if let Some(lane_idx) = lane {
                let target_stream = prepared.stream_ids[lane_idx];
                viewport.hovered = prepared.events.iter().enumerate()
                    .filter(|(_, e)| e.stream_id == target_stream)
                    .find(|(_, e)| e.start <= ns && e.start + e.duration >= ns)
                    .map(|(i, _)| i);
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
            vp.view_width_ns *= 0.7;
            vp.view_start_ns = center - vp.view_width_ns / 2.0;
            self.update_timeline_viewport();
        }
    }

    pub fn on_timeline_zoom_out(&mut self) {
        if let Some(ref mut vp) = self.timeline_viewport {
            let center = vp.view_start_ns + vp.view_width_ns / 2.0;
            vp.view_width_ns *= 1.4;
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
