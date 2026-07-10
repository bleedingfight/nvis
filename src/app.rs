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
use crate::source::browser::{BrowserNode, DirEntry, EntryKind};
use crate::source::{FetchedFile, SourceRegistry};
use crate::theme::Theme;
use crate::viz::renderer::VizRenderer;
use crate::viz::types::{PreparedVisualization, TimelineViewport, VizData};
use crate::viz::timeline;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    FileSelection,
    ViewBrowser,
    StatsView,
    Summary,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    BrowserTree,
    CloudDialog,
    FileInput,
    ViewList,
    DataTable,
    Chart,
    StatsTable,
    SqlInput,
    SqlResult,
    SummaryChart,
}

/// Which cloud protocol the dialog is collecting params for.
/// Re-exported from `credentials` so the dialog, browser nodes, and the
/// connection store all share one tag.
pub use crate::credentials::Protocol as CloudDialogType;

use crate::credentials::{
    CloudConnection, ConnectionFields, Protocol, add_connection, find_connection,
    load_connections, remove_connection, update_connection,
};

/// A single input field inside the cloud dialog.
pub struct CloudField {
    pub label: String,
    pub input: Input,
    /// When true the value is rendered as `***` (passwords / secret keys).
    pub is_secret: bool,
}

impl CloudField {
    fn new(label: &str, default: &str) -> Self {
        let mut input = Input::default();
        if !default.is_empty() {
            input = input.with_value(default.to_string());
        }
        Self {
            label: label.to_string(),
            input,
            is_secret: false,
        }
    }

    /// Secret field — value is masked in the UI but still editable & submitted.
    fn new_secret(label: &str, default: &str) -> Self {
        let mut f = Self::new(label, default);
        f.is_secret = true;
        f
    }

    /// Render value, masked with `*` if this is a secret field.
    pub fn display_value(&self) -> String {
        if self.is_secret {
            "*".repeat(self.input.value().chars().count())
        } else {
            self.input.value().to_string()
        }
    }
}

/// State for an in-flight background download, shared between the main thread
/// (progress display) and the worker thread.
pub struct DownloadState {
    pub label: String,
    pub dest: PathBuf,
    pub total: Option<u64>,
    pub started: Instant,
    pub done: Arc<std::sync::atomic::AtomicBool>,
    pub error: Arc<std::sync::Mutex<Option<String>>>,
    pub source_uri: String,
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

    // File source infrastructure (local / ssh / webdav / …)
    pub source_registry: SourceRegistry,
    pub fetched_files: Vec<FetchedFile>,

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

    // Summary page state
    pub summary_prepared: Option<crate::viz::summary_data::SummaryPrepared>,
    pub summary_viewport: Option<crate::viz::summary_data::SummaryViewport>,

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

    // Browser state (FileSelection screen)
    pub browser_nodes: Vec<BrowserNode>,
    pub browser_expanded: HashSet<String>,
    pub browser_selected: usize,
    pub browser_scroll: usize,

    // Cloud dialog state (modal overlay)
    pub cloud_dialog_active: bool,
    pub cloud_dialog_type: CloudDialogType,
    pub cloud_dialog_fields: Vec<CloudField>,
    pub cloud_dialog_selected: usize,
    /// None = creating a new connection; Some((protocol, name)) = editing.
    pub cloud_dialog_edit: Option<(Protocol, String)>,
    /// Pending delete confirmation: a second `x` confirms.
    pub pending_delete: Option<(Protocol, String)>,

