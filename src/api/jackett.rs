use reqwest::Client;
use crate::api::ServiceStatus;

pub async fn get_status(client: &Client, url: &str, api_key: &str) -> ServiceStatus {
    let base = url.trim_end_matches('/');

    // The /api/v2.0/indexers endpoint requires browser cookies.
    // The correct machine-to-machine health check is the search endpoint:
    let health_endpoint = format!("{}/api/v2.0/indexers/all/results?apikey={}&t=search&q=", base, api_key);

    match client.get(&health_endpoint).send().await {
        Ok(resp) if resp.status().is_success() => {
            // Use the Torznab ?t=indexers endpoint — accepts apikey without cookies, returns XML
            let torznab_endpoint = format!(
                "{}/api/v2.0/indexers/all/results/torznab/api?apikey={}&t=indexers",
                base, api_key
            );
            let (total, failed_count) = if let Ok(r) = client.get(&torznab_endpoint).send().await {
                if let Ok(xml) = r.text().await {
                    // Count only indexers with configured="true" in their opening tag
                    let total = count_configured_indexers(&xml);
                    (total, 0i64)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            ServiceStatus {
                name: "Jackett".to_string(),
                active: true,
                message: "Running".to_string(),
                version: None,
                extras: Some(serde_json::json!({
                    "total_indexers": total,
                    "failed_count": failed_count,
                    "failed_indexers": Vec::<String>::new()
                })),
            }
        }
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            ServiceStatus {
                name: "Jackett".to_string(),
                active: false,
                message: format!("HTTP — {}", body.chars().take(80).collect::<String>()),
                version: None,
                extras: None,
            }
        }
        Err(e) => ServiceStatus {
            name: "Jackett".to_string(),
            active: false,
            message: format!("Connection error: {}", e),
            version: None,
            extras: None,
        },
    }
}


// --- Indexer Listing ---

pub async fn list_indexers(client: &Client, url: &str, api_key: &str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    // The REST indexers endpoint requires browser cookies — use Torznab instead
    let base = url.trim_end_matches('/');
    let endpoint = format!("{}/api/v2.0/indexers/all/results/torznab/api?apikey={}&t=indexers", base, api_key);
    let resp = client.get(&endpoint).send().await?;

    if !resp.status().is_success() {
        return Err(format!("Jackett returned HTTP {}", resp.status()).into());
    }

    let xml = resp.text().await?;

    // Parse indexer elements from Torznab XML into JSON array for the frontend
    // XML format: <indexer id="..." type="public"><title>Name</title>...</indexer>
    let mut indexers = Vec::new();
    let mut remaining = xml.as_str();
    while let Some(start) = remaining.find("<indexer ") {
        remaining = &remaining[start + 9..]; // skip past "<indexer "
        // Extract id attribute
        let id = extract_attr(remaining, "id").unwrap_or_default();
        let itype = extract_attr(remaining, "type").unwrap_or_else(|| "public".to_string());
        // Extract <title> element
        let name = extract_tag(remaining, "title").unwrap_or_else(|| id.clone());
        // Check configured attribute — default false so unconfigured indexers are excluded
        let configured = extract_attr(remaining, "configured")
            .map(|v| v == "true")
            .unwrap_or(false);

        // Only include configured indexers
        if configured {
            indexers.push(serde_json::json!({
                "id": id,
                "name": name,
                "type": itype,
                "configured": configured
            }));
        }
    }

    Ok(serde_json::Value::Array(indexers))
}

fn count_configured_indexers(xml: &str) -> i64 {
    let mut count = 0i64;
    let mut search = xml;
    while let Some(pos) = search.find("<indexer ") {
        // Look at up to 300 chars of the opening tag attributes
        let tag_slice = &search[pos..].chars().take(300).collect::<String>();
        if tag_slice.contains("configured=\"true\"") {
            count += 1;
        }
        search = &search[pos + 9..];
    }
    count
}

fn extract_attr(s: &str, attr: &str) -> Option<String> {
    let needle = format!("{}=\"", attr);
    let start = s.find(&needle)? + needle.len();
    // Stop at end of opening tag
    let s = &s[start..];
    let end = s.find('"')?;
    if end < 200 { Some(s[..end].to_string()) } else { None }
}

fn extract_tag(s: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = s.find(&open)? + open.len();
    let s = &s[start..];
    let end = s.find(&close)?;
    if end < 500 { Some(s[..end].trim().to_string()) } else { None }
}
