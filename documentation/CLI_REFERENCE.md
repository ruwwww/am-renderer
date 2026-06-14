# CLI Reference

The workspace contains two executable binaries: the core renderer CLI (`am-renderer`) and the backend preview server daemon (`preview-service`).

---

## 1. Renderer CLI (`am-renderer`)

Run the core renderer via cargo:
```bash
cargo run --release -- <command> [options]
```

### Global Options
- `-h`, `--help`: Prints help information
- `-V`, `--version`: Prints version information

### Command: `info` — Get Project Metadata
Prints XML parsed information, target export boundaries, layers, duration, FPS, and distinct media file requirements.
```bash
cargo run --release -- info -i <input.xml>
```
* `-i`, `--input`: Path to Alight Motion XML file.

### Command: `render` — Render Project/Frame Range
```bash
cargo run --release -- render -i <input.xml> -a <assets_dir> -o <output_path> [options]
```

#### Required Arguments
- `-i`, `--input`: Path to Alight Motion XML file.
- `-a`, `--assets`: Path to physical directory containing media files.
- `-o`, `--output`: Target path (directory for PNG sequence, file path for MP4 video).

#### Options
| Flag | Description |
|------|-------------|
| `--format <png\|mp4>` | Force output format (otherwise inferred from `--output` extension). |
| `--frame <N>` | Render only a single frame index `N` to the output directory. |
| `--start-frame <N>` | Start frame of range to render (inclusive, zero-indexed). |
| `--end-frame <N>` | End frame of range to render (exclusive). |
| `--start-time <secs>` | Start time of range in seconds (inclusive). |
| `--end-time <secs>` | End time of range in seconds (exclusive). |
| `--dump-graph` | Dump evaluated timeline state parameters to stdout. |
| `--auto-pair` | Auto-pair XML virtual URIs to assets in directory (defaults to true). |
| `--debug-layout` | Render layout outlines, anchor crosshairs, and layer metadata overlay. |
| `--debug-effects` | Render separate debug images isolating every individual visual effect in the layer stack. |
| `--proxy-scale <factor>` | Downscale canvas size for lower resolution proxies (e.g. `0.25` for quarter size). |
| `--config <path.toml>` | Path to a TOML configuration file containing disabled effects options. |

#### CLI Examples

```bash
# Render frame 100 as PNG with debug layout lines
cargo run --release -- render -i preset.xml -a assets/ -o output/ --frame 100 --debug-layout

# Render video between 2.5s and 5.0s
cargo run --release -- render -i preset.xml -a assets/ -o video.mp4 --start-time 2.5 --end-time 5.0

# Render quarter-size proxy sequence to output directory
cargo run --release -- render -i preset.xml -a assets/ -o output_frames/ --proxy-scale 0.25
```

---

## 2. Preview Server Daemon (`preview-service`)

Starts the stateful Axum REST and WebSocket preview server.
```bash
cargo run --release -p preview-service
```

### Environment Variables
- `PORT` (default: `8080`): Port for the preview-service.
- `DATABASE_URL` (default: `sqlite://db.sqlite`): SQLite connection string.
- `RUST_LOG` (e.g. `RUST_LOG=debug`): Log level filtering.
