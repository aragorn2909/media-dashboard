use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::api::ServiceStatus;

#[derive(Serialize)]
struct RpcRequest {
    method: String,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TorrentInfo {
    #[serde(default)]
    status: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    percent_done: f64,
    #[serde(default)]
    rate_download: i64,
}

#[derive(Deserialize)]
struct TorrentGetResponse {
    arguments: Option<TorrentGetArgs>,
}

#[derive(Deserialize)]
struct TorrentGetArgs {
    torrents: Vec<TorrentInfo>,
}

/// Helper to handle Transmission's CSRF token mechanism.
async fn rpc_request(
    client: &Client,
    url: &str,
    user: &str,
    pass: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let endpoint = format!("{}/transmission/rpc", url);

    let mut builder = client.post(&endpoint).json(body);
    if !user.is_empty() {
        builder = builder.basic_auth(user, Some(pass));
    }

    let resp = builder.send().await?;

    if resp.status().as_u16() == 409 {
        let session_id = resp
            .headers()
            .get("x-transmission-session-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let mut builder2 = client
            .post(&endpoint)
            .header("X-Transmission-Session-Id", &session_id)
            .json(body);
        if !user.is_empty() {
            builder2 = builder2.basic_auth(user, Some(pass));
        }

        let resp2 = builder2.send().await?;
        let result: serde_json::Value = resp2.json().await?;
        Ok(result)
    } else if resp.status().is_success() {
        let result: serde_json::Value = resp.json().await?;
        Ok(result)
    } else {
        Err(format!("Transmission returned HTTP {}", resp.status()).into())
    }
}

pub async fn get_status(client: &Client, url: &str, user: &str, pass: &str) -> ServiceStatus {
    let endpoint = format!("{}/transmission/rpc", url);
    let rpc_req = RpcRequest {
        method: "session-get".to_string(),
    };

    let mut builder = client.post(&endpoint).json(&rpc_req);
    if !user.is_empty() {
        builder = builder.basic_auth(user, Some(pass));
    }

    match builder.send().await {
        Ok(resp) => {
            if resp.status().is_success() || resp.status().as_u16() == 409 {
                // Fetch downloading info
                let extras = fetch_extras(client, url, user, pass).await;
                ServiceStatus {
                    name: "Transmission".to_string(),
                    active: true,
                    message: "Running".to_string(),
                    version: None,
                    extras: Some(extras),
                }
            } else {
                ServiceStatus {
                    name: "Transmission".to_string(),
                    active: false,
                    message: format!("HTTP {}", resp.status()),
                    version: None,
                    extras: None,
                }
            }
        }
        Err(e) => ServiceStatus {
            name: "Transmission".to_string(),
            active: false,
            message: e.to_string(),
            version: None,
            extras: None,
        },
    }
}

async fn fetch_extras(client: &Client, url: &str, user: &str, pass: &str) -> serde_json::Value {
    let body = serde_json::json!({
        "method": "torrent-get",
        "arguments": {
            "fields": ["id", "name", "status", "percentDone", "rateDownload"]
        }
    });

    match rpc_request(client, url, user, pass, &body).await {
        Ok(data) => {
            if let Ok(parsed) = serde_json::from_value::<TorrentGetResponse>(data) {
                if let Some(args) = parsed.arguments {
                    let total = args.torrents.len() as i64;
                    // status 4 = downloading
                    let downloading: Vec<_> = args.torrents.iter()
                        .filter(|t| t.status == 4)
                        .collect();
                    let dl_count = downloading.len() as i64;
                    let dl_names: Vec<String> = downloading.iter()
                        .take(5)
                        .map(|t| {
                            let pct = (t.percent_done * 100.0).round() as i64;
                            format!("{} ({}%)", t.name, pct)
                        })
                        .collect();

                    return serde_json::json!({
                        "total_torrents": total,
                        "downloading": dl_count,
                        "downloading_names": dl_names
                    });
                }
            }
            serde_json::json!({})
        }
        Err(_) => serde_json::json!({}),
    }
}

pub async fn get_config(client: &Client, url: &str, user: &str, pass: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/transmission/rpc", url);
    let rpc_req = serde_json::json!({
        "method": "session-get"
    });
    let mut builder = client.post(&endpoint).json(&rpc_req);
    if !user.is_empty() {
        builder = builder.basic_auth(user, Some(pass));
    }
    builder.send().await?.json().await
}

pub async fn update_config(client: &Client, url: &str, user: &str, pass: &str, config: serde_json::Value) -> Result<(), reqwest::Error> {
    let endpoint = format!("{}/transmission/rpc", url);
    let rpc_req = serde_json::json!({
        "method": "session-set",
        "arguments": config
    });
    let mut builder = client.post(&endpoint).json(&rpc_req);
    if !user.is_empty() {
        builder = builder.basic_auth(user, Some(pass));
    }
    builder.send().await?.error_for_status()?;
    Ok(())
}

// --- Torrent CRUD Operations ---

pub async fn list_torrents(
    client: &Client,
    url: &str,
    user: &str,
    pass: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "method": "torrent-get",
        "arguments": {
            "fields": ["id", "name", "status", "percentDone", "rateDownload", "rateUpload", "sizeWhenDone", "eta", "errorString"]
        }
    });
    rpc_request(client, url, user, pass, &body).await
}

pub async fn add_torrent(
    client: &Client,
    url: &str,
    user: &str,
    pass: &str,
    filename: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "method": "torrent-add",
        "arguments": {
            "filename": filename
        }
    });
    rpc_request(client, url, user, pass, &body).await
}

pub async fn start_torrent(
    client: &Client,
    url: &str,
    user: &str,
    pass: &str,
    id: i64,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "method": "torrent-start",
        "arguments": {
            "ids": [id]
        }
    });
    rpc_request(client, url, user, pass, &body).await
}

pub async fn stop_torrent(
    client: &Client,
    url: &str,
    user: &str,
    pass: &str,
    id: i64,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "method": "torrent-stop",
        "arguments": {
            "ids": [id]
        }
    });
    rpc_request(client, url, user, pass, &body).await
}

pub async fn remove_torrent(
    client: &Client,
    url: &str,
    user: &str,
    pass: &str,
    id: i64,
    delete_data: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "method": "torrent-remove",
        "arguments": {
            "ids": [id],
            "delete-local-data": delete_data
        }
    });
    rpc_request(client, url, user, pass, &body).await
}
