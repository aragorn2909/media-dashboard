use axum::{
    routing::{get, post, delete},
    Json, Router,
    extract::{State, Path, Query},
};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::fs;
use reqwest::Client;

mod api;
mod db;
use api::ServiceStatus;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Default)]
struct Config {
    sonarr_url: String,
    sonarr_key: String,
    radarr_url: String,
    radarr_key: String,
    jackett_url: String,
    jackett_key: String,
    transmission_url: String,
    transmission_user: String,
    transmission_pass: String,
    plex_url: String,
    plex_token: String,
    jellyfin_url: String,
    jellyfin_key: String,
    emby_url: String,
    emby_key: String,
}

#[derive(Clone)]
struct AppState {
    config: Arc<tokio::sync::RwLock<Config>>,
    client: Client,
    db: SqlitePool,
}

#[derive(Deserialize)]
struct SearchQuery {
    term: Option<String>,
}

#[derive(Deserialize)]
struct DeleteQuery {
    #[serde(rename = "deleteFiles")]
    delete_files: Option<bool>,
    #[serde(rename = "deleteData")]
    delete_data: Option<bool>,
}

#[derive(Deserialize)]
struct TorrentAddPayload {
    filename: String,
}

type AppError = (axum::http::StatusCode, String);

fn internal_err(e: impl std::fmt::Display) -> AppError {
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[tokio::main]
async fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC occurred: {:?}", info);
    }));

    eprintln!("STAGE 0: Starting Media Dashboard...");
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "media_dashboard=debug,tower_http=debug,axum=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::fmt::layer().with_writer(std::fs::File::create("data/app.log").expect("Failed to create log file")))
        .init();

    tracing::info!("STAGE 1: Logger initialized (Console + File)");

    tracing::info!("STAGE 2: Initializing DB");
    let db = db::init_db().await;
    
    tracing::info!("STAGE 3: Running migrations");
    migrate_config_if_needed(&db).await;
    
    tracing::info!("STAGE 4: Loading config");
    let config = load_config_from_db(&db).await;
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .unwrap_or_else(|_| Client::new());
    let state = Arc::new(AppState { 
        config: Arc::new(tokio::sync::RwLock::new(config)), 
        client, 
        db 
    });

    tracing::info!("STAGE 5: Setting up router");
    let app = Router::new()
        // Dashboard status
        .route("/api/status", get(get_all_status))
        .route("/api/search", get(global_search))
        .route("/api/calendar", get(get_calendar_data))
        .route("/api/stats", get(get_library_stats))
        .route("/api/config", get(get_dashboard_config).post(update_dashboard_config))
        .route("/api/settings/:service", get(get_service_settings).post(update_service_settings))
        .route("/api/logs/audit", get(get_audit_logs))
        .route("/api/logs/system", get(get_system_logs))
        // Sonarr CRUD
        .route("/api/sonarr/series", get(sonarr_list_series).post(sonarr_add_series))
        .route("/api/sonarr/series/search", get(sonarr_search_series))
        .route("/api/sonarr/series/:id", delete(sonarr_delete_series))
        .route("/api/sonarr/rootfolders", get(sonarr_root_folders))
        .route("/api/sonarr/qualityprofiles", get(sonarr_quality_profiles))
        // Radarr CRUD
        .route("/api/radarr/movies", get(radarr_list_movies).post(radarr_add_movie))
        .route("/api/radarr/movies/search", get(radarr_search_movies))
        .route("/api/radarr/movies/:id", delete(radarr_delete_movie))
        .route("/api/radarr/rootfolders", get(radarr_root_folders))
        .route("/api/radarr/qualityprofiles", get(radarr_quality_profiles))
        // Jackett
        .route("/api/jackett/indexers", get(jackett_list_indexers))
        // Plex
        .route("/api/plex/libraries", get(plex_get_libraries))
        .route("/api/plex/recently-added", get(plex_recently_added))
        .route("/api/plex/server-info", get(plex_server_info))
        // Transmission CRUD
        .route("/api/transmission/torrents", get(transmission_list_torrents).post(transmission_add_torrent))
        .route("/api/transmission/torrents/:id", delete(transmission_remove_torrent))
        .route("/api/transmission/torrents/:id/start", post(transmission_start_torrent))
        .route("/api/transmission/torrents/:id/stop", post(transmission_stop_torrent))
        // Static files
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 7778));
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ===================== Dashboard Handlers =====================

