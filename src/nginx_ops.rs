use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, Utc};
use tokio::process::Command;

use crate::app_state::AppConfig;
use crate::models::{Certificate, Site};

pub async fn service_control(cfg: &AppConfig, action: &str) -> anyhow::Result<String> {
    let allowed = ["start", "stop", "restart", "reload", "status"];
    if !allowed.contains(&action) {
        bail!("不支持的服务操作");
    }
    let output = Command::new(&cfg.systemctl_bin)
        .arg(action)
        .arg("nginx")
        .output()
        .await
        .context("执行 systemctl 失败")?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        text.push_str(&format!("\n{err}"));
        bail!(text);
    }
    Ok(text)
}

pub async fn apply_site(
    cfg: &AppConfig,
    site: &Site,
    cert: Option<&Certificate>,
) -> anyhow::Result<()> {
    if site.domain.trim().is_empty() {
        bail!("域名不能为空");
    }

    let available_file = Path::new(&cfg.nginx_sites_available).join(format!("{}.conf", site.name));
    let enabled_link = Path::new(&cfg.nginx_sites_enabled).join(format!("{}.conf", site.name));

    tokio::fs::create_dir_all(&cfg.nginx_sites_available)
        .await
        .ok();
    tokio::fs::create_dir_all(&cfg.nginx_sites_enabled)
        .await
        .ok();

    let old_conf = tokio::fs::read(&available_file).await.ok();
    let old_enabled = enabled_link.exists();

    let content = render_site_config(site, cert)?;
    let tmp_path = available_file.with_extension("conf.tmp");
    tokio::fs::write(&tmp_path, content.as_bytes())
        .await
        .context("写入临时 Nginx 配置失败")?;
    tokio::fs::rename(&tmp_path, &available_file)
        .await
        .context("替换 Nginx 配置失败")?;

    set_enabled_link(&available_file, &enabled_link, site.enabled == 1).await?;

    if let Err(e) = validate_and_reload(cfg).await {
        if let Some(bytes) = old_conf {
            let _ = tokio::fs::write(&available_file, bytes).await;
        } else {
            let _ = tokio::fs::remove_file(&available_file).await;
        }
        let _ = set_enabled_link(&available_file, &enabled_link, old_enabled).await;
        return Err(e);
    }

    Ok(())
}

pub async fn delete_site(cfg: &AppConfig, site_name: &str) -> anyhow::Result<()> {
    let available_file = Path::new(&cfg.nginx_sites_available).join(format!("{site_name}.conf"));
    let enabled_link = Path::new(&cfg.nginx_sites_enabled).join(format!("{site_name}.conf"));

    let old_conf = tokio::fs::read(&available_file).await.ok();
    let old_enabled = enabled_link.exists();

    if enabled_link.exists() {
        tokio::fs::remove_file(&enabled_link)
            .await
            .context("删除已启用链接失败")?;
    }
    if available_file.exists() {
        tokio::fs::remove_file(&available_file)
            .await
            .context("删除站点配置文件失败")?;
    }

    if let Err(e) = validate_and_reload(cfg).await {
        if let Some(bytes) = old_conf {
            let _ = tokio::fs::write(&available_file, bytes).await;
        }
        let _ = set_enabled_link(&available_file, &enabled_link, old_enabled).await;
        return Err(e);
    }

    Ok(())
}

pub async fn request_letsencrypt(cfg: &AppConfig, domain: &str, email: &str) -> anyhow::Result<()> {
    let output = Command::new(&cfg.certbot_bin)
        .args([
            "certonly",
            "--nginx",
            "--non-interactive",
            "--agree-tos",
            "-m",
            email,
            "-d",
            domain,
        ])
        .output()
        .await
        .context("执行 certbot 失败")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("certbot 执行失败：{err}");
    }
    Ok(())
}

