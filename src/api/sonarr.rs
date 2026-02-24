use serde::Deserialize;
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

pub async fn get_status(client: &Client, url: &str, api_key: &str) -> ServiceStatus {
    let endpoint = format!("{}/api/v3/system/status?apikey={}", url, api_key);
    match client.get(&endpoint).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<SystemStatus>().await {
                    Ok(status) => {
                        // Fetch missing episodes count
                        let extras = fetch_extras(client, url, api_key).await;
                        ServiceStatus {
                            name: "Sonarr".to_string(),
                            active: true,
                            message: "Running".to_string(),
                            version: Some(status.version),
                            extras: Some(extras),
                        }
                    },
                    Err(_) => ServiceStatus {
                        name: "Sonarr".to_string(),
                        active: true,
                        message: "Parse Error".to_string(),
                        version: None,
                        extras: None,
                    },
                }
            } else {
                ServiceStatus {
                    name: "Sonarr".to_string(),
                    active: false,
                    message: format!("HTTP {}", resp.status()),
                    version: None,
                    extras: None,
                }
            }
        }
        Err(e) => ServiceStatus {
            name: "Sonarr".to_string(),
            active: false,
            message: e.to_string(),
            version: None,
            extras: None,
        },
    }
}

async fn fetch_extras(client: &Client, url: &str, api_key: &str) -> serde_json::Value {
    // Get missing episodes count
    let missing = match client
        .get(format!("{}/api/v3/wanted/missing?apikey={}&pageSize=1&sortKey=airDateUtc&sortDirection=descending", url, api_key))
        .send().await
    {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<WantedResponse>().await.map(|w| w.total_records).unwrap_or(0)
        }
        _ => 0,
    };

    // Get total series count
    let total_series: i64 = match client
        .get(format!("{}/api/v3/series?apikey={}", url, api_key))
        .send().await
    {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<Vec<serde_json::Value>>().await.map(|v| v.len() as i64).unwrap_or(0)
        }
        _ => 0,
    };

    serde_json::json!({
        "missing_episodes": missing,
        "total_series": total_series
    })
}

pub async fn get_config(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/config/host?apikey={}", url, api_key);
    client.get(&endpoint).send().await?.json().await
}

pub async fn update_config(client: &Client, url: &str, api_key: &str, config: serde_json::Value) -> Result<(), reqwest::Error> {
    let endpoint = format!("{}/api/v3/config/host?apikey={}", url, api_key);
    client.put(&endpoint).json(&config).send().await?.error_for_status()?;
    Ok(())
}

// --- CRUD Operations ---

pub async fn list_series(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/series?apikey={}", url, api_key);
    client.get(&endpoint).send().await?.json().await
}

pub async fn search_series(client: &Client, url: &str, api_key: &str, term: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/series/lookup?apikey={}&term={}", url, api_key, urlencoding::encode(term));
    client.get(&endpoint).send().await?.json().await
}

pub async fn add_series(client: &Client, url: &str, api_key: &str, body: serde_json::Value) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/series?apikey={}", url, api_key);
    client.post(&endpoint).json(&body).send().await?.json().await
}

pub async fn delete_series(client: &Client, url: &str, api_key: &str, id: i64, delete_files: bool) -> Result<(), reqwest::Error> {
    let endpoint = format!("{}/api/v3/series/{}?apikey={}&deleteFiles={}", url, id, api_key, delete_files);
    client.delete(&endpoint).send().await?.error_for_status()?;
    Ok(())
}

pub async fn get_calendar(client: &Client, url: &str, api_key: &str, start: &str, end: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/calendar?apikey={}&start={}&end={}&includeSeries=true", url, api_key, start, end);
    client.get(&endpoint).send().await?.json().await
}

pub async fn get_disk_space(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/diskspace?apikey={}", url, api_key);
    client.get(&endpoint).send().await?.json().await
}

pub async fn get_root_folders(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/rootfolder?apikey={}", url, api_key);
    client.get(&endpoint).send().await?.json().await
}

pub async fn get_quality_profiles(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let endpoint = format!("{}/api/v3/qualityprofile?apikey={}", url, api_key);
    client.get(&endpoint).send().await?.json().await
}
