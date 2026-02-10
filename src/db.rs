use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use crate::app::TableData;

pub fn load_tables(db_path: &Path) -> Result<Vec<String>> {
    let conn = Connection::open(db_path)?;
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;

    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tables)
}

pub fn load_table_data(db_path: &Path, table_name: &str) -> Result<TableData> {
    let conn = Connection::open(db_path)?;

    // Get column names
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();

    // Get data (limit to 1000 rows for performance)
    let query = format!("SELECT * FROM {} LIMIT 1000", table_name);
    let mut stmt = conn.prepare(&query)?;

    let column_count = stmt.column_count();
    let mut rows = Vec::new();

    let mut query_rows = stmt.query([])?;
    while let Some(row) = query_rows.next()? {
        let mut row_data = Vec::new();
        for i in 0..column_count {
            let value: String = row
                .get::<_, Option<rusqlite::types::Value>>(i)?
                .map(|v| format_value(v))
                .unwrap_or_else(|| "NULL".to_string());
            row_data.push(value);
        }
        rows.push(row_data);
    }

    Ok(TableData { columns, rows })
}

fn format_value(value: rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Null => "NULL".to_string(),
        rusqlite::types::Value::Integer(i) => i.to_string(),
        rusqlite::types::Value::Real(f) => format!("{:.2}", f),
        rusqlite::types::Value::Text(s) => s,
        rusqlite::types::Value::Blob(b) => format!("<BLOB: {} bytes>", b.len()),
    }
}
