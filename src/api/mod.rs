pub mod sonarr;
pub mod radarr;
pub mod jackett;
pub mod transmission;
pub mod plex;
pub mod jellyfin;
pub mod emby;

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub active: bool,
    pub message: String,
    pub version: Option<String>,
    pub extras: Option<serde_json::Value>,
}
