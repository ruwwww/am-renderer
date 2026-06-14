use std::path::Path;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Serialize, de::DeserializeOwned};

use graph_resolver::model::{
    Project, MediaRef, AudioTrack, Layer, LayerTransform,
    BlendMode, FillType, Gradient, Effect, Animated, Keyframe, EasingType
};

/// Simple struct representing a lightweight project summary for selection lists
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ProjectBrief {
    pub id: i64,
    pub title: Option<String>,
    pub width: u32,
    pub height: u32,
    pub duration_secs: f32,
    pub fps: f32,
}

/// Initialize the database, create tables if they do not exist
pub fn init_db(db_path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute("PRAGMA foreign_keys = ON;", [])?;
    
    conn.execute_batch(r#"
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
            fps REAL NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
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
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS audio_tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            track_id INTEGER NOT NULL,
            label TEXT,
            start_time REAL NOT NULL,
            end_time REAL NOT NULL,
            src TEXT,
            in_time REAL NOT NULL,
            out_time REAL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS layers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            layer_id INTEGER NOT NULL,
            label TEXT,
            start_time REAL NOT NULL,
            end_time REAL NOT NULL,
            hidden INTEGER NOT NULL DEFAULT 0,
            fill_type TEXT NOT NULL,
            fill_image TEXT,
            fill_color_r REAL,
            fill_color_g REAL,
            fill_color_b REAL,
            fill_color_a REAL,
            gradient_json TEXT,
            blend_mode TEXT NOT NULL DEFAULT 'Normal',
            media_fill_mode TEXT,
            size_w REAL NOT NULL,
            size_h REAL NOT NULL,
            shape_primitive TEXT,
            effects_json TEXT,
            sort_order INTEGER NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS properties (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            layer_id INTEGER NOT NULL,
            property_name TEXT NOT NULL,
            is_animated INTEGER NOT NULL DEFAULT 0,
            static_value TEXT,
            FOREIGN KEY (layer_id) REFERENCES layers(id) ON DELETE CASCADE,
            UNIQUE(layer_id, property_name)
        );

        CREATE TABLE IF NOT EXISTS keyframes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            property_id INTEGER NOT NULL,
            t REAL NOT NULL,
            value TEXT NOT NULL,
            easing_type TEXT NOT NULL DEFAULT 'Linear',
            easing_params TEXT,
            FOREIGN KEY (property_id) REFERENCES properties(id) ON DELETE CASCADE
        );
    "#)?;
    
    Ok(conn)
}

/// Helper: Map BlendMode enum to String representation
pub fn blend_mode_to_str(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "Normal",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
        BlendMode::Overlay => "Overlay",
        BlendMode::Darken => "Darken",
        BlendMode::Lighten => "Lighten",
        BlendMode::Subtract => "Subtract",
        BlendMode::Add => "Add",
    }
}

/// Helper: Map String representation back to BlendMode enum
fn str_to_blend_mode(s: &str) -> BlendMode {
    match s {
        "Multiply" => BlendMode::Multiply,
        "Screen" => BlendMode::Screen,
        "Overlay" => BlendMode::Overlay,
        "Darken" => BlendMode::Darken,
        "Lighten" => BlendMode::Lighten,
        "Subtract" => BlendMode::Subtract,
        "Add" => BlendMode::Add,
        _ => BlendMode::Normal,
    }
}

/// Helper: Map FillType enum to String representation
pub fn fill_type_to_str(t: FillType) -> &'static str {
    match t {
        FillType::None => "None",
        FillType::Media => "Media",
        FillType::Color => "Color",
        FillType::Gradient => "Gradient",
    }
}

/// Helper: Map String representation back to FillType enum
fn str_to_fill_type(s: &str) -> FillType {
    match s {
        "Media" => FillType::Media,
        "Color" => FillType::Color,
        "Gradient" => FillType::Gradient,
        _ => FillType::None,
    }
}

