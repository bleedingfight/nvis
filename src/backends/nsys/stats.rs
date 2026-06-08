use anyhow::Result;
use rusqlite::Connection;

use crate::core::types::{ColumnSchema, ColumnType, ColumnValue, ProfilerData};

/// Compute CUDA API aggregates similar to `nsys stats --report cuda_api_sum`.
pub fn compute_cuda_api_aggregates(conn: &Connection, limit: usize) -> Result<ProfilerData> {
    let sql = r#"
        WITH runtime AS (
            SELECT
                r.nameId,
                (CASE
                    WHEN r.end IS NOT NULL AND r.start IS NOT NULL THEN (r.end - r.start)
                    ELSE NULL
                END) AS dur_ns
            FROM CUPTI_ACTIVITY_KIND_RUNTIME r
        ), named AS (
            SELECT COALESCE(s.value, printf('id_%d', runtime.nameId)) AS name,
                   dur_ns
            FROM runtime
            LEFT JOIN StringIds s ON s.id = runtime.nameId
            WHERE dur_ns IS NOT NULL AND dur_ns >= 0
        ), ranked AS (
            SELECT name,
                   (dur_ns / 1000.0) AS dur_us,
                   ROW_NUMBER() OVER (PARTITION BY name ORDER BY dur_ns) AS rn,
                   COUNT(*) OVER (PARTITION BY name) AS cnt
            FROM named
        ), percentiles AS (
            SELECT name,
                   cnt AS calls,
                   SUM(dur_us) OVER (PARTITION BY name) AS total_us,
                   AVG(dur_us) OVER (PARTITION BY name) AS mean_us,
                   (SELECT dur_us FROM ranked r2 WHERE r2.name = ranked.name AND r2.rn = CAST(ROUND(0.50 * (cnt)) AS INTEGER) LIMIT 1) AS p50_us,
                   (SELECT dur_us FROM ranked r2 WHERE r2.name = ranked.name AND r2.rn = CAST(ROUND(0.95 * (cnt)) AS INTEGER) LIMIT 1) AS p95_us,
                   (SELECT dur_us FROM ranked r2 WHERE r2.name = ranked.name AND r2.rn = CAST(ROUND(0.99 * (cnt)) AS INTEGER) LIMIT 1) AS p99_us
            FROM ranked
        )
        SELECT name,
               MAX(calls) AS calls,
               MAX(total_us) AS total_us,
               MAX(mean_us) AS mean_us,
               MAX(p50_us) AS p50_us,
               MAX(p95_us) AS p95_us,
               MAX(p99_us) AS p99_us
        FROM percentiles
        GROUP BY name
        ORDER BY mean_us DESC
        LIMIT ?1;
    "#;

    let schema = vec![
        ColumnSchema { name: "name".into(), dtype: ColumnType::Text },
        ColumnSchema { name: "calls".into(), dtype: ColumnType::Integer },
        ColumnSchema { name: "total_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "mean_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "p50_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "p95_us".into(), dtype: ColumnType::Float },
        ColumnSchema { name: "p99_us".into(), dtype: ColumnType::Float },
    ];

    let col_count = schema.len();
    let mut columns: Vec<Vec<ColumnValue>> = vec![Vec::new(); col_count];
    let mut row_count = 0usize;

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, f64>(4)?,
            row.get::<_, f64>(5)?,
            row.get::<_, f64>(6)?,
        ))
    })?;

    for r in rows {
        let r = r?;
        columns[0].push(ColumnValue::Text(r.0));
        columns[1].push(ColumnValue::Integer(r.1));
        columns[2].push(ColumnValue::Float(r.2));
        columns[3].push(ColumnValue::Float(r.3));
        columns[4].push(ColumnValue::Float(r.4));
        columns[5].push(ColumnValue::Float(r.5));
        columns[6].push(ColumnValue::Float(r.6));
        row_count += 1;
    }

    Ok(ProfilerData {
        schema,
        columns,
        row_count,
    })
}
