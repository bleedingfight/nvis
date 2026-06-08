use anyhow::Result;
use std::path::Path;

/// Detect if a file is an NCU CSV export.
/// Returns a confidence score 0.0-1.0.
pub fn detect_ncu(path: &Path) -> Result<f64> {
    let mut score = 0.0;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("csv") => score += 0.2,
        _ => return Ok(score),
    }

    // Try reading the first line to check for NCU header patterns
    if let Ok(content) = std::fs::read_to_string(path) {
        let first_line = content.lines().next().unwrap_or("");
        if first_line.contains("Kernel Name") || first_line.contains("ID") {
            score += 0.3;
        }
        // Check for NCU-specific metric names
        if first_line.contains("SM Occupancy")
            || first_line.contains("DRAM Throughput")
            || first_line.contains("Launch Binding")
            || first_line.contains("Achieved Occupancy")
        {
            score += 0.5;
        }
    }

    Ok(score)
}
