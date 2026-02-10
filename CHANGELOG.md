# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] - 2026-02-10

### Fixed
- **BoxPlot visualization now works** - Fixed critical issue where BoxPlot showed no data
  - Enhanced `generate_boxplot_data()` to calculate duration from start/end columns
  - Previously only worked with explicit "duration" columns
  - Now supports NSYS tables that use separate "start" and "end" columns
  - Added automatic duration calculation: `duration = end - start`
  - Added negative duration filtering
  
- **Kernel table visualization** - Fixed detection priority issue
  - Reordered `detect_table_type()` to check kernel/event/memcpy BEFORE generic cuda
  - `CUPTI_ACTIVITY_KIND_KERNEL` now correctly shows Timeline instead of BoxPlot
  - Prevents kernel execution tables from being misclassified
  
- **StringIds table confirmed working** - Investigation showed no crash occurs
  - Properly handles 1000+ row tables
  - Displays as BarChart visualization
  - No panics or errors

### Added
- **Library interface** - Created `src/lib.rs` for external testing
  - Exports app, db, visualization, and ui modules
  - Enables example programs and unit tests
  
- **Comprehensive test suite** - Added `examples/test_viz.rs`
  - Tests all 4 visualization types
  - Validates detection logic
  - Demonstrates proper functionality
  - Run with: `cargo run --example test_viz`

### Improved
- Better column detection logic for BoxPlot generation
- More robust handling of tables without duration columns
- Enhanced Timeline detection to include memcpy tables
- Added INVESTIGATION_REPORT.md documenting all findings

### Verified
- ✓ BarChart: StringIds table (1000 rows)
- ✓ BoxPlot: CUPTI_ACTIVITY_KIND_RUNTIME (88 rows, 10 plots)
- ✓ Timeline: CUPTI_ACTIVITY_KIND_KERNEL (10 rows, 10 events)
- ✓ Statistics: TARGET_INFO_SYSTEM_ENV (86 rows)

## [0.2.2] - 2026-02-10

### Fixed
- **Bar chart data generation crash** - Fixed panic when displaying tables with numeric data
  - Added empty data check before processing
  - Added start/end boundary validation to prevent slice panics
  - Fixed issue with StringIds and similar tables causing crashes
  - Improved robustness of numeric column detection

### Improved
- Better error handling in visualization generation
- More defensive programming in data slicing operations

## [0.2.1] - 2026-02-10

### Fixed
- **Input cursor navigation** - Arrow keys now move cursor in input field
  - `←/→` moves cursor left/right in file path input
  - `Home` moves cursor to beginning
  - `End` moves cursor to end
  - `Delete` deletes character after cursor
  - Arrow keys still switch panels when not in input mode

### Changed
- Improved cursor position display using `tui-input`'s cursor tracking
- Enhanced text editing experience in file selection

## [0.2.0] - 2026-02-10

### Added - Smart Visualization System
- **Intelligent table type detection** - Automatically identifies table types based on name and columns
- **Box Plot visualization** - For CUDA API call statistics showing:
  - Min, Q1, Median, Q3, Max values
  - Mean and call count
  - Top 10 most time-consuming functions
  - Visual box plot representation
- **Timeline visualization** - For kernel/event sequences showing:
  - Execution order and timing
  - Duration bars
  - Up to 20 events
- **Statistics text view** - For summary tables showing:
  - Table metadata
  - Statistical summaries (mean, min, max) for numeric columns
- **Enhanced sample database** - Includes realistic CUDA profiling data:
  - CUPTI_ACTIVITY_KIND_RUNTIME table (API calls)
  - CUPTI_ACTIVITY_KIND_KERNEL table (kernel executions)
  - Multiple example tables for different visualization types

### Changed
- Refactored visualization logic into dedicated `visualization.rs` module
- UI now dynamically selects visualization based on table content
- Improved chart rendering with better formatting and colors
- Updated sample database script with more comprehensive test data

### Technical
- Added `BoxPlotData` struct for statistical aggregation
- Added `TimelineEvent` struct for temporal data
- Implemented percentile calculation for box plots
- Added smart column detection algorithms

### Documentation
- Added VISUALIZATION.md explaining all visualization types
- Updated README with new features
- Enhanced sample database creation script with comments

## [0.1.1] - 2026-02-10

### Fixed
- Fixed issue where typing 'q' in file path input would quit the application
- Now 'q' and 'Esc' keys only exit the app when not in input mode
- Added `is_inputting()` method to properly detect input state

### Changed
- Updated documentation to clarify input behavior
- Improved key handling logic in main event loop

## [0.1.0] - 2026-02-10

### Added
- Initial release
- File selection interface with text input
- SQLite database loading and table listing
- Left sidebar for table selection
- Right panel split into:
  - Top: Bar chart visualization
  - Bottom: Data table display
- Full keyboard navigation support
- Mouse interaction support (click and scroll)
- Multiple focus areas (FileInput, TableList, Chart, DataTable)
- Error handling and user-friendly messages
- Sample database creation script
- Comprehensive documentation

### Features
- Load any SQLite3 database
- Automatic table detection
- Data visualization with bar charts
- Scrollable data views
- Support for up to 1000 rows per table
- Handles multiple data types (INTEGER, REAL, TEXT, BLOB, NULL)
- Cross-platform support (Linux, macOS, Windows)
