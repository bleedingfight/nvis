pub mod types;
pub mod renderer;
pub mod boxplot;
pub mod timeline;
pub mod barchart;
pub mod statistics;

pub use types::{VizData, PreparedVisualization, BoxPlotData, TimelineEvent, TimelineEventType, TimelinePrepared, TimelineViewport};
pub use renderer::VizRenderer;

use std::sync::Arc;
use crate::core::registry::Registry;

pub fn register_all(registry: &mut Registry) {
    registry.register_renderer(Arc::new(boxplot::BoxPlotRenderer));
    registry.register_renderer(Arc::new(timeline::TimelineRenderer));
    registry.register_renderer(Arc::new(barchart::BarChartRenderer));
    registry.register_renderer(Arc::new(statistics::StatisticsRenderer));
}