async fn get_all_status(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ServiceStatus>> {
    let mut statuses = Vec::new();
    let config = state.config.read().await;
    let client = &state.client;

    if !config.plex_url.is_empty() {
        statuses.push(api::plex::get_status(client, &config.plex_url, &config.plex_token).await);
    }
    if !config.sonarr_url.is_empty() {
        statuses.push(api::sonarr::get_status(client, &config.sonarr_url, &config.sonarr_key).await);
    }
    if !config.radarr_url.is_empty() {
        statuses.push(api::radarr::get_status(client, &config.radarr_url, &config.radarr_key).await);
    }
    if !config.jackett_url.is_empty() {
        statuses.push(api::jackett::get_status(client, &config.jackett_url, &config.jackett_key).await);
    }
    if !config.transmission_url.is_empty() {
        statuses.push(api::transmission::get_status(client, &config.transmission_url, &config.transmission_user, &config.transmission_pass).await);
    }
    if !config.jellyfin_url.is_empty() {
        statuses.push(api::jellyfin::get_status(client, &config.jellyfin_url, &config.jellyfin_key).await);
    }
    if !config.emby_url.is_empty() {
        statuses.push(api::emby::get_status(client, &config.emby_url, &config.emby_key).await);
    }

    Json(statuses)
}

async fn global_search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let term = q.term.unwrap_or_default();
    if term.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "Missing 'term' parameter".to_string()));
    }
    let config = state.config.read().await;
    let client = &state.client;

    let mut sonarr_results = serde_json::Value::Null;
    let mut radarr_results = serde_json::Value::Null;

    if !config.sonarr_url.is_empty() {
        if let Ok(res) = api::sonarr::search_series(client, &config.sonarr_url, &config.sonarr_key, &term).await {
            sonarr_results = res;
        }
    }
    if !config.radarr_url.is_empty() {
        if let Ok(res) = api::radarr::search_movies(client, &config.radarr_url, &config.radarr_key, &term).await {
            radarr_results = res;
        }
    }

    Ok(Json(serde_json::json!({
        "sonarr": sonarr_results,
        "radarr": radarr_results
    })))
}

async fn get_calendar_data(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let client = &state.client;
    
    let now = chrono::Utc::now();
    let end = now + chrono::Duration::days(7);
    let start_str = now.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    let mut sonarr_cal = serde_json::Value::Null;
    let mut radarr_cal = serde_json::Value::Null;

    if !config.sonarr_url.is_empty() {
        if let Ok(res) = api::sonarr::get_calendar(client, &config.sonarr_url, &config.sonarr_key, &start_str, &end_str).await {
            sonarr_cal = res;
        }
    }
    if !config.radarr_url.is_empty() {
        if let Ok(res) = api::radarr::get_calendar(client, &config.radarr_url, &config.radarr_key, &start_str, &end_str).await {
            radarr_cal = res;
        }
    }

    Ok(Json(serde_json::json!({
        "sonarr": sonarr_cal,
        "radarr": radarr_cal
    })))
}

async fn get_library_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let client = &state.client;

    let mut sonarr_disk = serde_json::Value::Null;
    let mut radarr_disk = serde_json::Value::Null;

    if !config.sonarr_url.is_empty() {
        if let Ok(res) = api::sonarr::get_disk_space(client, &config.sonarr_url, &config.sonarr_key).await {
            sonarr_disk = res;
        }
    }
    if !config.radarr_url.is_empty() {
        if let Ok(res) = api::radarr::get_disk_space(client, &config.radarr_url, &config.radarr_key).await {
            radarr_disk = res;
        }
    }

    Ok(Json(serde_json::json!({
        "sonarr_disk": sonarr_disk,
        "radarr_disk": radarr_disk,
    })))
}