/// Helper: Insert an Animated property and its optional keyframes
pub fn insert_property<T: Serialize + Clone>(
    tx: &rusqlite::Transaction,
    layer_db_id: i64,
    name: &str,
    prop: &Animated<T>
) -> anyhow::Result<()> {
    match prop {
        Animated::Static(val) => {
            let val_str = serde_json::to_string(val)?;
            tx.execute(
                "INSERT INTO properties (layer_id, property_name, is_animated, static_value)
                 VALUES (?1, ?2, 0, ?3)",
                params![layer_db_id, name, val_str],
            )?;
        }
        Animated::Keyframed(kfs) => {
            tx.execute(
                "INSERT INTO properties (layer_id, property_name, is_animated, static_value)
                 VALUES (?1, ?2, 1, NULL)",
                params![layer_db_id, name],
            )?;
            let property_id = tx.last_insert_rowid();
            
            for kf in kfs {
                let val_str = serde_json::to_string(&kf.value)?;
                let (easing_type, easing_params) = match &kf.easing {
                    EasingType::Linear => ("Linear".to_string(), None),
                    EasingType::CubicBezier(x1, y1, x2, y2) => {
                        ("CubicBezier".to_string(), Some(serde_json::to_string(&[*x1, *y1, *x2, *y2])?))
                    }
                };
                tx.execute(
                    "INSERT INTO keyframes (property_id, t, value, easing_type, easing_params)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![property_id, kf.t, val_str, easing_type, easing_params],
                )?;
            }
        }
    }
    Ok(())
}

/// Helper: Retrieve an Animated property and reconstruct keyframes
fn get_property<T: DeserializeOwned + Clone>(
    conn: &Connection,
    layer_db_id: i64,
    name: &str,
    default_val: T
) -> anyhow::Result<Animated<T>> {
    let prop_opt = conn.query_row(
        "SELECT id, is_animated, static_value FROM properties 
         WHERE layer_id = ?1 AND property_name = ?2",
        params![layer_db_id, name],
        |row| {
            let id: i64 = row.get(0)?;
            let is_animated: i32 = row.get(1)?;
            let static_value: Option<String> = row.get(2)?;
            Ok((id, is_animated, static_value))
        }
    ).optional()?;
    
    let (prop_id, is_animated, static_value) = match prop_opt {
        Some(val) => val,
        None => return Ok(Animated::Static(default_val)),
    };
    
    if is_animated == 0 {
        let val_str = static_value.ok_or_else(|| anyhow::anyhow!("missing static value"))?;
        let val: T = serde_json::from_str(&val_str)?;
        Ok(Animated::Static(val))
    } else {
        let mut kf_stmt = conn.prepare(
            "SELECT t, value, easing_type, easing_params FROM keyframes 
             WHERE property_id = ?1 ORDER BY t ASC"
        )?;
        
        let kfs_iter = kf_stmt.query_map(params![prop_id], |row| {
            let t: f32 = row.get(0)?;
            let val_str: String = row.get(1)?;
            let easing_type: String = row.get(2)?;
            let easing_params_str: Option<String> = row.get(3)?;
            
            Ok((t, val_str, easing_type, easing_params_str))
        })?;
        
        let mut keyframes = Vec::new();
        for kf_row in kfs_iter {
            let (t, val_str, easing_type, easing_params_str) = kf_row?;
            let value: T = serde_json::from_str(&val_str)?;
            let easing = match easing_type.as_str() {
                "CubicBezier" => {
                    let params_str = easing_params_str.ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0, rusqlite::types::Type::Text, "missing easing params".into()
                        )
                    })?;
                    let p: [f32; 4] = serde_json::from_str(&params_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0, rusqlite::types::Type::Text, e.to_string().into()
                        )
                    })?;
                    EasingType::CubicBezier(p[0], p[1], p[2], p[3])
                }
                _ => EasingType::Linear,
            };
            
            keyframes.push(Keyframe { t, value, easing });
        }
        
        if keyframes.is_empty() {
            return Ok(Animated::Static(default_val));
        }
        
        Ok(Animated::Keyframed(keyframes))
    }
}