    // Background download state (remote files → ~/Downloads/nvis)
    pub download_state: Option<DownloadState>,
    pub downloaded_bytes: u64,
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
        let mut source_registry = SourceRegistry::new();
        crate::source::register_all(&mut source_registry);
        Self {
            state: AppState::FileSelection,
            focus: Focus::BrowserTree,
            file_input: Input::default(),
            error_message: None,
            registry,
            backend: None,
            session: None,
            source_registry,
            fetched_files: Vec::new(),
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
            summary_prepared: None,
            summary_viewport: None,
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
            browser_nodes: Self::initial_tree(),
            browser_expanded: HashSet::new(),
            browser_selected: 0,
            browser_scroll: 0,
            cloud_dialog_active: false,
            cloud_dialog_type: CloudDialogType::Ssh,
            cloud_dialog_fields: Vec::new(),
            cloud_dialog_selected: 0,
            cloud_dialog_edit: None,
            pending_delete: None,
            download_state: None,
            downloaded_bytes: 0,
        }
    }

    pub fn load_database(&mut self, path: &std::path::Path) -> Result<()> {
        log::info!("Loading database: {:?}", path);

        // Only one download at a time.
        if self.download_state.is_some() {
            self.error_message = Some("已有下载进行中，请等待完成".into());
            return Ok(());
        }

        // Clean up any previously-fetched remote files before loading a new one.
        self.fetched_files.clear();

        let path_str = path.to_string_lossy().to_string();
        let is_remote = self
            .source_registry
            .resolve(&path_str)
            .map(|s| s.id() != "local")
            .unwrap_or(false);

        // Local path: open directly.
        if !is_remote {
            return self.finish_load_local(path.to_path_buf(), path_str, false);
        }

        // Remote: download to ~/Downloads/nvis/<basename>, reusing a cached copy.
        let Some(downloads_dir) = dirs::download_dir() else {
            // No Downloads dir — fall back to synchronous temp fetch.
            return self.fetch_and_finish_sync(&path_str);
        };
        let basename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "remote-file".to_string());
        let dest_dir = downloads_dir.join("nvis");
        let _ = std::fs::create_dir_all(&dest_dir);
        let dest = dest_dir.join(&basename);

        if dest.exists() {
            log::info!("Reusing cached copy at {:?}", dest);
            return self.finish_load_local(dest, path_str, true);
        }

        // No cache → background download with progress.
        self.start_download(path_str, basename, dest);
        Ok(())
    }

    /// Synchronous fetch fallback (no Downloads dir). Downloads to a temp path
    /// then loads it; UI blocks during download (legacy behaviour).
    fn fetch_and_finish_sync(&mut self, uri: &str) -> Result<()> {
        let fetched = match self.source_registry.fetch(uri) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Source fetch failed: {}", e);
                self.error_message = Some(format!("Failed to open: {}", e));
                return Ok(());
            }
        };
        let local = fetched.local_path.clone();
        self.fetched_files.push(fetched);
        self.finish_load_local(local, uri.to_string(), false)
    }

    /// Spawn a background thread to download `uri` → `dest`, recording a
    /// `DownloadState` so the UI can show progress. Returns immediately.
    fn start_download(&mut self, uri: String, label: String, dest: PathBuf) {
        let total = self.source_registry.head_size(&uri);
        let source = match self.source_registry.resolve(&uri) {
            Ok(s) => s,
            Err(e) => {
                self.error_message = Some(format!("Failed to resolve source: {}", e));
                return;
            }
        };
        let dest_clone = dest.clone();
        let uri_clone = uri.clone();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let error: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
        let done_t = Arc::clone(&done);
        let error_t = Arc::clone(&error);

        std::thread::spawn(move || {
            let res = source.fetch_to(&uri_clone, &dest_clone);
            done_t.store(true, std::sync::atomic::Ordering::SeqCst);
            match res {
                Ok(mut f) => {
                    // The downloaded file lives at ~/Downloads/nvis and must
                    // persist for the main thread to open it. The FetchedFile
                    // defaults to cleanup-on-drop (it was built for temp files),
                    // so disable cleanup here or drop would delete the file we
                    // just finished downloading.
                    f.should_cleanup = false;
                    drop(f); // nothing to clean
                }
                Err(e) => {
                    // Clean up a partial file.
                    let _ = std::fs::remove_file(&dest_clone);
                    if let Ok(mut g) = error_t.lock() {
                        *g = Some(format!("{}", e));
                    }
                }
            }
        });

        self.download_state = Some(DownloadState {
            label,
            dest,
            total,
            started: Instant::now(),
            done,
            error,
            source_uri: uri,
        });
        self.downloaded_bytes = 0;
        log::info!("Background download started -> {:?}", self.download_state.as_ref().unwrap().dest);
    }

    /// Called each frame by the main loop while a download is active: stat the
    /// destination for live bytes, and finish (or report error) once done.
    pub fn tick_download(&mut self) {
        let Some(ds) = self.download_state.as_ref() else {
            return;
        };
        // Live progress from the file size.
        self.downloaded_bytes = std::fs::metadata(&ds.dest).map(|m| m.len()).unwrap_or(0);

        if ds.done.load(std::sync::atomic::Ordering::SeqCst) {
            let uri = ds.source_uri.clone();
            let dest = ds.dest.clone();
            let error = ds.error.lock().map(|g| g.clone()).ok().flatten();
            // Take the state out before finishing (finish_load_local may mutate app).
            self.download_state = None;
            self.downloaded_bytes = 0;
            match error {
                Some(e) => {
                    self.error_message = Some(format!("下载失败: {}", e));
                }
                None => {
                    if self.state == AppState::FileSelection {
                        // User still on the file screen → load now.
                        let _ = self.finish_load_local(dest, uri, true);
                    } else {
                        // User navigated away → just notify; cached copy is on disk.
                        self.error_message =
                            Some(format!("下载完成，重新打开 {} 即可查看", dest.display()));
                    }
                }
            }
        }
    }

    /// Open a local file with the profiler backend: detect backend, open
    /// session, list views, switch to ViewBrowser. Reused by local opens,
    /// cached remote copies, and completed downloads.
    ///
    /// `cached_remote` = true when this path is a downloaded copy under
    /// ~/Downloads/nvis; on backend failure such a file is likely a corrupt
    /// download, so we remove it (so the next attempt re-downloads) and give a
    /// clearer error.
    fn finish_load_local(&mut self, path: PathBuf, uri: String, cached_remote: bool) -> Result<()> {
        log::info!("Loading local file: {:?}", path);

        if !path.exists() {
            log::warn!("File does not exist: {:?}", path);
            self.error_message = Some("File does not exist".to_string());
            return Ok(());
        }

        // Remember the file so FetchedFile::drop cleans temp files (none here,
        // but keeps fetched_files consistent).
        self.fetched_files.push(FetchedFile::local(path.clone(), uri.clone()));

        let backend = match self.registry.detect_backend(&path) {
            Ok(b) => {
                log::info!("Detected backend: {:?}", b.name());
                b
            }
            Err(e) => {
                log::error!("Backend detection failed: {}", e);
                let msg = if cached_remote {
                    let _ = std::fs::remove_file(&path);
                    "下载的文件无法解析（可能传输损坏）。已删除损坏副本，请重试或换源/用网盘客户端下载".to_string()
                } else {
                    format!("Unsupported file: {}", e)
                };
                self.error_message = Some(msg);
                return Ok(());
            }
        };

        self.backend = Some(backend.clone());

        match backend.open(&path) {
            Ok(session) => {
                self.session = Some(session);
                let session_ref = self.session.as_ref().unwrap().as_ref();
                match backend.list_views(session_ref) {
                    Ok(views) => {
                        log::info!("Listed {} views", views.len());
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
                        log::error!("Failed to list views: {}", e);
                        self.error_message = Some(format!("Failed to list views: {}", e));
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to open file: {}", e);
                let msg = if cached_remote {
                    let _ = std::fs::remove_file(&path);
                    format!("下载的文件损坏，无法打开（{}）。已删除损坏副本", e)
                } else {
                    format!("Failed to open file: {}", e)
                };
                self.error_message = Some(msg);
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
        } else if self.state == AppState::Summary {
            if self.focus == Focus::SummaryChart {
                self.fullscreen = Some(Focus::SummaryChart);
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
                d.clone()
                 .without_all_null_columns()
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
        log::info!("Executing SQL: {}", sql);
        if let (Some(ref backend), Some(ref session)) = (&self.backend, &self.session) {
            match backend.execute_sql(session.as_ref(), &sql) {
                Ok(data) => {
                    log::info!("SQL returned {} rows", data.row_count);
                    self.sql_result = Some(data);
                    self.sql_error = None;
                    self.table_scroll = 0;
                    self.invalidate_display_cache();
                }
                Err(e) => {
                    log::error!("SQL error: {}", e);
                    self.sql_error = Some(format!("{}", e));
                }
            }
        }
    }

    pub fn on_up(&mut self) {
        match self.focus {
            Focus::CloudDialog => self.cloud_dialog_field_up(),
            Focus::BrowserTree => self.browser_tree_up(),
            Focus::ViewList => {
                if self.views.is_empty() { return; }
                if self.selected_view_index > 0 {
                    self.selected_view_index -= 1;
                } else {
                    self.selected_view_index = self.views.len() - 1;
                }
                let _ = self.load_view_data();
            }
            Focus::DataTable | Focus::StatsTable => {
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
            Focus::CloudDialog => self.cloud_dialog_field_down(),
            Focus::BrowserTree => self.browser_tree_down(),
            Focus::ViewList => {
                if self.views.is_empty() { return; }
                if self.selected_view_index + 1 < self.views.len() {
                    self.selected_view_index += 1;
                } else {
                    self.selected_view_index = 0;
                }
                let _ = self.load_view_data();
            }
            Focus::DataTable | Focus::StatsTable => {
                if let Some(data) = self.cached_display_data.as_ref() {
                    if data.row_count == 0 { return; }
                    self.table_scroll = if self.table_scroll + 1 < data.row_count { self.table_scroll + 1 } else { 0 };
                }
            }
            _ => {}
        }
    }

    pub fn on_left(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::BrowserTree {
            let _ = self.browser_tree_left();
        } else if self.state == AppState::ViewBrowser {
            self.focus = Focus::ViewList;
        }
    }

    pub fn on_right(&mut self) {
        if self.state == AppState::FileSelection && self.focus == Focus::BrowserTree {
            let _ = self.browser_enter();
        } else if self.state == AppState::ViewBrowser && !self.views.is_empty() {
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
        if self.state == AppState::FileSelection {
            if self.focus == Focus::CloudDialog {
                // Cycle fields + the trailing Connect button.
                let n = self.cloud_dialog_fields.len() + 1;
                if n > 1 {
                    self.cloud_dialog_selected = (self.cloud_dialog_selected + 1) % n;
                }
            } else {
                self.focus = match self.focus {
                    Focus::BrowserTree => Focus::FileInput,
                    Focus::FileInput => Focus::BrowserTree,
                    _ => Focus::BrowserTree,
                };
            }
        } else if self.state == AppState::ViewBrowser {
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
        } else if self.state == AppState::Summary {
            self.focus = Focus::SummaryChart;
        }
    }

    pub fn on_enter(&mut self) -> Result<()> {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if self.cloud_dialog_selected >= self.cloud_dialog_fields.len() {
                // The Connect button — confirm.
                return self.confirm_cloud_dialog();
            } else {
                // A field — move to the next item (button is last).
                self.cloud_dialog_field_down();
                return Ok(());
            }
        }
        if self.state == AppState::FileSelection {
            match self.focus {
                Focus::FileInput => {
                    let path_str = self.file_input.value();
                    if !path_str.is_empty() {
                        self.load_database(&PathBuf::from(path_str))?;
                    }
                }
                Focus::BrowserTree => self.browser_enter()?,
                _ => {}
            }
        } else if self.is_sql_inputting() {
            self.execute_current_sql();
        }
        Ok(())
    }

    pub fn on_char(&mut self, c: char) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::InsertChar(c));
            }
            return;
        }
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::InsertChar(c));
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::InsertChar(c));
        }
    }

    pub fn on_backspace(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::DeletePrevChar);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::DeletePrevChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::DeletePrevChar);
        }
    }

    pub fn on_delete(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::DeleteNextChar);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::DeleteNextChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::DeleteNextChar);
        }
    }

    pub fn on_input_left(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::GoToPrevChar);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::GoToPrevChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::GoToPrevChar);
        }
    }

    pub fn on_input_right(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::GoToNextChar);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input
                .handle(tui_input::InputRequest::GoToNextChar);
        } else if self.is_sql_inputting() {
            self.sql_input
                .handle(tui_input::InputRequest::GoToNextChar);
        }
    }

    pub fn on_input_ctrl_a(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::GoToStart);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToStart);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToStart);
        }
    }

    pub fn on_input_ctrl_e(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::GoToEnd);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToEnd);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToEnd);
        }
    }

    pub fn on_input_ctrl_u(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::DeleteLine);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeleteLine);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::DeleteLine);
        }
    }

    pub fn on_input_ctrl_k(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::DeleteTillEnd);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeleteTillEnd);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::DeleteTillEnd);
        }
    }

    pub fn on_input_ctrl_w(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::DeletePrevWord);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::DeletePrevWord);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::DeletePrevWord);
        }
    }

    pub fn on_input_alt_b(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::GoToPrevWord);
            }
            return;
        }
        if self.is_inputting() {
            self.file_input.handle(tui_input::InputRequest::GoToPrevWord);
        } else if self.is_sql_inputting() {
            self.sql_input.handle(tui_input::InputRequest::GoToPrevWord);
        }
    }

    pub fn on_input_alt_f(&mut self) {
        if self.focus == Focus::CloudDialog && self.cloud_dialog_active {
            if let Some(field) = self.cloud_dialog_fields.get_mut(self.cloud_dialog_selected) {
                field.input.handle(tui_input::InputRequest::GoToNextWord);
            }
            return;
        }
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

    pub fn enter_summary(&mut self) -> Result<()> {
        log::info!("Entering summary page, backend={}, session={}",
            self.backend.is_some(), self.session.is_some());
        if self.sql_mode {
            self.close_sql_mode();
        }
        if self.backend.is_none() || self.session.is_none() {
            log::warn!("Summary: no database loaded");
            self.error_message = Some("No database loaded".into());
            return Ok(());
        }
        let session = self.session.as_ref().unwrap();
        let nsys_sess = session.as_any().downcast_ref::<crate::backends::nsys::NsysSession>();
        log::info!("Summary: downcast to NsysSession = {}", nsys_sess.is_some());
        if let Some(sess) = nsys_sess {
            log::info!("Summary: acquiring DB lock...");
            let conn = sess.conn()?;
            log::info!("Summary: DB lock acquired, querying events...");
            let events = crate::backends::nsys::summary::build_summary_events(&conn)?;
            log::info!("Summary: {} events loaded", events.len());
            if events.is_empty() {
                self.error_message = Some("No GPU/CPU events found in database".into());
                return Ok(());
            }
            let prepared = crate::viz::summary_data::SummaryPrepared::from_events(events);
            log::info!("Summary: {} GPU lanes, {} CPU lanes, range {:.0}-{:.0} ns",
                prepared.gpu_lanes.len(), prepared.cpu_lanes.len(),
                prepared.global_start_ns, prepared.global_end_ns);
            let vp = crate::viz::summary_data::SummaryViewport {
                view_start_ns: prepared.global_start_ns,
                view_width_ns: prepared.global_end_ns - prepared.global_start_ns,
                ..Default::default()
            };
            self.summary_prepared = Some(prepared);
            self.summary_viewport = Some(vp);
            self.state = AppState::Summary;
            self.focus = Focus::SummaryChart;
            self.fullscreen = None;
            self.error_message = None;
        } else {
            log::warn!("Summary: non-nsys backend, cannot enter");
            self.error_message = Some("Summary requires an nsys database".into());
        }
        Ok(())
    }

    pub fn leave_summary(&mut self) {
        if self.state == AppState::Summary {
            log::info!("Leaving summary page");
            self.state = AppState::ViewBrowser;
            self.focus = Focus::ViewList;
            self.summary_prepared = None;
            self.summary_viewport = None;
        }
    }

    pub fn on_summary_zoom_in(&mut self) {
        if let Some(ref mut vp) = self.summary_viewport {
            if let Some(ref prep)= self.summary_prepared {
                let total_ns = prep.global_end_ns - prep.global_start_ns;
                let min_width = (total_ns / 5000.0).max(10.0);
                let center = vp.view_start_ns + vp.view_width_ns / 2.0;
                vp.view_width_ns = (vp.view_width_ns * 0.5).max(min_width);
                vp.view_start_ns = center - vp.view_width_ns / 2.0;
                vp.view_start_ns = vp.view_start_ns.max(prep.global_start_ns);
                vp.view_start_ns = vp.view_start_ns.min(prep.global_end_ns - vp.view_width_ns);
                log::info!("Summary zoom in: start={:.0} width={:.0}ns", vp.view_start_ns, vp.view_width_ns);
            }
        }
    }

    pub fn on_summary_zoom_out(&mut self) {
        if let Some(ref mut vp) = self.summary_viewport {
            if let Some(ref prep) = self.summary_prepared {
                let total_ns = prep.global_end_ns - prep.global_start_ns;
                let center = vp.view_start_ns + vp.view_width_ns / 2.0;
                vp.view_width_ns = (vp.view_width_ns * 2.0).min(total_ns);
                vp.view_start_ns = center - vp.view_width_ns / 2.0;
                vp.view_start_ns = vp.view_start_ns.max(prep.global_start_ns);
                vp.view_start_ns = vp.view_start_ns.min(prep.global_end_ns - vp.view_width_ns);
                log::info!("Summary zoom out: start={:.0} width={:.0}ns", vp.view_start_ns, vp.view_width_ns);
            }
        }
    }

    pub fn on_summary_pan_left(&mut self) {
        if let Some(ref mut vp) = self.summary_viewport {
            if let Some(ref prep) = self.summary_prepared {
                let step = vp.view_width_ns * 0.2;
                vp.view_start_ns = (vp.view_start_ns - step).max(prep.global_start_ns);
            }
        }
    }

    pub fn on_summary_pan_right(&mut self) {
        if let Some(ref mut vp) = self.summary_viewport {
            if let Some(ref prep) = self.summary_prepared {
                let step = vp.view_width_ns * 0.2;
                let max_start = prep.global_end_ns - vp.view_width_ns;
                vp.view_start_ns = (vp.view_start_ns + step).min(max_start.max(prep.global_start_ns));
            }
        }
    }

    pub fn on_summary_reset_view(&mut self) {
        if let Some(ref mut vp) = self.summary_viewport {
            if let Some(ref prep) = self.summary_prepared {
                vp.view_start_ns = prep.global_start_ns;
                vp.view_width_ns = prep.global_end_ns - prep.global_start_ns;
                vp.selection = None;
                vp.hovered = None;
            }
        }
    }

    fn load_view_data(&mut self) -> Result<()> {
        if let (Some(ref backend), Some(ref session), Some(view)) =
            (&self.backend, &self.session, self.views.get(self.selected_view_index))
        {
            log::info!("Loading view: {} ({})", view.display_name, view.id);
            match backend.get_view_data(session.as_ref(), &view.id) {
                Ok(data) => {
                    let rows = data.row_count;
                    let cols = data.schema.len();
                    self.view_data = Some(data);
                    log::info!("View {} loaded: {} rows, {} cols", view.id, rows, cols);
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
                    log::error!("Failed to load view {}: {}", view.id, e);
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

    // ── Browser tree methods ──────────────────────────────────────

    fn initial_tree() -> Vec<BrowserNode> {
        vec![
            BrowserNode::Category {
                label: "Local".into(),
                id: "local".into(),
                depth: 0,
            },
            BrowserNode::Category {
                label: "Cloud Storage".into(),
                id: "cloud".into(),
                depth: 0,
            },
        ]
    }

    pub fn browser_tree_up(&mut self) {
        if self.browser_selected > 0 {
            self.browser_selected -= 1;
            self.clear_pending_delete_if_stale();
        }
    }

    pub fn browser_tree_down(&mut self) {
        if self.browser_selected + 1 < self.browser_nodes.len() {
            self.browser_selected += 1;
            self.clear_pending_delete_if_stale();
        }
    }

    /// Enter/expand the currently selected tree node, or populate file list.
    pub fn browser_enter(&mut self) -> Result<()> {
        if self.browser_selected >= self.browser_nodes.len() {
            return Ok(());
        }
        let node = self.browser_nodes[self.browser_selected].clone();

        match &node {
            BrowserNode::File { uri, .. } => {
                self.load_database(&PathBuf::from(uri))?;
            }
            BrowserNode::NewConn { protocol, .. } => {
                // "+ 新建连接" — open the dialog in create mode.
                self.open_cloud_dialog(*protocol, None);
            }
            _ => {
                let node_id = node.id();
                if !self.browser_expanded.contains(&node_id) {
                    self.expand_node(node)?;
                }
            }
        }

        Ok(())
    }

    /// Collapse or go to parent.
    pub fn browser_tree_left(&mut self) -> Result<()> {
        if self.browser_selected >= self.browser_nodes.len() {
            return Ok(());
        }
        let node_id = self.browser_nodes[self.browser_selected].id();
        if self.browser_expanded.contains(&node_id) {
            self.collapse_node(&node_id);
        }
        Ok(())
    }

    // ── Cloud connection dialog ──────────────────────────────────

    /// Open the connection dialog. `edit` = None creates a new connection;
    /// Some((protocol, name)) edits an existing saved connection.
    fn open_cloud_dialog(&mut self, protocol: Protocol, edit: Option<(Protocol, String)>) {
        self.cloud_dialog_type = protocol;
        self.cloud_dialog_active = true;
        self.cloud_dialog_edit = edit.clone();
        self.cloud_dialog_selected = 0;
        self.focus = Focus::CloudDialog;

        let user = whoami::username();
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("/home/{}", user));

        // In edit mode, prefill from the saved connection.
        let saved = edit
            .as_ref()
            .and_then(|(p, n)| find_connection(*p, n))
            .map(|c| ConnectionFields::from(&c));

        self.cloud_dialog_fields = match protocol {
            Protocol::Ssh => {
                let s = saved.as_ref();
                vec![
                    CloudField::new("Name", edit.as_ref().map(|(_, n)| n.as_str()).unwrap_or("")),
                    CloudField::new("Host", s.map(|s| s.host.as_str()).unwrap_or("")),
                    CloudField::new("Port", s.map(|s| s.port.as_str()).unwrap_or("22")),
                    CloudField::new("User", s.map(|s| s.user.as_str()).unwrap_or(&user)),
                    CloudField::new_secret("Password", s.map(|s| s.pass.as_str()).unwrap_or("")),
                    CloudField::new("Path", s.map(|s| s.path.as_str()).unwrap_or(&format!("{}/", home))),
                ]
            }
            Protocol::WebDav => {
                let s = saved.as_ref();
                vec![
                    CloudField::new("Name", edit.as_ref().map(|(_, n)| n.as_str()).unwrap_or("")),
                    CloudField::new("URL", s.map(|s| s.url.as_str()).unwrap_or("https://")),
                    CloudField::new("User", s.map(|s| s.user.as_str()).unwrap_or("")),
                    CloudField::new_secret("Password", s.map(|s| s.pass.as_str()).unwrap_or("")),
                ]
            }
            Protocol::S3 => {
                let s = saved.as_ref();
                vec![
                    CloudField::new("Name", edit.as_ref().map(|(_, n)| n.as_str()).unwrap_or("")),
                    CloudField::new("Bucket", s.map(|s| s.bucket.as_str()).unwrap_or("")),
                    CloudField::new("Region", s.map(|s| s.region.as_str()).unwrap_or("us-east-1")),
                    CloudField::new("Endpoint", s.map(|s| s.endpoint.as_str()).unwrap_or("")),
                    CloudField::new("Access Key", s.map(|s| s.access_key.as_str()).unwrap_or("")),
                    CloudField::new_secret("Secret Key", s.map(|s| s.secret_key.as_str()).unwrap_or("")),
                ]
            }
            Protocol::Ks3 => {
                let s = saved.as_ref();
                let (endpoint, ak, sk) = if let Some(s) = s {
                    (s.endpoint.clone(), s.access_key.clone(), s.secret_key.clone())
                } else {
                    let config = crate::source::s3::read_ks3_config().ok();
                    let endpoint = config
                        .as_ref()
                        .map(|c| format!("https://{}", c.endpoint))
                        .unwrap_or_default();
                    let ak = config.as_ref().map(|c| c.access_key_id.clone()).unwrap_or_default();
                    let sk = config.as_ref().map(|c| c.access_key_secret.clone()).unwrap_or_default();
                    (endpoint, ak, sk)
                };
                vec![
                    CloudField::new("Name", edit.as_ref().map(|(_, n)| n.as_str()).unwrap_or("")),
                    CloudField::new("Bucket", s.map(|s| s.bucket.as_str()).unwrap_or("")),
                    CloudField::new("Endpoint", &endpoint),
                    CloudField::new("Access Key", &ak),
                    CloudField::new_secret("Secret Key", &sk),
                ]
            }
        };
    }

    pub fn close_cloud_dialog(&mut self) {
        self.cloud_dialog_active = false;
        self.cloud_dialog_edit = None;
        self.focus = Focus::BrowserTree;
    }

    /// Read the dialog fields (skipping the leading Name field) into
    /// `ConnectionFields` for the current protocol.
    fn dialog_fields_to_conn_fields(&self, protocol: Protocol) -> ConnectionFields {
        let val = |i: usize| -> String {
            self.cloud_dialog_fields
                .get(i)
                .map(|f| f.input.value().to_string())
                .unwrap_or_default()
        };
        match protocol {
            Protocol::Ssh => ConnectionFields {
                host: val(1),
                port: val(2),
                user: val(3),
                pass: val(4),
                path: val(5),
                ..Default::default()
            },
            Protocol::WebDav => ConnectionFields {
                url: val(1),
                user: val(2),
                pass: val(3),
                ..Default::default()
            },
            Protocol::S3 => ConnectionFields {
                bucket: val(1),
                region: val(2),
                endpoint: val(3),
                access_key: val(4),
                secret_key: val(5),
                ..Default::default()
            },
            Protocol::Ks3 => ConnectionFields {
                bucket: val(1),
                endpoint: val(2),
                access_key: val(3),
                secret_key: val(4),
                ..Default::default()
            },
        }
    }

    /// Build the root URI for browsing a saved connection.
    fn conn_uri(protocol: Protocol, c: &ConnectionFields) -> String {
        match protocol {
            Protocol::Ssh => {
                let path = if c.path.is_empty() { "/".to_string() } else { c.path.clone() };
                if c.user.is_empty() {
                    format!("ssh://{}:{}/{}", c.host, if c.port.is_empty() { "22" } else { &c.port }, path)
                } else {
                    format!("ssh://{}@{}:{}/{}", c.user, c.host, if c.port.is_empty() { "22" } else { &c.port }, path)
                }
            }
            Protocol::WebDav => {
                let mut url = if c.url.starts_with("webdav://") {
                    c.url.replacen("webdav://", "http://", 1)
                } else if c.url.starts_with("webdavs://") {
                    c.url.replacen("webdavs://", "https://", 1)
                } else {
                    c.url.clone()
                };
                if !url.ends_with('/') {
                    url.push('/');
                }
                url
            }
            Protocol::S3 => {
                let mut uri = format!("s3://{}", c.bucket);
                if !c.region.is_empty() {
                    uri.push_str(&format!("?region={}", c.region));
                }
                if !c.endpoint.is_empty() {
                    let sep = if uri.contains('?') { '&' } else { '?' };
                    uri.push_str(&format!("{}endpoint={}", sep, c.endpoint));
                }
                uri
            }
            Protocol::Ks3 => {
                let mut uri = format!("ks3://{}", c.bucket);
                if !c.endpoint.is_empty() {
                    let ep = c.endpoint.trim_start_matches("https://").trim_start_matches("http://");
                    uri.push_str(&format!("?endpoint={}", ep));
                }
                uri
            }
        }
    }

    fn confirm_cloud_dialog(&mut self) -> Result<()> {
        let protocol = self.cloud_dialog_type;
        let name = self
            .cloud_dialog_fields
            .get(0)
            .map(|f| f.input.value().to_string())
            .unwrap_or_default();
        if name.trim().is_empty() {
            self.error_message = Some("Connection name is required".into());
            return Ok(());
        }
        let cf = self.dialog_fields_to_conn_fields(protocol);
        let conn = CloudConnection::from_protocol(protocol, &name, &cf);

        let editing = self.cloud_dialog_edit.clone();
        let res = match &editing {
            None => add_connection(&conn),
            Some((proto, old_name)) => update_connection(*proto, old_name, &name, &conn),
        };
        if let Err(e) = res {
            self.error_message = Some(format!("Failed to save connection: {}", e));
            return Ok(());
        }

        self.close_cloud_dialog();

        // Refresh the protocol category so the (possibly renamed) connection
        // appears in the right sorted slot, then select + expand it.
        self.refresh_category(protocol)?;
        self.select_and_expand_cloud_conn(protocol, &name);
        Ok(())
    }

    /// Category node id for a protocol, e.g. `cloud.webdav`.
    fn category_id(protocol: Protocol) -> String {
        format!("cloud.{}", protocol.as_str())
    }

    /// Collapse + re-expand a protocol category to regenerate its saved
    /// connection children from the store.
    fn refresh_category(&mut self, protocol: Protocol) -> Result<()> {
        let cat_id = Self::category_id(protocol);
        if let Some(idx) = self.browser_nodes.iter().position(|n| n.id() == cat_id) {
            self.browser_expanded.remove(&cat_id);
            let node = self.browser_nodes[idx].clone();
            self.expand_node(node)?;
        }
        Ok(())
    }

    /// Find the saved connection's `CloudConn` node, select it, and expand it
    /// so its remote directory is listed.
    fn select_and_expand_cloud_conn(&mut self, protocol: Protocol, name: &str) {
        let target = format!("cloud.conn:{}:{}", protocol.as_str(), name);
        if let Some(idx) = self.browser_nodes.iter().position(|n| n.id() == target) {
            self.browser_selected = idx;
            self.browser_scroll = 0;
            let node = self.browser_nodes[idx].clone();
            let _ = self.expand_node(node);
        }
    }

    /// Rename the selected saved connection: open the dialog in edit mode.
    pub fn rename_selected_cloud_conn(&mut self) {
        if let Some((protocol, name)) = self.selected_cloud_conn() {
            self.open_cloud_dialog(protocol, Some((protocol, name)));
        }
    }

    /// Delete the selected saved connection, with a two-step confirmation.
    /// First `x` arms `pending_delete`; a second `x` confirms.
    pub fn delete_selected_cloud_conn(&mut self) {
        let Some((protocol, name)) = self.selected_cloud_conn() else {
            return;
        };
        if self.pending_delete.as_ref() == Some(&(protocol, name.clone())) {
            // Confirmed — remove from store and tree.
            if let Err(e) = remove_connection(protocol, &name) {
                self.error_message = Some(format!("Failed to delete connection: {}", e));
            }
            self.pending_delete = None;
            let _ = self.refresh_category(protocol);
        } else {
            self.pending_delete = Some((protocol, name.clone()));
            self.error_message =
                Some(format!("再按 x 确认删除 '{}'（其他键取消）", name));
        }
    }

    /// The selected node as a `(protocol, name)` if it's a saved CloudConn.
    fn selected_cloud_conn(&self) -> Option<(Protocol, String)> {
        let node = self.browser_nodes.get(self.browser_selected)?;
        match node {
            BrowserNode::CloudConn { protocol, name, .. } => Some((*protocol, name.clone())),
            _ => None,
        }
    }

    /// Clear any pending-delete state when the selection changes.
    pub fn clear_pending_delete_if_stale(&mut self) {
        if let Some((p, n)) = self.pending_delete.clone() {
            let cur = self.selected_cloud_conn();
            let same = cur
                .as_ref()
                .map(|(cp, cn)| cp == &p && cn == &n)
                .unwrap_or(false);
            if !same {
                self.pending_delete = None;
                if self.error_message.as_deref()
                    == Some(&format!("再按 x 确认删除 '{}'（其他键取消）", n))
                {
                    self.error_message = None;
                }
            }
        }
    }

    pub fn cloud_dialog_field_up(&mut self) {
        // +1 for the trailing "Connect" button.
        let n = self.cloud_dialog_fields.len() + 1;
        if n > 1 {
            if self.cloud_dialog_selected == 0 {
                self.cloud_dialog_selected = n - 1;
            } else {
                self.cloud_dialog_selected -= 1;
            }
        }
    }

    pub fn cloud_dialog_field_down(&mut self) {
        // +1 for the trailing "Connect" button.
        let n = self.cloud_dialog_fields.len() + 1;
        if n > 1 {
            self.cloud_dialog_selected = (self.cloud_dialog_selected + 1) % n;
        }
    }

    /// Index of the trailing Connect button (= number of fields).
    pub fn cloud_dialog_connect_index(&self) -> usize {
        self.cloud_dialog_fields.len()
    }

    /// Expand a node: insert children right after it.
    fn expand_node(&mut self, node: BrowserNode) -> Result<()> {
        let node_id = node.id();
        let depth = node.depth() + 1;
        let idx = self.browser_selected;

        let children: Vec<BrowserNode> = match &node {
            BrowserNode::Category { id, .. } if id == "local" => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                let entries = self
                    .source_registry
                    .list(&cwd.to_string_lossy())
                    .unwrap_or_default();
                let mut kids = local_dir_children(entries, depth, false);
                // Prepend a ".." parent entry.
                if let Some(parent) = cwd.parent().filter(|p| *p != cwd.as_path()) {
                    kids.insert(0, BrowserNode::LocalParent {
                        path: parent.to_path_buf(),
                        depth,
                    });
                }
                kids
            }
            BrowserNode::Category { id, .. } if id == "cloud" => {
                vec![
                    BrowserNode::Category { label: "S3".into(), id: "cloud.s3".into(), depth },
                    BrowserNode::Category { label: "KS3".into(), id: "cloud.ks3".into(), depth },
                    BrowserNode::Category { label: "WebDAV".into(), id: "cloud.webdav".into(), depth },
                    BrowserNode::Category { label: "SSH".into(), id: "cloud.ssh".into(), depth },
                ]
            }
            BrowserNode::Category { id, .. }
                if id == "cloud.ks3" || id == "cloud.webdav" || id == "cloud.ssh" || id == "cloud.s3" =>
            {
                let protocol = Protocol::from_id(id.strip_prefix("cloud.").unwrap_or(""))
                    .unwrap_or(Protocol::WebDav);
                // List saved connections of this protocol (sorted by name) +
                // a trailing "+ 新建连接" node.
                let mut conns: Vec<CloudConnection> = load_connections()
                    .into_iter()
                    .filter(|c| c.protocol() == Some(protocol))
                    .collect();
                conns.sort_by(|a, b| a.name.cmp(&b.name));
                let mut kids: Vec<BrowserNode> = conns
                    .into_iter()
                    .map(|c| {
                        let uri = Self::conn_uri(protocol, &ConnectionFields::from(&c));
                        BrowserNode::CloudConn {
                            protocol,
                            name: c.name,
                            uri,
                            depth,
                        }
                    })
                    .collect();
                kids.push(BrowserNode::NewConn { protocol, depth });
                kids
            }
            BrowserNode::LocalDir { path, .. } => {
                let entries = self
                    .source_registry
                    .list(&path.to_string_lossy())
                    .unwrap_or_default();
                let mut kids = local_dir_children(entries, depth, false);
                // Prepend a ".." parent entry (unless at filesystem root).
                if let Some(parent) = path.parent().filter(|p| *p != path.as_path()) {
                    kids.insert(0, BrowserNode::LocalParent {
                        path: parent.to_path_buf(),
                        depth,
                    });
                }
                kids
            }
            BrowserNode::LocalParent { path, .. } => {
                let entries = self
                    .source_registry
                    .list(&path.to_string_lossy())
                    .unwrap_or_default();
                let mut kids = local_dir_children(entries, depth, false);
                if let Some(parent) = path.parent().filter(|p| *p != path.as_path()) {
                    kids.insert(0, BrowserNode::LocalParent {
                        path: parent.to_path_buf(),
                        depth,
                    });
                }
                kids
            }
            BrowserNode::Bucket {
                scheme, name, endpoint, region, ..
            } => {
                let uri = node.list_uri();
                match self.source_registry.list(&uri) {
                    Ok(entries) => entries
                        .into_iter()
                        .map(|e| match e.kind {
                            EntryKind::Directory => {
                                let prefix = extract_s3_prefix(&e.uri, scheme, name);
                                BrowserNode::Prefix {
                                    scheme: scheme.clone(),
                                    bucket: name.clone(),
                                    prefix,
                                    endpoint: endpoint.clone(),
                                    region: region.clone(),
                                    depth,
                                }
                            }
                            EntryKind::File => BrowserNode::File {
                                name: e.name,
                                uri: e.uri,
                                size: e.size,
                                depth,
                            },
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                }
            }
            BrowserNode::Prefix {
                scheme, bucket, prefix: _, endpoint, region, ..
            } => {
                let uri = node.list_uri();
                match self.source_registry.list(&uri) {
                    Ok(entries) => entries
                        .into_iter()
                        .map(|e| match e.kind {
                            EntryKind::Directory => {
                                let prefix = extract_s3_prefix(&e.uri, scheme, bucket);
                                BrowserNode::Prefix {
                                    scheme: scheme.clone(),
                                    bucket: bucket.clone(),
                                    prefix,
                                    endpoint: endpoint.clone(),
                                    region: region.clone(),
                                    depth,
                                }
                            }
                            EntryKind::File => BrowserNode::File {
                                name: e.name,
                                uri: e.uri,
                                size: e.size,
                                depth,
                            },
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                }
            }
            BrowserNode::WebDavDir { url, .. } => {
                match self.source_registry.list(url) {
                    Ok(entries) => entries
                        .into_iter()
                        .map(|e| match e.kind {
                            EntryKind::Directory => BrowserNode::WebDavDir {
                                url: e.uri,
                                depth,
                            },
                            EntryKind::File => BrowserNode::File {
                                name: e.name,
                                uri: e.uri,
                                size: e.size,
                                depth,
                            },
                        })
                        .collect(),
                    Err(e) => {
                        self.error_message = Some(format!("WebDAV listing failed: {}", e));
                        Vec::new()
                    }
                }
            }
            BrowserNode::SshDir { uri, .. } => {
                match self.source_registry.list(uri) {
                    Ok(entries) => entries
                        .into_iter()
                        .map(|e| match e.kind {
                            EntryKind::Directory => BrowserNode::SshDir {
                                uri: e.uri,
                                depth,
                            },
                            EntryKind::File => BrowserNode::File {
                                name: e.name,
                                uri: e.uri,
                                size: e.size,
                                depth,
                            },
                        })
                        .collect(),
                    Err(e) => {
                        self.error_message = Some(format!("SSH listing failed: {}", e));
                        Vec::new()
                    }
                }
            }
            BrowserNode::CloudConn { protocol, name: _, uri, .. } => {
                match self.source_registry.list(uri) {
                    Ok(entries) => entries
                        .into_iter()
                        .map(|e| match e.kind {
                            EntryKind::Directory => dir_node_for(*protocol, &e.uri, depth),
                            EntryKind::File => BrowserNode::File {
                                name: e.name,
                                uri: e.uri,
                                size: e.size,
                                depth,
                            },
                        })
                        .collect(),
                    Err(e) => {
                        self.error_message =
                            Some(format!("{} listing failed: {}", protocol.title(), e));
                        Vec::new()
                    }
                }
            }
            _ => Vec::new(),
        };

        if children.is_empty() {
            // For nodes with no sub-items, the caller (browser_enter)
            // will handle entering the directory.
            return Ok(());
        }

        // Insert children after the parent node
        self.browser_expanded.insert(node_id);
        for (i, child) in children.into_iter().enumerate() {
            self.browser_nodes.insert(idx + 1 + i, child);
        }
        Ok(())
    }

    /// Collapse a node: remove all children (recursively).
    fn collapse_node(&mut self, node_id: &str) {
        self.browser_expanded.remove(node_id);
        let idx = match self.browser_nodes.iter().position(|n| n.id() == node_id) {
            Some(i) => i,
            None => return,
        };
        let parent_depth = self.browser_nodes[idx].depth();
        // Remove everything after idx that has depth > parent_depth
        let mut remove_end = idx + 1;
        while remove_end < self.browser_nodes.len()
            && self.browser_nodes[remove_end].depth() > parent_depth
        {
            remove_end += 1;
        }
        self.browser_nodes.drain(idx + 1..remove_end);
        // Adjust selection if needed
        if self.browser_selected >= self.browser_nodes.len() {
            self.browser_selected = self.browser_nodes.len().saturating_sub(1);
        }
    }
}

/// Build the directory-kind child node for a given protocol's listing.
fn dir_node_for(protocol: Protocol, uri: &str, depth: usize) -> BrowserNode {
    match protocol {
        Protocol::WebDav => BrowserNode::WebDavDir { url: uri.to_string(), depth },
        Protocol::Ssh => BrowserNode::SshDir { uri: uri.to_string(), depth },
        Protocol::S3 => {
            let p = crate::source::s3::parse_s3_uri(uri);
            BrowserNode::Prefix {
                scheme: "s3".into(),
                bucket: p.bucket,
                prefix: if p.key.is_empty() { "/".into() } else { p.key },
                endpoint: p.endpoint,
                region: p.region,
                depth,
            }
        }
        Protocol::Ks3 => {
            let p = crate::source::s3::parse_ks3_uri(uri);
            BrowserNode::Prefix {
                scheme: "ks3".into(),
                bucket: p.bucket,
                prefix: if p.key.is_empty() { "/".into() } else { p.key },
                endpoint: p.endpoint,
                region: None,
                depth,
            }
        }
    }
}

/// Extract the S3 prefix portion from a DirEntry URI.
/// Input: `ks3://bucket/path/to/dir/?endpoint=...`
/// Output: `path/to/dir/`
fn extract_s3_prefix(uri: &str, scheme: &str, bucket: &str) -> String {
    let base = uri.split('?').next().unwrap_or(uri);
    let prefix = format!("{}://{}/", scheme, bucket);
    base.strip_prefix(&prefix).unwrap_or("").to_string()
}

/// Build the child nodes for a local directory listing: directories first,
/// then files. `dirs_only` keeps just directories (used for the top-level
/// `Local` category so the root stays compact).
fn local_dir_children(entries: Vec<DirEntry>, depth: usize, dirs_only: bool) -> Vec<BrowserNode> {
    let mut dirs: Vec<BrowserNode> = Vec::new();
    let mut files: Vec<BrowserNode> = Vec::new();
    for e in entries {
        match e.kind {
            EntryKind::Directory => dirs.push(BrowserNode::LocalDir {
                path: PathBuf::from(&e.uri),
                depth,
            }),
            EntryKind::File if !dirs_only => files.push(BrowserNode::File {
                name: e.name,
                uri: e.uri,
                size: e.size,
                depth,
            }),
            EntryKind::File => {}
        }
    }
    dirs.extend(files);
    dirs
}

