use serde::Deserialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Site {
    pub id: i64,
    pub name: String,
    pub domain: String,
    pub site_type: String,
    pub upstream: Option<String>,
    pub root_dir: Option<String>,
    pub port: i64,
    pub enabled: i64,
    pub cert_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Certificate {
    pub id: i64,
    pub name: String,
    pub domain: String,
    pub cert_path: String,
    pub key_path: String,
    pub issuer: String,
    pub expires_at: Option<String>,
    pub auto_managed: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSiteForm {
    pub name: String,
    pub domain: String,
    pub site_type: String,
    pub upstream: Option<String>,
    pub root_dir: Option<String>,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct ToggleSiteForm {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct BindCertForm {
    pub cert_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct RequestCertForm {
    pub name: String,
    pub domain: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ServiceControlForm {
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
}
