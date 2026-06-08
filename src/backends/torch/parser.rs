use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::core::types::{
    ColumnSchema, ColumnType, ColumnValue, ProfilerData, ViewCategory, ViewDescriptor,
};

#[cfg(feature = "torch")]
use serde::Deserialize;

#[cfg_attr(feature = "torch", derive(Deserialize))]
#[derive(Debug, Clone)]
struct TraceEvent {
    name: String,
    cat: String,
    ts: f64,
    dur: Option<f64>,
    pid: i64,
    tid: i64,
}

/// Parse a Chrome Trace JSON file into ProfilerData.
pub fn parse_trace(path: &Path) -> Result<(ProfilerData, Vec<ViewDescriptor>)> {
    let content = std::fs::read_to_string(path)?;

    // Chrome trace format: { "traceEvents": [...], ... }  OR  just [...]
    let events: Vec<TraceEvent> = if let Ok(wrapper) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
        if let Some(arr) = wrapper.get("traceEvents") {
            serde_json::from_value(arr.clone())?
        } else {
            Vec::new()
        }
    } else {
        serde_json::from_str(&content)?
    };

    let schema = vec![
        ColumnSchema { name: "name".into(), dtype: ColumnType::Text },
        ColumnSchema { name: "cat".into(), dtype: ColumnType::Text },
        ColumnSchema { name: "ts_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "duration_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "pid".into(), dtype: ColumnType::Integer },
        ColumnSchema { name: "tid".into(), dtype: ColumnType::Integer },
    ];

    let col_count = schema.len();
    let mut columns: Vec<Vec<ColumnValue>> = vec![Vec::new(); col_count];
    let mut row_count = 0usize;
    let mut categories: HashMap<String, usize> = HashMap::new();

    for event in &events {
        // Convert ts from microseconds to microseconds (usually already in us for Chrome trace)
        // dur is in microseconds too
        let duration = event.dur.unwrap_or(0.0);

        columns[0].push(ColumnValue::Text(event.name.clone()));
        columns[1].push(ColumnValue::Text(event.cat.clone()));
        columns[2].push(ColumnValue::Float(event.ts));
        columns[3].push(ColumnValue::Float(duration));
        columns[4].push(ColumnValue::Integer(event.pid));
        columns[5].push(ColumnValue::Integer(event.tid));
        row_count += 1;

        *categories.entry(event.cat.clone()).or_insert(0) += 1;
    }

    let data = ProfilerData {
        schema,
        columns,
        row_count,
    };

    let views = derive_views(&categories);

    Ok((data, views))
}

/// Compute op-level aggregation statistics.
pub fn compute_torch_stats(data: &ProfilerData) -> Result<ProfilerData> {
    let name_col = data.column_index("name");
    let dur_col = data.column_index("duration_us");

    if name_col.is_none() || dur_col.is_none() {
        return Ok(ProfilerData::empty());
    }

    let name_idx = name_col.unwrap();
    let dur_idx = dur_col.unwrap();

    let mut groups: HashMap<String, Vec<f64>> = HashMap::new();

    for row_idx in 0..data.row_count {
        let name = match &data.columns[name_idx].get(row_idx) {
            Some(ColumnValue::Text(s)) => s.clone(),
            _ => continue,
        };
        let dur = match data.columns[dur_idx].get(row_idx) {
            Some(ColumnValue::Float(f)) => *f,
            Some(ColumnValue::Integer(i)) => *i as f64,
            _ => continue,
        };
        if dur > 0.0 {
            groups.entry(name).or_default().push(dur);
        }
    }

    let schema = vec![
        ColumnSchema { name: "name".into(), dtype: ColumnType::Text },
        ColumnSchema { name: "calls".into(), dtype: ColumnType::Integer },
        ColumnSchema { name: "total_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "mean_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "p50_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "p95_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "p99_us".into(), dtype: ColumnType::Float },
    ];

    let mut entries: Vec<(String, Vec<f64>)> = groups.into_iter().collect();
    entries.sort_by(|a, b| {
        let mean_a = a.1.iter().sum::<f64>() / a.1.len() as f64;
        let mean_b = b.1.iter().sum::<f64>() / b.1.len() as f64;
        mean_b.partial_cmp(&mean_a).unwrap()
    });
    entries.truncate(50);

    let col_count = schema.len();
    let mut columns: Vec<Vec<ColumnValue>> = vec![Vec::new(); col_count];
    let mut row_count = 0usize;

    for (name, mut values) in entries {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let calls = values.len() as i64;
        let total: f64 = values.iter().sum();
        let mean = total / calls as f64;
        let p50 = percentile_val(&values, 50.0);
        let p95 = percentile_val(&values, 95.0);
        let p99 = percentile_val(&values, 99.0);

        columns[0].push(ColumnValue::Text(name));
        columns[1].push(ColumnValue::Integer(calls));
        columns[2].push(ColumnValue::Float(total));
        columns[3].push(ColumnValue::Float(mean));
        columns[4].push(ColumnValue::Float(p50));
        columns[5].push(ColumnValue::Float(p95));
        columns[6].push(ColumnValue::Float(p99));
        row_count += 1;
    }

    Ok(ProfilerData {
        schema,
        columns,
        row_count,
    })
}

fn percentile_val(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

fn derive_views(categories: &HashMap<String, usize>) -> Vec<ViewDescriptor> {
    let mut views = Vec::new();

    let has_cpu_op = categories.keys().any(|k| k.contains("cpu_op"));
    let has_cuda = categories.keys().any(|k| k.contains("cuda") || k.contains("kernel"));
    let has_python = categories.keys().any(|k| k.contains("python"));

    if has_cpu_op {
        views.push(ViewDescriptor {
            id: "cpu_ops".into(),
            display_name: "CPU Ops".into(),
            category: ViewCategory::Timeline,
            default_viz: Some("timeline".into()),
        });
    }

    if has_cuda {
        views.push(ViewDescriptor {
            id: "cuda_kernels".into(),
            display_name: "CUDA Kernels".into(),
            category: ViewCategory::Timeline,
            default_viz: Some("timeline".into()),
        });
    }

    if has_python {
        views.push(ViewDescriptor {
            id: "python_calls".into(),
            display_name: "Python Calls".into(),
            category: ViewCategory::RawData,
            default_viz: Some("barchart".into()),
        });
    }

    views.push(ViewDescriptor {
        id: "all_events".into(),
        display_name: "All Events".into(),
        category: ViewCategory::RawData,
        default_viz: Some("barchart".into()),
    });

    views.push(ViewDescriptor {
        id: "stats".into(),
        display_name: "Op Statistics".into(),
        category: ViewCategory::Statistics,
        default_viz: Some("boxplot".into()),
    });

    if views.is_empty() {
        views.push(ViewDescriptor {
            id: "default".into(),
            display_name: "Events".into(),
            category: ViewCategory::RawData,
            default_viz: Some("barchart".into()),
        });
    }

    views
}
