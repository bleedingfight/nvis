# CLAUDE.md

This file provides guidance to Kscc (claudexxxxxx.ai/code) when working with code in this repository.

## Project Overview

NVIS is a Terminal User Interface (TUI) tool for visualizing NVIDIA Nsight Systems (nsys) SQLite3 database files. It provides intelligent visualization selection based on table types.

## Build Commands

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run the application
cargo run

# Run with a database file directly
cargo run -- /path/to/database.sqlite
```

## Architecture

The codebase is organized into these modules:

- **main.rs**: Entry point and event loop handling keyboard/mouse input
- **app.rs**: Application state machine with `AppState` (FileSelection, TableView, StatsView) and `Focus` management
- **db.rs**: SQLite operations; `load_table_data_resolved()` joins `StringIds` table to resolve `nameId` references
- **ui.rs**: ratatui rendering for all views; `draw()` dispatches based on `AppState`
- **visualization.rs**: Smart visualization detection (`detect_table_type`) and data generation for BoxPlot, Timeline, Statistics, and BarChart
- **stats.rs**: CUDA API aggregate statistics computation with percentile calculations

## Visualization Types

The `detect_table_type()` function automatically selects visualization based on table name and columns:

1. **Timeline**: Tables with `kernel`/`event`/`memcpy` names and `start`+`end`/`duration` columns
2. **BoxPlot**: CUDA/Runtime/API tables with time-related columns
3. **Statistics**: Tables with `stat`/`summary`/`info` in name
4. **BarChart**: Default fallback

## Key Data Structures

- `TableData { columns, rows }`: Column headers and stringified row values
- `Focus`: Tracks which panel is active (FileInput, TableList, DataTable, Chart, StatsTable)
- `BoxPlotData`: Statistical aggregates (min, max, median, q1, q3, mean, count)

## Database Schema Assumptions

- `StringIds` table maps numeric IDs to string names
- Tables may have `nameId` column referencing `StringIds.id`
- `CUPTI_ACTIVITY_KIND_RUNTIME` table contains CUDA API call traces
