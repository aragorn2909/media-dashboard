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
struct IndexerResponse {
    id: i64,
    name: String,
    protocol: String,
    enabled: bool,
}

fn clean_prowlarr_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn prowlarr_request(client: &Client, url: &str, api_key: &str) -> reqwest::RequestBuilder {
    client.get(url)
        .header("X-Api-Key", api_key.trim())
        .header("Accept", "application/json")
}

pub async fn get_status(client: &Client, url: &str, api_key: &str) -> ServiceStatus {
    let base = clean_prowlarr_url(url);
    let endpoint = format!("{}/api/v1/system/status", base);

    match prowlarr_request(client, &endpoint, api_key).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<SystemStatus>().await {
                    Ok(status) => {
                        let extras = fetch_extras(client, url, api_key).await;
                        ServiceStatus {
                            name: "Prowlarr".to_string(),
                            active: true,
                            message: "Running".to_string(),
                            version: Some(status.version),
                            extras: Some(extras),
                        }
                    },
                    Err(_) => ServiceStatus {
                        name: "Prowlarr".to_string(),
                        active: true,
                        message: "Parse Error".to_string(),
                        version: None,
                        extras: None,
                    },
                }
            } else {
                ServiceStatus {
                    name: "Prowlarr".to_string(),
                    active: false,
                    message: format!("HTTP {}", resp.status()),
                    version: None,
                    extras: None,
                }
            }
        }
        Err(e) => ServiceStatus {
            name: "Prowlarr".to_string(),
            active: false,
            message: e.to_string(),
            version: None,
            extras: None,
        },
    }
}

async fn fetch_extras(client: &Client, url: &str, api_key: &str) -> serde_json::Value {
    let base = clean_prowlarr_url(url);
    let endpoint = format!("{}/api/v1/indexer", base);

    let indexers = match prowlarr_request(client, &endpoint, api_key).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<Vec<IndexerResponse>>().await.unwrap_or_default()
        }
        _ => Vec::new(),
    };

    let total = indexers.len();
    let enabled = indexers.iter().filter(|i| i.enabled).count();

    serde_json::json!({
        "total_indexers": total,
        "enabled_indexers": enabled
    })
}

pub async fn get_config(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let base = clean_prowlarr_url(url);
    let endpoint = format!("{}/api/v1/config/host", base);
    prowlarr_request(client, &endpoint, api_key).send().await?.json().await
}

pub async fn update_config(client: &Client, url: &str, api_key: &str, config: serde_json::Value) -> Result<(), reqwest::Error> {
    let base = clean_prowlarr_url(url);
    let endpoint = format!("{}/api/v1/config/host", base);
    client.put(&endpoint)
        .header("X-Api-Key", api_key.trim())
        .json(&config)
        .send().await?
        .error_for_status()?;
    Ok(())
}

pub async fn list_indexers(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, reqwest::Error> {
    let base = clean_prowlarr_url(url);
    let endpoint = format!("{}/api/v1/indexer", base);
    prowlarr_request(client, &endpoint, api_key).send().await?.json().await
}
