use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum SummarySide {
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryEventType {
    Kernel,
    Memcpy,
    Memset,
    Runtime,
}

#[derive(Debug, Clone)]
pub struct SummaryEvent {
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub side: SummarySide,
    pub lane_key: String,
    pub event_type: SummaryEventType,
    pub correlation_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SummaryPrepared {
    pub events: Vec<SummaryEvent>,
    pub global_start_ns: f64,
    pub global_end_ns: f64,
    pub gpu_lanes: Vec<String>,
    pub cpu_lanes: Vec<String>,
    /// Maps raw lane_key (e.g. "S7", "T326569971745663") to display label (e.g. "S7", "T0")
    pub lane_labels: HashMap<String, String>,
    pub lane_map: HashMap<String, usize>,
}

impl SummaryPrepared {
    pub fn from_events(mut events: Vec<SummaryEvent>) -> Self {
        if events.is_empty() {
            return Self {
                events,
                global_start_ns: 0.0,
                global_end_ns: 0.0,
                gpu_lanes: Vec::new(),
                cpu_lanes: Vec::new(),
                lane_labels: HashMap::new(),
                lane_map: HashMap::new(),
            };
        }

        events.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

        let mut gpu_lanes: Vec<String> = events
            .iter()
            .filter(|e| e.side == SummarySide::Gpu)
            .map(|e| e.lane_key.clone())
            .collect();
        gpu_lanes.sort();
        gpu_lanes.dedup();

        let mut cpu_lanes: Vec<String> = events
            .iter()
            .filter(|e| e.side == SummarySide::Cpu)
            .map(|e| e.lane_key.clone())
            .collect();
        cpu_lanes.sort();
        cpu_lanes.dedup();

        let global_start_ns = events.iter().map(|e| e.start).fold(f64::INFINITY, f64::min);
        let global_end_ns = events.iter().map(|e| e.end).fold(f64::NEG_INFINITY, f64::max);

        // Build lane labels: GPU "S<streamId>", CPU lane_key is already human-readable from SQL
        let mut lane_labels: HashMap<String, String> = HashMap::new();
        for key in &gpu_lanes {
            lane_labels.insert(key.clone(), key.clone());
        }
        for key in &cpu_lanes {
            lane_labels.insert(key.clone(), key.clone());
        }

        let mut lane_map: HashMap<String, usize> = HashMap::new();
        for (i, key) in gpu_lanes.iter().chain(cpu_lanes.iter()).enumerate() {
            lane_map.insert(key.clone(), i);
        }

        Self {
            events,
            global_start_ns,
            global_end_ns,
            gpu_lanes,
            cpu_lanes,
            lane_labels,
            lane_map,
        }
    }

    pub fn total_lanes(&self) -> usize {
        self.gpu_lanes.len() + self.cpu_lanes.len()
    }

    pub fn gpu_lane_count(&self) -> usize {
        self.gpu_lanes.len()
    }
}

#[derive(Debug, Clone)]
pub struct SummaryViewport {
    pub view_start_ns: f64,
    pub view_width_ns: f64,
    pub hovered: Option<usize>,
    pub selection: Option<(f64, f64)>,
    pub drag_origin: Option<(u16, u16)>,
    pub panning: bool,
    pub pan_anchor_ns: f64,
    pub pan_anchor_col: u16,
}

impl Default for SummaryViewport {
    fn default() -> Self {
        Self {
            view_start_ns: 0.0,
            view_width_ns: 1.0,
            hovered: None,
            selection: None,
            drag_origin: None,
            panning: false,
            pan_anchor_ns: 0.0,
            pan_anchor_col: 0,
        }
    }
}

pub const LANE_LABEL_COLS: u16 = 14;
pub const TIME_AXIS_ROWS: u16 = 2;
pub const MIN_PLOT_WIDTH: u16 = 20;
pub const MIN_PLOT_HEIGHT: u16 = 4;

pub fn ns_to_col(ns: f64, view_start_ns: f64, view_width_ns: f64, plot_width: u16) -> f64 {
    if view_width_ns <= 0.0 { return 0.0; }
    ((ns - view_start_ns) / view_width_ns) * plot_width as f64
}

pub fn col_to_ns(col: f64, view_start_ns: f64, view_width_ns: f64, plot_width: u16) -> f64 {
    view_start_ns + (col / plot_width as f64) * view_width_ns
}

pub fn format_duration(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.1}ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.1}us", ns / 1e3)
    } else {
        format!("{:.0}ns", ns)
    }
}
