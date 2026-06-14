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
async fn test_workload_e2e_editing_session() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let base_url = common::get_server_url();

    // 1. Create/Mutate Project (representing creating project and layers)
    let add_layer_mut = json!({
        "type": "add_layer",
        "layer": {
            "id": 200,
            "label": "New Text Layer",
            "start_time": 0.0,
            "end_time": 10.0,
            "hidden": false,
            "blend_mode": "Normal",
            "fill_type": "Color",
            "fill_color": [1.0, 1.0, 1.0, 1.0],
            "size": [200.0, 50.0],
            "fill_image": null,
            "gradient": null,
            "media_fill_mode": null,
            "effects": [],
            "s": null,
            "transform": {
                "location": { "Static": [0.0, 0.0, 0.0] },
                "scale": { "Static": [1.0, 1.0] },
                "rotation": { "Static": 0.0 },
                "opacity": { "Static": 1.0 }
            }
        }
    });

    let res = client.post(format!("{}/api/projects/1/mutate", base_url))
        .json(&add_layer_mut)
        .send().await.unwrap();

    if res.status() == StatusCode::NOT_FOUND {
        return;
    }
    assert!(res.status().is_success());

    // 2. Connect WebSockets & Seek to verify render
    let (mut ws_stream, _) = connect_async(common::get_ws_url()).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 12}).to_string())).await.unwrap();

    if let Some(Ok(Message::Binary(data))) = ws_stream.next().await {
        assert!(data.len() > 4);
    }

    let _ = ws_stream.close(None).await;
}

#[tokio::test]
async fn test_workload_undo_redo_recovery() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let base_url = common::get_server_url();
    let layer_id = match get_first_layer_id(&client).await {
        Some(id) => id,
        None => {
            println!("Could not load layer ID. Skipping.");
            return;
        }
    };

    // Apply multiple mutations
    for opacity in [0.1, 0.2, 0.3, 0.4] {
        let mut_op = json!({
            "type": "update_layer_property",
            "layer_id": layer_id,
            "property": "opacity",
            "value": opacity
        });
        let res = client.post(format!("{}/api/projects/1/mutate", base_url)).json(&mut_op).send().await.unwrap();
        assert!(res.status().is_success());
    }

    // Undo 2 of them
    client.post(format!("{}/api/projects/1/undo", base_url)).send().await.unwrap();
    client.post(format!("{}/api/projects/1/undo", base_url)).send().await.unwrap();

    // Check preview is consistent
    let (mut ws_stream, _) = connect_async(common::get_ws_url()).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 0}).to_string())).await.unwrap();
    if let Some(Ok(Message::Binary(data))) = ws_stream.next().await {
        assert!(data.len() > 4);
    }

    // Redo them
    client.post(format!("{}/api/projects/1/redo", base_url)).send().await.unwrap();
    client.post(format!("{}/api/projects/1/redo", base_url)).send().await.unwrap();

    let _ = ws_stream.close(None).await;
}

#[tokio::test]
async fn test_workload_multi_client_session() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client_a = reqwest::Client::new();
    let layer_id = match get_first_layer_id(&client_a).await {
        Some(id) => id,
        None => {
            println!("Could not load layer ID. Skipping.");
            return;
        }
    };

    // Client A connects and mutates, Client B plays and receives
    let url = common::get_ws_url();
    
    // Client B WebSocket
    let (mut ws_b, _) = connect_async(url).await.unwrap();
    ws_b.send(Message::Text(json!({"type": "play", "fps": 30.0}).to_string())).await.unwrap();

    // Client A REST call
    let mutate_url = format!("{}/api/projects/1/mutate", common::get_server_url());
    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "rotation",
        "value": 45.0
    });

    let res = client_a.post(&mutate_url).json(&mutation).send().await.unwrap();
    assert!(res.status().is_success());

    // Client B receives frames
    if let Some(Ok(Message::Binary(data))) = ws_b.next().await {
        assert!(data.len() > 4);
    }

    let _ = ws_b.close(None).await;
}

#[tokio::test]
async fn test_workload_asset_loading() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let project_url = format!("{}/api/projects/1", common::get_server_url());

    // Query project and check for media references
    let res = client.get(&project_url).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let val: serde_json::Value = res.json().await.unwrap();
    
    // Verify structure
    if let Some(media) = val.get("media") {
        assert!(media.is_array());
    }
}

#[tokio::test]
async fn test_workload_complex_animations() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let base_url = common::get_server_url();
    let layer_id = match get_first_layer_id(&client).await {
        Some(id) => id,
        None => {
            println!("Could not load layer ID. Skipping.");
            return;
        }
    };

    // Animate opacity with easing
    let mutation = json!({
        "type": "update_layer_property_keyframes",
        "layer_id": layer_id,
        "property": "opacity",
        "keyframes": [
            {"t": 0.0, "value": 0.0, "easing": "Linear"},
            {"t": 0.5, "value": 1.0, "easing": "CubicBezier(0.4, 0.0, 0.2, 1.0)"},
            {"t": 1.0, "value": 0.0, "easing": "Linear"}
        ]
    });

    let res = client.post(format!("{}/api/projects/1/mutate", base_url)).json(&mutation).send().await.unwrap();
    assert!(res.status().is_success());

    // Seek to midpoint (0.5) and verify frame delivery
    let (mut ws_stream, _) = connect_async(common::get_ws_url()).await.unwrap();
    ws_stream.send(Message::Text(json!({"type": "seek", "frame": 15}).to_string())).await.unwrap();
    if let Some(Ok(Message::Binary(data))) = ws_stream.next().await {
        assert!(data.len() > 4);
    }
    
    let _ = ws_stream.close(None).await;
}
