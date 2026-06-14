use rusqlite::{params, Connection};
use graph_resolver::model::{Project, Layer, EasingType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::db::{blend_mode_to_str, fill_type_to_str, insert_property};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Mutation {
    UpdateLayerProperty {
        layer_id: u64,
        property: String,
        value: Value,
    },
    UpdateLayerPropertyKeyframes {
        layer_id: u64,
        property: String,
        keyframes: Vec<KeyframeInput>,
    },
    AddLayer {
        layer: Layer,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyframeInput {
    pub t: f32,
    pub value: Value,
    pub easing: String,
}

pub fn parse_easing_string(s: &str) -> EasingType {
    let s = s.trim();
    if s == "Linear" {
        EasingType::Linear
    } else if s.starts_with("CubicBezier(") && s.ends_with(')') {
        let inner = &s["CubicBezier(".len()..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 4 {
            let x1 = parts[0].parse::<f32>().unwrap_or(0.0);
            let y1 = parts[1].parse::<f32>().unwrap_or(0.0);
            let x2 = parts[2].parse::<f32>().unwrap_or(0.0);
            let y2 = parts[3].parse::<f32>().unwrap_or(1.0);
            EasingType::CubicBezier(x1, y1, x2, y2)
        } else {
            EasingType::Linear
        }
    } else {
        EasingType::Linear
    }
}

/// Helper function to overwrite a project in the database with a snapshot
pub fn overwrite_project(conn: &mut Connection, project_id: i64, project: &Project) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    
    // Delete existing project details (cascading deletes will handle media_refs, audio_tracks, layers, properties, keyframes)
    tx.execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
    
    // Insert with the exact same project ID
    tx.execute(
        "INSERT INTO projects (id, title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            project_id,
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
    Ok(())
}

/// Apply a mutation to the project in the database.
pub fn apply_mutation(conn: &mut Connection, project_id: i64, mutation: &Mutation) -> anyhow::Result<()> {
    // Verify project exists
    let project_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        params![project_id],
        |row| row.get(0),
    )?;
    if !project_exists {
        return Err(anyhow::anyhow!("Project not found"));
    }

    match mutation {
        Mutation::UpdateLayerProperty { layer_id, property, value } => {
            // Find layer DB ID
            let layer_db_id: Option<i64> = conn.query_row(
                "SELECT id FROM layers WHERE project_id = ?1 AND layer_id = ?2",
                params![project_id, layer_id],
                |row| row.get(0),
            ).ok();

            let layer_db_id = match layer_db_id {
                Some(id) => id,
                None => return Err(anyhow::anyhow!("Layer not found")),
            };

            let tx = conn.transaction()?;

            if property == "start_time" {
                let val_f32 = value.as_f64().ok_or_else(|| anyhow::anyhow!("Invalid start_time"))? as f32;
                tx.execute(
                    "UPDATE layers SET start_time = ?1 WHERE id = ?2",
                    params![val_f32, layer_db_id],
                )?;
            } else if property == "end_time" {
                let val_f32 = value.as_f64().ok_or_else(|| anyhow::anyhow!("Invalid end_time"))? as f32;
                tx.execute(
                    "UPDATE layers SET end_time = ?1 WHERE id = ?2",
                    params![val_f32, layer_db_id],
                )?;
            } else {
                // Parse property value based on property name to ensure validity
                let val_str = match property.as_str() {
                    "location" => {
                        let val: [f32; 3] = serde_json::from_value(value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    "scale" => {
                        let val: [f32; 2] = serde_json::from_value(value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    "rotation" => {
                        let val: f32 = serde_json::from_value(value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    "opacity" => {
                        let val: f32 = serde_json::from_value(value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    _ => return Err(anyhow::anyhow!("Unknown property: {}", property)),
                };

                // Get property row ID in db
                let property_id: i64 = tx.query_row(
                    "SELECT id FROM properties WHERE layer_id = ?1 AND property_name = ?2",
                    params![layer_db_id, property],
                    |row| row.get(0),
                )?;

                // Delete keyframes if any
                tx.execute("DELETE FROM keyframes WHERE property_id = ?1", params![property_id])?;

                // Update property to static value
                tx.execute(
                    "UPDATE properties SET is_animated = 0, static_value = ?1 WHERE id = ?2",
                    params![val_str, property_id],
                )?;
            }

            tx.commit()?;
        }

        Mutation::UpdateLayerPropertyKeyframes { layer_id, property, keyframes } => {
            // Find layer DB ID
            let layer_db_id: Option<i64> = conn.query_row(
                "SELECT id FROM layers WHERE project_id = ?1 AND layer_id = ?2",
                params![project_id, layer_id],
                |row| row.get(0),
            ).ok();

            let layer_db_id = match layer_db_id {
                Some(id) => id,
                None => return Err(anyhow::anyhow!("Layer not found")),
            };

            let tx = conn.transaction()?;

            // Get property row ID in db
            let property_id: i64 = tx.query_row(
                "SELECT id FROM properties WHERE layer_id = ?1 AND property_name = ?2",
                params![layer_db_id, property],
                |row| row.get(0),
            )?;

            // Delete keyframes if any
            tx.execute("DELETE FROM keyframes WHERE property_id = ?1", params![property_id])?;

            // Insert new keyframes
            for kf in keyframes {
                // Validate value based on property name
                let val_str = match property.as_str() {
                    "location" => {
                        let val: [f32; 3] = serde_json::from_value(kf.value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    "scale" => {
                        let val: [f32; 2] = serde_json::from_value(kf.value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    "rotation" => {
                        let val: f32 = serde_json::from_value(kf.value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    "opacity" => {
                        let val: f32 = serde_json::from_value(kf.value.clone())?;
                        serde_json::to_string(&val)?
                    }
                    _ => return Err(anyhow::anyhow!("Unknown property: {}", property)),
                };

                let easing_type = parse_easing_string(&kf.easing);
                let (easing_type_str, easing_params) = match easing_type {
                    EasingType::Linear => ("Linear".to_string(), None),
                    EasingType::CubicBezier(x1, y1, x2, y2) => {
                        ("CubicBezier".to_string(), Some(serde_json::to_string(&[x1, y1, x2, y2])?))
                    }
                };

                tx.execute(
                    "INSERT INTO keyframes (property_id, t, value, easing_type, easing_params)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![property_id, kf.t, val_str, easing_type_str, easing_params],
                )?;
            }

            // Update property to animated/keyframed
            tx.execute(
                "UPDATE properties SET is_animated = 1, static_value = NULL WHERE id = ?1",
                params![property_id],
            )?;

            tx.commit()?;
        }

        Mutation::AddLayer { layer } => {
            let tx = conn.transaction()?;

            // Find current max sort_order
            let max_sort_order: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM layers WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )?;
            let new_sort_order = max_sort_order + 1;

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
                    new_sort_order
                ],
            )?;
            let layer_db_id = tx.last_insert_rowid();
            
            // Save animated transform properties
            insert_property(&tx, layer_db_id, "location", &layer.transform.location)?;
            insert_property(&tx, layer_db_id, "scale", &layer.transform.scale)?;
            insert_property(&tx, layer_db_id, "rotation", &layer.transform.rotation)?;
            insert_property(&tx, layer_db_id, "opacity", &layer.transform.opacity)?;

            tx.commit()?;
        }
    }

    Ok(())
}
