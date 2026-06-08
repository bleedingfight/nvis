use anyhow::Result;
use rusqlite::Connection;

use crate::core::types::{
    ColumnSchema, ColumnType, ColumnValue, ProfilerData, ViewCategory, ViewDescriptor,
};

/// Known NSYS table patterns mapped to view metadata.
struct TableMeta {
    category: ViewCategory,
    display_name: String,
    default_viz: &'static str,
}

fn get_table_meta(table_name: &str) -> TableMeta {
    let tn = table_name.to_uppercase();
    if tn.contains("CUPTI_ACTIVITY_KIND_RUNTIME") {
        return TableMeta {
            category: ViewCategory::Statistics,
            display_name: "CUDA Runtime API".into(),
            default_viz: "boxplot",
        };
    }
    if tn.contains("CUPTI_ACTIVITY_KIND_KERNEL") {
        return TableMeta {
            category: ViewCategory::Timeline,
            display_name: "CUDA Kernels".into(),
            default_viz: "timeline",
        };
    }
    if tn.contains("CUPTI_ACTIVITY_KIND_MEMCPY") {
        return TableMeta {
            category: ViewCategory::Timeline,
            display_name: "CUDA Memcpy".into(),
            default_viz: "timeline",
        };
    }
    if tn.starts_with("CUPTI_") {
        return TableMeta {
            category: ViewCategory::RawData,
            display_name: table_name.into(),
            default_viz: "barchart",
        };
    }
    if tn.starts_with("TARGET_INFO_") || tn.starts_with("ANALYSIS_") {
        return TableMeta {
            category: ViewCategory::Metadata,
            display_name: table_name.into(),
            default_viz: "statistics",
        };
    }
    TableMeta {
        category: ViewCategory::RawData,
        display_name: table_name.into(),
        default_viz: "barchart",
    }
}

pub fn list_views(conn: &Connection) -> Result<Vec<ViewDescriptor>> {
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let views = tables
        .into_iter()
        .map(|name| {
            let meta = get_table_meta(&name);
            ViewDescriptor {
                id: name.clone(),
                display_name: meta.display_name.to_string(),
                category: meta.category,
                default_viz: Some(meta.default_viz.to_string()),
            }
        })
        .collect();

    Ok(views)
}

pub fn get_view_data(conn: &Connection, view_id: &str) -> Result<ProfilerData> {
    let tn = view_id.to_uppercase();
    let is_timeline = tn.contains("KERNEL") || tn.contains("MEMCPY") || tn.contains("MEMSET");

    // Specialized MEMCPY path: resolve enum IDs for readable names
    if tn.contains("CUPTI_ACTIVITY_KIND_MEMCPY") {
        return get_memcpy_data(conn, view_id);
    }

    // Check if the table has a nameId column and StringIds table exists
    let has_name_id = has_nameid_column(conn, view_id)?;
    let has_string_ids = has_stringids_table(conn)?;

    let limit = if is_timeline { 50000 } else { 1000 };

    if has_name_id && has_string_ids {
        get_view_data_resolved(conn, view_id, limit)
    } else {
        get_view_data_plain(conn, view_id, limit)
    }
}

fn has_nameid_column(conn: &Connection, table: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(cols
        .iter()
        .any(|c| c.eq_ignore_ascii_case("nameId") || c.to_lowercase() == "nameid"))
}

fn has_stringids_table(conn: &Connection) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='StringIds'",
    )?;
    let count: i64 = stmt.query_row([], |row| row.get(0))?;
    Ok(count > 0)
}

fn get_view_data_plain(conn: &Connection, table: &str, limit: usize) -> Result<ProfilerData> {
    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table))?;
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

    let query = format!("SELECT * FROM {} LIMIT {}", table, limit);
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

    // Rebuild schema with actual column names from SELECT
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

fn get_view_data_resolved(conn: &Connection, table: &str, limit: usize) -> Result<ProfilerData> {
    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table))?;
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

    let query = format!(
        "SELECT r.*, s.value as name FROM {} r LEFT JOIN StringIds s ON s.id = r.nameId LIMIT {}",
        table, limit
    );

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
        .enumerate()
        .map(|(i, name)| {
            if i < schema.len() {
                ColumnSchema {
                    name: name.clone(),
                    dtype: schema[i].dtype.clone(),
                }
            } else {
                // The appended 'name' column from LEFT JOIN
                ColumnSchema {
                    name: name.clone(),
                    dtype: ColumnType::Text,
                }
            }
        })
        .collect();

    Ok(ProfilerData {
        schema: final_schema,
        columns,
        row_count,
    })
}

fn sqlite_type_to_column_type(typ: &str) -> ColumnType {
    match typ.to_uppercase().as_str() {
        "INTEGER" | "INT" | "BIGINT" | "SMALLINT" | "TINYINT" => ColumnType::Integer,
        "REAL" | "FLOAT" | "DOUBLE" | "NUMERIC" | "DECIMAL" => ColumnType::Float,
        "TEXT" | "VARCHAR" | "CHAR" | "CLOB" => ColumnType::Text,
        "BLOB" => ColumnType::Unknown,
        "" => ColumnType::Unknown, // SQLite allows typeless columns
        _ => ColumnType::Unknown,
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

fn get_memcpy_data(conn: &Connection, table: &str) -> Result<ProfilerData> {
    let mut pragma = conn.prepare(&format!("PRAGMA table_info({})", table))?;
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

    // Build a query that resolves enum IDs and nameId for MEMCPY
    let query = format!(
        "SELECT r.*, \
         s.value as name, \
         ec.label as copyKind, \
         es.label as srcKind, \
         ed.label as dstKind \
         FROM {} r \
         LEFT JOIN StringIds s ON s.id = r.nameId \
         LEFT JOIN ENUM_CUDA_MEMCPY_OPER ec ON ec.id = r.copyKind \
         LEFT JOIN ENUM_CUDA_MEM_KIND es ON es.id = r.srcKind \
         LEFT JOIN ENUM_CUDA_MEM_KIND ed ON ed.id = r.dstKind \
         ORDER BY r.start \
         LIMIT 50000",
        table
    );

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
        .enumerate()
        .map(|(i, name)| {
            if i < schema.len() {
                ColumnSchema {
                    name: name.clone(),
                    dtype: schema[i].dtype.clone(),
                }
            } else {
                ColumnSchema {
                    name: name.clone(),
                    dtype: ColumnType::Text,
                }
            }
        })
        .collect();

    Ok(ProfilerData {
        schema: final_schema,
        columns,
        row_count,
    })
}
