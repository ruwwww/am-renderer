# API Reference Guide

This document details the REST APIs, WebSocket protocol, and database schema for the stateful `preview-service` daemon.

---

## 1. REST Endpoints

All REST routes return JSON and accept `application/json` payload structures.

### `GET /api/projects`
Lists all project headers registered in the SQLite database.
- **Response**: `200 OK`
- **Body**: Array of project details:
  ```json
  [
    {
      "id": 1,
      "title": "Preset 1",
      "width": 1280,
      "height": 720,
      "fps": 30.0,
      "total_time": 5000
    }
  ]
  ```

### `GET /api/projects/:id`
Retrieves the full parsed and structured `Project` object for the given database ID.
- **Response**: `200 OK` or `404 Not Found`
- **Body**: Nested project model representation containing media references, audio tracks, and layers.

### `POST /api/projects/:id/mutate`
Applies a transactional mutation to a project's fields, writes it to the SQLite database, saves a snapshot on the in-memory undo stack, and pushes the updated project schema to the scheduler.
- **Request Body**: A JSON variant matching the `Mutation` enum schema:
  - **Option A: `update_layer_property`**
    ```json
    {
      "type": "update_layer_property",
      "layer_id": 2,
      "property": "location",
      "value": [100.0, 250.0]
    }
    ```
  - **Option B: `update_layer_property_keyframes`**
    ```json
    {
      "type": "update_layer_property_keyframes",
      "layer_id": 2,
      "property": "rotation",
      "keyframes": [
        { "t": 0.0, "value": 0.0, "easing": "Linear" },
        { "t": 1.0, "value": 360.0, "easing": "CubicBezier(0.4, 0.0, 0.2, 1.0)" }
      ]
    }
    ```
  - **Option C: `add_layer`**
    ```json
    {
      "type": "add_layer",
      "layer": {
        "id": 3,
        "label": "My Layer",
        "start_time": 0,
        "end_time": 5000,
        "visible": true,
        "transform": {
          "anchor": [0.0, 0.0],
          "position": [640.0, 360.0],
          "scale": [1.0, 1.0],
          "rotation": 0.0,
          "opacity": 1.0
        },
        "fill_type": "Color",
        "fill_color": [1.0, 0.0, 0.0, 1.0],
        "fill_image": null,
        "gradient": null,
        "media_fill_mode": null,
        "blend_mode": "Normal",
        "effects": [],
        "s": null
      }
    }
    ```
    > [!IMPORTANT]
    > To avoid parsing errors, the `add_layer` JSON payload must explicitly contain all optional/nullable fields (e.g. `fill_image`, `gradient`, `media_fill_mode`, `s`, `effects`) since no default-fallback deserialization is declared on the structs.

- **Response**: `200 OK` on success containing the mutated `Project` JSON body.

### `POST /api/projects/:id/undo`
Reverts the project state to the last snapshot on the memory-based undo stack, updates SQLite, and pushes the snapshot back to the redo stack.
- **Response**: `200 OK` (with current `Project` JSON) or `400 Bad Request` (if undo stack is empty).

### `POST /api/projects/:id/redo`
Re-applies the last undone project mutation.
- **Response**: `200 OK` (with current `Project` JSON) or `400 Bad Request` (if redo stack is empty).

---

## 2. WebSocket Protocol (`ws://localhost:8080/ws`)

Clients connect via standard WebSockets to stream visual frames and issue playback controls.

### Client-to-Server JSON Commands
The server parses incoming text frames as JSON matching the `IncomingMessage` schema:

- **Seek to Frame**:
  ```json
  { "type": "seek", "frame": 45 }
  ```
- **Play Timeline**:
  ```json
  { "type": "play", "fps": 30.0 }
  ```
- **Pause Timeline**:
  ```json
  { "type": "pause" }
  ```
- **Set Rendering Proxy Resolution Scale**:
  ```json
  { "type": "config", "scale": 0.5 }
  ```
  *(e.g., `0.5` renders at 50% width and height for high-FPS performance)*.

### Server-to-Client Binary Streams
Whenever a seek occurs or during active playback, the server emits a binary message over the socket:
- **First 4 Bytes (Big Endian `u32`)**: The frame number index.
- **Remaining Bytes**: The WebP compressed image data of the rendered frame.

The frontend receives the bytes, extracts the frame index, converts the remaining buffer into a blob URL, and renders it to the screen.

---

## 3. SQLite Database Schema

The database `db.sqlite` maintains persistent project and layer layout details:

- **`projects`**: Top-level configuration.
- **`media_refs`**: Cached asset metadata mapped to external media URIs.
- **`audio_tracks`**: Ambient sound assets, offsets, and durations.
- **`layers`**: Canvas layer bounds, names, blend options, and order hierarchies.
- **`layer_properties`**: Tracks property definitions (e.g. `position`, `scale`, `opacity`).
- **`layer_property_keyframes`**: Keyframe ease curves, normalized time frames, and target property values.
- **`layer_effects`**: Chain array sequence of effects applied to each layer.
