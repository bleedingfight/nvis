use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use crate::core::types::{
    ColumnSchema, ColumnType, ColumnValue, ProfilerData,
};

/// Load table names from an SQLite database.
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

/// Load table data as ProfilerData (typed columns).
pub fn load_table_data(db_path: &Path, table_name: &str) -> Result<ProfilerData> {
    let conn = Connection::open(db_path)?;

    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let col_info: Vec<(String, String)> = pragma
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2).unwrap_or_else(|_| "".to_string()),
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let schema: Vec<ColumnSchema> = col_info
        .iter()
        .map(|(name, typ)| ColumnSchema {
            name: name.clone(),
            dtype: sqlite_type_to_column_type(typ),
        })
        .collect();

    let query = format!("SELECT * FROM {} LIMIT 1000", table_name);
    let mut stmt = conn.prepare(&query)?;
    let column_count = stmt.column_count();
    let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

    let mut columns: Vec<Vec<ColumnValue>> = vec![Vec::new(); column_count];
    let mut row_count = 0usize;

    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        for i in 0..column_count {
            let val = row
                .get::<_, Option<rusqlite::types::Value>>(i)?
                .map(sqlite_value_to_column_value)
                .unwrap_or(ColumnValue::Null);
            columns[i].push(val);
        }
        row_count += 1;
    }

    let final_schema: Vec<ColumnSchema> = column_names
        .iter()
        .zip(schema.iter())
        .map(|(name, original)| ColumnSchema {
            name: name.clone(),
            dtype: original.dtype.clone(),
        })
        .collect();

    Ok(ProfilerData {
        schema: final_schema,
        columns,
        row_count,
    })
}

/// Load table data with nameId/StringIds resolution, returning ProfilerData.
pub fn load_table_data_resolved(db_path: &Path, table_name: &str) -> Result<ProfilerData> {
    let conn = Connection::open(db_path)?;

    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let cols: Vec<String> = pragma
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();

    let has_name_id = cols
        .iter()
        .any(|c| c.eq_ignore_ascii_case("nameId") || c.to_lowercase() == "nameid");

    if has_name_id {
        // Check if StringIds table exists
        let mut check =
            conn.prepare("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='StringIds'")?;
        let count: i64 = check.query_row([], |row| row.get(0))?;

        if count > 0 {
            let query = format!(
                "SELECT r.*, s.value as name FROM {} r LEFT JOIN StringIds s ON s.id = r.nameId LIMIT 1000",
                table_name
            );

            let mut stmt = conn.prepare(&query)?;
            let column_count = stmt.column_count();
            let column_names: Vec<String> =
                stmt.column_names().iter().map(|s| s.to_string()).collect();

            let schema: Vec<ColumnSchema> = column_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let dtype = if i < cols.len() {
                        sqlite_type_to_column_type_by_name(&cols[i])
                    } else {
                        ColumnType::Text // the appended 'name' column
                    };
                    ColumnSchema {
                        name: name.clone(),
                        dtype,
                    }
                })
                .collect();

            let mut columns: Vec<Vec<ColumnValue>> = vec![Vec::new(); column_count];
            let mut row_count = 0usize;

            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                for i in 0..column_count {
                    let val = row
                        .get::<_, Option<rusqlite::types::Value>>(i)?
                        .map(sqlite_value_to_column_value)
                        .unwrap_or(ColumnValue::Null);
                    columns[i].push(val);
                }
                row_count += 1;
            }

            return Ok(ProfilerData {
                schema,
                columns,
                row_count,
            });
        }
    }

    load_table_data(db_path, table_name)
}

fn sqlite_type_to_column_type(typ: &str) -> ColumnType {
    match typ.to_uppercase().as_str() {
        "INTEGER" | "INT" | "BIGINT" | "SMALLINT" | "TINYINT" => ColumnType::Integer,
        "REAL" | "FLOAT" | "DOUBLE" | "NUMERIC" | "DECIMAL" => ColumnType::Float,
        "TEXT" | "VARCHAR" | "CHAR" | "CLOB" => ColumnType::Text,
        "BLOB" | "" => ColumnType::Unknown,
        _ => ColumnType::Unknown,
    }
}

fn sqlite_type_to_column_type_by_name(col_name: &str) -> ColumnType {
    let lower = col_name.to_lowercase();
    if lower.contains("id") || lower == "start" || lower == "end" || lower == "cbid" || lower == "correlation" {
        ColumnType::Integer
    } else if lower.contains("value") || lower.contains("duration") || lower.contains("time") || lower.contains("throughput") || lower.contains("occupancy") || lower.contains("ratio") || lower.contains("rate") || lower.contains("percent") {
        ColumnType::Float
    } else {
        ColumnType::Text
    }
}

fn sqlite_value_to_column_value(value: rusqlite::types::Value) -> ColumnValue {
    match value {
        rusqlite::types::Value::Null => ColumnValue::Null,
        rusqlite::types::Value::Integer(i) => ColumnValue::Integer(i),
        rusqlite::types::Value::Real(f) => ColumnValue::Float(f),
        rusqlite::types::Value::Text(s) => ColumnValue::Text(s),
        rusqlite::types::Value::Blob(_) => ColumnValue::Null,
    }
}