async fn get_dashboard_config(
    State(state): State<Arc<AppState>>,
) -> Json<Config> {
    let mut config = state.config.read().await.clone();
    
    let mask = "********".to_string();
    if !config.sonarr_key.is_empty() { config.sonarr_key = mask.clone(); }
    if !config.radarr_key.is_empty() { config.radarr_key = mask.clone(); }
    if !config.jackett_key.is_empty() { config.jackett_key = mask.clone(); }
    if !config.transmission_pass.is_empty() { config.transmission_pass = mask.clone(); }
    if !config.plex_token.is_empty() { config.plex_token = mask.clone(); }
    if !config.jellyfin_key.is_empty() { config.jellyfin_key = mask.clone(); }
    if !config.emby_key.is_empty() { config.emby_key = mask; }
    
    Json(config)
}

async fn update_dashboard_config(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<Config>,
) -> axum::http::StatusCode {
    let mask = "********";
    {
        let mut config = state.config.write().await;
        
        // Preserve existing keys if incoming payload has the mask
        if payload.sonarr_key == mask { payload.sonarr_key = config.sonarr_key.clone(); }
        if payload.radarr_key == mask { payload.radarr_key = config.radarr_key.clone(); }
        if payload.jackett_key == mask { payload.jackett_key = config.jackett_key.clone(); }
        if payload.transmission_pass == mask { payload.transmission_pass = config.transmission_pass.clone(); }
        if payload.plex_token == mask { payload.plex_token = config.plex_token.clone(); }
        if payload.jellyfin_key == mask { payload.jellyfin_key = config.jellyfin_key.clone(); }
        if payload.emby_key == mask { payload.emby_key = config.emby_key.clone(); }
        
        *config = payload.clone();
    }
    save_config_to_db(&state.db, &payload).await;
    db::log_event(&state.db, "System", "Config Updated", "Connection settings updated via Dashboard").await;
    axum::http::StatusCode::OK
}

async fn get_service_settings(
    State(state): State<Arc<AppState>>,
    Path(service): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let client = &state.client;
    match service.as_str() {
        "sonarr" => api::sonarr::get_config(client, &config.sonarr_url, &config.sonarr_key)
            .await.map(Json).map_err(|e| internal_err(e)),
        "radarr" => api::radarr::get_config(client, &config.radarr_url, &config.radarr_key)
            .await.map(Json).map_err(|e| internal_err(e)),
        "transmission" => api::transmission::get_config(client, &config.transmission_url, &config.transmission_user, &config.transmission_pass)
            .await.map(Json).map_err(|e| internal_err(e)),
        _ => Err((axum::http::StatusCode::NOT_FOUND, "Service not found".to_string())),
    }
}

async fn update_service_settings(
    State(state): State<Arc<AppState>>,
    Path(service): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<axum::http::StatusCode, AppError> {
    let config = state.config.read().await;
    let client = &state.client;
    let res = match service.as_str() {
        "sonarr" => api::sonarr::update_config(client, &config.sonarr_url, &config.sonarr_key, payload).await,
        "radarr" => api::radarr::update_config(client, &config.radarr_url, &config.radarr_key, payload).await,
        "transmission" => api::transmission::update_config(client, &config.transmission_url, &config.transmission_user, &config.transmission_pass, payload).await,
        _ => return Err((axum::http::StatusCode::NOT_FOUND, "Service not found".to_string())),
    };
    if res.is_ok() {
        db::log_event(&state.db, &service, "Settings Updated", "Configuration changes applied via Dashboard").await;
        Ok(axum::http::StatusCode::OK)
    } else {
        Err(internal_err(res.err().unwrap()))
    }
}

async fn get_audit_logs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let logs = sqlx::query_as::<_, db::AuditLog>("SELECT id, timestamp, service, action, details FROM audit_logs ORDER BY timestamp DESC LIMIT 100")
        .fetch_all(&state.db)
        .await
        .map_err(|e| internal_err(e))?;
    Ok(Json(serde_json::to_value(logs).unwrap()))
}

// ===================== Sonarr Handlers =====================

async fn sonarr_list_series(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::sonarr::list_series(&state.client, &config.sonarr_url, &config.sonarr_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn sonarr_search_series(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let term = q.term.unwrap_or_default();
    if term.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "Missing 'term' parameter".to_string()));
    }
    let config = state.config.read().await;
    api::sonarr::search_series(&state.client, &config.sonarr_url, &config.sonarr_key, &term)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn sonarr_add_series(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let result = api::sonarr::add_series(&state.client, &config.sonarr_url, &config.sonarr_key, body)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Sonarr", "Series Added", "New series added via Dashboard").await;
    Ok(Json(result))
}

