use anyhow::Result;
use base64::Engine;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::app::TableData;

/// JSON export structure for entire database
#[derive(Serialize)]
pub struct DatabaseExport {
    pub tables: HashMap<String, TableExport>,
}

#[derive(Serialize)]
pub struct TableExport {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<JsonValue>>,
}

/// JSON value wrapper that handles SQLite types
#[derive(Serialize)]
#[serde(untagged)]
pub enum JsonValue {
    Null,
    Integer(i64),
    Real(f64),
    String(String),
    Blob(String), // Base64 encoded
}

pub fn load_tables(db_path: &Path) -> Result<Vec<String>> {
    let conn = Connection::open(db_path)?;
    load_tables_conn(&conn)
}

pub fn load_tables_conn(conn: &Connection) -> Result<Vec<String>> {
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
    load_table_data_conn(&conn, table_name)
}

pub fn load_table_data_conn(conn: &Connection, table_name: &str) -> Result<TableData> {
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

/// Load table data, resolving common string-id references into human-readable names when possible.
/// Specifically, if the table contains a `nameId` column and a `StringIds` table exists, this
/// will append a new `name` column to the result by joining `StringIds`.
pub fn load_table_data_resolved(db_path: &Path, table_name: &str) -> Result<TableData> {
    let conn = Connection::open(db_path)?;

    // Detect if table has a `nameId` column
    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let cols: Vec<String> = pragma
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();

    let has_name_id = cols
        .iter()
        .any(|c| c.eq_ignore_ascii_case("nameId") || c.to_lowercase() == "nameid");

    if has_name_id {
        // Try joined query: original columns + resolved `name`
        let query = format!(
            "SELECT r.*, s.value as name FROM {} r LEFT JOIN StringIds s ON s.id = r.nameId LIMIT 1000",
            table_name
        );

        // Columns are original + name
        let mut columns = cols.clone();
        columns.push("name".to_string());

        let mut stmt = conn.prepare(&query)?;
        let column_count = stmt.column_count();
        let mut rows = Vec::new();
        let mut query_rows = stmt.query([])?;
        while let Some(row) = query_rows.next()? {
            let mut row_data = Vec::with_capacity(column_count as usize);
            for i in 0..column_count {
                let value: String = row
                    .get::<_, Option<rusqlite::types::Value>>(i)?
                    .map(|v| format_value(v))
                    .unwrap_or_else(|| "NULL".to_string());
                row_data.push(value);
            }
            rows.push(row_data);
        }

        return Ok(TableData { columns, rows });
    }

    // Fallback to default loader
    load_table_data(db_path, table_name)
}

/// Load table data using an existing connection.
/// For ncu databases (no StringIds table), falls back to plain load.
pub fn load_table_data_resolved_conn(
    conn: &Connection,
    table_name: &str,
) -> Result<TableData> {
    // Check if StringIds table exists
    let has_string_ids: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='StringIds'")?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .next()
        .is_some();

    if !has_string_ids {
        return load_table_data_conn(conn, table_name);
    }

    // Detect if table has a `nameId` column
    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let cols: Vec<String> = pragma
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();

    let has_name_id = cols
        .iter()
        .any(|c| c.eq_ignore_ascii_case("nameId") || c.to_lowercase() == "nameid");

    if has_name_id {
        let query = format!(
            "SELECT r.*, s.value as name FROM {} r LEFT JOIN StringIds s ON s.id = r.nameId LIMIT 1000",
            table_name
        );
        let mut columns = cols.clone();
        columns.push("name".to_string());

        let mut stmt = conn.prepare(&query)?;
        let column_count = stmt.column_count();
        let mut rows = Vec::new();
        let mut query_rows = stmt.query([])?;
        while let Some(row) = query_rows.next()? {
            let mut row_data = Vec::with_capacity(column_count as usize);
            for i in 0..column_count {
                let value: String = row
                    .get::<_, Option<rusqlite::types::Value>>(i)?
                    .map(|v| format_value(v))
                    .unwrap_or_else(|| "NULL".to_string());
                row_data.push(value);
            }
            rows.push(row_data);
        }
        return Ok(TableData { columns, rows });
    }

    load_table_data_conn(conn, table_name)
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

/// Export entire database to JSON
pub fn export_database_to_json(db_path: &Path) -> Result<String> {
    let conn = Connection::open(db_path)?;

    // Get all table names
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut export = DatabaseExport {
        tables: HashMap::new(),
    };

    for table_name in tables {
        // Get column names
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({})", table_name))?
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();

        // Get all rows
        let query = format!("SELECT * FROM {}", table_name);
        let mut stmt = conn.prepare(&query)?;
        let column_count = stmt.column_count();
        let mut rows = Vec::new();

        let mut query_rows = stmt.query([])?;
        while let Some(row) = query_rows.next()? {
            let mut row_data = Vec::new();
            for i in 0..column_count {
                let value = row.get::<_, Option<rusqlite::types::Value>>(i)?;
                let json_value = match value {
                    None => JsonValue::Null,
                    Some(v) => match v {
                        rusqlite::types::Value::Null => JsonValue::Null,
                        rusqlite::types::Value::Integer(i) => JsonValue::Integer(i),
                        rusqlite::types::Value::Real(f) => JsonValue::Real(f),
                        rusqlite::types::Value::Text(s) => JsonValue::String(s),
                        rusqlite::types::Value::Blob(b) => {
                            JsonValue::Blob(base64::engine::general_purpose::STANDARD.encode(&b))
                        }
                    },
                };
                row_data.push(json_value);
            }
            rows.push(row_data);
        }

        export.tables.insert(
            table_name,
            TableExport { columns, rows },
        );
    }

    Ok(serde_json::to_string_pretty(&export)?)
}

/// Export a single table to JSON
pub fn export_table_to_json(db_path: &Path, table_name: &str) -> Result<String> {
    let conn = Connection::open(db_path)?;

    // Get column names
    let columns: Vec<String> = conn
        .prepare(&format!("PRAGMA table_info({})", table_name))?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();

    // Get all rows (no limit for export)
    let query = format!("SELECT * FROM {}", table_name);
    let mut stmt = conn.prepare(&query)?;
    let column_count = stmt.column_count();
    let mut rows = Vec::new();

    let mut query_rows = stmt.query([])?;
    while let Some(row) = query_rows.next()? {
        let mut row_data = Vec::new();
        for i in 0..column_count {
            let value = row.get::<_, Option<rusqlite::types::Value>>(i)?;
            let json_value = match value {
                None => JsonValue::Null,
                Some(v) => match v {
                    rusqlite::types::Value::Null => JsonValue::Null,
                    rusqlite::types::Value::Integer(i) => JsonValue::Integer(i),
                    rusqlite::types::Value::Real(f) => JsonValue::Real(f),
                    rusqlite::types::Value::Text(s) => JsonValue::String(s),
                    rusqlite::types::Value::Blob(b) => {
                        JsonValue::Blob(base64::engine::general_purpose::STANDARD.encode(&b))
                    }
                },
            };
            row_data.push(json_value);
        }
        rows.push(row_data);
    }

    let export = TableExport { columns, rows };
    Ok(serde_json::to_string_pretty(&export)?)
}

/// Export entire database to JSON using an existing connection
pub fn export_database_to_json_conn(conn: &Connection) -> Result<String> {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut export = DatabaseExport {
        tables: HashMap::new(),
    };

    for table_name in tables {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({})", table_name))?
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();

        let query = format!("SELECT * FROM {}", table_name);
        let mut stmt = conn.prepare(&query)?;
        let column_count = stmt.column_count();
        let mut rows = Vec::new();

        let mut query_rows = stmt.query([])?;
        while let Some(row) = query_rows.next()? {
            let mut row_data = Vec::new();
            for i in 0..column_count {
                let value = row.get::<_, Option<rusqlite::types::Value>>(i)?;
                let json_value = match value {
                    None => JsonValue::Null,
                    Some(v) => match v {
                        rusqlite::types::Value::Null => JsonValue::Null,
                        rusqlite::types::Value::Integer(i) => JsonValue::Integer(i),
                        rusqlite::types::Value::Real(f) => JsonValue::Real(f),
                        rusqlite::types::Value::Text(s) => JsonValue::String(s),
                        rusqlite::types::Value::Blob(b) => {
                            JsonValue::Blob(base64::engine::general_purpose::STANDARD.encode(&b))
                        }
                    },
                };
                row_data.push(json_value);
            }
            rows.push(row_data);
        }

        export.tables.insert(
            table_name,
            TableExport { columns, rows },
        );
    }

    Ok(serde_json::to_string_pretty(&export)?)
}

/// Export a single table to JSON using an existing connection
pub fn export_table_to_json_conn(conn: &Connection, table_name: &str) -> Result<String> {
    let columns: Vec<String> = conn
        .prepare(&format!("PRAGMA table_info({})", table_name))?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();

    let query = format!("SELECT * FROM {}", table_name);
    let mut stmt = conn.prepare(&query)?;
    let column_count = stmt.column_count();
    let mut rows = Vec::new();

    let mut query_rows = stmt.query([])?;
    while let Some(row) = query_rows.next()? {
        let mut row_data = Vec::new();
        for i in 0..column_count {
            let value = row.get::<_, Option<rusqlite::types::Value>>(i)?;
            let json_value = match value {
                None => JsonValue::Null,
                Some(v) => match v {
                    rusqlite::types::Value::Null => JsonValue::Null,
                    rusqlite::types::Value::Integer(i) => JsonValue::Integer(i),
                    rusqlite::types::Value::Real(f) => JsonValue::Real(f),
                    rusqlite::types::Value::Text(s) => JsonValue::String(s),
                    rusqlite::types::Value::Blob(b) => {
                        JsonValue::Blob(base64::engine::general_purpose::STANDARD.encode(&b))
                    }
                },
            };
            row_data.push(json_value);
        }
        rows.push(row_data);
    }

    let export = TableExport { columns, rows };
    Ok(serde_json::to_string_pretty(&export)?)
}
