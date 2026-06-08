use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct NcuRecord {
    pub id: String,
    pub kernel_name: String,
    pub block_size: String,
    pub grid_size: String,
    pub section_name: String,
    pub metric_name: String,
    pub metric_unit: String,
    pub metric_value: String,
}

#[derive(Debug)]
pub struct NcuData {
    pub records: Vec<NcuRecord>,
    pub kernel_ids: Vec<String>,
    pub metric_names: Vec<String>,
}

/// Parse a metric value string that may contain commas as thousands separators.
fn parse_metric_value(s: &str) -> Option<f64> {
    let cleaned: String = s.chars().filter(|c| *c != ',').collect();
    cleaned.parse().ok()
}

/// Sanitize a metric name to be a valid SQLite column name.
/// NCU CSV uses patterns like `Compute (SM) Throughput (%)`
/// which should become `Compute__SM_Throughput__pct`.
fn sanitize_col_name(m: &str) -> String {
    let s = m
        .replace('%', "pct")
        .replace('/', "_")
        .replace(' ', "_")
        .replace('(', "_")
        .replace(')', "")
        .replace('[', "_")
        .replace(']', "");
    // Collapse multiple consecutive underscores
    let mut result = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for c in s.chars() {
        if c == '_' {
            if !prev_underscore {
                result.push(c);
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    result
}

/// Shorten a kernel name for display.
pub fn shorten_kernel_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        // Try to extract the template name before the first '<'
        if let Some(pos) = name.find('<') {
            let prefix = &name[..pos];
            if prefix.len() <= max_len - 3 {
                format!("{}...", prefix)
            } else {
                format!("{}...", &name[..max_len - 3])
            }
        } else {
            format!("{}...", &name[..max_len - 3])
        }
    }
}

pub fn parse_ncu_csv(path: &Path) -> Result<NcuData> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true) // NCU CSV can have variable field counts
        .from_path(path)
        .context("Failed to open CSV file")?;

    let mut records = Vec::new();
    let mut kernel_id_set = std::collections::HashSet::new();
    let mut metric_name_set = std::collections::HashSet::new();
    let mut kernel_ids = Vec::new();

    for result in reader.records() {
        let row = result.context("Failed to read CSV record")?;

        let id = row.get(0).unwrap_or("").to_string();
        let kernel_name = row.get(4).unwrap_or("").to_string();
        let block_size = row.get(7).unwrap_or("").to_string();
        let grid_size = row.get(8).unwrap_or("").to_string();
        let section_name = row.get(11).unwrap_or("").to_string();
        let metric_name = row.get(12).unwrap_or("").to_string();
        let metric_unit = row.get(13).unwrap_or("").to_string();
        let metric_value = row.get(14).unwrap_or("").to_string();

        // Skip empty metric rows (section header rows in NCU CSV)
        if metric_name.is_empty() {
            continue;
        }

        if kernel_id_set.insert(id.clone()) {
            kernel_ids.push(id.clone());
        }

        // Store metric name with unit suffix for uniqueness
        let metric_key = if metric_unit.is_empty() {
            metric_name.clone()
        } else {
            format!("{} ({})", metric_name, metric_unit)
        };
        metric_name_set.insert(metric_key);

        records.push(NcuRecord {
            id,
            kernel_name,
            block_size,
            grid_size,
            section_name,
            metric_name,
            metric_unit,
            metric_value,
        });
    }

    let mut metric_names: Vec<String> = metric_name_set.into_iter().collect();
    metric_names.sort();

    Ok(NcuData {
        records,
        kernel_ids,
        metric_names,
    })
}

