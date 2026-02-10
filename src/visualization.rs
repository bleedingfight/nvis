use crate::app::TableData;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum VisualizationType {
    BarChart,   // 简单柱状图
    BoxPlot,    // 箱线图 - 用于显示统计分布
    Timeline,   // 时间线 - 用于事件序列
    Statistics, // 统计文本 - 用于总体信息
}

#[derive(Debug, Clone)]
pub struct BoxPlotData {
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub mean: f64,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub name: String,
    pub start: f64,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub enum ChartData {
    Bars(Vec<(&'static str, u64)>),
    BoxPlots(Vec<BoxPlotData>),
    Timeline(Vec<TimelineEvent>),
    Stats(String),
}

pub struct Visualization {
    pub _viz_type: VisualizationType, // Keep for future use
    pub data: ChartData,
    pub title: String,
}

/// 检测表格类型并返回适合的可视化类型
pub fn detect_table_type(table_name: &str, data: &TableData) -> VisualizationType {
    let table_lower = table_name.to_lowercase();
    let columns_lower: Vec<String> = data.columns.iter().map(|c| c.to_lowercase()).collect();

    // 先检查是否是事件/内核表 (优先级高于通用CUDA检查)
    if (table_lower.contains("kernel")
        || table_lower.contains("event")
        || table_lower.contains("memcpy"))
        && columns_lower.iter().any(|c| c.contains("start"))
        && columns_lower
            .iter()
            .any(|c| c.contains("end") || c.contains("duration"))
    {
        return VisualizationType::Timeline;
    }

    // 检查是否是CUDA API调用表
    if table_lower.contains("cuda")
        || table_lower.contains("runtime")
        || table_lower.contains("api")
        || columns_lower
            .iter()
            .any(|c| c.contains("correlation") || c.contains("cbid"))
    {
        // 如果有时间相关的列，使用箱线图显示统计信息
        if columns_lower
            .iter()
            .any(|c| c.contains("start") || c.contains("end") || c.contains("duration"))
        {
            return VisualizationType::BoxPlot;
        }
    }

    // 检查是否是统计信息表
    if table_lower.contains("stat")
        || table_lower.contains("summary")
        || table_lower.contains("info")
    {
        return VisualizationType::Statistics;
    }

    // 默认使用柱状图
    VisualizationType::BarChart
}

/// 为CUDA API调用生成箱线图数据
pub fn generate_boxplot_data(data: &TableData) -> Vec<BoxPlotData> {
    // 找到name/function列
    let name_col = data.columns.iter().position(|c| {
        let cl = c.to_lowercase();
        cl.contains("name") || cl.contains("function") || cl.contains("api")
    });

    if name_col.is_none() {
        return Vec::new();
    }

    let name_idx = name_col.unwrap();

    // 找到duration列，或者start/end列来计算duration
    let duration_col = data.columns.iter().position(|c| {
        let cl = c.to_lowercase();
        cl.contains("duration") || cl.contains("time") || cl.contains("elapsed")
    });

    let start_col = data
        .columns
        .iter()
        .position(|c| c.to_lowercase().contains("start"));

    let end_col = data
        .columns
        .iter()
        .position(|c| c.to_lowercase().contains("end"));

    // 如果既没有duration列，也没有start/end列对，则返回空
    if duration_col.is_none() && (start_col.is_none() || end_col.is_none()) {
        return Vec::new();
    }

    // 按函数名分组收集duration数据
    let mut groups: HashMap<String, Vec<f64>> = HashMap::new();

    for row in &data.rows {
        if let Some(name) = row.get(name_idx) {
            let duration = if let Some(dur_idx) = duration_col {
                // 直接从duration列获取
                row.get(dur_idx).and_then(|v| v.parse::<f64>().ok())
            } else if let (Some(start_idx), Some(end_idx)) = (start_col, end_col) {
                // 从start和end计算duration
                if let (Some(start_str), Some(end_str)) = (row.get(start_idx), row.get(end_idx)) {
                    if let (Ok(start), Ok(end)) = (start_str.parse::<f64>(), end_str.parse::<f64>())
                    {
                        Some(end - start)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(dur_value) = duration {
                if dur_value >= 0.0 {
                    // 过滤负数duration
                    groups
                        .entry(name.clone())
                        .or_insert_with(Vec::new)
                        .push(dur_value);
                }
            }
        }
    }

    // 计算每个函数的统计信息
    let mut result: Vec<BoxPlotData> = groups
        .into_iter()
        .map(|(name, mut values)| {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let count = values.len();

            let min = values[0];
            let max = values[count - 1];
            let mean = values.iter().sum::<f64>() / count as f64;

            let q1 = percentile(&values, 25.0);
            let median = percentile(&values, 50.0);
            let q3 = percentile(&values, 75.0);

            BoxPlotData {
                label: name,
                min,
                max,
                median,
                q1,
                q3,
                mean,
                count,
            }
        })
        .collect();

    // 按平均时间排序
    result.sort_by(|a, b| b.mean.partial_cmp(&a.mean).unwrap());

    // 限制显示前10个
    result.truncate(10);

    result
}

/// 计算百分位数
fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    let n = sorted_values.len();
    let idx = (p / 100.0) * (n - 1) as f64;
    let lower = idx.floor() as usize;
    let upper = idx.ceil() as usize;
    let weight = idx - lower as f64;

    if lower == upper {
        sorted_values[lower]
    } else {
        sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
    }
}

/// 生成时间线数据
pub fn generate_timeline_data(data: &TableData, limit: usize) -> Vec<TimelineEvent> {
    let name_col = data.columns.iter().position(|c| {
        let cl = c.to_lowercase();
        cl.contains("name") || cl.contains("kernel")
    });

    let start_col = data
        .columns
        .iter()
        .position(|c| c.to_lowercase().contains("start"));

    let duration_col = data.columns.iter().position(|c| {
        let cl = c.to_lowercase();
        cl.contains("duration") || cl.contains("time")
    });

    if name_col.is_none() || start_col.is_none() {
        return Vec::new();
    }

    let name_idx = name_col.unwrap();
    let start_idx = start_col.unwrap();

    let mut events = Vec::new();

    for (i, row) in data.rows.iter().enumerate().take(limit) {
        if let Some(name) = row.get(name_idx) {
            let start = row
                .get(start_idx)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(i as f64);

            let duration = duration_col
                .and_then(|idx| row.get(idx))
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0);

            events.push(TimelineEvent {
                name: name.clone(),
                start,
                duration,
            });
        }
    }

    events
}

/// 生成统计文本
pub fn generate_statistics_text(table_name: &str, data: &TableData) -> String {
    let mut text = format!("Table: {}\n", table_name);
    text.push_str(&format!("Columns: {}\n", data.columns.len()));
    text.push_str(&format!("Rows: {}\n\n", data.rows.len()));

    // 尝试找到数值列并计算统计信息
    for (col_idx, col_name) in data.columns.iter().enumerate() {
        let values: Vec<f64> = data
            .rows
            .iter()
            .filter_map(|row| row.get(col_idx))
            .filter_map(|v| v.parse::<f64>().ok())
            .collect();

        if !values.is_empty() && values.len() > data.rows.len() / 2 {
            let sum: f64 = values.iter().sum();
            let mean = sum / values.len() as f64;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            text.push_str(&format!("{}:\n", col_name));
            text.push_str(&format!("  Mean: {:.2}\n", mean));
            text.push_str(&format!("  Min: {:.2}\n", min));
            text.push_str(&format!("  Max: {:.2}\n\n", max));
        }
    }

    text
}

/// 生成简单柱状图数据（保留原有功能）
pub fn generate_bar_chart_data(data: &TableData, scroll: usize) -> Vec<(&'static str, u64)> {
    // 检查是否有数据
    if data.rows.is_empty() {
        return Vec::new();
    }

    let mut numeric_col = None;
    for (col_idx, _col_name) in data.columns.iter().enumerate() {
        if data.rows.iter().any(|row| {
            row.get(col_idx)
                .and_then(|v| v.parse::<f64>().ok())
                .is_some()
        }) {
            numeric_col = Some(col_idx);
            break;
        }
    }

    if let Some(col_idx) = numeric_col {
        let start = scroll.min(data.rows.len().saturating_sub(1));
        let end = (start + 10).min(data.rows.len());

        // 确保 start < end
        if start >= end {
            return Vec::new();
        }

        data.rows[start..end]
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                row.get(col_idx)
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|val| {
                        let label: &'static str =
                            Box::leak(format!("R{}", start + i).into_boxed_str());
                        (label, val.abs() as u64)
                    })
            })
            .collect()
    } else {
        Vec::new()
    }
}

/// 主要的可视化生成函数
pub fn generate_visualization(table_name: &str, data: &TableData, scroll: usize) -> Visualization {
    let viz_type = detect_table_type(table_name, data);

    let (chart_data, title) = match viz_type {
        VisualizationType::BoxPlot => {
            let boxplots = generate_boxplot_data(data);
            let title = format!("{} - API Call Statistics", table_name);
            (ChartData::BoxPlots(boxplots), title)
        }
        VisualizationType::Timeline => {
            let events = generate_timeline_data(data, 20);
            let title = format!("{} - Timeline", table_name);
            (ChartData::Timeline(events), title)
        }
        VisualizationType::Statistics => {
            let stats = generate_statistics_text(table_name, data);
            let title = format!("{} - Statistics", table_name);
            (ChartData::Stats(stats), title)
        }
        VisualizationType::BarChart => {
            let bars = generate_bar_chart_data(data, scroll);
            let title = format!("{} - Data", table_name);
            (ChartData::Bars(bars), title)
        }
    };

    Visualization {
        _viz_type: viz_type,
        data: chart_data,
        title,
    }
}
