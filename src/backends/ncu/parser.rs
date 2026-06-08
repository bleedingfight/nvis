use anyhow::Result;
use std::path::Path;

use crate::core::types::{
    ColumnSchema, ColumnType, ColumnValue, ProfilerData, ViewCategory, ViewDescriptor,
};

/// Parse an NCU CSV export file into ProfilerData and derive view descriptors.
pub fn parse_csv(path: &Path) -> Result<(ProfilerData, Vec<ViewDescriptor>)> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(path)?;

    let headers = reader.headers()?.clone();
    let header_strs: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

    let schema: Vec<ColumnSchema> = header_strs
        .iter()
        .map(|name| ColumnSchema {
            name: name.clone(),
            dtype: ColumnType::Unknown, // Will refine after seeing data
        })
        .collect();

    let col_count = schema.len();
    let mut columns: Vec<Vec<ColumnValue>> = vec![Vec::new(); col_count];
    let mut row_count = 0usize;

    for result in reader.records() {
        let record = result?;
        for (i, field) in record.iter().enumerate() {
            if i >= col_count {
                break;
            }
            let val = parse_field(field);
            columns[i].push(val);
        }
        // Fill missing columns with Null
        for i in record.len()..col_count {
            columns[i].push(ColumnValue::Null);
        }
        row_count += 1;
    }

    // Refine schema types based on data
    let schema = schema
        .into_iter()
        .enumerate()
        .map(|(i, mut s)| {
            s.dtype = infer_column_type(&columns[i], row_count);
            s
        })
        .collect();

    let data = ProfilerData {
        schema,
        columns,
        row_count,
    };

    let views = derive_views(&data);

    Ok((data, views))
}

/// Compute summary statistics for NCU metrics across all kernels.
pub fn compute_ncu_stats(data: &ProfilerData) -> Result<ProfilerData> {
    let mut metric_names = Vec::new();
    let mut metric_means = Vec::new();
    let mut metric_mins = Vec::new();
    let mut metric_maxs = Vec::new();
    let mut metric_medians = Vec::new();

    for (col_idx, col_schema) in data.schema.iter().enumerate() {
        if col_schema.dtype != ColumnType::Float && col_schema.dtype != ColumnType::Integer {
            continue;
        }
        let values: Vec<f64> = data.columns[col_idx]
            .iter()
            .filter_map(|v| match v {
                ColumnValue::Float(f) => Some(*f),
                ColumnValue::Integer(i) => Some(*i as f64),
                _ => None,
            })
            .collect();

        if values.is_empty() || values.len() < data.row_count / 2 {
            continue;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mut sorted = values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = if sorted.is_empty() {
            0.0
        } else {
            sorted[sorted.len() / 2]
        };

        metric_names.push(ColumnValue::Text(col_schema.name.clone()));
        metric_means.push(ColumnValue::Float(mean));
        metric_mins.push(ColumnValue::Float(min));
        metric_maxs.push(ColumnValue::Float(max));
        metric_medians.push(ColumnValue::Float(median));
    }

    let row_count = metric_names.len();
    let schema = vec![
        ColumnSchema { name: "metric".into(), dtype: ColumnType::Text },
        ColumnSchema { name: "mean".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "min".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "max".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "median".into(), dtype: ColumnType::Float },
    ];

    Ok(ProfilerData {
        schema,
        columns: vec![metric_names, metric_means, metric_mins, metric_maxs, metric_medians],
        row_count,
    })
}

fn parse_field(field: &str) -> ColumnValue {
    if field.is_empty() || field == "N/A" || field == "-" {
        return ColumnValue::Null;
    }
    if let Ok(i) = field.parse::<i64>() {
        return ColumnValue::Integer(i);
    }
    if let Ok(f) = field.parse::<f64>() {
        return ColumnValue::Float(f);
    }
    ColumnValue::Text(field.to_string())
}

fn infer_column_type(col: &[ColumnValue], row_count: usize) -> ColumnType {
    if row_count == 0 {
        return ColumnType::Unknown;
    }
    let int_count = col.iter().filter(|v| matches!(v, ColumnValue::Integer(_))).count();
    let float_count = col.iter().filter(|v| matches!(v, ColumnValue::Float(_))).count();
    let text_count = col.iter().filter(|v| matches!(v, ColumnValue::Text(_))).count();
    let numeric_count = int_count + float_count;

    if numeric_count > row_count / 2 {
        if float_count > 0 {
            ColumnType::Float
        } else {
            ColumnType::Integer
        }
    } else if text_count > row_count / 2 {
        ColumnType::Text
    } else {
        ColumnType::Unknown
    }
}

fn derive_views(data: &ProfilerData) -> Vec<ViewDescriptor> {
    let has_occupancy = data.schema.iter().any(|s| s.name.contains("Occupancy"));
    let has_throughput = data.schema.iter().any(|s| s.name.contains("Throughput"));
    let has_kernel_name = data
        .schema
        .iter()
        .any(|s| s.name.contains("Kernel Name") || s.name.contains("Kernel"));

    let mut views = Vec::new();

    if has_kernel_name {
        views.push(ViewDescriptor {
            id: "kernel_metrics".into(),
            display_name: "Kernel Metrics".into(),
            category: if has_occupancy || has_throughput {
                ViewCategory::Metrics
            } else {
                ViewCategory::RawData
            },
            default_viz: Some(if has_occupancy || has_throughput {
                "statistics"
            } else {
                "barchart"
            }
            .to_string()),
        });
    }

    if has_occupancy {
        views.push(ViewDescriptor {
            id: "occupancy".into(),
            display_name: "SM Occupancy".into(),
            category: ViewCategory::Metrics,
            default_viz: Some("barchart".into()),
        });
    }

    if has_throughput {
        views.push(ViewDescriptor {
            id: "throughput".into(),
            display_name: "Memory Throughput".into(),
            category: ViewCategory::Metrics,
            default_viz: Some("barchart".into()),
        });
    }

    views.push(ViewDescriptor {
        id: "raw".into(),
        display_name: "Raw Data".into(),
        category: ViewCategory::RawData,
        default_viz: Some("barchart".into()),
    });

    if views.is_empty() {
        views.push(ViewDescriptor {
            id: "default".into(),
            display_name: "Data".into(),
            category: ViewCategory::RawData,
            default_viz: Some("barchart".into()),
        });
    }

    views
}