pub fn letsencrypt_paths(domain: &str) -> (String, String) {
    (
        format!("/etc/letsencrypt/live/{domain}/fullchain.pem"),
        format!("/etc/letsencrypt/live/{domain}/privkey.pem"),
    )
}

pub async fn read_cert_expiry(cert_path: &str) -> anyhow::Result<Option<(String, i64)>> {
    let output = Command::new("openssl")
        .args(["x509", "-enddate", "-noout", "-in", cert_path])
        .output()
        .await
        .context("执行 openssl 读取证书到期时间失败")?;
    if !output.status.success() {
        return Ok(None);
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let Some(raw) = text.strip_prefix("notAfter=") else {
        return Ok(None);
    };
    let expires = DateTime::parse_from_str(raw.trim(), "%b %e %H:%M:%S %Y GMT")
        .context("解析证书到期时间失败")?;
    let expires_utc = expires.with_timezone(&Utc);
    let now = Utc::now();
    let days_left = (expires_utc - now).num_days();
    Ok(Some((expires_utc.to_rfc3339(), days_left)))
}

fn render_site_config(site: &Site, cert: Option<&Certificate>) -> anyhow::Result<String> {
    let mut server_block = format!(
        "server {{\n    listen {};\n    server_name {};\n",
        site.port, site.domain
    );

    match site.site_type.as_str() {
        "proxy" => {
            let upstream = site
                .upstream
                .as_deref()
                .ok_or_else(|| anyhow!("反向代理站点缺少 upstream"))?;
            server_block.push_str(&format!(
                "    location / {{\n        proxy_set_header Host $host;\n        proxy_set_header X-Real-IP $remote_addr;\n        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n        proxy_set_header X-Forwarded-Proto $scheme;\n        proxy_pass {};\n    }}\n",
                upstream
            ));
        }
        "static" => {
            let root = site
                .root_dir
                .as_deref()
                .ok_or_else(|| anyhow!("静态站点缺少 root_dir"))?;
            server_block.push_str(&format!(
                "    root {};\n    index index.html index.htm;\n",
                root
            ));
        }
        _ => bail!("无效的站点类型"),
    }

    if let Some(c) = cert {
        server_block.push_str(&format!(
            "    listen 443 ssl;\n    ssl_certificate {};\n    ssl_certificate_key {};\n",
            c.cert_path, c.key_path
        ));
    }
    server_block.push_str("}\n");
    Ok(server_block)
}

async fn validate_and_reload(cfg: &AppConfig) -> anyhow::Result<()> {
    let test = Command::new(&cfg.nginx_bin)
        .arg("-t")
        .output()
        .await
        .context("执行 nginx -t 失败")?;
    if !test.status.success() {
        let err = String::from_utf8_lossy(&test.stderr);
        bail!("nginx -t 校验失败：{err}");
    }
    let output = Command::new(&cfg.systemctl_bin)
        .args(["reload", "nginx"])
        .output()
        .await
        .context("重载 Nginx 失败")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("重载失败：{err}");
    }
    Ok(())
}

async fn set_enabled_link(available: &Path, enabled: &Path, on: bool) -> anyhow::Result<()> {
    if on {
        if enabled.exists() {
            return Ok(());
        }
        create_symlink(available.to_path_buf(), enabled.to_path_buf()).await?;
    } else if enabled.exists() {
        tokio::fs::remove_file(enabled)
            .await
            .context("删除启用链接失败")?;
    }
    Ok(())
}

#[cfg(unix)]
async fn create_symlink(src: PathBuf, dst: PathBuf) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || std::os::unix::fs::symlink(src, dst))
        .await
        .context("创建软链接任务失败")?
        .context("创建软链接失败")?;
    Ok(())
}

#[cfg(not(unix))]
async fn create_symlink(src: PathBuf, dst: PathBuf) -> anyhow::Result<()> {
    tokio::fs::copy(src, dst)
        .await
        .context("复制配置文件失败")?;
    Ok(())
}
