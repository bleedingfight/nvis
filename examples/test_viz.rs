// Test script to verify all visualization types

fn main() -> anyhow::Result<()> {
    use std::path::Path;

    let db_path = Path::new("report1.sqlite");

    // Test 1: StringIds - Should be BarChart
    println!("{}", "=".repeat(70));
    println!("TEST 1: StringIds - BarChart Visualization");
    println!("{}", "=".repeat(70));

    let table_data = nvis::db::load_table_data(db_path, "StringIds")?;
    println!("Columns: {:?}", table_data.columns);
    println!("Rows loaded: {}", table_data.rows.len());

    let viz_type = nvis::visualization::detect_table_type("StringIds", &table_data);
    println!("Detected type: {:?}", viz_type);

    let viz = nvis::visualization::generate_visualization("StringIds", &table_data, 0);
    match &viz.data {
        nvis::visualization::ChartData::Bars(bars) => {
            println!("✓ Generated {} bars (showing first 3)", bars.len());
            for (label, value) in bars.iter().take(3) {
                println!("  {} = {}", label, value);
            }
        }
        _ => println!("✗ Wrong visualization type!"),
    }

    // Test 2: CUPTI_ACTIVITY_KIND_RUNTIME - Should be BoxPlot
    println!("\n{}", "=".repeat(70));
    println!("TEST 2: CUPTI_ACTIVITY_KIND_RUNTIME - BoxPlot Visualization");
    println!("{}", "=".repeat(70));

    // Use resolved loader to get function names instead of nameId
    let table_data = nvis::load_table_data_resolved(db_path, "CUPTI_ACTIVITY_KIND_RUNTIME")?;
    println!("Columns: {:?}", table_data.columns);
    println!("Rows loaded: {}", table_data.rows.len());

    let viz_type =
        nvis::visualization::detect_table_type("CUPTI_ACTIVITY_KIND_RUNTIME", &table_data);
    println!("Detected type: {:?}", viz_type);

    let viz =
        nvis::visualization::generate_visualization("CUPTI_ACTIVITY_KIND_RUNTIME", &table_data, 0);
    match &viz.data {
        nvis::visualization::ChartData::BoxPlots(plots) => {
            println!("✓ Generated {} box plots (showing first 3)", plots.len());
            for plot in plots.iter().take(3) {
                println!(
                    "  {} - Count: {}, Mean: {:.2}, Median: {:.2}",
                    plot.label, plot.count, plot.mean, plot.median
                );
            }
        }
        _ => println!("✗ Wrong visualization type!"),
    }

    // Test 3: CUPTI_ACTIVITY_KIND_KERNEL - Should be Timeline
    println!("\n{}", "=".repeat(70));
    println!("TEST 3: CUPTI_ACTIVITY_KIND_KERNEL - Timeline Visualization");
    println!("{}", "=".repeat(70));

    let table_data = nvis::db::load_table_data(db_path, "CUPTI_ACTIVITY_KIND_KERNEL")?;
    println!("Columns: {:?}", table_data.columns);
    println!("Rows loaded: {}", table_data.rows.len());

    let viz_type =
        nvis::visualization::detect_table_type("CUPTI_ACTIVITY_KIND_KERNEL", &table_data);
    println!("Detected type: {:?}", viz_type);

    let viz =
        nvis::visualization::generate_visualization("CUPTI_ACTIVITY_KIND_KERNEL", &table_data, 0);
    match &viz.data {
        nvis::visualization::ChartData::Timeline(events) => {
            println!(
                "✓ Generated {} timeline events (showing first 3)",
                events.len()
            );
            for event in events.iter().take(3) {
                println!(
                    "  {} - Start: {:.2}, Duration: {:.2}",
                    event.name, event.start, event.duration
                );
            }
        }
        _ => println!("✗ Wrong visualization type!"),
    }

    // Test 4: TARGET_INFO_SYSTEM_ENV - Should be Statistics
    println!("\n{}", "=".repeat(70));
    println!("TEST 4: TARGET_INFO_SYSTEM_ENV - Statistics Visualization");
    println!("{}", "=".repeat(70));

    let table_data = nvis::db::load_table_data(db_path, "TARGET_INFO_SYSTEM_ENV")?;
    println!("Columns: {:?}", table_data.columns);
    println!("Rows loaded: {}", table_data.rows.len());

    let viz_type = nvis::visualization::detect_table_type("TARGET_INFO_SYSTEM_ENV", &table_data);
    println!("Detected type: {:?}", viz_type);

    let viz = nvis::visualization::generate_visualization("TARGET_INFO_SYSTEM_ENV", &table_data, 0);
    match &viz.data {
        nvis::visualization::ChartData::Stats(text) => {
            println!("✓ Generated statistics text ({} chars)", text.len());
            println!(
                "First 200 chars: {}",
                &text.chars().take(200).collect::<String>()
            );
        }
        _ => println!("✗ Wrong visualization type!"),
    }

    println!("\n{}", "=".repeat(70));
    println!("ALL TESTS COMPLETED");
    println!("{}", "=".repeat(70));

    Ok(())
}
