# am-renderer

Headless CPU-based renderer for **Alight Motion XML projects**. Parses `.xml` project files, evaluates animated properties, and renders frames as PNG sequences or MP4 video — entirely on the CPU, no GPU required.

Designed for rendering Anime edits, AMV edits, Jedag-jedug edits, and Velocity edits — transform-heavy compositions with effects like motion blur, oscillate, swing, tile, mirror, and color grading.

## Features

- **Parses** Alight Motion XML project files with all major element types
- **Evaluates** animated keyframes with cubic bezier easing (Newton-Raphson solver)
- **Renders** 25+ effects including transforms, blurs, color grading, tile/mirror, edge detection, and more
- **Exports** to PNG sequences or H.264 MP4 via ffmpeg
- **Parallelized** per-row and per-frame rendering with rayon
- **Debug mode** for visualizing layer bounding boxes

## Quick Start

```bash
# Build
cargo build --release

# Get project info
cargo run --release -- info -i presets/preset1.xml

# Render a single frame
cargo run --release -- render -i presets/preset1.xml -a assets -o frame.png --frame 0

# Render full video
cargo run --release -- render -i presets/preset1.xml -a assets -o output.mp4
```

## Requirements

- Rust 2021 edition (1.70+)
- ffmpeg (for MP4 export)

## Documentation

- [Architecture](documentation/ARCHITECTURE.md) — Overall design and pipeline
- [Module Reference](documentation/MODULE_REFERENCE.md) — Per-module API documentation
- [Effects Catalog](documentation/EFFECTS_CATALOG.md) — All supported effects with parameters
- [CLI Reference](documentation/CLI_REFERENCE.md) — Command-line usage
- [AI Agent Guide](documentation/AGENT_GUIDE.md) — Context for AI agents working with this codebase

## License

MIT
