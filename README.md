# NVIS - NSYS SQLite3 TUI Viewer

A Terminal User Interface (TUI) tool for visualizing and exploring NVIDIA Nsight Systems (nsys) SQLite3 database files with intelligent visualization selection.

## Features

- **File Selection Interface**: Easy-to-use file path input for loading SQLite databases
- **Table Browser**: Left sidebar showing all available tables in the database
- **Smart Data Visualization**: Automatically selects the best visualization based on table type
  - **Box Plot**: CUDA API call statistics with min/max/median/mean/quartiles
  - **Timeline**: Kernel execution sequences with duration bars
  - **Statistics**: Numerical summaries and metadata
  - **Bar Chart**: General-purpose data visualization
- **Interactive Navigation**:
  - Keyboard controls (arrow keys, Tab, Enter)
  - Mouse support (clicking and scrolling)
  - Multiple focus areas for efficient navigation

## Installation

### Prerequisites

- Rust (1.70 or later)
- Cargo

### Build from Source

```bash
git clone <repository-url>
cd nvis
cargo build --release
```

The binary will be available at `target/release/nvis`.

## Usage

### Starting the Application

```bash
cargo run
# or
./target/release/nvis
```

### Navigation

#### File Selection Screen

1. Type or paste the path to your NSYS SQLite database file
2. Press `Enter` to load the database
3. Press `q` or `Esc` to quit
   - **Note:** When typing in the file path input, 'q' will be treated as a regular character

#### Table View Screen

**Keyboard Controls:**
- `↑/↓` - Navigate through tables (when focused on table list) or scroll data
- `←/→` - Switch focus between table list and data panels
- `Tab` - Cycle through focus areas (Table List → Chart → Data Table)
- `Enter` - Load selected table data
- `q` or `Esc` - Quit application (when not in input mode)

**Mouse Controls:**
- Click on panels to focus them
- Scroll wheel to navigate through lists and data
- Click on table names to select them

**Focus Areas:**
1. **Table List** (left sidebar): Browse and select tables
2. **Chart** (top right): View bar chart visualization of numeric data
3. **Data Table** (bottom right): View raw table data in tabular format

### Layout

```
┌─────────────┬──────────────────────────────┐
│   Tables    │      Visualization           │
│             │      (Bar Chart)             │
│  - Table1   │                              │
│  - Table2   ├──────────────────────────────┤
│  - Table3   │                              │
│  - ...      │      Data Table              │
│             │      (Rows & Columns)        │
│             │                              │
└─────────────┴──────────────────────────────┘
[q]Quit [↑↓]Navigate [←→]Focus [Tab]Switch...
```

## Features in Detail

### Data Loading
- Loads all tables from the SQLite database
- Supports up to 1000 rows per table for performance
- Handles various SQLite data types (INTEGER, REAL, TEXT, BLOB)

### Visualization
- Automatically detects numeric columns for charting
- Bar chart shows up to 10 data points at a time
- Scroll through chart data using mouse or keyboard when chart is focused

### Data Table
- Displays all columns with headers
- Truncates long cell values (>20 chars) with ellipsis
- Scrollable view for large datasets
- Dynamic column width adjustment

## Technical Details

### Built With
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite bindings
- [tui-input](https://github.com/sayanarijit/tui-input) - Text input widget

### Project Structure

```
nvis/
├── src/
│   ├── main.rs      # Entry point and event loop
│   ├── app.rs       # Application state management
│   ├── db.rs        # SQLite database operations
│   └── ui.rs        # UI rendering logic
├── Cargo.toml       # Dependencies and project config
└── README.md        # This file
```

## Example NSYS Database

To test with an actual NSYS database:

1. Profile an application with NVIDIA Nsight Systems:
   ```bash
   nsys profile -o myapp ./myapp
   ```

2. This creates a `.nsys-rep` file which contains a SQLite database

3. Export or directly use the SQLite database:
   ```bash
   nsys export -t sqlite myapp.nsys-rep
   ```

4. Load the database file in NVIS

## Known Limitations

- Currently loads only the first 1000 rows per table for performance
- Chart visualization limited to numeric columns
- Bar chart shows maximum 10 data points at once (use scroll to see more)
- Static labels in chart due to lifetime constraints

## Future Enhancements

- [ ] Support for more chart types (line, scatter, etc.)
- [ ] Column sorting and filtering
- [ ] Search functionality
- [ ] Export data to CSV
- [ ] Custom SQL query interface
- [ ] Configuration file support
- [ ] Theme customization

## License

MIT License (or your preferred license)

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.
