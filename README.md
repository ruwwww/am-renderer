# am-renderer

Headless CPU-based renderer for **Alight Motion XML projects**. Parses `.xml` project files, evaluates animated properties, renders frames as PNG/MP4, and provides a stateful collaborative web editor.

The project is organized as a Cargo workspace containing the core Rust libraries, a preview/editing backend server, a React frontend, and comprehensive E2E integration tests.

## Workspace Packages

- **`graph-resolver`**: Core scene evaluation and animated keyframe interpolation.
- **`alight-parser`**: Deserialization of Alight Motion XML scenes to model definitions, applying coordinate system scaling.
- **`renderer-core`**: Layouts, Porter-Duff compositing, and separable pixel effects.
- **`export-service`**: Handles rendering PNG sequences and compiling MP4 video (using `ffmpeg` on PATH).
- **`preview-service`**: Stateful Axum REST API and WebSocket preview server backed by SQLite (with transactional mutations, undo/redo). Serves the editor static bundle at root.
- **`web-editor`**: React + TypeScript frontend timeline editor, canvas viewer, and real-time websocket frame synchronizer.
- **`integration-tests`**: Black-box integration, database, WebSocket, and workload tests.

## Features

- **Parses** Alight Motion XML project files.
- **Evaluates** animated keyframes with cubic bezier easing (using a Newton-Raphson solver).
- **Renders** 25+ pixel and transform effects (oscillate, swing, tile, mirror, blurs, color grading).
- **Exports** to PNG sequences or H.264 MP4.
- **Stateful Backend & Live Editing**: Local preview-service server tracks projects, mutations, history (undo/redo), and streams WebP-encoded preview frames over WebSockets.
- **Interactive Web Editor**: Visual timeline editor and playback viewer.

## Quick Start

### Core CLI Commands

```bash
# Build CLI & libraries
cargo build --release

# Print XML project metadata
cargo run --release -- info -i presets/preset1.xml

# Render a single frame
cargo run --release -- render -i presets/preset1.xml -a assets -o frame.png --frame 0

# Render a full video (requires ffmpeg)
cargo run --release -- render -i presets/preset1.xml -a assets -o output.mp4
```

### Running the Web Editor

1. **Start the Backend Preview Server**:
   ```bash
   cargo run --release -p preview-service
   ```
   This initializes `db.sqlite` and starts the API server on `http://localhost:8080/`.

2. **Run the React Frontend (Vite Dev Mode)**:
   ```bash
   cd packages/web-editor
   npm install
   npm run dev
   ```
   This opens the frontend editor on `http://localhost:5173/`.

### Running Tests

```bash
# Run Unit Tests
cargo test --workspace --exclude integration-tests

# Run Integration/E2E Tests
# (Note: Requires a running preview-service server on port 8080, and must be run sequentially)
cargo test -p integration-tests -- --test-threads=1
```

## Requirements

- **Rust** (Edition 2021, 1.70+)
- **Node.js & npm** (for the web-editor)
- **ffmpeg** (optional, on PATH for MP4 export)

## Documentation

- [Architecture Guide](documentation/ARCHITECTURE.md) — Multi-crate workflow and websocket frame rendering pipeline.
- [API Reference](documentation/API_REFERENCE.md) — REST endpoints, WebSocket command protocols, and SQLite schema.
- [AI Agent Guide](documentation/AGENT_GUIDE.md) — Guide for AI agents contextually working with this repository.
- [Module Reference](documentation/MODULE_REFERENCE.md) — Workspace packages and source code layouts.
- [Effects Catalog](documentation/EFFECTS_CATALOG.md) — Parameters and behavior for all supported effects.
- [CLI Reference](documentation/CLI_REFERENCE.md) — Command-line interface usage.

## License

MIT
