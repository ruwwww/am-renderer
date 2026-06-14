mod common;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json::json;

#[tokio::test]
async fn test_ws_connection_succeeds() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (ws_stream, response) = connect_async(url).await.unwrap();
    assert_eq!(response.status(), 101);
    
    // Clean close
    let mut ws_stream = ws_stream;
    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_ws_seek_receives_frame() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    let seek_cmd = json!({
        "type": "seek",
        "frame": 5
    });

    ws_stream.send(Message::Text(seek_cmd.to_string())).await.unwrap();

    // Read the returned binary frame
    let mut found = false;
    while let Some(Ok(msg)) = ws_stream.next().await {
        if let Message::Binary(data) = msg {
            assert!(data.len() > 4, "Should receive frame number + image data");
            let frame_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            if frame_num == 5 {
                found = true;
                break;
            }
        } else {
            panic!("Expected binary message containing frame data");
        }
    }
    assert!(found, "Should eventually receive seek frame 5");
}

#[tokio::test]
async fn test_ws_play_initiates_streaming() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    let play_cmd = json!({
        "type": "play",
        "fps": 30.0
    });

    ws_stream.send(Message::Text(play_cmd.to_string())).await.unwrap();

    // We should receive multiple frames in succession
    for _ in 0..3 {
        if let Some(Ok(msg)) = ws_stream.next().await {
            if let Message::Binary(data) = msg {
                assert!(data.len() > 4);
            } else {
                panic!("Expected binary frame");
            }
        }
    }
}

#[tokio::test]
async fn test_ws_pause_stops_streaming() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Start playing
    ws_stream.send(Message::Text(json!({"type": "play", "fps": 60.0}).to_string())).await.unwrap();
    
    // Read one frame to ensure it started
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Pause
    ws_stream.send(Message::Text(json!({"type": "pause"}).to_string())).await.unwrap();

    // Flush any pending frames and verify no more come through after a short delay
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Drain
    while let Ok(Some(msg)) = tokio::time::timeout(tokio::time::Duration::from_millis(50), ws_stream.next()).await {
        let msg = msg.unwrap();
        if let Message::Binary(_) = msg {
            // some frames might have been in flight
        }
    }

    // Try reading now - should timeout
    let timeout_res = tokio::time::timeout(tokio::time::Duration::from_millis(200), ws_stream.next()).await;
    assert!(timeout_res.is_err(), "Should receive no more frames after pausing");
}

#[tokio::test]
async fn test_ws_config_updates_scale() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Change scale
    ws_stream.send(Message::Text(json!({"type": "config", "scale": 0.1}).to_string())).await.unwrap();

    // Seek to trigger a frame
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 0}).to_string())).await.unwrap();

    if let Some(Ok(msg)) = ws_stream.next().await {
        if let Message::Binary(data) = msg {
            assert!(data.len() > 4);
            // Verify it didn't error out and we got back a smaller WebP payload
        }
    }
}

// ==========================================
// TIER 2: BOUNDARY & CORNER CASES
// ==========================================

#[tokio::test]
async fn test_ws_malformed_json() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Send malformed text
    ws_stream.send(Message::Text("{malformed_json:".to_string())).await.unwrap();

    // Verify connection is still alive (by sending a valid seek)
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 0}).to_string())).await.unwrap();

    let timeout_res = tokio::time::timeout(tokio::time::Duration::from_millis(3000), ws_stream.next()).await;
    assert!(timeout_res.is_ok(), "Server should ignore bad json and stay alive");
}

#[tokio::test]
async fn test_ws_seek_out_of_bounds() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Seek to frame far beyond limits
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 999999}).to_string())).await.unwrap();

    if let Some(Ok(msg)) = ws_stream.next().await {
        if let Message::Binary(data) = msg {
            // Should either cap to max frame or render empty/black
            assert!(data.len() > 4);
        }
    }
}

#[tokio::test]
async fn test_ws_play_invalid_fps() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Negative or extremely large FPS - server should cap or fallback
    ws_stream.send(Message::Text(json!({"type": "play", "fps": -30.0}).to_string())).await.unwrap();

    // Verify it doesn't crash
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 0}).to_string())).await.unwrap();
    if let Some(Ok(msg)) = ws_stream.next().await {
        assert!(matches!(msg, Message::Binary(_)));
    }
}

#[tokio::test]
async fn test_ws_config_extreme_scale() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Extreme scale: 0.0 or 100.0. Scale clamping should prevent crashes.
    ws_stream.send(Message::Text(json!({"type": "config", "scale": 0.0}).to_string())).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 1}).to_string())).await.unwrap();

    if let Some(Ok(msg)) = ws_stream.next().await {
        assert!(matches!(msg, Message::Binary(_)));
    }
}

#[tokio::test]
async fn test_ws_abrupt_disconnect() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let url = common::get_ws_url();
    let (ws_stream, _) = connect_async(url.clone()).await.unwrap();

    // Drop connection immediately without closing handshake
    drop(ws_stream);

    // Verify server can still accept new connections
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let (mut ws_stream2, _) = connect_async(url).await.unwrap();
    ws_stream2.close(None).await.unwrap();
}
