// Library interface for nvis. Keep modules private-ish and
// re-export the public API for ergonomic crate usage.
pub mod app;
pub mod db;
pub mod ui;
pub mod visualization;
pub mod stats;

// Re-exports: prefer `nvis::Thing` over deep module paths
pub use app::{App, AppState, Focus, TableData};
pub use db::load_table_data_resolved;
pub use db::{load_table_data, load_tables};
pub use ui::draw;
pub use visualization::{
    detect_table_type, generate_bar_chart_data, generate_visualization, BoxPlotData, ChartData,
    TimelineEvent, Visualization, VisualizationType,
};
pub use stats::compute_cuda_api_aggregates;
