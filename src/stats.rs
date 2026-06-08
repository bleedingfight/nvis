use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CudaApiAggregateRow {
    pub name: String,
    pub calls: u64,
    pub total_us: f64,
    pub mean_us: f64,
    pub p50_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
}

#[derive(Debug, Clone)]
pub struct NcuSpeedOfLightRow {
    pub kernel_id: String,
    pub kernel_name: String,
    pub duration_ns: f64,
    pub sm_throughput_pct: f64,
    pub memory_throughput_pct: f64,
    pub dram_throughput_pct: f64,
    pub compute_throughput_pct: f64,
}

/// Compute CUDA API aggregates similar to `nsys stats --report cuda_api_sum`.
/// Joins CUPTI_ACTIVITY_KIND_RUNTIME with StringIds to resolve names, then aggregates.
pub fn compute_cuda_api_aggregates(
    db_path: &Path,
    limit: usize,
) -> Result<Vec<CudaApiAggregateRow>> {
    let conn = Connection::open(db_path)?;

    // We'll compute: calls, total, mean, p50, p95, p99 in microseconds
    // Note: SQLite lacks built-in percentile; approximate via window functions over ordered values.
    // We compute percentiles by selecting the value at given rank.
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
                   -- approximate percentiles by nearest rank method
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

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok(CudaApiAggregateRow {
            name: row.get::<_, String>(0)?,
            calls: row.get::<_, i64>(1)? as u64,
            total_us: row.get::<_, f64>(2)?,
            mean_us: row.get::<_, f64>(3)?,
            p50_us: row.get::<_, f64>(4)?,
            p95_us: row.get::<_, f64>(5)?,
            p99_us: row.get::<_, f64>(6)?,
        })
    })?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Compute NCU Speed of Light throughput comparison across kernels.
/// Queries the kernels table for key throughput metrics.
pub fn compute_ncu_speed_of_light(conn: &Connection) -> Result<Vec<NcuSpeedOfLightRow>> {
    let sql = r#"
        SELECT
            "ID",
            "Kernel_Name",
            COALESCE("Duration_ns", 0),
            COALESCE("Compute_SM_Throughput_pct", 0),
            COALESCE("Memory_Throughput_pct", 0),
            COALESCE("DRAM_Throughput_pct", 0),
            COALESCE("Compute_SM_Throughput_pct", 0)
        FROM kernels
        ORDER BY "Duration_ns" DESC
    "#;

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(NcuSpeedOfLightRow {
            kernel_id: row.get::<_, String>(0)?,
            kernel_name: row.get::<_, String>(1)?,
            duration_ns: row.get::<_, f64>(2)?,
            sm_throughput_pct: row.get::<_, f64>(3)?,
            memory_throughput_pct: row.get::<_, f64>(4)?,
            dram_throughput_pct: row.get::<_, f64>(5)?,
            compute_throughput_pct: row.get::<_, f64>(6)?,
        })
    })?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}
