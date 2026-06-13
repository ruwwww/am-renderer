use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

mod scheduler;
use scheduler::{FramePackage, IncomingMessage, PlaybackScheduler};

struct AppState {
    scheduler: Arc<PlaybackScheduler>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Default project path & assets (we can make this configurable via CLI parameters later)
    let xml_path = PathBuf::from("presets/preset10.xml");
    let assets_dir = PathBuf::from("assets");

    log::info!("Loading project from XML: {}", xml_path.display());
    let xml_scene = alight_parser::parse_xml(&xml_path)?;
    let project = alight_parser::convert_project(&xml_scene, None)?;

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
    });

    // Setup axum router
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::info!("Preview server running at ws://localhost:8080/ws");
    axum::serve(listener, app).await?;

    Ok(())
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
    
    // We replace the global scheduler's sender with ours, or we multiplex it.
    // To keep it simple for a single connection, we update the scheduler's sender field.
    // In a multi-client scenario we would multiplex, but for editing, it's 1 active web editor.
    // Let's directly swap/redirect frames:
    let _sched_state = scheduler.state.lock().await;
    // Update the frame sender
    let mut scheduler_writer = scheduler.clone_scheduler();
    scheduler_writer.frame_sender = local_sender;
    drop(sched_state);

    // 1. Task: Send frames to the client
    let send_task = tokio::spawn(async move {
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
    let scheduler_control = scheduler.clone_scheduler();
    let receive_task = tokio::spawn(async move {
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
                            scheduler_control.play(fps).await;
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
        _ = send_task => {},
        _ = receive_task => {},
    }
    
    log::info!("WS client connection closed");
}
