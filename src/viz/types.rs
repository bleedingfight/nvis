#[derive(Debug, Clone)]
pub struct BoxPlotData {
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub mean: f64,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimelineEventType {
    Kernel,
    Memcpy,
    Memset,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct TimelineViewport {
    pub view_start_ns: f64,
    pub view_width_ns: f64,
    pub stream_lanes: Vec<i64>,
    pub hovered: Option<usize>,
    pub selection: Option<(f64, f64)>,
    pub drag_origin: Option<(u16, u16)>,
    pub panning: bool,
    pub pan_anchor_ns: f64,
    pub pan_anchor_col: u16,
}

#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub name: String,
    pub start: f64,
    pub duration: f64,
    pub stream_id: i64,
    pub event_type: TimelineEventType,
}

#[derive(Debug, Clone)]
pub struct TimelinePrepared {
    pub events: Vec<TimelineEvent>,
    pub global_start_ns: f64,
    pub global_end_ns: f64,
    pub stream_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub enum VizData {
    Bars { labels: Vec<String>, values: Vec<f64> },
    BoxPlots(Vec<BoxPlotData>),
    Timeline(TimelinePrepared),
    StatsText(String),
}

#[derive(Debug, Clone)]
pub struct PreparedVisualization {
    pub title: String,
    pub data: VizData,
    pub viewport: Option<TimelineViewport>,
}
