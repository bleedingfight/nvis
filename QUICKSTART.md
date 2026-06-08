# Quick Start

## Build

```bash
cargo build --release
```

## Run

```bash
# With a file (auto-detect format)
./target/release/nvis report.sqlite
./target/release/nvis ncu_report.csv
./target/release/nvis trace.json

# Without a file (use TUI file selector)
./target/release/nvis
```

## Prepare Profiler Data

```bash
# nsys
nsys profile -o myapp ./myapp
nsys export -t sqlite myapp.nsys-rep   # produces .sqlite

# ncu
ncu --csv -o report.csv ./myapp        # produces .csv

# torch (Python)
# prof.export_chrome_trace("trace.json")
```

## Navigation

1. Type file path → `Enter` to load
2. `↑↓` select view, `Tab` switch panel
3. `s` open statistics, `b` go back
4. `q` quit

Minimum terminal: 80x24, UTF-8 support required.
