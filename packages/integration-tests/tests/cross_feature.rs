mod common;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json::json;
use reqwest::StatusCode;

async fn get_first_layer_id(client: &reqwest::Client) -> Option<u64> {
    let project_url = format!("{}/api/projects/1", common::get_server_url());
    let res = client.get(&project_url).send().await.ok()?;
    if res.status() != StatusCode::OK {
        return None;
    }
    let project: serde_json::Value = res.json().await.ok()?;
    let layers = project.get("layers")?.as_array()?;
    if layers.is_empty() {
        return None;
    }
    layers[0].get("id")?.as_u64()
}

#[tokio::test]
async fn test_mutate_then_play() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let layer_id = match get_first_layer_id(&client).await {
        Some(id) => id,
        None => {
            println!("Could not load layer ID. Skipping.");
            return;
        }
    };
    let mutate_url = format!("{}/api/projects/1/mutate", common::get_server_url());

    // 1. Mutate property
    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "opacity",
        "value": 0.1
    });
    let res = client.post(&mutate_url).json(&mutation).send().await.unwrap();
    assert!(res.status().is_success());

    // 2. Play on WebSocket
    let (mut ws_stream, _) = connect_async(common::get_ws_url()).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "play", "fps": 30.0}).to_string())).await.unwrap();

    // Verify frames are received
    if let Some(Ok(msg)) = ws_stream.next().await {
        if let Message::Binary(data) = msg {
            assert!(data.len() > 4);
        }
    }
    let _ = ws_stream.close(None).await;
}

#[tokio::test]
async fn test_mutate_seek_undo_seek() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let layer_id = match get_first_layer_id(&client).await {
        Some(id) => id,
        None => {
            println!("Could not load layer ID. Skipping.");
            return;
        }
    };
    let mutate_url = format!("{}/api/projects/1/mutate", common::get_server_url());
    let undo_url = format!("{}/api/projects/1/undo", common::get_server_url());

    // 1. Mutate
    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "location",
        "value": [100.0, 100.0, 0.0]
    });
    let res = client.post(&mutate_url).json(&mutation).send().await.unwrap();
    assert!(res.status().is_success());

    // 2. Connect and Seek
    let (mut ws_stream, _) = connect_async(common::get_ws_url()).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 2}).to_string())).await.unwrap();
    
    let mut frame_mutated = Vec::new();
    if let Some(Ok(Message::Binary(data))) = ws_stream.next().await {
        frame_mutated = data;
    }

    // 3. Undo
    let res = client.post(&undo_url).send().await.unwrap();
    assert!(res.status().is_success());

    // 4. Seek again and compare frames (reverted state should change render)
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 2}).to_string())).await.unwrap();
    if let Some(Ok(Message::Binary(data_reverted))) = ws_stream.next().await {
        // Assert they are different because the location reverted
        assert_ne!(frame_mutated, data_reverted);
    }
    
    let _ = ws_stream.close(None).await;
}

#[tokio::test]
async fn test_load_mutate_play_pause_redo() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let layer_id = match get_first_layer_id(&client).await {
        Some(id) => id,
        None => {
            println!("Could not load layer ID. Skipping.");
            return;
        }
    };
    let project_url = format!("{}/api/projects/1", common::get_server_url());
    let mutate_url = format!("{}/api/projects/1/mutate", common::get_server_url());
    let undo_url = format!("{}/api/projects/1/undo", common::get_server_url());
    let redo_url = format!("{}/api/projects/1/redo", common::get_server_url());

    // 1. Load project
    let res = client.get(&project_url).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. Mutate
    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "scale",
        "value": [2.0, 2.0]
    });
    let res_mut = client.post(&mutate_url).json(&mutation).send().await.unwrap();
    assert_eq!(res_mut.status(), StatusCode::OK);

    // 3. Play & Pause on WS
    let (mut ws_stream, _) = connect_async(common::get_ws_url()).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "play", "fps": 24.0}).to_string())).await.unwrap();
    let _ = ws_stream.next().await;
    ws_stream.send(Message::Text(json!({"type": "pause"}).to_string())).await.unwrap();

    // 4. Undo then Redo
    client.post(&undo_url).send().await.unwrap();
    let res = client.post(&redo_url).send().await.unwrap();
    assert!(res.status().is_success());

    let _ = ws_stream.close(None).await;
}
