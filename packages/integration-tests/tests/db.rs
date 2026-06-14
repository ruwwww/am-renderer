mod common;

use rusqlite::{params, Connection};

// Helpers for testing
fn init_test_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
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

         CREATE TABLE IF NOT EXISTS layers (
             id INTEGER PRIMARY KEY,
             project_id INTEGER NOT NULL,
             label TEXT,
             start_time REAL NOT NULL,
             end_time REAL NOT NULL,
             hidden INTEGER NOT NULL,
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
             sort_order INTEGER NOT NULL DEFAULT 0,
             FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS properties (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             layer_id INTEGER,
             effect_id INTEGER,
             name TEXT NOT NULL,
             is_static INTEGER NOT NULL,
             static_val TEXT,
             FOREIGN KEY(layer_id) REFERENCES layers(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS keyframes (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             property_id INTEGER NOT NULL,
             t REAL NOT NULL,
             value TEXT NOT NULL,
             easing TEXT,
             FOREIGN KEY(property_id) REFERENCES properties(id) ON DELETE CASCADE
         );"
    ).unwrap();
}

// ==========================================
// TIER 1: FEATURE COVERAGE
// ==========================================

#[test]
fn test_db_init_and_schema_validation() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    // Verify tables exist
    let tables = vec!["projects", "media_refs", "layers", "properties", "keyframes"];
    for table in tables {
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "Table {} does not exist", table);
    }
}

#[test]
fn test_insert_and_query_project() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    conn.execute(
        "INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params!["Test Project", 1920, 1080, 1920, 1080, 0.1, 0.1, 0.1, 1.0, 5.0, 30.0],
    ).unwrap();

    let id = conn.last_insert_rowid();
    assert!(id > 0);

    let (title, width, height): (String, i32, i32) = conn.query_row(
        "SELECT title, width, height FROM projects WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();

    assert_eq!(title, "Test Project");
    assert_eq!(width, 1920);
    assert_eq!(height, 1080);
}

#[test]
fn test_insert_and_query_layers() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    // Insert Project
    conn.execute(
        "INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params!["Project for Layers", 1280, 720, 1280, 720, 0.0, 0.0, 0.0, 1.0, 10.0, 60.0],
    ).unwrap();
    let project_id = conn.last_insert_rowid();

    // Insert Layer
    conn.execute(
        "INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, fill_image, fill_color_r, fill_color_g, fill_color_b, fill_color_a, size_w, size_h, s, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![101, project_id, "Layer 1", 0.0, 5.0, 0, "Normal", "Color", None::<String>, 1.0, 0.0, 0.0, 1.0, 100.0, 100.0, "rect", 0],
    ).unwrap();

    let label: String = conn.query_row(
        "SELECT label FROM layers WHERE id = 101",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(label, "Layer 1");
}

#[test]
fn test_insert_and_query_properties() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    // Seed project & layer
    conn.execute("INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps) VALUES ('P', 1, 1, 1, 1, 0, 0, 0, 1, 1, 30)", []).unwrap();
    let project_id = conn.last_insert_rowid();
    conn.execute("INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, size_w, size_h, sort_order) VALUES (1, ?1, 'L', 0, 1, 0, 'Normal', 'Color', 10, 10, 0)", params![project_id]).unwrap();

    // Insert static property
    conn.execute(
        "INSERT INTO properties (layer_id, name, is_static, static_val) VALUES (?1, ?2, ?3, ?4)",
        params![1, "opacity", 1, "0.5"],
    ).unwrap();

    let val: String = conn.query_row(
        "SELECT static_val FROM properties WHERE layer_id = 1 AND name = 'opacity'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(val, "0.5");
}

