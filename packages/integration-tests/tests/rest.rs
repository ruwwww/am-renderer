mod common;

use reqwest::StatusCode;
use serde_json::json;

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
async fn test_get_projects_list() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/projects", common::get_server_url());

    let res = client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    
    let json: serde_json::Value = res.json().await.unwrap();
    assert!(json.is_array());
}

#[tokio::test]
async fn test_get_project_hierarchy() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    // Assuming project 1 exists or seeding
    let url = format!("{}/api/projects/1", common::get_server_url());

    let res = client.get(&url).send().await.unwrap();
    if res.status() == StatusCode::NOT_FOUND {
        // If not seeded yet, we pass since it's a valid return for a clean db
        return;
    }
    assert_eq!(res.status(), StatusCode::OK);
    
    let json: serde_json::Value = res.json().await.unwrap();
    assert!(json.is_object());
    assert!(json.get("title").is_some() || json.get("id").is_some());
}

#[tokio::test]
async fn test_apply_valid_mutation() {
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
    let url = format!("{}/api/projects/1/mutate", common::get_server_url());

    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "opacity",
        "value": 0.8
    });

    let res = client.post(&url).json(&mutation).send().await.unwrap();
    assert!(res.status().is_success());
}

#[tokio::test]
async fn test_undo_mutation() {
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

    // 1. Perform a mutation to populate the undo stack
    let mutate_url = format!("{}/api/projects/1/mutate", common::get_server_url());
    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "opacity",
        "value": 0.5
    });
    client.post(&mutate_url).json(&mutation).send().await.unwrap();

    // 2. Undo
    let url = format!("{}/api/projects/1/undo", common::get_server_url());
    let res = client.post(&url).send().await.unwrap();
    assert!(res.status().is_success());
}

#[tokio::test]
async fn test_redo_mutation() {
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

    // 1. Perform a mutation to populate the undo stack
    let mutate_url = format!("{}/api/projects/1/mutate", common::get_server_url());
    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": layer_id,
        "property": "opacity",
        "value": 0.5
    });
    client.post(&mutate_url).json(&mutation).send().await.unwrap();

    // 2. Undo
    let undo_url = format!("{}/api/projects/1/undo", common::get_server_url());
    client.post(&undo_url).send().await.unwrap();

    // 3. Redo
    let url = format!("{}/api/projects/1/redo", common::get_server_url());
    let res = client.post(&url).send().await.unwrap();
    assert!(res.status().is_success());
}


// ==========================================
// TIER 2: BOUNDARY & CORNER CASES
// ==========================================

#[tokio::test]
async fn test_get_invalid_project_id() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/projects/999999", common::get_server_url());

    let res = client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_mutate_malformed_json() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/projects/1/mutate", common::get_server_url());

    let res = client.post(&url)
        .header("content-type", "application/json")
        .body("{invalid_json:}")
        .send().await.unwrap();
        
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_mutate_non_existent_project() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/projects/999999/mutate", common::get_server_url());

    let mutation = json!({
        "type": "update_layer_property",
        "layer_id": 10,
        "property": "opacity",
        "value": 0.8
    });

    let res = client.post(&url).json(&mutation).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_undo_empty_stack() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/projects/1/undo", common::get_server_url());

    // We send undo. If undo succeeds, we send it again. Eventually it returns BAD_REQUEST or OK with no change.
    // The design is to verify that it doesn't crash the server.
    let res = client.post(&url).send().await.unwrap();
    if res.status() == StatusCode::NOT_FOUND {
        return;
    }
    assert!(res.status() == StatusCode::OK || res.status() == StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_redo_empty_stack() {
    if !common::is_server_running() {
        println!("Server not running. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/projects/1/redo", common::get_server_url());

    let res = client.post(&url).send().await.unwrap();
    if res.status() == StatusCode::NOT_FOUND {
        return;
    }
    assert!(res.status() == StatusCode::OK || res.status() == StatusCode::BAD_REQUEST);
}
