use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::api::ServiceStatus;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct PlexLibrary {
    pub key: String,
    pub title: String,
    pub lib_type: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct PlexRecentItem {
    pub title: String,
    pub media_type: String,
    pub year: Option<i32>,
    pub thumb: Option<String>,
    pub grandparent_title: Option<String>,
    pub added_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlexConfig {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaContainer {
    #[serde(alias = "size")]
    #[serde(default)]
    pub size: i32,
    #[serde(alias = "Metadata")]
    pub metadata: Option<Vec<PlexSession>>,
}

#[derive(Debug, Deserialize)]
struct PlexSession {
    pub title: Option<String>,
    #[serde(rename = "User")]
    pub user: Option<PlexUser>,
    #[serde(rename = "Player")]
    pub player: Option<PlexPlayer>,
}

#[derive(Debug, Deserialize)]
struct PlexUser {
    pub title: String,
}

#[derive(Debug, Deserialize)]
struct PlexPlayer {
    pub state: String,
}

pub async fn get_status(client: &Client, url: &str, token: &str) -> ServiceStatus {
    let endpoint = format!("{}/status/sessions?X-Plex-Token={}", url, token);
    match client.get(&endpoint).header("Accept", "application/json").send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<MediaContainerWrapper>().await {
                    Ok(wrapper) => {
                        let sessions = wrapper.media_container;
                        let active_count = sessions.size;
                        let details = format!("{} active session(s)", active_count);
                        
                        let extras = if let Some(meta) = sessions.metadata {
                            let names: Vec<String> = meta.iter()
                                .map(|s| format!("{} ({})", 
                                    s.title.as_deref().unwrap_or("Unknown"),
                                    s.user.as_ref().map(|u| u.title.as_str()).unwrap_or("Unknown")
                                ))
                                .collect();
                            Some(serde_json::json!({
                                "active_sessions": active_count,
                                "sessions": names
                            }))
                        } else {
                            Some(serde_json::json!({
                                "active_sessions": active_count,
                                "sessions": Vec::<String>::new()
                            }))
                        };

                        ServiceStatus {
                            name: "Plex".to_string(),
                            active: true,
                            message: details,
                            version: None,
                            extras,
                        }
                    }
                    Err(e) => ServiceStatus {
                        name: "Plex".to_string(),
                        active: true,
                        message: format!("Parse Error: {}", e),
                        version: None,
                        extras: None,
                    },
                }
            } else {
                ServiceStatus {
                    name: "Plex".to_string(),
                    active: false,
                    message: format!("HTTP {}", resp.status()),
                    version: None,
                    extras: None,
                }
            }
        }
        Err(e) => ServiceStatus {
            name: "Plex".to_string(),
            active: false,
            message: e.to_string(),
            version: None,
            extras: None,
        },
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaContainerWrapper {
    #[serde(alias = "MediaContainer")]
    pub media_container: MediaContainer,
}

// ── Server Info ─────────────────────────────────────────────────

pub async fn get_server_info(
    client: &Client,
    url: &str,
    token: &str,
) -> Result<serde_json::Value, String> {
    let base = url.trim_end_matches('/');
    let endpoint = format!("{}/?X-Plex-Token={}", base, token);
    let resp = client
        .get(&endpoint)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    let machine_id = json
        .pointer("/MediaContainer/machineIdentifier")
        .or_else(|| json.get("machineIdentifier"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let server_name = json
        .pointer("/MediaContainer/friendlyName")
        .or_else(|| json.get("friendlyName"))
        .and_then(|v| v.as_str())
        .unwrap_or("Plex")
        .to_string();
    Ok(serde_json::json!({ "machine_id": machine_id, "server_name": server_name }))
}

// ── Libraries ──────────────────────────────────────────────────

pub async fn get_libraries(
    client: &Client,
    url: &str,
    token: &str,
) -> Result<Vec<PlexLibrary>, String> {
    let base = url.trim_end_matches('/');
    let endpoint = format!("{}/library/sections?X-Plex-Token={}", base, token);
    let resp = client
        .get(&endpoint)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    let dirs = json
        .pointer("/MediaContainer/Directory")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut libraries: Vec<PlexLibrary> = dirs
        .iter()
        .map(|d| PlexLibrary {
            key: d["key"].as_str().unwrap_or("").to_string(),
            title: d["title"].as_str().unwrap_or("Unknown").to_string(),
            lib_type: d["type"].as_str().unwrap_or("unknown").to_string(),
            count: 0, // filled in below
        })
        .collect();

    // Fetch item count for each library (sections endpoint doesn't include it)
    for lib in &mut libraries {
        let count_url = format!(
            "{}/library/sections/{}/all?X-Plex-Token={}&X-Plex-Container-Start=0&X-Plex-Container-Size=0",
            base, lib.key, token
        );
        if let Ok(r) = client.get(&count_url).header("Accept", "application/json").send().await {
            if let Ok(j) = r.json::<Value>().await {
                lib.count = j.pointer("/MediaContainer/totalSize")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
            }
        }
    }

    Ok(libraries)
}

// ── Recently Added ──────────────────────────────────────────────

pub async fn get_recently_added(
    client: &Client,
    url: &str,
    token: &str,
    limit: usize,
) -> Result<Vec<PlexRecentItem>, String> {
    let base = url.trim_end_matches('/');
    let endpoint = format!(
        "{}/library/recentlyAdded?X-Plex-Token={}&X-Plex-Container-Start=0&X-Plex-Container-Size={}",
        base, token, limit
    );
    let resp = client
        .get(&endpoint)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    let items = json
        .pointer("/MediaContainer/Metadata")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(items
        .iter()
        .map(|m| {
            let thumb = m["thumb"].as_str().or_else(|| m["grandparentThumb"].as_str()).map(|t| {
                // Use ? not & — the thumb path has no existing query string
                format!("{}{}?X-Plex-Token={}", base, t, token)
            });
            PlexRecentItem {
                title: m["title"].as_str().unwrap_or("Unknown").to_string(),
                media_type: m["type"].as_str().unwrap_or("unknown").to_string(),
                year: m["year"].as_i64().map(|y| y as i32),
                thumb,
                grandparent_title: m["grandparentTitle"].as_str().map(|s| s.to_string()),
                added_at: m["addedAt"].as_i64(),
            }
        })
        .collect())
}
