use anyhow::Result;
use rusqlite::Connection;

use crate::viz::summary_data::{SummaryEvent, SummaryEventType, SummarySide};

/// Build the merged GPU+CPU event list for the Summary page.
pub fn build_summary_events(conn: &Connection) -> Result<Vec<SummaryEvent>> {
    let mut events = Vec::new();

    // GPU: Kernels
    let has_kernel = table_exists(conn, "CUPTI_ACTIVITY_KIND_KERNEL");
    log::debug!("Summary: table KERNEL exists = {}", has_kernel);
    if has_kernel {
        let name_col = find_name_column(conn, "CUPTI_ACTIVITY_KIND_KERNEL");
        let join = if name_col.is_empty() { String::new() } else { format!("LEFT JOIN StringIds s ON s.id = r.{}", name_col) };
        let select_name = if name_col.is_empty() { "'Kernel'".to_string() } else { "s.value".to_string() };
        let query = format!(r#"
            SELECT 'S' || r.streamId, {}, r.start, r.end, r.correlationId
            FROM CUPTI_ACTIVITY_KIND_KERNEL r
            {}
            WHERE r.start IS NOT NULL AND r.end IS NOT NULL
            ORDER BY r.start
            LIMIT 50000
        "#, select_name, join);
        append_events(conn, &query, SummarySide::Gpu, SummaryEventType::Kernel, &mut events)?;
    }

    // GPU: Memcpy
    let has_memcpy = table_exists(conn, "CUPTI_ACTIVITY_KIND_MEMCPY");
    log::debug!("Summary: table MEMCPY exists = {}", has_memcpy);
    if has_memcpy {
        let query = r#"
            SELECT 'S' || r.streamId,
                   CASE r.copyKind
                       WHEN 1 THEN 'HtoD'
                       WHEN 2 THEN 'DtoH'
                       WHEN 3 THEN 'HtoA'
                       WHEN 4 THEN 'AtoH'
                       WHEN 5 THEN 'AtoA'
                       WHEN 6 THEN 'AtoD'
                       WHEN 7 THEN 'DtoA'
                       WHEN 8 THEN 'DtoD'
                       WHEN 9 THEN 'HtoH'
                       WHEN 10 THEN 'P2P'
                       WHEN 11 THEN 'HtoD'
                       WHEN 12 THEN 'DtoH'
                       WHEN 13 THEN 'DtoD'
                       ELSE 'Memcpy'
                   END || ' ' ||
                   CASE WHEN r.bytes >= 1048576 THEN (r.bytes/1048576) || 'MB'
                        WHEN r.bytes >= 1024 THEN (r.bytes/1024) || 'KB'
                        ELSE r.bytes || 'B' END,
                   r.start, r.end, r.correlationId
            FROM CUPTI_ACTIVITY_KIND_MEMCPY r
            WHERE r.start IS NOT NULL AND r.end IS NOT NULL
            ORDER BY r.start
            LIMIT 50000
        "#;
        append_events(conn, query, SummarySide::Gpu, SummaryEventType::Memcpy, &mut events)?;
    }

    // GPU: Memset
    let has_memset = table_exists(conn, "CUPTI_ACTIVITY_KIND_MEMSET");
    log::debug!("Summary: table MEMSET exists = {}", has_memset);
    if has_memset {
        // MEMSET table typically has no nameId; use a fixed label
        let query = r#"
            SELECT 'S' || r.streamId, 'Memset', r.start, r.end, r.correlationId
            FROM CUPTI_ACTIVITY_KIND_MEMSET r
            WHERE r.start IS NOT NULL AND r.end IS NOT NULL
            ORDER BY r.start
            LIMIT 50000
        "#;
        append_events(conn, query, SummarySide::Gpu, SummaryEventType::Memset, &mut events)?;
    }

    // CPU: Runtime API
    let has_runtime = table_exists(conn, "CUPTI_ACTIVITY_KIND_RUNTIME");
    log::debug!("Summary: table RUNTIME exists = {}", has_runtime);
    if has_runtime {
        let has_thread_names = table_exists(conn, "ThreadNames");
        let name_col = find_name_column(conn, "CUPTI_ACTIVITY_KIND_RUNTIME");
        let name_join = if name_col.is_empty() { String::new() } else { format!("LEFT JOIN StringIds s ON s.id = r.{}", name_col) };
        // Strip CUDA version suffix (e.g. "_v3020") for readability
        let select_name = if name_col.is_empty() {
            "'Runtime'".to_string()
        } else {
            r#"CASE WHEN INSTR(s.value, '_v') > 0 THEN SUBSTR(s.value, 1, INSTR(s.value, '_v') - 1) ELSE s.value END"#.to_string()
        };
        // Decode lane_key: use "T<pid>.<tid>" or "T<pid>.<thread_name>" for readability
        let thread_join = if has_thread_names { "LEFT JOIN ThreadNames tn ON tn.globalTid = r.globalTid LEFT JOIN StringIds sn ON sn.id = tn.nameId" } else { "" };
        let lane_key = if has_thread_names {
            r#"COALESCE('T' || (r.globalTid >> 32) || '.' || NULLIF(sn.value, ''), 'T' || (r.globalTid >> 32) || '.' || (r.globalTid & 0xFFFFFFFF))"#
        } else {
            r#"'T' || (r.globalTid >> 32) || '.' || (r.globalTid & 0xFFFFFFFF)"#
        };
        let query = format!(r#"
            SELECT {}, {}, r.start, r.end, r.correlationId
            FROM CUPTI_ACTIVITY_KIND_RUNTIME r
            {} {}
            WHERE r.start IS NOT NULL AND r.end IS NOT NULL
            ORDER BY r.start
            LIMIT 50000
        "#, lane_key, select_name, name_join, thread_join);
        append_events(conn, &query, SummarySide::Cpu, SummaryEventType::Runtime, &mut events)?;
    }

    events.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    log::debug!("Summary: total {} events after merge", events.len());
    Ok(events)
}

fn append_events(
    conn: &Connection,
    query: &str,
    side: SummarySide,
    event_type: SummaryEventType,
    events: &mut Vec<SummaryEvent>,
) -> Result<()> {
    let mut stmt = conn.prepare(query)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let lane_key: String = row.get(0)?;
        let name: String = row.get::<_, String>(1).unwrap_or_default();
        let start: f64 = row.get::<_, f64>(2).unwrap_or(0.0);
        let end: f64 = row.get::<_, f64>(3).unwrap_or(0.0);
        let correlation_id: Option<i64> = row.get(4).ok();
        events.push(SummaryEvent {
            name,
            start,
            end,
            side: side.clone(),
            lane_key,
            event_type: event_type.clone(),
            correlation_id,
        });
    }
    Ok(())
}

fn table_exists(conn: &Connection, name_fragment: &str) -> bool {
    let Ok(mut stmt) = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'") else {
        return false;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
        return false;
    };
    for row in rows.flatten() {
        if row.to_uppercase().contains(name_fragment) {
            return true;
        }
    }
    false
}

/// Find a column in the table that references StringIds (nameId, shortName, demangledName, etc.)
fn find_name_column(conn: &Connection, table: &str) -> String {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({})", table)) else {
        return String::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return String::new();
    };
    let cols: Vec<String> = rows.flatten().collect();
    cols.iter()
        .find(|c| {
            let lc = c.to_lowercase();
            lc == "nameid" || lc == "shortname" || lc == "demangledname" || lc == "mangledname"
        })
        .cloned()
        .unwrap_or_default()
}
