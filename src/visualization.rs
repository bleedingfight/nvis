use crate::core::types::{ProfilerData, ViewCategory, ViewDescriptor};

use crate::viz::boxplot::BoxPlotRenderer;
use crate::viz::timeline::TimelineRenderer;
use crate::viz::barchart::BarChartRenderer;
use crate::viz::statistics::StatisticsRenderer;
use crate::viz::types::{BoxPlotData, TimelineEvent, VizData};
use crate::viz::renderer::VizRenderer;

/// Detect the best visualization type for a given view + data combination.
pub fn detect_visualization_type(data: &ProfilerData, view: &ViewDescriptor) -> &'static str {
    let lowered: Vec<String> = data.schema.iter().map(|s| s.name.to_lowercase()).collect();

    match view.category {
        ViewCategory::Timeline => {
            let has_name = lowered.iter().any(|c| c.contains("name") || c.contains("kernel"));
            let has_start = lowered.iter().any(|c| c.contains("start"));
            if has_name && has_start {
                return "timeline";
            }
        }
        ViewCategory::Statistics => {
            let has_name = lowered.iter().any(|c| {
                (c.contains("function") || c.contains("api") || c.contains("name"))
                    && !c.contains("id")
            });
            let has_duration = lowered.iter().any(|c| {
                c.contains("duration")
                    || c.contains("elapsed")
                    || (c.contains("time") && !c.contains("start") && !c.contains("end"))
                    || (c.contains("start") && lowered.iter().any(|d| d.contains("end")))
            });
            if has_name && has_duration {
                return "boxplot";
            }
            return "statistics";
        }
        ViewCategory::Metrics => {
            return "barchart";
        }
        ViewCategory::Metadata => {
            return "statistics";
        }
        ViewCategory::RawData => {
            return "barchart";
        }
    }

    "barchart"
}

/// Generate boxplot data from ProfilerData (kept for backward compat with stats view).
pub fn generate_boxplot_data(data: &ProfilerData) -> Vec<BoxPlotData> {
    let renderer = BoxPlotRenderer;
    let view = ViewDescriptor {
        id: "_compat".into(),
        display_name: "Stats".into(),
        category: ViewCategory::Statistics,
        default_viz: Some("boxplot".into()),
    };
    if let Ok(viz) = renderer.prepare(data, &view, 0) {
        if let VizData::BoxPlots(plots) = viz.data {
            return plots;
        }
    }
    Vec::new()
}

/// Generate timeline data from ProfilerData (kept for backward compat).
pub fn generate_timeline_data(data: &ProfilerData, limit: usize) -> Vec<TimelineEvent> {
    let renderer = TimelineRenderer;
    let view = ViewDescriptor {
        id: "_compat".into(),
        display_name: "Timeline".into(),
        category: ViewCategory::Timeline,
        default_viz: Some("timeline".into()),
    };
    if let Ok(viz) = renderer.prepare(data, &view, limit) {
        if let VizData::Timeline(ref prepared) = viz.data {
            return prepared.events.clone();
        }
    }
    Vec::new()
}

/// Generate statistics text from ProfilerData (kept for backward compat).
pub fn generate_statistics_text(view_name: &str, data: &ProfilerData) -> String {
    let renderer = StatisticsRenderer;
    let view = ViewDescriptor {
        id: "_compat".into(),
        display_name: view_name.into(),
        category: ViewCategory::Metadata,
        default_viz: Some("statistics".into()),
    };
    if let Ok(viz) = renderer.prepare(data, &view, 0) {
        if let VizData::StatsText(text) = viz.data {
            return text;
        }
    }
    String::new()
}

/// Generate bar chart data from ProfilerData (kept for backward compat).
pub fn generate_bar_chart_data(data: &ProfilerData, scroll: usize) -> Vec<(String, f64)> {
    let renderer = BarChartRenderer;
    let view = ViewDescriptor {
        id: "_compat".into(),
        display_name: "Data".into(),
        category: ViewCategory::RawData,
        default_viz: Some("barchart".into()),
    };
    if let Ok(viz) = renderer.prepare(data, &view, scroll) {
        if let VizData::Bars { labels, values } = viz.data {
            return labels.into_iter().zip(values).collect();
        }
    }
    Vec::new()
}