/// Insert a complete Project into the database in a transaction
pub fn insert_project(conn: &mut Connection, project: &Project) -> anyhow::Result<i64> {
    let tx = conn.transaction()?;
    
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
            project.fps
        ],
    )?;
    let project_id = tx.last_insert_rowid();
    
    // 1. Insert Media refs
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
                media.height
            ],
        )?;
    }
    
    // 2. Insert Audio Tracks
    for audio in &project.audio_tracks {
        tx.execute(
            "INSERT INTO audio_tracks (project_id, track_id, label, start_time, end_time, src, in_time, out_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project_id,
                audio.id,
                audio.label,
                audio.start_time,
                audio.end_time,
                audio.src,
                audio.in_time,
                audio.out_time
            ],
        )?;
    }
    
    // 3. Insert Visual Layers (shapes)
    for (i, layer) in project.layers.iter().enumerate() {
        let gradient_str = layer.gradient.as_ref()
            .map(|g| serde_json::to_string(g))
            .transpose()?;
        
        let effects_str = serde_json::to_string(&layer.effects)?;

        tx.execute(
            "INSERT INTO layers (project_id, layer_id, label, start_time, end_time, hidden, fill_type, fill_image, fill_color_r, fill_color_g, fill_color_b, fill_color_a, gradient_json, blend_mode, media_fill_mode, size_w, size_h, shape_primitive, effects_json, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                project_id,
                layer.id,
                layer.label,
                layer.start_time,
                layer.end_time,
                if layer.hidden { 1 } else { 0 },
                fill_type_to_str(layer.fill_type),
                layer.fill_image,
                layer.fill_color[0],
                layer.fill_color[1],
                layer.fill_color[2],
                layer.fill_color[3],
                gradient_str,
                blend_mode_to_str(layer.blend_mode),
                layer.media_fill_mode,
                layer.size[0],
                layer.size[1],
                layer.s,
                effects_str,
                i as i64
            ],
        )?;
        let layer_db_id = tx.last_insert_rowid();
        
        // Save animated transform properties
        insert_property(&tx, layer_db_id, "location", &layer.transform.location)?;
        insert_property(&tx, layer_db_id, "scale", &layer.transform.scale)?;
        insert_property(&tx, layer_db_id, "rotation", &layer.transform.rotation)?;
        insert_property(&tx, layer_db_id, "opacity", &layer.transform.opacity)?;
    }
    
    tx.commit()?;
    Ok(project_id)
}

