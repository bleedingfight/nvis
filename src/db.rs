use rusqlite::{Connection, Result, Row};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub row_count: usize,
}

#[derive(Debug, Clone)]
pub struct TableData {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub start_time: f64,
    pub end_time: f64,
    pub duration: f64,
    pub depth: usize,
}

pub struct NsysDatabase {
    conn: Option<Connection>,
    pub tables: Vec<TableInfo>,
    pub function_calls: Vec<FunctionCall>,
}

impl NsysDatabase {
    pub fn new() -> Self {
        Self {
            conn: None,
            tables: Vec::new(),
            function_calls: Vec::new(),
        }
    }

    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        log::debug!("打开数据库连接: {}", path.display());
        let conn = Connection::open(path)?;

        // 获取所有表名
        let table_names: Vec<String> = {
            let mut stmt =
                conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            log::debug!("找到 {} 个表", names.len());
            names
        };

        // 获取每个表的行数
        let mut tables = Vec::new();
        for table_name in table_names {
            let count_query = format!("SELECT COUNT(*) FROM \"{}\"", table_name);
            let row_count: usize = conn
                .query_row(&count_query, [], |row| row.get(0))
                .unwrap_or(0);
            log::trace!("表 '{}' 有 {} 行", table_name, row_count);
            tables.push(TableInfo {
                name: table_name,
                row_count,
            });
        }

        // 尝试解析函数调用数据
        // NSYS通常在CUPTI_ACTIVITY_KIND_KERNEL表中存储内核调用
        // 或在NVTX_EVENTS表中存储标记事件
        log::debug!("开始解析函数调用数据");
        let function_calls = self.parse_function_calls(&conn);

        self.tables = tables;
        self.function_calls = function_calls;
        self.conn = Some(conn);

        Ok(())
    }

    fn parse_function_calls(&self, conn: &Connection) -> Vec<FunctionCall> {
        let mut calls = Vec::new();

        // 尝试多种可能的表名和列名组合
        let queries = vec![
            // NVIDIA Nsight Systems 常见格式
            "SELECT demangledName as name, start, end, (end - start) as duration FROM CUPTI_ACTIVITY_KIND_KERNEL ORDER BY start LIMIT 10000",
            "SELECT text as name, start, end, (end - start) as duration FROM NVTX_EVENTS ORDER BY start LIMIT 10000",
            "SELECT shortName as name, start, end, (end - start) as duration FROM CUPTI_ACTIVITY_KIND_KERNEL ORDER BY start LIMIT 10000",
            // 通用格式
            "SELECT name, start_time as start, end_time as end, duration FROM function_calls ORDER BY start_time LIMIT 10000",
        ];

        for (idx, query) in queries.iter().enumerate() {
            log::trace!("尝试查询 #{}: {}", idx + 1, query);
            if let Ok(mut stmt) = conn.prepare(query) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok(FunctionCall {
                        name: row
                            .get::<_, String>(0)
                            .unwrap_or_else(|_| "Unknown".to_string()),
                        start_time: row.get::<_, f64>(1).unwrap_or(0.0),
                        end_time: row.get::<_, f64>(2).unwrap_or(0.0),
                        duration: row.get::<_, f64>(3).unwrap_or(0.0),
                        depth: 0,
                    })
                }) {
                    calls = rows.filter_map(|r| r.ok()).collect();
                    if !calls.is_empty() {
                        log::info!("查询 #{} 成功，找到 {} 个函数调用", idx + 1, calls.len());
                        break;
                    }
                }
            }
        }

        if calls.is_empty() {
            log::warn!("所有查询都未找到函数调用数据");
        }

        // 计算深度（基于重叠的时间段）
        log::debug!("开始计算函数调用深度");
        self.calculate_depth(&mut calls);

        calls
    }

    fn calculate_depth(&self, calls: &mut Vec<FunctionCall>) {
        let mut active_calls: Vec<(f64, usize)> = Vec::new(); // (end_time, depth)

        for call in calls.iter_mut() {
            // 移除已经结束的调用
            active_calls.retain(|(end_time, _)| *end_time > call.start_time);

            // 找到可用的深度
            let mut depth = 0;
            let used_depths: Vec<usize> = active_calls.iter().map(|(_, d)| *d).collect();
            while used_depths.contains(&depth) {
                depth += 1;
            }

            call.depth = depth;
            active_calls.push((call.end_time, depth));
        }
    }

    pub fn get_time_range(&self) -> Option<(f64, f64)> {
        if self.function_calls.is_empty() {
            return None;
        }

        let min_time = self
            .function_calls
            .iter()
            .map(|c| c.start_time)
            .min_by(|a, b| a.partial_cmp(b).unwrap())?;

        let max_time = self
            .function_calls
            .iter()
            .map(|c| c.end_time)
            .max_by(|a, b| a.partial_cmp(b).unwrap())?;

        Some((min_time, max_time))
    }

    pub fn get_calls_in_range(&self, start: f64, end: f64) -> Vec<&FunctionCall> {
        self.function_calls
            .iter()
            .filter(|c| c.end_time >= start && c.start_time <= end)
            .collect()
    }

    /// 查询指定表的数据（带分页）
    pub fn get_table_data(
        &self,
        table_name: &str,
        limit: usize,
        offset: usize,
    ) -> Result<TableData> {
        log::debug!(
            "查询表数据: table='{}', limit={}, offset={}",
            table_name,
            limit,
            offset
        );

        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?;

        // 获取列名
        let columns = self.get_table_columns(conn, table_name)?;
        log::trace!("表 '{}' 有 {} 列", table_name, columns.len());

        // 查询数据
        let query = format!(
            "SELECT * FROM \"{}\" LIMIT {} OFFSET {}",
            table_name, limit, offset
        );

        let mut stmt = conn.prepare(&query)?;
        let column_count = stmt.column_count();

        let rows: Vec<Vec<String>> = stmt
            .query_map([], |row| {
                let mut row_data = Vec::new();
                for i in 0..column_count {
                    let value = self.get_cell_value(row, i);
                    row_data.push(value);
                }
                Ok(row_data)
            })?
            .filter_map(|r| r.ok())
            .collect();

        log::debug!("查询到 {} 行数据", rows.len());
        Ok(TableData { columns, rows })
    }

    /// 获取表的列名
    fn get_table_columns(&self, conn: &Connection, table_name: &str) -> Result<Vec<String>> {
        let query = format!("PRAGMA table_info(\"{}\")", table_name);
        let mut stmt = conn.prepare(&query)?;

        let columns: Vec<String> = stmt
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(columns)
    }

    /// 从Row中获取单元格值并转换为字符串
    fn get_cell_value(&self, row: &Row, index: usize) -> String {
        // 尝试不同的类型转换
        if let Ok(v) = row.get::<_, String>(index) {
            return v;
        }
        if let Ok(v) = row.get::<_, i64>(index) {
            return v.to_string();
        }
        if let Ok(v) = row.get::<_, f64>(index) {
            return format!("{:.6}", v);
        }
        if let Ok(v) = row.get::<_, Vec<u8>>(index) {
            return format!("<BLOB {} bytes>", v.len());
        }
        // NULL 或其他类型
        "NULL".to_string()
    }
}

impl Default for NsysDatabase {
    fn default() -> Self {
        Self::new()
    }
}
