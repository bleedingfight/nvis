use anyhow::Result;
use std::path::Path;

/// Detect if a file is a PyTorch Profiler Chrome Trace JSON.
/// Returns a confidence score 0.0-1.0.
pub fn detect_torch(path: &Path) -> Result<f64> {
    let mut score = 0.0;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("json") => score += 0.2,
        _ => return Ok(score),
    }

    // Try reading and parsing the beginning to check for trace event structure
    if let Ok(content) = std::fs::read_to_string(path) {
        let trimmed = content.trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            score += 0.3;
        }

        // Check for Chrome trace format markers
        if content.contains("\"traceEvents\"") {
            score += 0.3;
        }
        if content.contains("\"ts\"") && content.contains("\"dur\"") && content.contains("\"cat\"") {
            score += 0.2;
        }
    }

    Ok(score)
}
