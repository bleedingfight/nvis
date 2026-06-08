use std::path::Path;

#[test]
fn ncu_csv_parse_and_convert() {
    let path = Path::new("/Users/liushuai/fp8_gemm.csv");
    if !path.exists() {
        eprintln!("Skipping NCU CSV test: file not found");
        return;
    }

    let data = nvis::ncu_csv::parse_ncu_csv(path).expect("parse should succeed");
    assert!(!data.records.is_empty(), "should have records");
    assert!(!data.kernel_ids.is_empty(), "should have kernel IDs");
    assert!(!data.metric_names.is_empty(), "should have metric names");

    let conn = nvis::ncu_csv::csv_to_sqlite(&data).expect("sqlite conversion should succeed");

    // Check tables exist
    let tables = nvis::db::load_tables_conn(&conn).expect("load tables");
    assert!(tables.contains(&"kernels".to_string()), "kernels table should exist");
    assert!(tables.contains(&"metrics".to_string()), "metrics table should exist");
    assert!(tables.contains(&"sections".to_string()), "sections table should exist");

    // Check kernels table has data
    let kernel_data = nvis::db::load_table_data_conn(&conn, "kernels").expect("load kernels");
    assert!(!kernel_data.rows.is_empty(), "kernels should have rows");
    assert!(
        kernel_data.columns.iter().any(|c| c.contains("Duration") || c.contains("duration")),
        "kernels should have Duration column, got: {:?}",
        kernel_data.columns
    );

    // Check metrics table has data
    let metrics_data = nvis::db::load_table_data_conn(&conn, "metrics").expect("load metrics");
    assert!(!metrics_data.rows.is_empty(), "metrics should have rows");

    // Test Speed of Light computation
    let sol_rows = nvis::stats::compute_ncu_speed_of_light(&conn).expect("speed of light");
    assert!(!sol_rows.is_empty(), "should have speed of light rows");
}
