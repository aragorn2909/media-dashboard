use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::api::ServiceStatus;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemStatus {
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WantedResponse {
    total_records: i64,
}

#[derive(Deserialize)]
struct ArtistCountResponse {}

fn clean_lidarr_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

pub async fn get_status(client: &Client, url: &str, api_key: &str) -> ServiceStatus {
    let base = clean_lidarr_url(url);
    let endpoint = format!("{}/api/v1/system/status?apikey={}", base, api_key.trim());

    match client.get(&endpoint).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<SystemStatus>().await {
                    Ok(status) => {
                        let extras = fetch_extras(client, url, api_key).await;
                        ServiceStatus {
                            name: "Lidarr".to_string(),
                            active: true,
                            message: "Running".to_string(),
                            version: Some(status.version),
                            extras: Some(extras),
                        }
                    },
                    Err(_) => ServiceStatus {
                        name: "Lidarr".to_string(),
                        active: true,
                        message: "Parse Error".to_string(),
                        version: None,
                        extras: None,
                    },
                }
            } else {
                ServiceStatus {
                    name: "Lidarr".to_string(),
                    active: false,
                    message: format!("HTTP {}", resp.status()),
                    version: None,
                    extras: None,
                }
            }
        }
        Err(e) => ServiceStatus {
            name: "Lidarr".to_string(),
            active: false,
            message: e.to_string(),
            version: None,
            extras: None,
        },
    }
}

async fn fetch_extras(client: &Client, url: &str, api_key: &str) -> serde_json::Value {
    let base = clean_lidarr_url(url);
    let key = api_key.trim();

    // Get missing albums count
    let missing = match client
        .get(format!("{}/api/v1/wanted/missing?apikey={}&pageSize=1", base, key))
        .send().await
    {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<WantedResponse>().await.map(|w| w.total_records).unwrap_or(0)
        }
        _ => 0,
    };

    // Get total artists count
    let total_artists: i64 = match client
        .get(format!("{}/api/v1/artist?apikey={}", base, key))
        .send().await
    {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<Vec<serde_json::Value>>().await.map(|v| v.len() as i64).unwrap_or(0)
        }
        _ => 0,
    };

    serde_json::json!({
        "missing_albums": missing,
        "total_artists": total_artists
    })
}

pub async fn get_config(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let base = clean_lidarr_url(url);
    let endpoint = format!("{}/api/v1/config/host?apikey={}", base, api_key.trim());
    client.get(&endpoint).send().await?.json().await
}

pub async fn update_config(client: &Client, url: &str, api_key: &str, config: serde_json::Value) -> Result<(), reqwest::Error> {
    let base = clean_lidarr_url(url);
    let endpoint = format!("{}/api/v1/config/host?apikey={}", base, api_key.trim());
    client.put(&endpoint).json(&config).send().await?.error_for_status()?;
    Ok(())
}

pub async fn list_artists(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let base = clean_lidarr_url(url);
    let endpoint = format!("{}/api/v1/artist?apikey={}", base, api_key.trim());
    client.get(&endpoint).send().await?.json().await
}
