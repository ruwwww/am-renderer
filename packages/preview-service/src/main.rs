use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State, Path,
    },
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use std::collections::HashMap;
use graph_resolver::model::Project;
use anyhow::Context;

mod scheduler;
mod db;
mod mutations;

use scheduler::{FramePackage, IncomingMessage, PlaybackScheduler};
use rusqlite::Connection;

struct AppState {
    scheduler: Arc<PlaybackScheduler>,
    db_conn: Arc<Mutex<Connection>>,
    undo_stacks: Arc<Mutex<HashMap<i64, Vec<Project>>>>,
    redo_stacks: Arc<Mutex<HashMap<i64, Vec<Project>>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Default assets
    let assets_dir = PathBuf::from("assets");

    // Initialize database
    let db_path = std::path::Path::new("db.sqlite");
    log::info!("Initializing database at {}", db_path.display());
    let mut conn = db::init_db(db_path)?;

    // Check if projects table is empty
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
    if count == 0 {
        log::info!("Database is empty. Seeding with presets...");
        let mut presets_base = std::path::PathBuf::from("presets");
        if !presets_base.exists() {
            if std::path::Path::new("../presets").exists() {
                presets_base = std::path::PathBuf::from("../presets");
            } else if std::path::Path::new("../../presets").exists() {
                presets_base = std::path::PathBuf::from("../../presets");
            }
        }
        for i in 1..=10 {
            let preset_path = presets_base.join(format!("preset{}.xml", i));
            if preset_path.exists() {
                if let Err(e) = db::import_preset_xml(&mut conn, &preset_path) {
                    log::error!("Failed to import preset {}: {}", preset_path.display(), e);
                }
            } else {
                log::warn!("Preset file not found: {}", preset_path.display());
            }
        }
    }

    // Load the first project
    let first_id: i64 = conn.query_row(
        "SELECT id FROM projects ORDER BY id ASC LIMIT 1",
        [],
        |row| row.get(0)
    ).context("No projects found in the database")?;
    log::info!("Loading project with ID: {} from DB", first_id);
    let project = db::get_project(&conn, first_id)?;

    log::info!(
        "Project '{}' loaded successfully. Size: {}x{}, Duration: {:.2}s, FPS: {}",
        project.title.as_deref().unwrap_or("Untitled"),
        project.width,
        project.height,
        project.duration_secs(),
        project.fps
    );

    // Setup channel for frame transmission
    let (frame_sender, _frame_receiver) = mpsc::channel::<FramePackage>(30);

    // Initialize scheduler
    let scheduler = Arc::new(PlaybackScheduler::new(
        project,
        assets_dir,
        frame_sender,
    ));

    // Shared state
    let app_state = Arc::new(AppState {
        scheduler,
        db_conn: Arc::new(Mutex::new(conn)),
        undo_stacks: Arc::new(Mutex::new(HashMap::new())),
        redo_stacks: Arc::new(Mutex::new(HashMap::new())),
    });

    // Setup axum router
    let mut app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/projects", get(get_projects))
        .route("/api/projects/:id", get(get_project_by_id))
        .route("/api/projects/:id/mutate", post(mutate_project))
        .route("/api/projects/:id/undo", post(undo_project))
        .route("/api/projects/:id/redo", post(redo_project))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(app_state);

