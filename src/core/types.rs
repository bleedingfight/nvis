#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Integer,
    Float,
    Text,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub name: String,
    pub dtype: ColumnType,
}

#[derive(Debug, Clone)]
pub enum ColumnValue {
    Null,
    Integer(i64),
    Float(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewCategory {
    Timeline,
    Statistics,
    Metrics,
    Metadata,
    RawData,
}

#[derive(Debug, Clone)]
pub struct ViewDescriptor {
    pub id: String,
    pub display_name: String,
    pub category: ViewCategory,
    pub default_viz: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfilerData {
    pub schema: Vec<ColumnSchema>,
    pub columns: Vec<Vec<ColumnValue>>,
    pub row_count: usize,
}

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => format!("{}...", &s[..byte_idx]),
        None => s.to_string(),
    }
}

impl ProfilerData {
    pub fn empty() -> Self {
        Self {
            schema: Vec::new(),
            columns: Vec::new(),
            row_count: 0,
        }
    }

    pub fn float_column(&self, name: &str) -> Option<Vec<f64>> {
        let idx = self.schema.iter().position(|s| s.name == name)?;
        Some(
            self.columns[idx]
                .iter()
                .filter_map(|v| match v {
                    ColumnValue::Float(f) => Some(*f),
                    ColumnValue::Integer(i) => Some(*i as f64),
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn text_column(&self, name: &str) -> Option<Vec<String>> {
        let idx = self.schema.iter().position(|s| s.name == name)?;
        Some(
            self.columns[idx]
                .iter()
                .filter_map(|v| match v {
                    ColumnValue::Text(s) => Some(s.clone()),
                    ColumnValue::Integer(i) => Some(i.to_string()),
                    ColumnValue::Float(f) => Some(format!("{:.2}", f)),
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.schema.iter().position(|s| s.name == name)
    }

    pub fn to_row_strings(&self) -> (Vec<String>, Vec<Vec<String>>) {
        let headers = self.schema.iter().map(|s| s.name.clone()).collect();
        let rows = (0..self.row_count)
            .map(|row_idx| {
                self.columns
                    .iter()
                    .map(|col| match &col.get(row_idx) {
                        Some(ColumnValue::Null) => "NULL".to_string(),
                        Some(ColumnValue::Integer(i)) => i.to_string(),
                        Some(ColumnValue::Float(f)) => format!("{:.2}", f),
                        Some(ColumnValue::Text(s)) => s.clone(),
                        None => "NULL".to_string(),
                    })
                    .collect()
            })
            .collect();
        (headers, rows)
    }
}
