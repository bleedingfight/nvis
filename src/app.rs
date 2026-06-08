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

#[derive(Debug, Clone, PartialEq)]
pub enum DbType {
    Nsys,
    Ncu,
}

pub struct App {
    pub state: AppState,
    pub focus: Focus,
    pub file_input: Input,
    pub db_path: Option<PathBuf>,
    pub db_type: DbType,
    pub ncu_conn: Option<rusqlite::Connection>,
    pub tables: Vec<String>,
    pub selected_table_index: usize,
    pub selected_table: Option<String>,
    pub table_data: Option<TableData>,
    pub table_scroll: usize,
    pub chart_scroll: usize,
    pub error_message: Option<String>,
    pub status_message: Option<String>, // For export success/failure
    // Stats view data (nsys)
    pub stats_rows: Vec<crate::stats::CudaApiAggregateRow>,
    pub stats_scroll: usize,
    pub stats_runtime_data: Option<TableData>,
    // Stats view data (ncu)
    pub ncu_stats_rows: Vec<crate::stats::NcuSpeedOfLightRow>,
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
            db_type: DbType::Nsys,
            ncu_conn: None,
            tables: Vec::new(),
            selected_table_index: 0,
            selected_table: None,
            table_data: None,
            table_scroll: 0,
            chart_scroll: 0,
            error_message: None,
            status_message: None,
            stats_rows: Vec::new(),
            stats_scroll: 0,
            stats_runtime_data: None,
            ncu_stats_rows: Vec::new(),
        }
    }

    // Initialize the app with a database path directly (bypass file input)
    pub fn load_database(&mut self, path: &std::path::Path) -> Result<()> {
        if !path.exists() {
            self.error_message = Some("File does not exist".to_string());
            return Ok(());
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "csv" {
            // NCU CSV path: parse and convert to in-memory SQLite
            match crate::ncu_csv::parse_ncu_csv(path) {
                Ok(ncu_data) => match crate::ncu_csv::csv_to_sqlite(&ncu_data) {
                    Ok(conn) => {
                        self.ncu_conn = Some(conn);
                        self.db_path = Some(path.to_path_buf());
                        self.db_type = DbType::Ncu;
                        let conn = self.ncu_conn.as_ref().unwrap();
                        match crate::db::load_tables_conn(conn) {
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
                                    Some(format!("Failed to load tables: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        self.error_message =
                            Some(format!("Failed to convert CSV to SQLite: {}", e));
                    }
                },
                Err(e) => {
                    self.error_message = Some(format!("Failed to parse NCU CSV: {}", e));
                }
            }
        } else {
            // SQLite path (nsys)
            self.db_path = Some(path.to_path_buf());
            self.db_type = DbType::Nsys;
            self.ncu_conn = None;
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
                    self.load_database(&path)?;
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
                if self.db_type == DbType::Ncu {
                    if self.stats_scroll + 1 < self.ncu_stats_rows.len() {
                        self.stats_scroll += 1;
                    }
                } else if self.stats_scroll + 1 < self.stats_rows.len() {
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
        if let Some(table_name) = self.tables.get(self.selected_table_index) {
            self.selected_table = Some(table_name.clone());

            let result = if self.db_type == DbType::Ncu {
                if let Some(ref conn) = self.ncu_conn {
                    crate::db::load_table_data_resolved_conn(conn, table_name)
                } else {
                    Err(anyhow::anyhow!("No NCU connection"))
                }
            } else if let Some(ref db_path) = self.db_path {
                crate::db::load_table_data_resolved(db_path, table_name)
            } else {
                Err(anyhow::anyhow!("No database loaded"))
            };

            match result {
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
        Ok(())
    }

    pub fn enter_stats_view(&mut self) -> Result<()> {
        if self.db_type == DbType::Ncu {
            // NCU: compute Speed of Light throughput comparison
            if let Some(ref conn) = self.ncu_conn {
                match crate::stats::compute_ncu_speed_of_light(conn) {
                    Ok(rows) => {
                        self.ncu_stats_rows = rows;
                        self.stats_scroll = 0;
                        self.state = AppState::StatsView;
                        self.focus = Focus::Chart;
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message =
                            Some(format!("Failed to compute NCU stats: {}", e));
                    }
                }
            }
        } else if let Some(ref db_path) = self.db_path {
            // NSYS: existing CUDA API aggregates logic
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

    pub fn export_current_table(&mut self) -> Result<()> {
        if let Some(ref table_name) = self.selected_table {
            let json = if self.db_type == DbType::Ncu {
                if let Some(ref conn) = self.ncu_conn {
                    crate::db::export_table_to_json_conn(conn, table_name)?
                } else {
                    anyhow::bail!("No NCU connection")
                }
            } else if let Some(ref db_path) = self.db_path {
                crate::db::export_table_to_json(db_path, table_name)?
            } else {
                anyhow::bail!("No database loaded")
            };

            let output_path = if let Some(ref db_path) = self.db_path {
                db_path.with_file_name(format!("{}.json", table_name))
            } else {
                PathBuf::from(format!("{}.json", table_name))
            };

            std::fs::write(&output_path, &json)?;

            self.status_message = Some(format!(
                "Exported '{}' to '{}'",
                table_name,
                output_path.display()
            ));
        }
        Ok(())
    }

    /// Export entire database to JSON file
    pub fn export_database(&mut self) -> Result<()> {
        let json = if self.db_type == DbType::Ncu {
            if let Some(ref conn) = self.ncu_conn {
                crate::db::export_database_to_json_conn(conn)?
            } else {
                anyhow::bail!("No NCU connection")
            }
        } else if let Some(ref db_path) = self.db_path {
            crate::db::export_database_to_json(db_path)?
        } else {
            anyhow::bail!("No database loaded")
        };

        let output_path = if let Some(ref db_path) = self.db_path {
            db_path.with_extension("json")
        } else {
            PathBuf::from("export.json")
        };

        std::fs::write(&output_path, &json)?;

        self.status_message = Some(format!(
            "Exported database to '{}'",
            output_path.display()
        ));
        Ok(())
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
