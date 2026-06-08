use anyhow::Result;
use ratatui::{Frame, layout::Rect};

use crate::core::types::{ProfilerData, ViewDescriptor};
use super::types::PreparedVisualization;

pub trait VizRenderer: Send + Sync {
    fn id(&self) -> &str;
    fn can_render(&self, data: &ProfilerData, view: &ViewDescriptor) -> bool;
    fn prepare(&self, data: &ProfilerData, view: &ViewDescriptor, scroll: usize) -> Result<PreparedVisualization>;
    fn draw(&self, f: &mut Frame, area: Rect, viz: &PreparedVisualization, focused: bool);
}
