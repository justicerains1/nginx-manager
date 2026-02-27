use std::{collections::HashMap, sync::Arc};

use sqlx::{Pool, Sqlite};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub nginx_bin: String,
    pub systemctl_bin: String,
    pub certbot_bin: String,
    pub nginx_sites_available: String,
    pub nginx_sites_enabled: String,
    pub managed_cert_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("NGINX_MANAGER_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            database_url: std::env::var("NGINX_MANAGER_DB")
                .unwrap_or_else(|_| "sqlite:///var/lib/nginx-manager/manager.db".to_string()),
            nginx_bin: std::env::var("NGINX_BIN").unwrap_or_else(|_| "nginx".to_string()),
            systemctl_bin: std::env::var("SYSTEMCTL_BIN")
                .unwrap_or_else(|_| "systemctl".to_string()),
            certbot_bin: std::env::var("CERTBOT_BIN").unwrap_or_else(|_| "certbot".to_string()),
            nginx_sites_available: std::env::var("NGINX_SITES_AVAILABLE")
                .unwrap_or_else(|_| "/etc/nginx/sites-available".to_string()),
            nginx_sites_enabled: std::env::var("NGINX_SITES_ENABLED")
                .unwrap_or_else(|_| "/etc/nginx/sites-enabled".to_string()),
            managed_cert_dir: std::env::var("NGINX_MANAGER_CERT_DIR")
                .unwrap_or_else(|_| "/etc/nginx-manager/certs".to_string()),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub cfg: AppConfig,
    pub pool: Pool<Sqlite>,
    pub sessions: Arc<RwLock<HashMap<String, i64>>>,
}
