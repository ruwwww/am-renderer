# CLI Reference

## Usage

```
am-renderer <command> [options]
```

## Global Options

| Flag | Description |
|------|-------------|
| `-h`, `--help` | Print help information |
| `-V`, `--version` | Print version information |

## Commands

### `info` — Print project metadata

```
am-renderer info -i <input.xml>
```

Prints:
- Project title and dimensions
- Duration and FPS
- Total frame count
- Layer count and names
- Required media assets (virtual URIs)

### `render` — Render a project

```
am-renderer render -i <input.xml> -a <assets_dir> -o <output> [options]
```

#### Required Arguments

| Arg | Description |
|-----|-------------|
| `-i`, `--input` | Path to Alight Motion XML file |
| `-a`, `--assets` | Path to directory containing imported media assets |
| `-o`, `--output` | Output path (directory for PNG sequence, file for MP4) |

#### Output Format Detection

If `--format` is not specified, the format is auto-detected from the output path extension:
- `.mp4`, `.mov`, `.avi` → MP4 video
- `.png`, directory path, or other → PNG sequence

#### Options

| Option | Description |
|--------|-------------|
| `--format <png\|mp4>` | Force output format (overrides auto-detection) |
| `--frame <N>` | Render only frame N instead of full sequence |
| `--start-frame <N>` | Start frame of the range to render (inclusive) |
| `--end-frame <N>` | End frame of the range to render (exclusive) |
| `--start-time <secs>` | Start time of the range to render in seconds (inclusive) |
| `--end-time <secs>` | End time of the range to render in seconds (exclusive) |
| `--dump-graph` | Print the compiled render/effect graph for each evaluated frame |
| `--auto-pair` | Automatically pair XML virtual URIs to asset files in the assets directory (round-robin) |
| `--debug-layout` | Render debug overlay showing layer bounding boxes, names, and edge detection |

#### Examples

```bash
# Render a single frame as PNG
am-renderer render -i preset.xml -a assets -o frame_42.png --frame 42

# Render a specific frame range as a video (frames 100 to 200)
am-renderer render -i preset.xml -a assets -o output.mp4 --start-frame 100 --end-frame 200

# Render a specific time range as a video (from 1.5 seconds to 4.0 seconds)
am-renderer render -i preset.xml -a assets -o output.mp4 --start-time 1.5 --end-time 4.0

# Render full video
am-renderer render -i preset.xml -a assets -o output.mp4

# Render frame sequence to directory
am-renderer render -i preset.xml -a assets -o frames/

# Render with auto-pairing and debug layout
am-renderer render -i preset.xml -a assets -o output.mp4 --auto-pair --debug-layout
```

## Environment

- `RUST_LOG=debug` — Enable debug logging (uses `env_logger`)
- Requires `ffmpeg` on PATH for MP4 export
