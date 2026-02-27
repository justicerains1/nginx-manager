use anyhow::Context;
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use chrono::Utc;
use rand::rngs::OsRng;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

pub async fn connect(database_url: &str) -> anyhow::Result<Pool<Sqlite>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .context("连接 SQLite 失败")?;
    Ok(pool)
}

pub async fn init(pool: &Pool<Sqlite>) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS admin_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS certificates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            domain TEXT NOT NULL,
            cert_path TEXT NOT NULL,
            key_path TEXT NOT NULL,
            issuer TEXT NOT NULL,
            expires_at TEXT,
            auto_managed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sites (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            domain TEXT NOT NULL UNIQUE,
            site_type TEXT NOT NULL,
            upstream TEXT,
            root_dir TEXT,
            port INTEGER NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            cert_id INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY(cert_id) REFERENCES certificates(id)
        );

        CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            detail TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .context("创建数据表失败")?;

    let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin_users")
        .fetch_one(pool)
        .await
        .context("统计管理员用户失败")?;
    if cnt.0 == 0 {
        let username =
            std::env::var("NGINX_MANAGER_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let password =
            std::env::var("NGINX_MANAGER_ADMIN_PASS").unwrap_or_else(|_| "admin123!".to_string());
        let hash = hash_password(&password)?;
        sqlx::query(
            "INSERT INTO admin_users(username, password_hash, created_at) VALUES (?, ?, ?)",
        )
        .bind(username)
        .bind(hash)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .context("初始化管理员用户失败")?;
    }

    Ok(())
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("密码加密失败：{e}"))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(v) => v,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub async fn audit(pool: &Pool<Sqlite>, actor: &str, action: &str, detail: &str) {
    let _ = sqlx::query(
        "INSERT INTO audit_logs(actor, action, detail, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(actor)
    .bind(action)
    .bind(detail)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await;
}