async fn sonarr_delete_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<DeleteQuery>,
) -> Result<axum::http::StatusCode, AppError> {
    let config = state.config.read().await;
    let delete_files = q.delete_files.unwrap_or(false);
    api::sonarr::delete_series(&state.client, &config.sonarr_url, &config.sonarr_key, id, delete_files)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Sonarr", "Series Deleted", &format!("Series {} removed via Dashboard", id)).await;
    Ok(axum::http::StatusCode::OK)
}

async fn sonarr_root_folders(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::sonarr::get_root_folders(&state.client, &config.sonarr_url, &config.sonarr_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn sonarr_quality_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::sonarr::get_quality_profiles(&state.client, &config.sonarr_url, &config.sonarr_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

// ===================== Radarr Handlers =====================

async fn radarr_list_movies(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::radarr::list_movies(&state.client, &config.radarr_url, &config.radarr_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn radarr_search_movies(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let term = q.term.unwrap_or_default();
    if term.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "Missing 'term' parameter".to_string()));
    }
    let config = state.config.read().await;
    api::radarr::search_movies(&state.client, &config.radarr_url, &config.radarr_key, &term)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn radarr_add_movie(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let result = api::radarr::add_movie(&state.client, &config.radarr_url, &config.radarr_key, body)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Radarr", "Movie Added", "New movie added via Dashboard").await;
    Ok(Json(result))
}

async fn radarr_delete_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<DeleteQuery>,
) -> Result<axum::http::StatusCode, AppError> {
    let config = state.config.read().await;
    let delete_files = q.delete_files.unwrap_or(false);
    api::radarr::delete_movie(&state.client, &config.radarr_url, &config.radarr_key, id, delete_files)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Radarr", "Movie Deleted", &format!("Movie {} removed via Dashboard", id)).await;
    Ok(axum::http::StatusCode::OK)
}

async fn radarr_root_folders(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::radarr::get_root_folders(&state.client, &config.radarr_url, &config.radarr_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn radarr_quality_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::radarr::get_quality_profiles(&state.client, &config.radarr_url, &config.radarr_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

// ===================== Jackett Handlers =====================

async fn jackett_list_indexers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::jackett::list_indexers(&state.client, &config.jackett_url, &config.jackett_key)
        .await.map(Json).map_err(|e| internal_err(e))
}

// ===================== Plex Handlers =====================

async fn plex_get_libraries(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let libs = api::plex::get_libraries(&state.client, &config.plex_url, &config.plex_token)
        .await.map_err(|e| internal_err(e))?;
    Ok(Json(serde_json::to_value(libs).unwrap_or_default()))
}

async fn plex_recently_added(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let items = api::plex::get_recently_added(&state.client, &config.plex_url, &config.plex_token, 30)
        .await.map_err(|e| internal_err(e))?;
    Ok(Json(serde_json::to_value(items).unwrap_or_default()))
}

async fn plex_server_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::plex::get_server_info(&state.client, &config.plex_url, &config.plex_token)
        .await.map(Json).map_err(|e| internal_err(e))
}

// ===================== Transmission Handlers =====================

async fn transmission_list_torrents(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    api::transmission::list_torrents(&state.client, &config.transmission_url, &config.transmission_user, &config.transmission_pass)
        .await.map(Json).map_err(|e| internal_err(e))
}

async fn transmission_add_torrent(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TorrentAddPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let result = api::transmission::add_torrent(&state.client, &config.transmission_url, &config.transmission_user, &config.transmission_pass, &payload.filename)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Transmission", "Torrent Added", "New torrent added via Dashboard").await;
    Ok(Json(result))
}

async fn transmission_remove_torrent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<DeleteQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read().await;
    let delete_data = q.delete_data.unwrap_or(false);
    let result = api::transmission::remove_torrent(&state.client, &config.transmission_url, &config.transmission_user, &config.transmission_pass, id, delete_data)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Transmission", "Torrent Removed", &format!("Torrent {} removed via Dashboard", id)).await;
    Ok(Json(result))
}

// ===================== Config Helpers =====================

async fn migrate_config_if_needed(pool: &SqlitePool) {
    let path = "config.json";
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.is_dir() {
            tracing::warn!("config.json is a directory, skipping migration");
            return;
        }
    }

    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str::<Config>(&data) {
            println!("Migrating config.json to database...");
            save_config_to_db(pool, &config).await;
            db::log_event(pool, "System", "Migration", "Successfully migrated config.json to database").await;
            let _ = fs::rename("config.json", "config.json.bak");
        }
    }
}

