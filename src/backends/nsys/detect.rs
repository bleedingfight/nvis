use anyhow::Result;
use std::path::Path;
use rusqlite::Connection;

/// Detect if a file is an NSYS SQLite database.
/// Returns a confidence score 0.0-1.0.
pub fn detect_nsys(path: &Path) -> Result<f64> {
    let mut score = 0.0;

    // Extension check
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("sqlite") | Some("db") => score += 0.3,
        Some("nsys-rep") => score += 0.2,
        _ => {}
    }

    // Try opening as SQLite and check for CUPTI tables
    if let Ok(conn) = Connection::open(path) {
        let mut stmt = conn.prepare(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name LIKE 'CUPTI_%'",
        )?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        if count > 0 {
            score += 0.7;
        } else {
            // It's SQLite but not NSYS, so unlikely
            score += 0.0;
        }
    }

    Ok(score)
}
