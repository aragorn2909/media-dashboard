use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::api::ServiceStatus;

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbyConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EmbySession {
    pub id: String,
    pub user_name: Option<String>,
    pub now_playing_item: Option<NowPlayingItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NowPlayingItem {
    pub name: String,
}

pub async fn get_status(client: &Client, url: &str, api_key: &str) -> ServiceStatus {
    let endpoint = format!("{}/Sessions?api_key={}", url, api_key);
    match client.get(&endpoint).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Vec<EmbySession>>().await {
                    Ok(sessions) => {
                        let active_sessions: Vec<_> = sessions.into_iter()
                            .filter(|s| s.now_playing_item.is_some())
                            .collect();
                        
                        let active_count = active_sessions.len();
                        let names: Vec<String> = active_sessions.iter()
                            .map(|s| format!("{} ({})", 
                                s.now_playing_item.as_ref().map(|i| i.name.as_str()).unwrap_or("Unknown"),
                                s.user_name.as_deref().unwrap_or("Unknown")
                            ))
                            .collect();

                        ServiceStatus {
                            name: "Emby".to_string(),
                            active: true,
                            message: format!("{} active session(s)", active_count),
                            version: None,
                            extras: Some(serde_json::json!({
                                "active_sessions": active_count,
                                "sessions": names
                            })),
                        }
                    }
                    Err(e) => ServiceStatus {
                        name: "Emby".to_string(),
                        active: true,
                        message: format!("Parse Error: {}", e),
                        version: None,
                        extras: None,
                    },
                }
            } else {
                ServiceStatus {
                    name: "Emby".to_string(),
                    active: false,
                    message: format!("HTTP {}", resp.status()),
                    version: None,
                    extras: None,
                }
            }
        }
        Err(e) => ServiceStatus {
            name: "Emby".to_string(),
            active: false,
            message: e.to_string(),
            version: None,
            extras: None,
        },
    }
}