#[test]
fn test_insert_and_query_keyframes() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    // Seed project, layer, property
    conn.execute("INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps) VALUES ('P', 1, 1, 1, 1, 0, 0, 0, 1, 1, 30)", []).unwrap();
    let project_id = conn.last_insert_rowid();
    conn.execute("INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, size_w, size_h, sort_order) VALUES (1, ?1, 'L', 0, 1, 0, 'Normal', 'Color', 10, 10, 0)", params![project_id]).unwrap();
    conn.execute("INSERT INTO properties (layer_id, name, is_static, static_val) VALUES (1, 'location', 0, NULL)", []).unwrap();
    let prop_id = conn.last_insert_rowid();

    // Insert keyframes
    conn.execute("INSERT INTO keyframes (property_id, t, value, easing) VALUES (?1, 0.0, '[0.0, 0.0, 0.0]', 'Linear')", params![prop_id]).unwrap();
    conn.execute("INSERT INTO keyframes (property_id, t, value, easing) VALUES (?1, 1.0, '[100.0, 0.0, 0.0]', 'Linear')", params![prop_id]).unwrap();

    let count: i64 = conn.query_row(
        "SELECT count(*) FROM keyframes WHERE property_id = ?1",
        params![prop_id],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 2);
}

// ==========================================
// TIER 2: BOUNDARY & CORNER CASES
// ==========================================

#[test]
fn test_boundary_fps_and_dimensions() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    // Extremely small or large dimensions/fps
    let res = conn.execute(
        "INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params!["Extreme Bounds", 1, 1, 32768, 32768, 0.0, 0.0, 0.0, 0.0, 10000.0, 0.001],
    );
    assert!(res.is_ok());
}

#[test]
fn test_foreign_key_violation_layer() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    // Inserting layer that references a non-existent project_id
    let res = conn.execute(
        "INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, size_w, size_h, sort_order)
         VALUES (201, 9999, 'Orphan Layer', 0.0, 1.0, 0, 'Normal', 'Color', 10.0, 10.0, 0)",
        [],
    );
    assert!(res.is_err(), "Should fail foreign key check because project 9999 does not exist");
}

#[test]
fn test_duplicate_primary_keys() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    conn.execute("INSERT INTO projects (id, title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps) VALUES (1, 'P1', 1, 1, 1, 1, 0, 0, 0, 1, 1, 30)", []).unwrap();
    
    // Inserting duplicate project ID
    let res = conn.execute("INSERT INTO projects (id, title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps) VALUES (1, 'P2', 1, 1, 1, 1, 0, 0, 0, 1, 1, 30)", []);
    assert!(res.is_err(), "Duplicate primary key should fail");
}

#[test]
fn test_transaction_rollback_on_error() {
    let mut conn = common::create_test_db();
    init_test_schema(&conn);

    let tx = conn.transaction();
    assert!(tx.is_ok());
    let tx = tx.unwrap();

    tx.execute("INSERT INTO projects (title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps) VALUES ('Tx Project', 1, 1, 1, 1, 0, 0, 0, 1, 1, 30)", []).unwrap();

    // Trigger an error to force rollback
    let bad_exec = tx.execute("INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, size_w, size_h, sort_order) VALUES (1, 9999, 'Bad Layer', 0.0, 1.0, 0, 'Normal', 'Color', 10.0, 10.0, 0)", []);
    assert!(bad_exec.is_err());
    
    drop(tx); // Rolled back automatically

    let count: i64 = conn.query_row("SELECT count(*) FROM projects", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0, "Transaction should roll back all inserts");
}

#[test]
fn test_project_cascade_delete() {
    let conn = common::create_test_db();
    init_test_schema(&conn);

    conn.execute("INSERT INTO projects (id, title, width, height, export_width, export_height, bg_color_r, bg_color_g, bg_color_b, bg_color_a, total_time, fps) VALUES (1, 'P1', 1, 1, 1, 1, 0, 0, 0, 1, 1, 30)", []).unwrap();
    conn.execute("INSERT INTO layers (id, project_id, label, start_time, end_time, hidden, blend_mode, fill_type, size_w, size_h, sort_order) VALUES (10, 1, 'L1', 0, 1, 0, 'Normal', 'Color', 10, 10, 0)", []).unwrap();

    // Verify layer exists
    let count: i64 = conn.query_row("SELECT count(*) FROM layers WHERE project_id = 1", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);

    // Delete project
    conn.execute("DELETE FROM projects WHERE id = 1", []).unwrap();

    // Verify layer is cascade deleted
    let count: i64 = conn.query_row("SELECT count(*) FROM layers WHERE project_id = 1", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0, "Layers should be cascade deleted");
}