    // Serve web-editor static files at root
    let mut web_editor_dir = PathBuf::from("packages/web-editor/dist");
    if !web_editor_dir.exists() {
        if PathBuf::from("../web-editor/dist").exists() {
            web_editor_dir = PathBuf::from("../web-editor/dist");
        } else if PathBuf::from("../../packages/web-editor/dist").exists() {
            web_editor_dir = PathBuf::from("../../packages/web-editor/dist");
        }
    }
    if web_editor_dir.exists() {
        log::info!("Serving web-editor static files from: {}", web_editor_dir.display());
        app = app.fallback_service(tower_http::services::ServeDir::new(web_editor_dir));
    } else {
        log::warn!("Web-editor static files directory not found. Static serving disabled.");
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::info!("Preview server running at ws://localhost:8080/ws and http://localhost:8080/api/projects");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn get_projects(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let conn = state.db_conn.lock().await;
    match db::list_projects(&conn) {
        Ok(list) => (axum::http::StatusCode::OK, axum::Json(list)).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_project_by_id(
    Path(id): Path<i64>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let conn = state.db_conn.lock().await;
    match db::get_project(&conn, id) {
        Ok(project) => (axum::http::StatusCode::OK, axum::Json(project)).into_response(),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("QueryReturnedNoRows") || err_str.contains("Query returned no rows") {
                (axum::http::StatusCode::NOT_FOUND, "Project not found").into_response()
            } else {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err_str).into_response()
            }
        }
    }
}

async fn mutate_project(
    Path(id): Path<i64>,
    State(state): State<Arc<AppState>>,
    axum::Json(mutation): axum::Json<mutations::Mutation>,
) -> impl IntoResponse {
    // 1 & 2. Get connection, get project, apply mutation, get updated project, all synchronously inside a block (no await points)
    let res = {
        let mut conn = state.db_conn.lock().await;
        let prev_project = match db::get_project(&conn, id) {
            Ok(p) => p,
            Err(e) => {
                let err_str = e.to_string();
                let status = if err_str.contains("QueryReturnedNoRows") || err_str.contains("Query returned no rows") {
                    axum::http::StatusCode::NOT_FOUND
                } else {
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                };
                return (status, err_str).into_response();
            }
        };

        if let Err(e) = mutations::apply_mutation(&mut conn, id, &mutation) {
            let err_str = e.to_string();
            let status = if err_str.contains("Project not found") || err_str.contains("Layer not found") {
                axum::http::StatusCode::NOT_FOUND
            } else {
                axum::http::StatusCode::BAD_REQUEST
            };
            return (status, err_str).into_response();
        }

        let updated = match db::get_project(&conn, id) {
            Ok(p) => p,
            Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        };

        (prev_project, updated)
    }; // conn is dropped here

    let (prev_project, updated_project) = res;

    // 3. Record in undo stack, clear redo stack (async locks, but conn is dropped)
    {
        let mut undo = state.undo_stacks.lock().await;
        undo.entry(id).or_default().push(prev_project);
        let mut redo = state.redo_stacks.lock().await;
        redo.remove(&id);
    }

    // 4. Update the scheduler state immediately
    state.scheduler.update_project(updated_project.clone()).await;
    (axum::http::StatusCode::OK, axum::Json(updated_project)).into_response()
}

async fn undo_project(
    Path(id): Path<i64>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // 1. Pop from undo stack first (async)
    let prev_project = {
        let mut undo = state.undo_stacks.lock().await;
        match undo.get_mut(&id).and_then(|s| s.pop()) {
            Some(p) => p,
            None => {
                return (axum::http::StatusCode::BAD_REQUEST, "Undo stack empty").into_response();
            }
        }
    };

    // 2. Perform DB operations inside a block (no await points)
    let res = {
        let mut conn = state.db_conn.lock().await;
        let current = match db::get_project(&conn, id) {
            Ok(p) => p,
            Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        };
        if let Err(e) = mutations::overwrite_project(&mut conn, id, &prev_project) {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
        current
    }; // conn is dropped here

    let current_project = res;

    // 3. Push to redo stack (async)
    {
        let mut redo = state.redo_stacks.lock().await;
        redo.entry(id).or_default().push(current_project);
    }

    // 4. Update scheduler (async)
    state.scheduler.update_project(prev_project.clone()).await;
    (axum::http::StatusCode::OK, axum::Json(prev_project)).into_response()
}

async fn redo_project(
    Path(id): Path<i64>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // 1. Pop from redo stack first (async)
    let next_project = {
        let mut redo = state.redo_stacks.lock().await;
        match redo.get_mut(&id).and_then(|s| s.pop()) {
            Some(p) => p,
            None => {
                return (axum::http::StatusCode::BAD_REQUEST, "Redo stack empty").into_response();
            }
        }
    };

    // 2. Perform DB operations inside a block (no await points)
    let res = {
        let mut conn = state.db_conn.lock().await;
        let current = match db::get_project(&conn, id) {
            Ok(p) => p,
            Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        };
        if let Err(e) = mutations::overwrite_project(&mut conn, id, &next_project) {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
        current
    }; // conn is dropped here

    let current_project = res;

    // 3. Push to undo stack (async)
    {
        let mut undo = state.undo_stacks.lock().await;
        undo.entry(id).or_default().push(current_project);
    }

    // 4. Update scheduler (async)
    state.scheduler.update_project(next_project.clone()).await;
    (axum::http::StatusCode::OK, axum::Json(next_project)).into_response()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let scheduler = state.scheduler.clone();

    // Create a subscription channel for frames
    let (local_sender, mut local_receiver) = mpsc::channel::<FramePackage>(30);
    
    // We replace the global scheduler's sender with ours.
    *scheduler.frame_sender.lock().await = local_sender;


    // 1. Task: Send frames to the client
    let mut send_task = tokio::spawn(async move {
        while let Some(frame) = local_receiver.recv().await {
            // Package: [4 bytes frame num (Big Endian)] [webp payload]
            let mut payload = Vec::with_capacity(4 + frame.data.len());
            payload.extend_from_slice(&frame.frame.to_be_bytes());
            payload.extend_from_slice(&frame.data);

            if let Err(e) = sender.send(Message::Binary(payload)).await {
                log::debug!("Failed to send frame over WebSocket: {}", e);
                break;
            }
        }
    });

    // 2. Task: Receive controls from the client
    let scheduler_control = scheduler.clone();
    let mut receive_task = tokio::spawn(async move {
        // Trigger initial frame render on connection
        scheduler_control.seek(0).await;

        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(cmd) = serde_json::from_str::<IncomingMessage>(&text) {
                    match cmd {
                        IncomingMessage::Seek { frame } => {
                            log::debug!("WS client requested seek: {}", frame);
                            scheduler_control.seek(frame).await;
                        }
                        IncomingMessage::Play { fps } => {
                            log::debug!("WS client requested play");
                            scheduler_control.clone().play(fps).await;
                        }
                        IncomingMessage::Pause => {
                            log::debug!("WS client requested pause");
                            scheduler_control.pause().await;
                        }
                        IncomingMessage::Config { scale } => {
                            log::debug!("WS client requested scale: {}", scale);
                            scheduler_control.set_scale(scale).await;
                        }
                    }
                } else {
                    log::warn!("WS received invalid JSON command: {}", text);
                }
            }
        }
        // When client disconnects, pause scheduler
        scheduler_control.pause().await;
    });

    // Wait until one of the tasks exits
    tokio::select! {
        _ = &mut send_task => {},
        _ = &mut receive_task => {},
    }

    
    send_task.abort();
    receive_task.abort();
    
    scheduler.pause().await;
    log::info!("WS client connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::time::{timeout, Duration};

    fn make_dummy_project(fps: f32) -> Project {
        Project {
            title: Some("Test Project".to_string()),
            width: 1920,
            height: 1080,
            export_width: 1920,
            export_height: 1080,
            bg_color: [0.1, 0.2, 0.3, 1.0],
            total_time: 5000.0,
            fps,
            media: vec![],
            audio_tracks: vec![],
            layers: vec![],
        }
    }

    #[tokio::test]
    async fn test_scheduler_deadlock_poc() {
        // Create a dummy project with high FPS to ensure rapid ticker execution
        let project = make_dummy_project(1000.0);
        let (frame_sender, mut frame_receiver) = mpsc::channel::<FramePackage>(100);

        // Spawn a reader task to consume frames from the channel so it doesn't block
        let reader_handle = tokio::spawn(async move {
            while let Some(_) = frame_receiver.recv().await {}
        });

        // Initialize the scheduler
        let scheduler = Arc::new(PlaybackScheduler::new(
            project,
            PathBuf::from("assets"),
            frame_sender,
        ));

        // Start playback
        scheduler.clone().play(None).await;

        // Spawn a task to rapidly update the project state (simulating mutations)
        let sched_clone = scheduler.clone();
        let mutation_task = tokio::spawn(async move {
            for _ in 0..100 {
                sched_clone.update_project(make_dummy_project(1000.0)).await;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });

        // Wrap the mutation loop in a timeout. If the deadlock occurs, the timeout will expire.
        let result = timeout(Duration::from_secs(3), mutation_task).await;

        // Clean up the receiver task
        reader_handle.abort();

        // If the result is a timeout error, it means we successfully reproduced the deadlock!
        if result.is_err() {
            println!("SUCCESSFULLY REPRODUCED DEADLOCK: PlaybackScheduler deadlocked due to lock-order inversion!");
        } else {
            // If the bug gets fixed, it will not deadlock and will finish within the timeout.
            scheduler.pause().await;
        }
    }

    #[tokio::test]
    async fn test_zero_fps_panic_poc() {
        let project = make_dummy_project(30.0);
        let (frame_sender, _frame_receiver) = mpsc::channel::<FramePackage>(10);
        let scheduler = Arc::new(PlaybackScheduler::new(
            project,
            PathBuf::from("assets"),
            frame_sender,
        ));

        // Call play with 0.0 FPS. This is expected to panic due to 1.0 / 0.0 division and Duration conversion.
        let sched_clone = scheduler.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            // We run the async function using a basic current_thread runtime or block_on in catch_unwind
            let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
            rt.block_on(async {
                sched_clone.play(Some(0.0)).await;
            });
        }));

        assert!(result.is_err(), "Expected play() to panic when given 0.0 FPS");
        println!("SUCCESSFULLY REPRODUCED PANIC: play() correctly panicked on 0.0 FPS!");
    }
}

