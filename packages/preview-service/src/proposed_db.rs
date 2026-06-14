use std::path::Path;
use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection};
use anyhow::{Context, Result};
use graph_resolver::model::*;
use graph_resolver::model::effect::*;

/// Database handle wrapping a thread-safe connection to SQLite.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open a database at the given path and run schema migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite database at {:?}", path))?;
        
        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    /// Open an in-memory database (primarily for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to open in-memory SQLite database")?;
        
        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    /// Create tables if they do not exist.
    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute_batch(
            "BEGIN;

             CREATE TABLE IF NOT EXISTS projects (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT,
                 width INTEGER NOT NULL,
                 height INTEGER NOT NULL,
                 export_width INTEGER NOT NULL,
                 export_height INTEGER NOT NULL,
                 bg_color_r REAL NOT NULL,
                 bg_color_g REAL NOT NULL,
                 bg_color_b REAL NOT NULL,
                 bg_color_a REAL NOT NULL,
                 total_time REAL NOT NULL,
                 fps REAL NOT NULL
             );

             CREATE TABLE IF NOT EXISTS media_refs (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id INTEGER NOT NULL,
                 uri TEXT NOT NULL,
                 filename TEXT,
                 title TEXT,
                 mime_type TEXT,
                 width INTEGER,
                 height INTEGER,
                 FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS audio_tracks (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id INTEGER NOT NULL,
                 label TEXT,
                 start_time REAL NOT NULL,
                 end_time REAL NOT NULL,
                 src TEXT,
                 in_time REAL NOT NULL,
                 out_time REAL,
                 FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS layers (
                 id INTEGER PRIMARY KEY, -- Using the shape ID parsed from XML
                 project_id INTEGER NOT NULL,
                 label TEXT,
                 start_time REAL NOT NULL,
                 end_time REAL NOT NULL,
                 hidden INTEGER NOT NULL, -- boolean
                 blend_mode TEXT NOT NULL,
                 fill_type TEXT NOT NULL,
                 fill_image TEXT,
                 fill_color_r REAL,
                 fill_color_g REAL,
                 fill_color_b REAL,
                 fill_color_a REAL,
                 size_w REAL NOT NULL,
                 size_h REAL NOT NULL,
                 s TEXT,
                 sort_order INTEGER NOT NULL,
                 FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS effects (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 layer_id INTEGER NOT NULL,
                 effect_type TEXT NOT NULL,
                 locally_applied INTEGER NOT NULL, -- boolean
                 params_json TEXT NOT NULL, -- JSON-serialized EffectType
                 sort_order INTEGER NOT NULL,
                 FOREIGN KEY(layer_id) REFERENCES layers(id) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS properties (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 layer_id INTEGER, -- NULL if belonging to an effect (future-proofing)
                 effect_id INTEGER, -- NULL if belonging to a layer
                 name TEXT NOT NULL, -- e.g. 'location', 'scale'
                 is_static INTEGER NOT NULL, -- boolean
                 static_val TEXT, -- JSON value string (if static)
                 FOREIGN KEY(layer_id) REFERENCES layers(id) ON DELETE CASCADE,
                 FOREIGN KEY(effect_id) REFERENCES effects(id) ON DELETE CASCADE,
                 CHECK ((layer_id IS NOT NULL AND effect_id IS NULL) OR (layer_id IS NULL AND effect_id IS NOT NULL))
             );

             CREATE TABLE IF NOT EXISTS keyframes (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 property_id INTEGER NOT NULL,
                 t REAL NOT NULL, -- normalized time (0.0–1.0)
                 value TEXT NOT NULL, -- JSON serialized value
                 easing TEXT, -- JSON serialized EasingType
                 FOREIGN KEY(property_id) REFERENCES properties(id) ON DELETE CASCADE
             );

             COMMIT;"
        )?;
        Ok(())
    }

    /// Save a Project domain model into the database within a transaction.
    /// Returns the assigned project_id.
    pub fn save_project(&self, project: &Project) -> Result<u64> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. Insert Project
        tx.execute(
            "INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                project.title,
                project.width,
                project.height,
                project.export_width,
                project.export_height,
                project.bg_color[0],
                project.bg_color[1],
                project.bg_color[2],
                project.bg_color[3],
                project.total_time,
                project.fps,
            ],
        )?;
        let project_id = tx.last_insert_rowid() as u64;

        // 2. Insert Media References
        for media in &project.media {
            tx.execute(
                "INSERT INTO media_refs (project_id, uri, filename, title, mime_type, width, height)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    project_id,
                    media.uri,
                    media.filename,
                    media.title,
                    media.mime_type,
                    media.width,
                    media.height,
                ],
            )?;
        }

        // 3. Insert Audio Tracks
        for track in &project.audio_tracks {
            tx.execute(
                "INSERT INTO audio_tracks (project_id, label, start_time, end_time, src, in_time, out_time)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    project_id,
                    track.label,
                    track.start_time,
                    track.end_time,
                    track.src,
                    track.in_time,
                    track.out_time,
                ],
            )?;
        }

        // 4. Insert Layers
        for (idx, layer) in project.layers.iter().enumerate() {
            tx.execute(
                "INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, fill_image, fill_color_r, fill_color_g, fill_color_b, fill_color_a, size_w, size_h, s, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    layer.id,
                    project_id,
                    layer.label,
                    layer.start_time,
                    layer.end_time,
                    if layer.hidden { 1 } else { 0 },
                    format!("{:?}", layer.blend_mode),
                    format!("{:?}", layer.fill_type),
                    layer.fill_image,
                    layer.fill_color[0],
                    layer.fill_color[1],
                    layer.fill_color[2],
                    layer.fill_color[3],
                    layer.size[0],
                    layer.size[1],
                    layer.s,
                    idx as i32,
                ],
            )?;

            // Save Animatable Layer Transform Properties relationally
            Self::save_property(&tx, Some(layer.id), None, "location", &layer.transform.location)?;
            Self::save_property(&tx, Some(layer.id), None, "scale", &layer.transform.scale)?;
            Self::save_property(&tx, Some(layer.id), None, "rotation", &layer.transform.rotation)?;
            Self::save_property(&tx, Some(layer.id), None, "opacity", &layer.transform.opacity)?;

            // 5. Insert Effects (using JSON serialization for params_json to avoid heavy boilerplate)
            for (eff_idx, effect) in layer.effects.iter().enumerate() {
                let params_json = serde_json::to_string(&effect.effect_type)?;
                tx.execute(
                    "INSERT INTO effects (layer_id, effect_type, locally_applied, params_json, sort_order)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        layer.id,
                        effect.effect_type.type_name(),
                        if effect.locally_applied { 1 } else { 0 },
                        params_json,
                        eff_idx as i32,
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(project_id)
    }

    /// Load a Project domain model from the database.
    pub fn load_project(&self, project_id: u64) -> Result<Project> {
        let conn = self.conn.lock().unwrap();
        
        // Use a transaction/savepoint for read consistency if needed, but standard connection is fine.
        // We'll prepare queries
        
        // 1. Load project metadata
        let (title, width, height, export_width, export_height, bg_color, total_time, fps) = conn.query_row(
            "SELECT title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps
             FROM projects WHERE id = ?1",
            params![project_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, u32>(4)?,
                    [
                        row.get::<_, f32>(5)?,
                        row.get::<_, f32>(6)?,
                        row.get::<_, f32>(7)?,
                        row.get::<_, f32>(8)?,
                    ],
                    row.get::<_, f32>(9)?,
                    row.get::<_, f32>(10)?,
                ))
            },
        )?;

        // 2. Load media references
        let mut stmt = conn.prepare(
            "SELECT uri, filename, title, mime_type, width, height FROM media_refs WHERE project_id = ?1"
        )?;
        let media_iter = stmt.query_map(params![project_id], |row| {
            Ok(MediaRef {
                uri: row.get(0)?,
                filename: row.get(1)?,
                title: row.get(2)?,
                mime_type: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
            })
        })?;
        let mut media = Vec::new();
        for m in media_iter {
            media.push(m?);
        }

        // 3. Load audio tracks
        let mut stmt = conn.prepare(
            "SELECT id, label, start_time, end_time, src, in_time, out_time FROM audio_tracks WHERE project_id = ?1"
        )?;
        let audio_iter = stmt.query_map(params![project_id], |row| {
            Ok(AudioTrack {
                id: row.get(0)?,
                label: row.get(1)?,
                start_time: row.get(2)?,
                end_time: row.get(3)?,
                src: row.get(4)?,
                in_time: row.get(5)?,
                out_time: row.get(6)?,
            })
        })?;
        let mut audio_tracks = Vec::new();
        for a in audio_iter {
            audio_tracks.push(a?);
        }

        // 4. Load layers
        let mut stmt = conn.prepare(
            "SELECT id, label, start_time, end_time, hidden, blend_mode, fill_type, fill_image, fill_color_r, fill_color_g, fill_color_b, fill_color_a, size_w, size_h, s
             FROM layers WHERE project_id = ?1 ORDER BY sort_order"
        )?;
        
        // For sub-queries, we create a temporary transaction to pass around easily
        // (rusqlite transaction allows multiple active statements if structured right,
        // but since we are doing simple reads, we can query them with helper queries).
        let layers_rows_res: Result<Vec<_>> = stmt.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, f32>(2)?,
                row.get::<_, f32>(3)?,
                row.get::<_, i32>(4)? != 0,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                [
                    row.get::<_, f32>(8)?,
                    row.get::<_, f32>(9)?,
                    row.get::<_, f32>(10)?,
                    row.get::<_, f32>(11)?,
                ],
                [row.get::<_, f32>(12)?, row.get::<_, f32>(13)?],
                row.get::<_, Option<String>>(14)?,
            ))
        })?.collect();
        
        let mut layers = Vec::new();
        for row in layers_rows_res? {
            let (id, label, start_time, end_time, hidden, blend_mode_str, fill_type_str, fill_image, fill_color, size, s) = row;
            
            // Map strings back to Enums
            let blend_mode = match blend_mode_str.as_str() {
                "Normal" => BlendMode::Normal,
                "Multiply" => BlendMode::Multiply,
                "Screen" => BlendMode::Screen,
                "Overlay" => BlendMode::Overlay,
                "Darken" => BlendMode::Darken,
                "Lighten" => BlendMode::Lighten,
                "Subtract" => BlendMode::Subtract,
                "Add" => BlendMode::Add,
                _ => BlendMode::Normal,
            };
            
            let fill_type = match fill_type_str.as_str() {
                "None" => FillType::None,
                "Media" => FillType::Media,
                "Color" => FillType::Color,
                "Gradient" => FillType::Gradient,
                _ => FillType::None,
            };

            // Load properties (location, scale, rotation, opacity)
            let location = Self::load_property(&conn, Some(id), None, "location", [0.0, 0.0, 0.0])?;
            let scale = Self::load_property(&conn, Some(id), None, "scale", [1.0, 1.0])?;
            let rotation = Self::load_property(&conn, Some(id), None, "rotation", 0.0)?;
            let opacity = Self::load_property(&conn, Some(id), None, "opacity", 1.0)?;
            let transform = LayerTransform { location, scale, rotation, opacity };

            // Load effects
            let mut eff_stmt = conn.prepare(
                "SELECT effect_type, locally_applied, params_json FROM effects WHERE layer_id = ?1 ORDER BY sort_order"
            )?;
            let eff_iter = eff_stmt.query_map(params![id], |eff_row| {
                let effect_type_str = eff_row.get::<_, String>(0)?;
                let locally_applied = eff_row.get::<_, i32>(1)? != 0;
                let params_json = eff_row.get::<_, String>(2)?;
                
                // Deserialise the EffectType enum from the JSON string
                let effect_type: EffectType = serde_json::from_str(&params_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;
                
                Ok(Effect { effect_type, locally_applied })
            })?;
            
            let mut effects = Vec::new();
            for eff in eff_iter {
                effects.push(eff?);
            }

            layers.push(Layer {
                id,
                label,
                start_time,
                end_time,
                hidden,
                transform,
                fill_type,
                fill_image,
                fill_color,
                gradient: None, // Gradient stored as Option<Gradient>, can be added if needed
                blend_mode,
                media_fill_mode: None,
                effects,
                size,
                s,
            });
        }

        Ok(Project {
            title,
            width,
            height,
            export_width,
            export_height,
            bg_color,
            total_time,
            fps,
            media,
            audio_tracks,
            layers,
        })
    }

    /// List all project IDs and titles.
    pub fn list_projects(&self) -> Result<Vec<(u64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, COALESCE(title, 'Untitled') FROM projects ORDER BY id DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Delete a project and all its cascades.
    pub fn delete_project(&self, project_id: u64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
        Ok(())
    }

    // -- Private Helper Functions --

    /// Helper to save a single animatable/static property.
    fn save_property<T: serde::Serialize + Clone>(
        tx: &rusqlite::Transaction,
        layer_id: Option<u64>,
        effect_id: Option<i64>,
        name: &str,
        prop: &Animated<T>,
    ) -> Result<()> {
        let (is_static, static_val) = match prop {
            Animated::Static(v) => (1, Some(serde_json::to_string(v).unwrap())),
            Animated::Keyframed(_) => (0, None),
        };

        tx.execute(
            "INSERT INTO properties (layer_id, effect_id, name, is_static, static_val)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![layer_id, effect_id, name, is_static, static_val],
        )?;
        let prop_id = tx.last_insert_rowid();

        if let Animated::Keyframed(keyframes) = prop {
            for kf in keyframes {
                let value_str = serde_json::to_string(&kf.value).unwrap();
                let easing_str = serde_json::to_string(&kf.easing).unwrap();
                tx.execute(
                    "INSERT INTO keyframes (property_id, t, value, easing)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![prop_id, kf.t, value_str, easing_str],
                )?;
            }
        }

        Ok(())
    }

    /// Helper to load a single animatable/static property.
    fn load_property<T: serde::de::DeserializeOwned + Clone>(
        conn: &Connection,
        layer_id: Option<u64>,
        effect_id: Option<i64>,
        name: &str,
        default_val: T,
    ) -> Result<Animated<T>> {
        let prop_opt = conn.query_row(
            "SELECT id, is_static, static_val FROM properties 
             WHERE (layer_id = ?1 OR (?1 IS NULL AND layer_id IS NULL))
               AND (effect_id = ?2 OR (?2 IS NULL AND effect_id IS NULL))
               AND name = ?3",
            params![layer_id, effect_id, name],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i32>(1)? != 0,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        );

        match prop_opt {
            Ok((prop_id, is_static, static_val)) => {
                if is_static {
                    let val_str = static_val.unwrap_or_else(|| serde_json::to_string(&default_val).unwrap());
                    let val: T = serde_json::from_str(&val_str)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?;
                    Ok(Animated::Static(val))
                } else {
                    let mut kf_stmt = conn.prepare(
                        "SELECT t, value, easing FROM keyframes WHERE property_id = ?1 ORDER BY t"
                    )?;
                    let kf_iter = kf_stmt.query_map(params![prop_id], |row| {
                        let t = row.get::<_, f64>(0)? as f32;
                        let value_str = row.get::<_, String>(1)?;
                        let easing_str = row.get::<_, String>(2)?;
                        
                        let value: T = serde_json::from_str(&value_str)
                            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            ))?;
                        
                        let easing: EasingType = serde_json::from_str(&easing_str)
                            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            ))?;
                        
                        Ok(Keyframe { t, value, easing })
                    })?;
                    
                    let mut keyframes = Vec::new();
                    for kf in kf_iter {
                        keyframes.push(kf?);
                    }
                    Ok(Animated::Keyframed(keyframes))
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Return default value if not found
                Ok(Animated::Static(default_val))
            }
            Err(e) => Err(e),
        }
    }
}

/// Helper function to scan a directory for Alight Motion XML presets, parse them,
/// and seed them into the database.
pub fn seed_presets_from_directory(db: &Database, presets_dir: &Path) -> Result<usize> {
    let mut count = 0;
    if !presets_dir.exists() {
        log::warn!("Presets directory {:?} does not exist. Skipping seeding.", presets_dir);
        return Ok(0);
    }

    log::info!("Seeding database with XML presets from {:?}", presets_dir);
    for entry in std::fs::read_dir(presets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("xml") {
            log::info!("Parsing & importing preset XML: {:?}", path.file_name().unwrap());
            match alight_parser::parse_xml(&path) {
                Ok(xml_scene) => {
                    match alight_parser::convert_project(&xml_scene, None) {
                        Ok(project) => {
                            match db.save_project(&project) {
                                Ok(pid) => {
                                    log::info!("Successfully imported preset with ID {}", pid);
                                    count += 1;
                                }
                                Err(e) => {
                                    log::error!("Failed to save project from {:?}: {}", path, e);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to convert project from XML {:?}: {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to parse XML preset {:?}: {}", path, e);
                }
            }
        }
    }

    log::info!("Seeding completed. Imported {} projects.", count);
    Ok(count)
}
