use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::env;

pub async fn init_db() -> SqlitePool {
    let database_path = env::var("DATABASE_PATH")
        .unwrap_or_else(|_| "/app/data/media_dashboard.db".to_string());

    // Ensure the data directory exists — try absolute path first (container), then relative (local dev)
    let _ = tokio::fs::create_dir_all("/app/data").await;
    let _ = tokio::fs::create_dir_all("data").await;

    let opts = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("Failed to connect to database");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            service TEXT NOT NULL,
            action TEXT NOT NULL,
            details TEXT NOT NULL
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create audit_logs table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS login_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            username TEXT NOT NULL,
            ip_address TEXT NOT NULL,
            success BOOLEAN NOT NULL
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create login_events table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS dashboard_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create dashboard_settings table");

    pool
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) {
    let _ = sqlx::query("INSERT OR REPLACE INTO dashboard_settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await;
}

pub async fn get_setting(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM dashboard_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub timestamp: String,
    pub service: String,
    pub action: String,
    pub details: String,
}

pub async fn log_event(pool: &SqlitePool, service: &str, action: &str, details: &str) {
    let _ = sqlx::query("INSERT INTO audit_logs (service, action, details) VALUES (?, ?, ?)")
        .bind(service)
        .bind(action)
        .bind(details)
        .execute(pool)
        .await;
}

pub async fn log_login(pool: &SqlitePool, username: &str, ip_address: &str, success: bool) {
    let _ = sqlx::query("INSERT INTO login_events (username, ip_address, success) VALUES (?, ?, ?)")
        .bind(username)
        .bind(ip_address)
        .bind(success)
        .execute(pool)
        .await;
}
