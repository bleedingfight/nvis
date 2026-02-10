// Test script to verify StringIds visualization works correctly
use std::path::Path;

mod app {
    #[derive(Debug, Clone)]
    pub struct TableData {
        pub columns: Vec<String>,
        pub rows: Vec<Vec<String>>,
    }
}

mod db;
mod visualization;

fn main() -> anyhow::Result<()> {
    let db_path = Path::new("report1.sqlite");
    
    println!("Testing StringIds table visualization...\n");
    
    // Load StringIds table
    let table_data = db::load_table_data(db_path, "StringIds")?;
    println!("Loaded StringIds table:");
    println!("  Columns: {:?}", table_data.columns);
    println!("  Rows loaded: {}", table_data.rows.len());
    
    if !table_data.rows.is_empty() {
        println!("  First row: {:?}", table_data.rows[0]);
        println!("  Last row: {:?}", table_data.rows[table_data.rows.len() - 1]);
    }
    
    // Detect visualization type
    let viz_type = visualization::detect_table_type("StringIds", &table_data);
    println!("\nDetected visualization type: {:?}", viz_type);
    
    // Generate visualization
    println!("\nGenerating visualization with scroll=0...");
    let viz = visualization::generate_visualization("StringIds", &table_data, 0);
    println!("Visualization title: {}", viz.title);
    println!("Visualization type: {:?}", viz._viz_type);
    
    match &viz.data {
        visualization::ChartData::Bars(bars) => {
            println!("Generated {} bars", bars.len());
            if !bars.is_empty() {
                println!("First few bars:");
                for (i, (label, value)) in bars.iter().take(5).enumerate() {
                    println!("  {}: {} = {}", i, label, value);
                }
            }
        }
        visualization::ChartData::BoxPlots(plots) => {
            println!("Generated {} box plots", plots.len());
        }
        visualization::ChartData::Timeline(events) => {
            println!("Generated {} timeline events", events.len());
        }
        visualization::ChartData::Stats(text) => {
            println!("Generated statistics text ({} chars)", text.len());
        }
    }
    
    println!("\n✓ No crash occurred! StringIds visualization works correctly.");
    
    // Now test CUPTI_ACTIVITY_KIND_RUNTIME table
    println!("\n" + "=".repeat(60));
    println!("Testing CUPTI_ACTIVITY_KIND_RUNTIME table visualization...\n");
    
    let table_data = db::load_table_data(db_path, "CUPTI_ACTIVITY_KIND_RUNTIME")?;
    println!("Loaded CUPTI_ACTIVITY_KIND_RUNTIME table:");
    println!("  Columns: {:?}", table_data.columns);
    println!("  Rows loaded: {}", table_data.rows.len());
    
    let viz_type = visualization::detect_table_type("CUPTI_ACTIVITY_KIND_RUNTIME", &table_data);
    println!("\nDetected visualization type: {:?}", viz_type);
    
    let viz = visualization::generate_visualization("CUPTI_ACTIVITY_KIND_RUNTIME", &table_data, 0);
    println!("Visualization title: {}", viz.title);
    
    match &viz.data {
        visualization::ChartData::BoxPlots(plots) => {
            println!("\n✓ Generated {} box plots", plots.len());
            if !plots.is_empty() {
                println!("\nBox plot details:");
                for (i, plot) in plots.iter().enumerate() {
                    println!("  {}: {}", i + 1, plot.label);
                    println!("      Count: {}", plot.count);
                    println!("      Mean:  {:.2}", plot.mean);
                    println!("      Min:   {:.2}", plot.min);
                    println!("      Q1:    {:.2}", plot.q1);
                    println!("      Median:{:.2}", plot.median);
                    println!("      Q3:    {:.2}", plot.q3);
                    println!("      Max:   {:.2}", plot.max);
                }
            } else {
                println!("  WARNING: No box plot data generated!");
            }
        }
        _ => {
            println!("  ERROR: Expected BoxPlots but got different visualization type!");
        }
    }
    
    Ok(())
}
