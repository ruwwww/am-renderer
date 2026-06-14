// types.ts — Domain types mirroring the Rust backend models (graph-resolver)

// ---------------------------------------------------------------------------
// Animated<T> — matches the Rust enum: Static(T) | Keyframed(Vec<Keyframe<T>>)
// ---------------------------------------------------------------------------
export interface Keyframe<T> {
  t: number;       // normalized time [0.0, 1.0]
  value: T;
  easing: string;  // "Linear" | "CubicBezier(x1, y1, x2, y2)"
}

export type Animated<T> =
  | { Static: T }
  | { Keyframed: Keyframe<T>[] };

/** Extract the current/first value from an Animated<T> field */
export function getStaticValue<T>(animated: Animated<T> | undefined | null, defaultValue: T): T {
  if (!animated) return defaultValue;
  if ('Static' in animated) return animated.Static;
  if ('Keyframed' in animated && animated.Keyframed.length > 0)
    return animated.Keyframed[0].value;
  return defaultValue;
}

// ---------------------------------------------------------------------------
// Layer Transform
// ---------------------------------------------------------------------------
export interface LayerTransform {
  anchor: Animated<[number, number]>;
  location: Animated<[number, number, number]>;
  size: Animated<[number, number]>;
  scale: Animated<[number, number]>;
  rotation: Animated<number>;
  opacity: Animated<number>;
}

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------
export interface Layer {
  id: number;
  label: string | null;
  start_time: number;  // ms
  end_time: number;    // ms
  visible: boolean;
  transform: LayerTransform;
  blend_mode: string;
  fill_type: string;
  fill_color: [number, number, number, number] | null;
  fill_image: string | null;
  gradient: unknown | null;
  media_fill_mode: string | null;
  effects: unknown[];
  s: string | null;
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------
export interface Project {
  id: number;
  title: string | null;
  width: number;
  height: number;
  export_width: number;
  export_height: number;
  bg_color: [number, number, number, number];
  total_time: number;  // ms
  fps: number;
  layers: Layer[];
  media: MediaRef[];
  audio_tracks: AudioTrack[];
}

export interface MediaRef {
  uri: string;
  filename: string | null;
  title: string | null;
  mime_type: string | null;
  width: number | null;
  height: number | null;
}

export interface AudioTrack {
  track_id: string | null;
  label: string | null;
  start_time: number;
  end_time: number;
  src: string | null;
}

// ---------------------------------------------------------------------------
// Project list item returned by GET /api/projects
// ---------------------------------------------------------------------------
export interface ProjectListItem {
  id: number;
  title: string | null;
  width: number;
  height: number;
  fps: number;
  total_time: number;
  duration_secs: number;
}

// ---------------------------------------------------------------------------
// Mutations — matches the Rust Mutation enum
// ---------------------------------------------------------------------------
export type Mutation =
  | { type: 'update_layer_property'; layer_id: number; property: string; value: unknown }
  | { type: 'update_layer_property_keyframes'; layer_id: number; property: string; keyframes: KeyframeInput[] }
  | { type: 'add_layer'; layer: Layer };

export interface KeyframeInput {
  t: number;
  value: unknown;
  easing: string;
}

// ---------------------------------------------------------------------------
// WebSocket control messages — matches Rust IncomingMessage enum
// ---------------------------------------------------------------------------
export type WsOutgoing =
  | { type: 'seek'; frame: number }
  | { type: 'play'; fps?: number }
  | { type: 'pause' }
  | { type: 'config'; scale: number };
