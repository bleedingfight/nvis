# NVIS - Universal Profiler Viewer

A Terminal User Interface (TUI) tool for visualizing and exploring profiler data from multiple sources — NVIDIA Nsight Systems, NVIDIA Nsight Compute, and PyTorch Profiler.

## Features

- **Auto-detection**: Automatically identifies the profiler format (nsys SQLite, ncu CSV, torch Chrome Trace JSON)
- **Multiple Backends**:
  - **nsys**: SQLite3 database from `nsys export -t sqlite`
  - **ncu**: CSV export from `ncu --csv`
  - **torch**: Chrome Trace JSON from `torch.profiler.profile`
- **Smart Visualization**: Automatically selects the best chart type per view
  - **Box Plot**: API call latency statistics (min/max/median/Q1/Q3/mean)
  - **Timeline**: Kernel and operation execution sequences
  - **Bar Chart**: General numeric data
  - **Statistics**: Numerical summaries and metadata
- **Interactive Navigation**: Keyboard and mouse controls with multiple focus areas
- **Extensible Architecture**: Trait-based plugin system — add new profiler backends or visualization types without modifying core code

## Installation

```bash
git clone <repository-url>
cd nvis
cargo build --release
```

The binary is at `target/release/nvis`.

## Usage

### Command Line

```bash
# Open a file directly (auto-detect format)
nvis /path/to/profile.sqlite
nvis /path/to/ncu_report.csv
nvis /path/to/trace.json

# Open a remote file over SSH (downloads a local copy first)
nvis 'ssh:user@host:/data/profile.sqlite'
nvis 'ssh://host:2222/data/report.csv'

# Start with file selection UI
nvis
```

### Remote Files

nvis transparently resolves remote file URIs by downloading a local copy
via the appropriate tool (`scp` for SSH, `curl` for WebDAV/HTTP). The temp
file preserves the original extension for backend detection and is cleaned
up automatically when nvis exits.

**SSH** (`scp`):

| Format | Example |
|--------|---------|
| `ssh:[user@]host:/path` | `ssh:alice@gpu01:/data/prof.sqlite` |
| `ssh:[user@]host:port:/path` | `ssh:gpu01:2222:/data/prof.csv` |
| `ssh://[user@]host[:port]/path` | `ssh://alice@gpu01:2222/data/prof.json` |
| `scp:` alias | `scp:alice@gpu01:/data/prof.sqlite` |

- Requires key-based auth (or ssh-agent); `scp` runs in batch mode (`-B`)
- `~/.ssh/config` host aliases work

**WebDAV / HTTP** (`curl`):

| Format | Example |
|--------|---------|
| `webdav://host/path` | `webdav://nas.local/data/prof.sqlite` |
| `webdavs://host/path` | `webdavs://nas.local/data/prof.csv` |
| `http://` / `https://` | `https://server/report.json` |

- Auth via `NVIS_WEBDAV_USER` / `NVIS_WEBDAV_PASS` env vars, `~/.netrc`,
  or URL-embedded credentials
- Follows redirects (`curl -L`)

**Adding new sources** — implement the `Source` trait in `src/source/`
and register it in `source::register_all()`. See [Architecture](#architecture).

### Step-by-step

1. **Launch** — run `nvis` or `nvis <file>`
2. **Select file** — type the path and press `Enter` (or pass it as CLI argument)
3. **Browse views** — left panel lists available views with category icons
4. **View data** — right side shows chart (top) and data table (bottom)
5. **Statistics** — press `s` to see aggregated statistics, `b` to go back

### Generating Profiler Data

**nsys** (system-wide GPU profiling):
```bash
nsys profile -o myapp ./myapp
nsys export -t sqlite myapp.nsys-rep
# Open the .sqlite file in nvis
```

**ncu** (per-kernel deep analysis):
```bash
ncu --csv -o report.csv ./myapp
# Open the .csv file in nvis
```

**torch** (PyTorch profiler):
```python
import torch.profiler as profiler

with profiler.profile(
    activities=[profiler.ProfilerActivity.CPU, profiler.ProfilerActivity.CUDA],
    record_shapes=True,
) as prof:
    # your model code here
    pass

prof.export_chrome_trace("trace.json")
# Open trace.json in nvis
```

## Controls

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit (not in input mode) |
| `↑` `↓` | Navigate / scroll |
| `←` `→` | Switch focus panel |
| `Tab` | Cycle focus areas |
| `Enter` | Confirm / load |
| `s` | Enter statistics view |
| `b` | Back from statistics |
| Mouse click | Focus panel |
| Mouse wheel | Scroll |

### Focus Areas

In the main view, `Tab` cycles: **View List** → **Chart** → **Data Table**

In statistics view, `Tab` toggles: **Chart** ↔ **Stats Table**

Yellow border = focused panel.

## Layout

```
┌──────────────┬───────────────────────────────┐
│   Views      │   Visualization               │
│              │   (Box Plot / Timeline / ...)   │
│  ⏱ CUDA K.. ├───────────────────────────────┤
│  📊 Runtime  │                               │
│  📈 Memory   │   Data Table                  │
│  ℹ  Target   │   (Rows & Columns)            │
│  📋 String.. │                               │
└──────────────┴───────────────────────────────┘
[q]Quit [↑↓]Navigate [←→]Focus [Tab]Switch [s]Stats [b]Back
```

## Feature Gates

```toml
# Default: all backends enabled
cargo build --release

# Only nsys
cargo build --release --no-default-features --features nsys

# nsys + ncu
cargo build --release --no-default-features --features "nsys,ncu"
```

| Feature | Backend | Extra dependencies |
|---------|---------|--------------------|
| `nsys` | Nsight Systems SQLite | rusqlite |
| `ncu` | Nsight Compute CSV | csv |
| `torch` | PyTorch Chrome Trace JSON | serde, serde_json |

## Architecture

```
src/
  core/         — traits (ProfilerBackend, ProfilerSession, VizRenderer) + Registry
  backends/     — nsys, ncu, torch implementations
  source/       — file source abstraction (local / ssh / webdav) + Registry
  viz/          — boxplot, timeline, barchart, statistics renderers
  app.rs        — App state machine (generic over any backend)
  ui.rs         — TUI rendering (dispatches to VizRenderer::draw)
  main.rs       — Registry setup + event loop
```

To add a new profiler backend:
1. Create `src/backends/your_backend.rs` implementing `ProfilerBackend`
2. Create a `ProfilerSession` implementation holding your parsed data
3. Register it in `backends::register_all()`
4. Done — no changes to `app.rs`, `ui.rs`, or `viz/`

To add a new file source (e.g. S3, GCS):
1. Create `src/source/your_source.rs` implementing `Source`
2. Register it in `source::register_all()` (before `LocalSource`)
3. Done — `app.rs` picks it up automatically via `SourceRegistry`

## Technical Details

- Columnar `ProfilerData` storage — no string→float re-parsing
- `Mutex<Connection>` for nsys (single shared connection)
- CSV parsed once into memory for ncu
- JSON parsed once into memory for torch
- 1000-row limit on data loading for performance
- Cell values truncated to 20 chars in tables

## License

MIT
