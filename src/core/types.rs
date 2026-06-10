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

#[derive(Debug, Clone, PartialEq)]
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

    pub fn without_all_null_columns(&self) -> ProfilerData {
        if self.row_count == 0 || self.columns.is_empty() {
            return self.clone();
        }
        let keep: Vec<usize> = self
            .columns
            .iter()
            .enumerate()
            .filter(|(_, col)| col.iter().any(|v| *v != ColumnValue::Null))
            .map(|(i, _)| i)
            .collect();
        if keep.len() == self.columns.len() {
            return self.clone();
        }
        ProfilerData {
            schema: keep.iter().map(|&i| self.schema[i].clone()).collect(),
            columns: keep.iter().map(|&i| self.columns[i].clone()).collect(),
            row_count: self.row_count,
        }
    }

    pub fn without_redundant_name_columns(&self) -> ProfilerData {
        let skip: Vec<usize> = self
            .schema
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                let lc = s.name.to_lowercase();
                lc == "demangledname" || lc == "mangledname"
            })
            .map(|(i, _)| i)
            .collect();
        if skip.is_empty() {
            return self.clone();
        }
        let keep: Vec<usize> = (0..self.columns.len()).filter(|i| !skip.contains(i)).collect();
        ProfilerData {
            schema: keep.iter().map(|&i| self.schema[i].clone()).collect(),
            columns: keep.iter().map(|&i| self.columns[i].clone()).collect(),
            row_count: self.row_count,
        }
    }

    pub fn with_merged_grid_block(&self) -> ProfilerData {
        fn find_xyz_cols(schema: &[ColumnSchema], base: &str) -> Option<[usize; 3]> {
            let x = schema.iter().position(|s| s.name.eq_ignore_ascii_case(&format!("{base}X")))?;
            let y = schema.iter().position(|s| s.name.eq_ignore_ascii_case(&format!("{base}Y")))?;
            let z = schema.iter().position(|s| s.name.eq_ignore_ascii_case(&format!("{base}Z")))?;
            Some([x, y, z])
        }
        fn fmt_xyz(cols: &[Vec<ColumnValue>; 3], row: usize) -> ColumnValue {
            let x = cols[0].get(row);
            let y = cols[1].get(row);
            let z = cols[2].get(row);
            match (x, y, z) {
                (Some(ColumnValue::Integer(a)), Some(ColumnValue::Integer(b)), Some(ColumnValue::Integer(c))) => {
                    ColumnValue::Text(format!("({},{},{})", a, b, c))
                }
                _ => ColumnValue::Null,
            }
        }

        let grid_xyz = find_xyz_cols(&self.schema, "grid");
        let block_xyz = find_xyz_cols(&self.schema, "block");

        if grid_xyz.is_none() && block_xyz.is_none() {
            return self.clone();
        }

        let skip: Vec<usize> = [grid_xyz, block_xyz]
            .iter()
            .flat_map(|opt| opt.into_iter().flat_map(|a| a.iter().copied()))
            .collect();

        // Find shortName / name column to put kernel first
        let name_col = self.schema.iter().position(|s| {
            let lc = s.name.to_lowercase();
            lc == "shortname" || lc == "nameid" || lc == "name_id" || lc == "name"
        });

        let mut new_schema = Vec::new();
        let mut new_columns = Vec::new();

        // First: name column
        if let Some(ni) = name_col {
            new_schema.push(self.schema[ni].clone());
            new_columns.push(self.columns[ni].clone());
        }

        // Then: merged grid
        if let Some([x, y, z]) = grid_xyz {
            new_schema.push(ColumnSchema { name: "grid".into(), dtype: ColumnType::Text });
            new_columns.push((0..self.row_count)
                .map(|r| fmt_xyz(&[self.columns[x].clone(), self.columns[y].clone(), self.columns[z].clone()], r))
                .collect());
        }

        // Then: merged block
        if let Some([x, y, z]) = block_xyz {
            new_schema.push(ColumnSchema { name: "block".into(), dtype: ColumnType::Text });
            new_columns.push((0..self.row_count)
                .map(|r| fmt_xyz(&[self.columns[x].clone(), self.columns[y].clone(), self.columns[z].clone()], r))
                .collect());
        }

        // Then: remaining columns in original order, skipping merged/split ones and name
        let all_skip: Vec<usize> = skip.iter().chain(name_col.iter()).copied().collect();
        for (i, s) in self.schema.iter().enumerate() {
            if !all_skip.contains(&i) {
                new_schema.push(s.clone());
                new_columns.push(self.columns[i].clone());
            }
        }

        ProfilerData {
            schema: new_schema,
            columns: new_columns,
            row_count: self.row_count,
        }
    }

    pub fn without_hidden_columns(&self, hidden: &std::collections::HashSet<String>) -> ProfilerData {
        if hidden.is_empty() {
            return self.clone();
        }
        let keep: Vec<usize> = self
            .schema
            .iter()
            .enumerate()
            .filter(|(_, s)| !hidden.contains(&s.name))
            .map(|(i, _)| i)
            .collect();
        if keep.len() == self.columns.len() {
            return self.clone();
        }
        ProfilerData {
            schema: keep.iter().map(|&i| self.schema[i].clone()).collect(),
            columns: keep.iter().map(|&i| self.columns[i].clone()).collect(),
            row_count: self.row_count,
        }
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
