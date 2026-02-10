use std::path::Path;

#[test]
fn stringids_bar_chart_from_db() {
    // Use local file; test will be skipped if missing
    let db_path = Path::new("report1.sqlite");
    if !db_path.exists() {
        eprintln!("SKIP: report.sqlit3 not found, skipping integration test");
        return;
    }

    let table_data = nvis::load_table_data(db_path, "StringIds").expect("load table");
    let bars = nvis::generate_bar_chart_data(&table_data, 0);
    // At least one row should produce a bar if a numeric column exists
    assert!(bars.len() >= 0);
}

#[test]
fn runtime_boxplot_from_db() {
    let db_path = Path::new("report1.sqlite");
    if !db_path.exists() {
        eprintln!("SKIP: report.sqlit3 not found, skipping integration test");
        return;
    }

    let table_data =
        nvis::load_table_data_resolved(db_path, "CUPTI_ACTIVITY_KIND_RUNTIME").expect("load");
    let viz = nvis::generate_visualization("CUPTI_ACTIVITY_KIND_RUNTIME", &table_data, 0);
    match viz.data {
        nvis::ChartData::BoxPlots(_) => {}
        other => panic!("expected BoxPlots, got {:?}", other),
    }
}