async fn load_config_from_db(pool: &SqlitePool) -> Config {
    Config {
        sonarr_url: db::get_setting(pool, "sonarr_url").await.unwrap_or_default(),
        sonarr_key: db::get_setting(pool, "sonarr_key").await.unwrap_or_default(),
        radarr_url: db::get_setting(pool, "radarr_url").await.unwrap_or_default(),
        radarr_key: db::get_setting(pool, "radarr_key").await.unwrap_or_default(),
        jackett_url: db::get_setting(pool, "jackett_url").await.unwrap_or_default(),
        jackett_key: db::get_setting(pool, "jackett_key").await.unwrap_or_default(),
        transmission_url: db::get_setting(pool, "transmission_url").await.unwrap_or_default(),
        transmission_user: db::get_setting(pool, "transmission_user").await.unwrap_or_default(),
        transmission_pass: db::get_setting(pool, "transmission_pass").await.unwrap_or_default(),
        plex_url: db::get_setting(pool, "plex_url").await.unwrap_or_default(),
        plex_token: db::get_setting(pool, "plex_token").await.unwrap_or_default(),
        jellyfin_url: db::get_setting(pool, "jellyfin_url").await.unwrap_or_default(),
        jellyfin_key: db::get_setting(pool, "jellyfin_key").await.unwrap_or_default(),
        emby_url: db::get_setting(pool, "emby_url").await.unwrap_or_default(),
        emby_key: db::get_setting(pool, "emby_key").await.unwrap_or_default(),
    }
}

async fn save_config_to_db(pool: &SqlitePool, config: &Config) {
    db::set_setting(pool, "sonarr_url", &config.sonarr_url).await;
    db::set_setting(pool, "sonarr_key", &config.sonarr_key).await;
    db::set_setting(pool, "radarr_url", &config.radarr_url).await;
    db::set_setting(pool, "radarr_key", &config.radarr_key).await;
    db::set_setting(pool, "jackett_url", &config.jackett_url).await;
    db::set_setting(pool, "jackett_key", &config.jackett_key).await;
    db::set_setting(pool, "transmission_url", &config.transmission_url).await;
    db::set_setting(pool, "transmission_user", &config.transmission_user).await;
    db::set_setting(pool, "transmission_pass", &config.transmission_pass).await;
    db::set_setting(pool, "plex_url", &config.plex_url).await;
    db::set_setting(pool, "plex_token", &config.plex_token).await;
    db::set_setting(pool, "jellyfin_url", &config.jellyfin_url).await;
    db::set_setting(pool, "jellyfin_key", &config.jellyfin_key).await;
    db::set_setting(pool, "emby_url", &config.emby_url).await;
    db::set_setting(pool, "emby_key", &config.emby_key).await;
}

// ===================== System & Logs Handlers =====================

async fn get_system_logs() -> Result<String, AppError> {
    fs::read_to_string("data/app.log")
        .map_err(|e| (axum::http::StatusCode::NOT_FOUND, format!("Log file not found: {}", e)))
}

// ===================== Transmission Control Handlers =====================

async fn transmission_start_torrent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<axum::http::StatusCode, AppError> {
    let config = state.config.read().await;
    api::transmission::start_torrent(&state.client, &config.transmission_url, &config.transmission_user, &config.transmission_pass, id)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Transmission", "Torrent Started", &format!("ID: {}", id)).await;
    Ok(axum::http::StatusCode::OK)
}

async fn transmission_stop_torrent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<axum::http::StatusCode, AppError> {
    let config = state.config.read().await;
    api::transmission::stop_torrent(&state.client, &config.transmission_url, &config.transmission_user, &config.transmission_pass, id)
        .await.map_err(|e| internal_err(e))?;
    db::log_event(&state.db, "Transmission", "Torrent Stopped", &format!("ID: {}", id)).await;
    Ok(axum::http::StatusCode::OK)
}