/// Retrieve a fully populated Project from the database
pub fn get_project(conn: &Connection, id: i64) -> anyhow::Result<Project> {
    // 1. Load project root metadata
    let mut root: Project = conn.query_row(
        "SELECT title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps 
         FROM projects WHERE id = ?1",
        params![id],
        |row| {
            let r: f32 = row.get(5)?;
            let g: f32 = row.get(6)?;
            let b: f32 = row.get(7)?;
            let a: f32 = row.get(8)?;
            Ok(Project {
                title: row.get(0)?,
                width: row.get(1)?,
                height: row.get(2)?,
                export_width: row.get(3)?,
                export_height: row.get(4)?,
                bg_color: [r, g, b, a],
                total_time: row.get(9)?,
                fps: row.get(10)?,
                media: Vec::new(),
                audio_tracks: Vec::new(),
                layers: Vec::new(),
            })
        }
    )?;
    
    // 2. Load media refs
    let mut media_stmt = conn.prepare(
        "SELECT uri, filename, title, mime_type, width, height FROM media_refs WHERE project_id = ?1"
    )?;
    let media_rows = media_stmt.query_map(params![id], |row| {
        Ok(MediaRef {
            uri: row.get(0)?,
            filename: row.get(1)?,
            title: row.get(2)?,
            mime_type: row.get(3)?,
            width: row.get(4)?,
            height: row.get(5)?,
        })
    })?;
    for row in media_rows {
        root.media.push(row?);
    }
    
    // 3. Load audio tracks
    let mut audio_stmt = conn.prepare(
        "SELECT track_id, label, start_time, end_time, src, in_time, out_time FROM audio_tracks WHERE project_id = ?1"
    )?;
    let audio_rows = audio_stmt.query_map(params![id], |row| {
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
    for row in audio_rows {
        root.audio_tracks.push(row?);
    }
    
    // 4. Load visual layers (shapes) ordered by sort_order
    let mut layer_stmt = conn.prepare(
        "SELECT id, layer_id, label, start_time, end_time, hidden, fill_type, fill_image, fill_color_r, fill_color_g, fill_color_b, fill_color_a, gradient_json, blend_mode, media_fill_mode, size_w, size_h, shape_primitive, effects_json 
         FROM layers WHERE project_id = ?1 ORDER BY sort_order ASC"
    )?;
    
    let layer_rows = layer_stmt.query_map(params![id], |row| {
        let db_id: i64 = row.get(0)?;
        let layer_id: u64 = row.get(1)?;
        let label: Option<String> = row.get(2)?;
        let start_time: f32 = row.get(3)?;
        let end_time: f32 = row.get(4)?;
        let hidden_val: i32 = row.get(5)?;
        let fill_type_str: String = row.get(6)?;
        let fill_image: Option<String> = row.get(7)?;
        let fill_r: f32 = row.get(8)?;
        let fill_g: f32 = row.get(9)?;
        let fill_b: f32 = row.get(10)?;
        let fill_a: f32 = row.get(11)?;
        let gradient_str: Option<String> = row.get(12)?;
        let blend_mode_str: String = row.get(13)?;
        let media_fill_mode: Option<String> = row.get(14)?;
        let size_w: f32 = row.get(15)?;
        let size_h: f32 = row.get(16)?;
        let shape_primitive: Option<String> = row.get(17)?;
        let effects_str: String = row.get(18)?;
        
        Ok((
            db_id, layer_id, label, start_time, end_time, hidden_val, fill_type_str, fill_image,
            fill_r, fill_g, fill_b, fill_a, gradient_str, blend_mode_str, media_fill_mode,
            size_w, size_h, shape_primitive, effects_str
        ))
    })?;
    
    for row in layer_rows {
        let (
            db_id, layer_id, label, start_time, end_time, hidden_val, fill_type_str, fill_image,
            fill_r, fill_g, fill_b, fill_a, gradient_str, blend_mode_str, media_fill_mode,
            size_w, size_h, shape_primitive, effects_str
        ) = row?;
        
        // Reconstruct gradient
        let gradient: Option<Gradient> = gradient_str
            .map(|s| serde_json::from_str(&s))
            .transpose()?;
            
        // Reconstruct effects
        let effects: Vec<Effect> = serde_json::from_str(&effects_str)?;
        
        // Retrieve layer transform animated properties
        let location = get_property(conn, db_id, "location", [0.0, 0.0, 0.0])?;
        let scale = get_property(conn, db_id, "scale", [1.0, 1.0])?;
        let rotation = get_property(conn, db_id, "rotation", 0.0)?;
        let opacity = get_property(conn, db_id, "opacity", 1.0)?;
        
        let transform = LayerTransform { location, scale, rotation, opacity };
        
        root.layers.push(Layer {
            id: layer_id,
            label,
            start_time,
            end_time,
            hidden: hidden_val != 0,
            transform,
            fill_type: str_to_fill_type(&fill_type_str),
            fill_image,
            fill_color: [fill_r, fill_g, fill_b, fill_a],
            gradient,
            blend_mode: str_to_blend_mode(&blend_mode_str),
            media_fill_mode,
            effects,
            size: [size_w, size_h],
            s: shape_primitive,
        });
    }
    
    Ok(root)
}

/// List all projects in the database (brief summaries)
pub fn list_projects(conn: &Connection) -> anyhow::Result<Vec<ProjectBrief>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, width, height, total_time, fps FROM projects ORDER BY id DESC"
    )?;
    
    let rows = stmt.query_map([], |row| {
        let total_time_ms: f32 = row.get(4)?;
        Ok(ProjectBrief {
            id: row.get(0)?,
            title: row.get(1)?,
            width: row.get(2)?,
            height: row.get(3)?,
            duration_secs: total_time_ms / 1000.0,
            fps: row.get(5)?,
        })
    })?;
    
    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

/// Delete a project by ID
#[allow(dead_code)]
pub fn delete_project(conn: &Connection, id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(())
}

/// Parse Alight Motion XML preset from disk and insert it into the DB
pub fn import_preset_xml(conn: &mut Connection, xml_path: &Path) -> anyhow::Result<i64> {
    log::info!("Parsing XML preset: {}", xml_path.display());
    let xml_scene = alight_parser::parse_xml(xml_path)?;
    let project = alight_parser::convert_project(&xml_scene, None)?;
    
    let project_id = insert_project(conn, &project)?;
    log::info!("Preset '{}' successfully imported into DB with ID: {}", 
        project.title.as_deref().unwrap_or("Untitled"), project_id);
        
    Ok(project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_lifecycle() -> anyhow::Result<()> {
        // Create an in-memory connection
        let mut conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        
        // Initialize tables manually or using init_db path-like setup but on in-memory connection
        conn.execute_batch(r#"
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
                fps REAL NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
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
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS audio_tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL,
                track_id INTEGER NOT NULL,
                label TEXT,
                start_time REAL NOT NULL,
                end_time REAL NOT NULL,
                src TEXT,
                in_time REAL NOT NULL,
                out_time REAL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS layers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL,
                layer_id INTEGER NOT NULL,
                label TEXT,
                start_time REAL NOT NULL,
                end_time REAL NOT NULL,
                hidden INTEGER NOT NULL DEFAULT 0,
                fill_type TEXT NOT NULL,
                fill_image TEXT,
                fill_color_r REAL,
                fill_color_g REAL,
                fill_color_b REAL,
                fill_color_a REAL,
                gradient_json TEXT,
                blend_mode TEXT NOT NULL DEFAULT 'Normal',
                media_fill_mode TEXT,
                size_w REAL NOT NULL,
                size_h REAL NOT NULL,
                shape_primitive TEXT,
                effects_json TEXT,
                sort_order INTEGER NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS properties (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                layer_id INTEGER NOT NULL,
                property_name TEXT NOT NULL,
                is_animated INTEGER NOT NULL DEFAULT 0,
                static_value TEXT,
                FOREIGN KEY (layer_id) REFERENCES layers(id) ON DELETE CASCADE,
                UNIQUE(layer_id, property_name)
            );

            CREATE TABLE IF NOT EXISTS keyframes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                property_id INTEGER NOT NULL,
                t REAL NOT NULL,
                value TEXT NOT NULL,
                easing_type TEXT NOT NULL DEFAULT 'Linear',
                easing_params TEXT,
                FOREIGN KEY (property_id) REFERENCES properties(id) ON DELETE CASCADE
            );
        "#)?;

        // Build a dummy Project
        let project = Project {
            title: Some("Test Project".to_string()),
            width: 1920,
            height: 1080,
            export_width: 1920,
            export_height: 1080,
            bg_color: [0.1, 0.2, 0.3, 1.0],
            total_time: 5000.0,
            fps: 30.0,
            media: vec![
                MediaRef {
                    uri: "am-internal:///media1".to_string(),
                    filename: Some("image.png".to_string()),
                    title: Some("Image".to_string()),
                    mime_type: Some("image/png".to_string()),
                    width: Some(1920),
                    height: Some(1080),
                }
            ],
            audio_tracks: vec![
                AudioTrack {
                    id: 101,
                    label: Some("Audio".to_string()),
                    start_time: 0.0,
                    end_time: 5000.0,
                    src: Some("music.mp3".to_string()),
                    in_time: 0.0,
                    out_time: Some(5000.0),
                }
            ],
            layers: vec![
                Layer {
                    id: 1,
                    label: Some("Layer 1".to_string()),
                    start_time: 0.0,
                    end_time: 5000.0,
                    hidden: false,
                    transform: LayerTransform {
                        location: Animated::Static([0.0, 0.0, 0.0]),
                        scale: Animated::Keyframed(vec![
                            Keyframe {
                                t: 0.0,
                                value: [1.0, 1.0],
                                easing: EasingType::Linear,
                            },
                            Keyframe {
                                t: 1.0,
                                value: [2.0, 2.0],
                                easing: EasingType::CubicBezier(0.4, 0.0, 0.2, 1.0),
                            },
                        ]),
                        rotation: Animated::Static(0.0),
                        opacity: Animated::Static(1.0),
                    },
                    fill_type: FillType::Color,
                    fill_image: None,
                    fill_color: [1.0, 0.0, 0.0, 1.0],
                    gradient: None,
                    blend_mode: BlendMode::Normal,
                    media_fill_mode: None,
                    effects: vec![],
                    size: [100.0, 100.0],
                    s: Some(".rect".to_string()),
                }
            ],
        };

        // Test insertion
        let project_id = insert_project(&mut conn, &project)?;
        assert_eq!(project_id, 1);

        // Test list
        let list = list_projects(&conn)?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, Some("Test Project".to_string()));

        // Test get
        let retrieved = get_project(&conn, project_id)?;
        assert_eq!(retrieved.title, project.title);
        assert_eq!(retrieved.width, project.width);
        assert_eq!(retrieved.height, project.height);
        assert_eq!(retrieved.bg_color, project.bg_color);
        assert_eq!(retrieved.media.len(), 1);
        assert_eq!(retrieved.media[0].uri, "am-internal:///media1");
        assert_eq!(retrieved.audio_tracks.len(), 1);
        assert_eq!(retrieved.audio_tracks[0].id, 101);
        assert_eq!(retrieved.layers.len(), 1);
        assert_eq!(retrieved.layers[0].label, Some("Layer 1".to_string()));
        
        // Check property scale keyframes
        if let Animated::Keyframed(kfs) = &retrieved.layers[0].transform.scale {
            assert_eq!(kfs.len(), 2);
            assert_eq!(kfs[0].t, 0.0);
            assert_eq!(kfs[0].value, [1.0, 1.0]);
            assert_eq!(kfs[1].t, 1.0);
            assert_eq!(kfs[1].value, [2.0, 2.0]);
            match kfs[1].easing {
                EasingType::CubicBezier(x1, y1, x2, y2) => {
                    assert!((x1 - 0.4).abs() < 1e-5);
                    assert!((y1 - 0.0).abs() < 1e-5);
                    assert!((x2 - 0.2).abs() < 1e-5);
                    assert!((y2 - 1.0).abs() < 1e-5);
                }
                _ => panic!("Expected CubicBezier easing"),
            }
        } else {
            panic!("Expected scale to be Keyframed");
        }

        // Test deletion
        delete_project(&conn, project_id)?;
        let list_after_del = list_projects(&conn)?;
        assert!(list_after_del.is_empty());

        Ok(())
    }
}
