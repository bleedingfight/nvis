use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use anyhow::Result;

use super::backend::ProfilerBackend;
use super::types::{ViewDescriptor, ProfilerData};
use crate::viz::renderer::VizRenderer;

pub struct Registry {
    backends: Vec<Arc<dyn ProfilerBackend>>,
    renderers: HashMap<String, Arc<dyn VizRenderer>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            renderers: HashMap::new(),
        }
    }

    pub fn register_backend(&mut self, backend: Arc<dyn ProfilerBackend>) {
        self.backends.push(backend);
    }

    pub fn register_renderer(&mut self, renderer: Arc<dyn VizRenderer>) {
        self.renderers.insert(renderer.id().to_string(), renderer);
    }

    pub fn detect_backend(&self, path: &Path) -> Result<Arc<dyn ProfilerBackend>> {
        let mut best: Option<Arc<dyn ProfilerBackend>> = None;
        let mut best_score: f64 = 0.0;

        for backend in &self.backends {
            let score = backend.detect(path)?;
            if score > best_score {
                best_score = score;
                best = Some(Arc::clone(backend));
            }
        }

        if best_score < 0.5 {
            anyhow::bail!("No backend can handle file: {}", path.display());
        }

        best.ok_or_else(|| anyhow::anyhow!("No backend found for file: {}", path.display()))
    }

    pub fn find_renderer(
        &self,
        data: &ProfilerData,
        view: &ViewDescriptor,
    ) -> Option<Arc<dyn VizRenderer>> {
        if let Some(ref viz_id) = view.default_viz {
            if let Some(renderer) = self.renderers.get(viz_id) {
                if renderer.can_render(data, view) {
                    return Some(Arc::clone(renderer));
                }
            }
        }

        for renderer in self.renderers.values() {
            if renderer.can_render(data, view) {
                return Some(Arc::clone(renderer));
            }
        }

        None
    }

    pub fn get_renderer(&self, id: &str) -> Option<Arc<dyn VizRenderer>> {
        self.renderers.get(id).cloned()
    }
}