pub fn csv_to_sqlite(ncu_data: &NcuData) -> Result<Connection> {
    let conn = Connection::open_in_memory()?;

    // Build the wide-format kernels table
    // Group records by (ID, Kernel Name)
    let mut kernel_map: BTreeMap<(String, String), BTreeMap<String, f64>> = BTreeMap::new();
    let mut kernel_meta: BTreeMap<(String, String), (String, String)> = BTreeMap::new(); // (block_size, grid_size)

    for rec in &ncu_data.records {
        let key = (rec.id.clone(), rec.kernel_name.clone());
        let metric_key = if rec.metric_unit.is_empty() {
            rec.metric_name.clone()
        } else {
            format!("{} ({})", rec.metric_name, rec.metric_unit)
        };

        if let Some(val) = parse_metric_value(&rec.metric_value) {
            kernel_map
                .entry(key.clone())
                .or_insert_with(BTreeMap::new)
                .insert(metric_key, val);
        }

        kernel_meta
            .entry(key.clone())
            .or_insert((rec.block_size.clone(), rec.grid_size.clone()));
    }

    // Create kernels table
    // Fixed columns: ID, Kernel Name, Block Size, Grid Size
    // Plus all metric columns (deduplicated)
    let mut col_defs = vec![
        "ID TEXT".to_string(),
        "Kernel_Name TEXT".to_string(),
        "Block_Size TEXT".to_string(),
        "Grid_Size TEXT".to_string(),
    ];

    let mut seen_cols = std::collections::HashSet::new();
    seen_cols.insert("ID".to_string());
    seen_cols.insert("Kernel_Name".to_string());
    seen_cols.insert("Block_Size".to_string());
    seen_cols.insert("Grid_Size".to_string());

    let mut unique_metric_names: Vec<String> = Vec::new();
    for m in &ncu_data.metric_names {
        let col = sanitize_col_name(m);
        if seen_cols.insert(col.clone()) {
            unique_metric_names.push(m.clone());
            col_defs.push(format!("\"{}\" REAL", col));
        }
    }

    conn.execute_batch(&format!(
        "CREATE TABLE kernels ({});",
        col_defs.join(", ")
    ))?;

    // Insert rows
    let mut col_names = vec![
        "ID".to_string(),
        "Kernel_Name".to_string(),
        "Block_Size".to_string(),
        "Grid_Size".to_string(),
    ];
    for m in &unique_metric_names {
        let col = sanitize_col_name(m);
        col_names.push(format!("\"{}\"", col));
    }

    let insert_sql = format!(
        "INSERT INTO kernels ({}) VALUES ({})",
        col_names.join(", "),
        col_names.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
    );

    for kid in &ncu_data.kernel_ids {
        // Find all kernels with this ID
        let mut seen_kernels = std::collections::HashSet::new();
        for ((id, kname), metrics) in &kernel_map {
            if id != kid {
                continue;
            }
            if seen_kernels.contains(kname) {
                continue;
            }
            seen_kernels.insert(kname.clone());

            let meta = kernel_meta.get(&(id.clone(), kname.clone())).unwrap();

            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            params.push(Box::new(id.clone()));
            params.push(Box::new(kname.clone()));
            params.push(Box::new(meta.0.clone()));
            params.push(Box::new(meta.1.clone()));

            for m in &unique_metric_names {
                let val = metrics.get(m).copied();
                params.push(Box::new(val));
            }

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            conn.execute(&insert_sql, param_refs.as_slice())?;
        }
    }

    // Create metrics table (raw long format)
    conn.execute_batch(
        "CREATE TABLE metrics (
            ID TEXT,
            Kernel_Name TEXT,
            Section_Name TEXT,
            Metric_Name TEXT,
            Metric_Unit TEXT,
            Metric_Value TEXT
        );",
    )?;

    {
        let mut stmt = conn.prepare(
            "INSERT INTO metrics (ID, Kernel_Name, Section_Name, Metric_Name, Metric_Unit, Metric_Value) VALUES (?, ?, ?, ?, ?, ?)"
        )?;
        for rec in &ncu_data.records {
            stmt.execute(rusqlite::params![
                rec.id,
                rec.kernel_name,
                rec.section_name,
                rec.metric_name,
                rec.metric_unit,
                rec.metric_value,
            ])?;
        }
    }

    // Create sections table
    conn.execute_batch(
        "CREATE TABLE sections (
            ID TEXT,
            Kernel_Name TEXT,
            Section_Name TEXT,
            Metric_Count INTEGER
        );",
    )?;

    {
        let mut section_counts: BTreeMap<(String, String, String), usize> = BTreeMap::new();
        for rec in &ncu_data.records {
            if rec.metric_name.is_empty() {
                continue;
            }
            *section_counts
                .entry((rec.id.clone(), rec.kernel_name.clone(), rec.section_name.clone()))
                .or_insert(0) += 1;
        }

        let mut stmt = conn.prepare(
            "INSERT INTO sections (ID, Kernel_Name, Section_Name, Metric_Count) VALUES (?, ?, ?, ?)"
        )?;
        for ((id, kname, sname), count) in &section_counts {
            stmt.execute(rusqlite::params![id, kname, sname, *count as i64])?;
        }
    }

    Ok(conn)
}
