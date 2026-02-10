use anyhow::Result;
use std::path::PathBuf;
use tui_input::Input;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    FileSelection,
    TableView,
    StatsView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    FileInput,
    TableList,
    DataTable,
    Chart,
    StatsTable,
}

pub struct App {
    pub state: AppState,
    pub focus: Focus,
    pub file_input: Input,
    pub db_path: Option<PathBuf>,
    pub tables: Vec<String>,
    pub selected_table_index: usize,
    pub selected_table: Option<String>,
    pub table_data: Option<TableData>,
    pub table_scroll: usize,
    pub chart_scroll: usize,
    pub error_message: Option<String>,
    // Stats view data
    pub stats_rows: Vec<crate::stats::CudaApiAggregateRow>,
    pub stats_scroll: usize,
    pub stats_runtime_data: Option<TableData>,
}

#[derive(Debug, Clone)]
pub struct TableData {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::FileSelection,
            focus: Focus::FileInput,
            file_input: Input::default(),
            db_path: None,
            tables: Vec::new(),
            selected_table_index: 0,
            selected_table: None,
            table_data: None,
            table_scroll: 0,
            chart_scroll: 0,
            error_message: None,
            stats_rows: Vec::new(),
            stats_scroll: 0,
            stats_runtime_data: None,
        }
    }

    // Initialize the app with a database path directly (bypass file input)
    pub fn load_database(&mut self, path: &std::path::Path) -> Result<()> {
        if path.exists() {
            self.db_path = Some(path.to_path_buf());
            match crate::db::load_tables(path) {
                Ok(tables) => {
                    self.tables = tables;
                    self.state = AppState::TableView;
                    self.focus = Focus::TableList;
                    self.error_message = None;
                    if !self.tables.is_empty() {
                        self.selected_table_index = 0;
                        self.load_table_data()?;
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load database: {}", e));
                }
            }
        } else {
            self.error_message = Some("File does not exist".to_string());
        }
        Ok(())
    }

    pub fn is_inputting(&self) -> bool {
        self.state == AppState::FileSelection && self.focus == Focus::FileInput
    }

    pub fn on_up(&mut self) {
        match self.focus {
            Focus::TableList => {
                if self.selected_table_index > 0 {
                    self.selected_table_index -= 1;
                    let _ = self.load_table_data();
                }
            }
            Focus::DataTable => {
                if self.table_scroll > 0 {
                    self.table_scroll -= 1;
                }
            }
            _ => {}
        }
    }

    pub fn on_down(&mut self) {
        match self.focus {
            Focus::TableList => {
                if self.selected_table_index + 1 < self.tables.len() {
                    self.selected_table_index += 1;
                    let _ = self.load_table_data();
                }
            }
            Focus::DataTable => {
                if let Some(ref data) = self.table_data {
                    if self.table_scroll + 1 < data.rows.len() {
                        self.table_scroll += 1;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn on_left(&mut self) {
        if self.state == AppState::TableView {
            self.focus = Focus::TableList;
        }
    }

    pub fn on_right(&mut self) {
        if self.state == AppState::TableView && !self.tables.is_empty() {
            self.focus = Focus::DataTable;
        }
    }

    pub fn on_tab(&mut self) {
        if self.state == AppState::TableView {
            self.focus = match self.focus {
                Focus::TableList => Focus::Chart,
                Focus::Chart => Focus::DataTable,
                Focus::DataTable => Focus::TableList,
                _ => Focus::TableList,
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
        match self.state {
            AppState::FileSelection => {
                let path_str = self.file_input.value();
                if !path_str.is_empty() {
                    let path = PathBuf::from(path_str);
                    if path.exists() {
                        self.db_path = Some(path.clone());
                        match crate::db::load_tables(&path) {
                            Ok(tables) => {
                                self.tables = tables;
                                self.state = AppState::TableView;
                                self.focus = Focus::TableList;
                                self.error_message = None;
                                if !self.tables.is_empty() {
                                    self.selected_table_index = 0;
                                    self.load_table_data()?;
                                }
                            }
                            Err(e) => {
                                self.error_message =
                                    Some(format!("Failed to load database: {}", e));
                            }
                        }
                    } else {
                        self.error_message = Some("File does not exist".to_string());
                    }
                }
            }
            AppState::TableView => {}
            AppState::StatsView => {}
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
            Focus::TableList => self.on_down(),
            Focus::DataTable => {
                if let Some(ref data) = self.table_data {
                    if self.table_scroll + 1 < data.rows.len() {
                        self.table_scroll += 1;
                    }
                }
            }
            Focus::Chart => {
                self.chart_scroll = self.chart_scroll.saturating_add(1);
            }
            Focus::StatsTable => {
                if self.stats_scroll + 1 < self.stats_rows.len() {
                    self.stats_scroll += 1;
                }
            }
            _ => {}
        }
    }

    pub fn on_scroll_up(&mut self) {
        match self.focus {
            Focus::TableList => self.on_up(),
            Focus::DataTable => {
                if self.table_scroll > 0 {
                    self.table_scroll -= 1;
                }
            }
            Focus::Chart => {
                self.chart_scroll = self.chart_scroll.saturating_sub(1);
            }
            Focus::StatsTable => {
                if self.stats_scroll > 0 {
                    self.stats_scroll -= 1;
                }
            }
            _ => {}
        }
    }

    pub fn on_mouse_click(&mut self, x: u16, y: u16) -> Result<()> {
        if self.state == AppState::TableView {
            // Rough detection of which panel was clicked
            // This is simplified and would need adjustment based on actual layout
            let width = 100; // Approximate terminal width
            if x < width / 3 {
                self.focus = Focus::TableList;
            } else {
                if y < 20 {
                    self.focus = Focus::Chart;
                } else {
                    self.focus = Focus::DataTable;
                }
            }
        }
        Ok(())
    }

    fn load_table_data(&mut self) -> Result<()> {
        if let Some(ref db_path) = self.db_path {
            if let Some(table_name) = self.tables.get(self.selected_table_index) {
                self.selected_table = Some(table_name.clone());
                // Prefer resolved names when possible for better CUDA API labeling
                match crate::db::load_table_data_resolved(db_path, table_name) {
                    Ok(data) => {
                        self.table_data = Some(data);
                        self.table_scroll = 0;
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to load table: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn enter_stats_view(&mut self) -> Result<()> {
        if let Some(ref db_path) = self.db_path {
            match crate::stats::compute_cuda_api_aggregates(db_path, 50) {
                Ok(rows) => {
                    self.stats_rows = rows;
                    self.stats_scroll = 0;
                    match crate::db::load_table_data_resolved(
                        db_path,
                        "CUPTI_ACTIVITY_KIND_RUNTIME",
                    ) {
                        Ok(rt) => {
                            self.stats_runtime_data = Some(rt);
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Failed to load runtime: {}", e));
                            self.stats_runtime_data = None;
                        }
                    }
                    self.state = AppState::StatsView;
                    self.focus = Focus::Chart;
                    self.error_message = None;
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
            self.state = AppState::TableView;
            self.focus = Focus::TableList;
        }
    }
}
