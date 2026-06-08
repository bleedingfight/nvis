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

    if tn.contains("CUPTI_ACTIVITY_KIND_MEMCPY") {
        return get_memcpy_data(conn, view_id);
    }

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
    let cl = |c: &str| c.to_lowercase();
    Ok(cols.iter().any(|c| {
        let lc = cl(c);
        lc == "nameid"
            || lc == "name_id"
            || (lc.contains("name") && lc.contains("id"))
            || lc == "demangledname"
            || lc == "shortname"
            || lc == "mangledname"
    }))
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

    // Find which column to JOIN on StringIds: prefer nameId, else demangledName/shortName
    let join_col = col_info
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("nameId"))
        .or_else(|| col_info.iter().find(|(name, _)| name.eq_ignore_ascii_case("demangledName")))
        .or_else(|| col_info.iter().find(|(name, _)| name.eq_ignore_ascii_case("shortName")))
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "nameId".to_string());

    let query = format!(
        "SELECT r.*, s.value as name FROM {} r LEFT JOIN StringIds s ON s.id = r.{} LIMIT {}",
        table, join_col, limit
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

/// Execute an arbitrary SQL query and return results as ProfilerData.
/// Limits output to 500 rows for safety.
/// Also supports sqlite3 dot-commands (.tables, .schema, etc.) by translating them to SQL.
pub fn execute_sql(conn: &Connection, sql: &str) -> Result<ProfilerData> {
    let trimmed = sql.trim();
    if let Some(dot_cmd) = translate_dot_command(trimmed) {
        return execute_sql(conn, &dot_cmd);
    }

    let mut stmt = conn.prepare(trimmed)?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();

    let mut schema = Vec::new();
    let mut columns: Vec<Vec<ColumnValue>> = Vec::new();
    for name in &col_names {
        schema.push(ColumnSchema {
            name: name.clone(),
            dtype: ColumnType::Unknown,
        });
        columns.push(Vec::new());
    }

    let mut row_count = 0usize;
    let rows = stmt.query_map([], |row| {
        let mut vals = Vec::new();
        for i in 0..col_count {
            let val: Result<String, _> = row.get(i);
            vals.push(val.unwrap_or_else(|_| "NULL".to_string()));
        }
        Ok(vals)
    })?;

    for row_result in rows {
        if row_count >= 500 {
            break;
        }
        let vals = row_result?;
        for (i, v) in vals.into_iter().enumerate() {
            columns[i].push(ColumnValue::Text(v));
        }
        row_count += 1;
    }

    if row_count > 0 {
        for (i, col) in columns.iter_mut().enumerate() {
            if let Some(ColumnValue::Text(s)) = col.first() {
                let dtype = if s.parse::<i64>().is_ok() {
                    ColumnType::Integer
                } else if s.parse::<f64>().is_ok() {
                    ColumnType::Float
                } else {
                    ColumnType::Text
                };
                schema[i].dtype = dtype;
            }
        }
    }

    Ok(ProfilerData {
        schema,
        columns,
        row_count,
    })
}

/// Translate sqlite3 dot-commands to equivalent SQL.
fn translate_dot_command(input: &str) -> Option<String> {
    if !input.starts_with('.') {
        return None;
    }
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).unwrap_or(&"").trim();

    match cmd.as_str() {
        ".tables" | ".table" => {
            if arg.is_empty() {
                Some("SELECT name FROM sqlite_master WHERE type IN ('table','view') ORDER BY name".into())
            } else {
                Some(format!(
                    "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name LIKE '%{}%' ORDER BY name",
                    arg.replace('\'', "''")
                ))
            }
        }
        ".schema" => {
            if arg.is_empty() {
                Some("SELECT sql FROM sqlite_master WHERE type IN ('table','view','index','trigger') ORDER BY name".into())
            } else {
                Some(format!(
                    "SELECT sql FROM sqlite_master WHERE type IN ('table','view','index','trigger') AND name LIKE '%{}%'",
                    arg.replace('\'', "''")
                ))
            }
        }
        ".indexes" | ".index" => {
            if arg.is_empty() {
                Some("SELECT name, tbl_name FROM sqlite_master WHERE type='index' ORDER BY name".into())
            } else {
                Some(format!(
                    "SELECT name, tbl_name FROM sqlite_master WHERE type='index' AND tbl_name LIKE '%{}%' ORDER BY name",
                    arg.replace('\'', "''")
                ))
            }
        }
        ".databases" | ".dbs" => {
            Some("PRAGMA database_list".into())
        }
        ".headers" => None,
        ".mode" => None,
        ".quit" | ".exit" => None,
        _ => None,
    }
}

fn sqlite_type_to_column_type(typ: &str) -> ColumnType {
    match typ.to_uppercase().as_str() {
        "INTEGER" | "INT" | "BIGINT" | "SMALLINT" | "TINYINT" => ColumnType::Integer,
        "REAL" | "FLOAT" | "DOUBLE" | "NUMERIC" | "DECIMAL" => ColumnType::Float,
        "TEXT" | "VARCHAR" | "CHAR" | "CLOB" => ColumnType::Text,
        "BLOB" => ColumnType::Unknown,
        "" => ColumnType::Unknown,
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
