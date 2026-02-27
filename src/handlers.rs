use anyhow::Context;
use axum::extract::{Form, Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::{require_user, server_error};
use crate::db;
use crate::models::{
    BindCertForm, Certificate, ChangePasswordForm, CreateSiteForm, LoginForm, RequestCertForm,
    ServiceControlForm, Site, ToggleSiteForm,
};
use crate::nginx_ops;

#[derive(Debug, Clone)]
struct CertReminder {
    domain: String,
    days_left: i64,
    expires_at: String,
}

#[derive(Debug, Clone)]
struct AuditLog {
    actor: String,
    action: String,
    detail: String,
    created_at: String,
}

pub async fn login_page() -> Html<String> {
    Html(layout(
        "登录",
        r#"
        <h1>Nginx 管理器</h1>
        <form method="post" action="/login">
            <label>用户名 <input name="username" required /></label>
            <label>密码 <input type="password" name="password" required /></label>
            <button type="submit">登录</button>
        </form>
    "#,
    ))
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    let row = match sqlx::query("SELECT id, password_hash FROM admin_users WHERE username = ?")
        .bind(&form.username)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("数据库错误"),
    };
    let Some(row) = row else {
        return (StatusCode::UNAUTHORIZED, "用户名或密码错误").into_response();
    };
    let id: i64 = row.get("id");
    let hash: String = row.get("password_hash");
    if !db::verify_password(&form.password, &hash) {
        return (StatusCode::UNAUTHORIZED, "用户名或密码错误").into_response();
    }

    let token = Uuid::new_v4().to_string();
    state.sessions.write().await.insert(token.clone(), id);
    db::audit(&state.pool, &form.username, "login", "管理员登录").await;
    let cookie = Cookie::build(("nm_session", token))
        .path("/")
        .http_only(true)
        .build();
    (jar.add(cookie), Redirect::to("/")).into_response()
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> Response {
    if let Some(c) = jar.get("nm_session") {
        state.sessions.write().await.remove(c.value());
    }
    let cookie = Cookie::build(("nm_session", "")).path("/").build();
    (jar.remove(cookie), Redirect::to("/login")).into_response()
}

pub async fn dashboard(State(state): State<AppState>, jar: CookieJar) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let reminders = match refresh_and_collect_expiring_certs(&state, 30).await {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };
    let sites: (i64,) = match sqlx::query_as("SELECT COUNT(*) FROM sites")
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("查询失败"),
    };
    let certs: (i64,) = match sqlx::query_as("SELECT COUNT(*) FROM certificates")
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("查询失败"),
    };
    let reminder_html = if reminders.is_empty() {
        "<p>未来 30 天内无即将到期证书。</p>".to_string()
    } else {
        let mut items = String::new();
        for r in &reminders {
            items.push_str(&format!(
                "<li>{} 将在 {} 天后过期（{}）</li>",
                r.domain, r.days_left, r.expires_at
            ));
        }
        format!("<ul>{items}</ul>")
    };
    let danger_7 = reminders.iter().filter(|r| r.days_left <= 7).count();
    let warn_15 = reminders.iter().filter(|r| r.days_left <= 15).count();
    let notice_30 = reminders.len();
    let body = format!(
        r#"
        {}
        <h1>仪表盘</h1>
        <div class="cards">
            <div class="card"><h3>站点数量</h3><p>{}</p></div>
            <div class="card"><h3>证书数量</h3><p>{}</p></div>
            <div class="card"><h3>7 天内到期</h3><p>{}</p></div>
            <div class="card"><h3>15 天内到期</h3><p>{}</p></div>
            <div class="card"><h3>30 天内到期</h3><p>{}</p></div>
        </div>
        <section class="card">
            <h3>证书到期提醒</h3>
            {}
        </section>
    "#,
        nav(),
        sites.0,
        certs.0,
        danger_7,
        warn_15,
        notice_30,
        reminder_html
    );
    Html(layout("仪表盘", &body)).into_response()
}

pub async fn list_sites(State(state): State<AppState>, jar: CookieJar) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }

    let sites: Vec<Site> = match sqlx::query_as("SELECT * FROM sites ORDER BY id DESC")
        .fetch_all(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("查询站点失败"),
    };
    let certs: Vec<Certificate> =
        match sqlx::query_as("SELECT * FROM certificates ORDER BY id DESC")
            .fetch_all(&state.pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return server_error("查询证书失败"),
        };

    let mut rows = String::new();
    for s in &sites {
        let status = if s.enabled == 1 {
            "已启用"
        } else {
            "已停用"
        };
        let cert_options = certs
            .iter()
            .map(|c| format!(r#"<option value="{}">{}</option>"#, c.id, c.name))
            .collect::<Vec<_>>()
            .join("");
        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>
                <td>
                    <form method="post" action="/sites/{}/toggle" style="display:inline">
                        <input type="hidden" name="enabled" value="{}" />
                        <button type="submit">{}</button>
                    </form>
                </td>
                <td>
                    <form method="post" action="/sites/{}/bind-cert" style="display:flex;gap:8px">
                        <select name="cert_id">{}</select>
                        <button type="submit">绑定</button>
                    </form>
                </td>
                <td>
                    <a href="/sites/{}/edit">编辑</a>
                    <form method="post" action="/sites/{}/delete" style="display:inline;margin-left:8px" onsubmit="return confirm('确认删除该站点吗？此操作不可恢复。')">
                        <button type="submit">删除</button>
                    </form>
                </td>
            </tr>"#,
            s.id,
            s.name,
            s.domain,
            s.site_type,
            s.port,
            status,
            s.id,
            if s.enabled == 1 { "false" } else { "true" },
            if s.enabled == 1 { "停用" } else { "启用" },
            s.id,
            cert_options,
            s.id,
            s.id
        ));
    }

    let body = format!(
        r#"
        {}
        <h1>站点管理</h1>
        <form method="post" action="/sites" class="form-grid">
            <label>站点名称 <input name="name" required /></label>
            <label>域名 <input name="domain" required /></label>
            <label>站点类型
                <select name="site_type">
                    <option value="proxy">反向代理</option>
                    <option value="static">静态站点</option>
                </select>
            </label>
            <label>代理目标地址（反向代理）<input name="upstream" placeholder="http://127.0.0.1:3000" /></label>
            <label>站点目录（静态站点）<input name="root_dir" placeholder="/var/www/html" /></label>
            <label>端口 <input name="port" type="number" value="80" /></label>
            <button type="submit">创建站点</button>
        </form>
        <p>系统会自动检查域名和端口冲突，并提示被哪个站点占用。</p>
        <table>
            <thead><tr><th>ID</th><th>名称</th><th>域名</th><th>类型</th><th>端口</th><th>状态</th><th>启停</th><th>证书</th><th>操作</th></tr></thead>
            <tbody>{}</tbody>
        </table>
    "#,
        nav(),
        rows
    );
    Html(layout("站点管理", &body)).into_response()
}

pub async fn create_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateSiteForm>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let port = i64::from(form.port);
    let site_type = form.site_type.trim().to_string();
    if site_type != "proxy" && site_type != "static" {
        return (
            StatusCode::BAD_REQUEST,
            "站点类型必须是“反向代理”或“静态站点”",
        )
            .into_response();
    }
    let upstream = normalize_opt(form.upstream.clone());
    let root_dir = normalize_opt(form.root_dir.clone());
    if site_type == "proxy" && upstream.is_none() {
        return (StatusCode::BAD_REQUEST, "反向代理站点必须填写代理目标地址").into_response();
    }
    if site_type == "static" && root_dir.is_none() {
        return (StatusCode::BAD_REQUEST, "静态站点必须填写站点目录").into_response();
    }

    let conflict_domain: Option<(i64, String)> =
        match sqlx::query_as("SELECT id, name FROM sites WHERE domain = ? LIMIT 1")
            .bind(&form.domain)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return server_error("域名冲突检查失败"),
        };
    if let Some((id, name)) = conflict_domain {
        return (
            StatusCode::BAD_REQUEST,
            format!("域名已被站点 #{id}（{name}）使用"),
        )
            .into_response();
    }

    let conflict_port: Option<(i64, String, String)> =
        match sqlx::query_as("SELECT id, name, domain FROM sites WHERE port = ? LIMIT 1")
            .bind(port)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return server_error("端口冲突检查失败"),
        };
    if let Some((id, name, domain)) = conflict_port {
        return (
            StatusCode::BAD_REQUEST,
            format!("端口 {port} 已被站点 #{id}（{name}, {domain}）使用"),
        )
            .into_response();
    }

    let inserted = sqlx::query(
        "INSERT INTO sites(name, domain, site_type, upstream, root_dir, port, enabled, created_at) VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
    )
    .bind(&form.name)
    .bind(&form.domain)
    .bind(&site_type)
    .bind(upstream.as_deref())
    .bind(root_dir.as_deref())
    .bind(port)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await;

    if inserted.is_err() {
        return (
            StatusCode::BAD_REQUEST,
            "站点创建失败（名称重复或输入无效）",
        )
            .into_response();
    }

    let site: Site = match sqlx::query_as("SELECT * FROM sites WHERE name = ?")
        .bind(&form.name)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("读取新建站点失败"),
    };
    if let Err(e) = nginx_ops::apply_site(&state.cfg, &site, None).await {
        return (StatusCode::BAD_REQUEST, format!("应用 Nginx 配置失败：{e}")).into_response();
    }
    db::audit(
        &state.pool,
        "admin",
        "创建站点",
        &format!("site={} domain={}", site.name, site.domain),
    )
    .await;
    Redirect::to("/sites").into_response()
}

pub async fn edit_site_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }

    let site: Site = match sqlx::query_as("SELECT * FROM sites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return (StatusCode::NOT_FOUND, "站点不存在").into_response(),
    };

    let body = format!(
        r#"
        {}
        <h1>编辑站点 #{}</h1>
        <form method="post" action="/sites/{}/update" class="form-grid">
            <label>站点名称 <input name="name" value="{}" required /></label>
            <label>域名 <input name="domain" value="{}" required /></label>
            <label>站点类型
                <select name="site_type">
                    <option value="proxy" {}>反向代理</option>
                    <option value="static" {}>静态站点</option>
                </select>
            </label>
            <label>代理目标地址（反向代理）<input name="upstream" value="{}" /></label>
            <label>站点目录（静态站点）<input name="root_dir" value="{}" /></label>
            <label>端口 <input name="port" type="number" value="{}" /></label>
            <button type="submit">保存修改</button>
        </form>
        <p><a href="/sites">返回列表</a></p>
    "#,
        nav(),
        site.id,
        site.id,
        site.name,
        site.domain,
        if site.site_type == "proxy" {
            "selected"
        } else {
            ""
        },
        if site.site_type == "static" {
            "selected"
        } else {
            ""
        },
        site.upstream.unwrap_or_default(),
        site.root_dir.unwrap_or_default(),
        site.port
    );
    Html(layout("编辑站点", &body)).into_response()
}

pub async fn update_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<i64>,
    Form(form): Form<CreateSiteForm>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }

    let existing: Site = match sqlx::query_as("SELECT * FROM sites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return (StatusCode::NOT_FOUND, "站点不存在").into_response(),
    };

    let port = i64::from(form.port);
    let site_type = form.site_type.trim().to_string();
    if site_type != "proxy" && site_type != "static" {
        return (
            StatusCode::BAD_REQUEST,
            "站点类型必须是“反向代理”或“静态站点”",
        )
            .into_response();
    }
    let upstream = normalize_opt(form.upstream.clone());
    let root_dir = normalize_opt(form.root_dir.clone());
    if site_type == "proxy" && upstream.is_none() {
        return (StatusCode::BAD_REQUEST, "反向代理站点必须填写代理目标地址").into_response();
    }
    if site_type == "static" && root_dir.is_none() {
        return (StatusCode::BAD_REQUEST, "静态站点必须填写站点目录").into_response();
    }

    let conflict_domain: Option<(i64, String)> =
        match sqlx::query_as("SELECT id, name FROM sites WHERE domain = ? AND id != ? LIMIT 1")
            .bind(&form.domain)
            .bind(id)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return server_error("域名冲突检查失败"),
        };
    if let Some((sid, name)) = conflict_domain {
        return (
            StatusCode::BAD_REQUEST,
            format!("域名已被站点 #{sid}（{name}）使用"),
        )
            .into_response();
    }

    let conflict_port: Option<(i64, String, String)> = match sqlx::query_as(
        "SELECT id, name, domain FROM sites WHERE port = ? AND id != ? LIMIT 1",
    )
    .bind(port)
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(v) => v,
        Err(_) => return server_error("端口冲突检查失败"),
    };
    if let Some((sid, name, domain)) = conflict_port {
        return (
            StatusCode::BAD_REQUEST,
            format!("端口 {port} 已被站点 #{sid}（{name}, {domain}）使用"),
        )
            .into_response();
    }

    if let Err(e) = sqlx::query(
        "UPDATE sites SET name = ?, domain = ?, site_type = ?, upstream = ?, root_dir = ?, port = ? WHERE id = ?",
    )
    .bind(&form.name)
    .bind(&form.domain)
    .bind(&site_type)
    .bind(upstream.as_deref())
    .bind(root_dir.as_deref())
    .bind(port)
    .bind(id)
    .execute(&state.pool)
    .await
    {
        return (StatusCode::BAD_REQUEST, format!("更新失败：{e}")).into_response();
    }

    let site: Site = match sqlx::query_as("SELECT * FROM sites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("读取更新后的站点失败"),
    };
    let cert = match site.cert_id {
        Some(cert_id) => {
            sqlx::query_as::<_, Certificate>("SELECT * FROM certificates WHERE id = ?")
                .bind(cert_id)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten()
        }
        None => None,
    };

    if let Err(e) = nginx_ops::apply_site(&state.cfg, &site, cert.as_ref()).await {
        let _ = sqlx::query(
            "UPDATE sites SET name = ?, domain = ?, site_type = ?, upstream = ?, root_dir = ?, port = ? WHERE id = ?",
        )
        .bind(existing.name)
        .bind(existing.domain)
        .bind(existing.site_type)
        .bind(existing.upstream)
        .bind(existing.root_dir)
        .bind(existing.port)
        .bind(id)
        .execute(&state.pool)
        .await;
        return (StatusCode::BAD_REQUEST, format!("应用 Nginx 配置失败：{e}")).into_response();
    }

    db::audit(
        &state.pool,
        "admin",
        "更新站点",
        &format!("site_id={id} name={} domain={}", form.name, form.domain),
    )
    .await;
    Redirect::to("/sites").into_response()
}

pub async fn delete_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<i64>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let site: Site = match sqlx::query_as("SELECT * FROM sites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return (StatusCode::NOT_FOUND, "站点不存在").into_response(),
    };
    if let Err(e) = nginx_ops::delete_site(&state.cfg, &site.name).await {
        return (StatusCode::BAD_REQUEST, format!("删除后应用配置失败：{e}")).into_response();
    }
    if let Err(e) = sqlx::query("DELETE FROM sites WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await
    {
        return (StatusCode::BAD_REQUEST, format!("删除数据库记录失败：{e}")).into_response();
    }
    db::audit(
        &state.pool,
        "admin",
        "删除站点",
        &format!("site_id={id} name={} domain={}", site.name, site.domain),
    )
    .await;
    Redirect::to("/sites").into_response()
}

pub async fn toggle_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<i64>,
    Form(form): Form<ToggleSiteForm>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let enabled = if form.enabled { 1_i64 } else { 0_i64 };
    if let Err(e) = sqlx::query("UPDATE sites SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(&state.pool)
        .await
    {
        return (StatusCode::BAD_REQUEST, format!("更新启用状态失败：{e}")).into_response();
    }

    let site: Site = match sqlx::query_as("SELECT * FROM sites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("站点不存在"),
    };
    let cert = match site.cert_id {
        Some(cert_id) => {
            sqlx::query_as::<_, Certificate>("SELECT * FROM certificates WHERE id = ?")
                .bind(cert_id)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten()
        }
        None => None,
    };
    if let Err(e) = nginx_ops::apply_site(&state.cfg, &site, cert.as_ref()).await {
        return (
            StatusCode::BAD_REQUEST,
            format!("切换启停后应用配置失败：{e}"),
        )
            .into_response();
    }
    Redirect::to("/sites").into_response()
}

pub async fn bind_cert(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(site_id): Path<i64>,
    Form(form): Form<BindCertForm>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }

    let cert: Certificate = match sqlx::query_as("SELECT * FROM certificates WHERE id = ?")
        .bind(form.cert_id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "证书不存在").into_response(),
    };
    if let Err(e) = sqlx::query("UPDATE sites SET cert_id = ? WHERE id = ?")
        .bind(cert.id)
        .bind(site_id)
        .execute(&state.pool)
        .await
    {
        return (StatusCode::BAD_REQUEST, format!("绑定证书失败：{e}")).into_response();
    }
    let site: Site = match sqlx::query_as("SELECT * FROM sites WHERE id = ?")
        .bind(site_id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "站点不存在").into_response(),
    };
    if let Err(e) = nginx_ops::apply_site(&state.cfg, &site, Some(&cert)).await {
        return (StatusCode::BAD_REQUEST, format!("绑定后应用配置失败：{e}")).into_response();
    }
    Redirect::to("/sites").into_response()
}

pub async fn list_certs(State(state): State<AppState>, jar: CookieJar) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let _ = refresh_cert_expiry_metadata(&state).await;
    let certs: Vec<Certificate> =
        match sqlx::query_as("SELECT * FROM certificates ORDER BY id DESC")
            .fetch_all(&state.pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return server_error("查询证书失败"),
        };
    let mut rows = String::new();
    for c in certs {
        let expires = c.expires_at.clone().unwrap_or_else(|| "未知".to_string());
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            c.id, c.name, c.domain, c.issuer, expires, c.cert_path
        ));
    }
    let body = format!(
        r#"
        {}
        <h1>证书管理</h1>
        <h2>上传已有证书</h2>
        <form method="post" action="/certs/upload" enctype="multipart/form-data" class="form-grid">
            <label>证书名称 <input name="name" required /></label>
            <label>域名 <input name="domain" required /></label>
            <label>CRT 文件 <input type="file" name="cert_file" required /></label>
            <label>KEY 文件 <input type="file" name="key_file" required /></label>
            <button type="submit">上传</button>
        </form>
        <h2>Let's Encrypt 申请（HTTP-01）</h2>
        <form method="post" action="/certs/request" class="form-grid">
            <label>证书名称 <input name="name" required /></label>
            <label>域名 <input name="domain" required /></label>
            <label>邮箱 <input name="email" required /></label>
            <button type="submit">申请证书</button>
        </form>
        <table>
            <thead><tr><th>ID</th><th>名称</th><th>域名</th><th>签发方</th><th>到期时间</th><th>证书路径</th></tr></thead>
            <tbody>{}</tbody>
        </table>
    "#,
        nav(),
        rows
    );
    Html(layout("证书管理", &body)).into_response()
}

pub async fn upload_cert(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }

    let mut name = String::new();
    let mut domain = String::new();
    let mut cert_bytes: Option<Vec<u8>> = None;
    let mut key_bytes: Option<Vec<u8>> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(v) => v,
            Err(_) => return (StatusCode::BAD_REQUEST, "上传表单格式错误").into_response(),
        };
        let Some(field) = field else { break };
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "name" => {
                name = field.text().await.unwrap_or_default();
            }
            "domain" => {
                domain = field.text().await.unwrap_or_default();
            }
            "cert_file" => {
                cert_bytes = field.bytes().await.ok().map(|b| b.to_vec());
            }
            "key_file" => {
                key_bytes = field.bytes().await.ok().map(|b| b.to_vec());
            }
            _ => {}
        }
    }

    if name.is_empty() || domain.is_empty() || cert_bytes.is_none() || key_bytes.is_none() {
        return (StatusCode::BAD_REQUEST, "缺少必填字段").into_response();
    }

    if let Err(e) = tokio::fs::create_dir_all(&state.cfg.managed_cert_dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("创建证书目录失败：{e}"),
        )
            .into_response();
    }
    let cert_path = format!("{}/{}.crt", &state.cfg.managed_cert_dir, name);
    let key_path = format!("{}/{}.key", &state.cfg.managed_cert_dir, name);
    if let Err(e) = write_cert_files(
        &cert_path,
        cert_bytes.unwrap(),
        &key_path,
        key_bytes.unwrap(),
    )
    .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("写入证书文件失败：{e}"),
        )
            .into_response();
    }

    let res = sqlx::query(
        "INSERT INTO certificates(name, domain, cert_path, key_path, issuer, expires_at, auto_managed, created_at) VALUES (?, ?, ?, ?, 'uploaded', NULL, 0, ?)",
    )
    .bind(&name)
    .bind(&domain)
    .bind(&cert_path)
    .bind(&key_path)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        return (StatusCode::BAD_REQUEST, format!("保存证书记录失败：{e}")).into_response();
    }
    db::audit(
        &state.pool,
        "admin",
        "上传证书",
        &format!("name={name} domain={domain}"),
    )
    .await;
    Redirect::to("/certs").into_response()
}

pub async fn request_cert(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RequestCertForm>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    if let Err(e) = nginx_ops::request_letsencrypt(&state.cfg, &form.domain, &form.email).await {
        return (StatusCode::BAD_REQUEST, format!("证书申请失败：{e}")).into_response();
    }

    let (cert_path, key_path) = nginx_ops::letsencrypt_paths(&form.domain);
    let res = sqlx::query(
        "INSERT INTO certificates(name, domain, cert_path, key_path, issuer, expires_at, auto_managed, created_at) VALUES (?, ?, ?, ?, 'letsencrypt', NULL, 1, ?)",
    )
    .bind(&form.name)
    .bind(&form.domain)
    .bind(cert_path)
    .bind(key_path)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        return (StatusCode::BAD_REQUEST, format!("保存证书记录失败：{e}")).into_response();
    }
    db::audit(
        &state.pool,
        "admin",
        "申请证书",
        &format!("name={} domain={}", form.name, form.domain),
    )
    .await;
    Redirect::to("/certs").into_response()
}

pub async fn service_page(State(state): State<AppState>, jar: CookieJar) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let body = format!(
        r#"
        {}
        <h1>Nginx 服务控制</h1>
        <form method="post" action="/service/control">
            <select name="action">
                <option value="status">查看状态</option>
                <option value="start">启动</option>
                <option value="stop">停止</option>
                <option value="restart">重启</option>
                <option value="reload">重载</option>
            </select>
            <button type="submit">执行</button>
        </form>
    "#,
        nav(),
    );
    Html(layout("服务控制", &body)).into_response()
}

pub async fn service_control(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ServiceControlForm>,
) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let result = nginx_ops::service_control(&state.cfg, &form.action).await;
    match result {
        Ok(out) => {
            Html(layout("执行结果", &format!("{}<pre>{}</pre>", nav(), out))).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, format!("命令执行失败：{e}")).into_response(),
    }
}

pub async fn settings_page(State(state): State<AppState>, jar: CookieJar) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let body = format!(
        r#"
        {}
        <h1>系统设置</h1>
        <form method="post" action="/settings/password" class="form-grid">
            <label>当前密码 <input type="password" name="current_password" required /></label>
            <label>新密码 <input type="password" name="new_password" required /></label>
            <button type="submit">修改密码</button>
        </form>
    "#,
        nav()
    );
    Html(layout("系统设置", &body)).into_response()
}

pub async fn logs_page(State(state): State<AppState>, jar: CookieJar) -> Response {
    if require_user(&jar, &state).await.is_err() {
        return Redirect::to("/login").into_response();
    }
    let rows = match sqlx::query(
        "SELECT actor, action, detail, created_at FROM audit_logs ORDER BY id DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(v) => v,
        Err(_) => return server_error("查询日志失败"),
    };
    let logs = rows
        .into_iter()
        .map(|r| AuditLog {
            actor: r.get("actor"),
            action: r.get("action"),
            detail: r.get("detail"),
            created_at: r.get("created_at"),
        })
        .collect::<Vec<_>>();

    let mut html_rows = String::new();
    for l in logs {
        let action = action_to_cn(&l.action);
        html_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            l.created_at, l.actor, action, l.detail
        ));
    }

    let body = format!(
        r#"
        {}
        <h1>操作日志</h1>
        <table>
            <thead><tr><th>时间</th><th>操作者</th><th>动作</th><th>详情</th></tr></thead>
            <tbody>{}</tbody>
        </table>
    "#,
        nav(),
        html_rows
    );
    Html(layout("操作日志", &body)).into_response()
}

pub async fn change_password(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    let user_id = match require_user(&jar, &state).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if form.new_password.len() < 8 {
        return (StatusCode::BAD_REQUEST, "新密码长度至少 8 位").into_response();
    }

    let row = match sqlx::query("SELECT username, password_hash FROM admin_users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(v) => v,
        Err(_) => return server_error("查询当前用户失败"),
    };
    let username: String = row.get("username");
    let current_hash: String = row.get("password_hash");
    if !db::verify_password(&form.current_password, &current_hash) {
        return (StatusCode::BAD_REQUEST, "当前密码不正确").into_response();
    }

    let new_hash = match db::hash_password(&form.new_password) {
        Ok(v) => v,
        Err(_) => return server_error("密码加密失败"),
    };
    if let Err(e) = sqlx::query("UPDATE admin_users SET password_hash = ? WHERE id = ?")
        .bind(new_hash)
        .bind(user_id)
        .execute(&state.pool)
        .await
    {
        return (StatusCode::BAD_REQUEST, format!("更新密码失败：{e}")).into_response();
    }
    db::audit(&state.pool, &username, "修改密码", "管理员修改密码").await;
    Html(layout(
        "系统设置",
        &format!("{}<p>密码修改成功，请妥善保管。</p>", nav()),
    ))
    .into_response()
}

async fn refresh_cert_expiry_metadata(state: &AppState) -> anyhow::Result<()> {
    let certs: Vec<Certificate> = sqlx::query_as("SELECT * FROM certificates")
        .fetch_all(&state.pool)
        .await?;
    for cert in certs {
        let expiry = nginx_ops::read_cert_expiry(&cert.cert_path).await?;
        if let Some((expires_at, _)) = expiry {
            let _ = sqlx::query("UPDATE certificates SET expires_at = ? WHERE id = ?")
                .bind(expires_at)
                .bind(cert.id)
                .execute(&state.pool)
                .await;
        }
    }
    Ok(())
}

async fn refresh_and_collect_expiring_certs(
    state: &AppState,
    within_days: i64,
) -> anyhow::Result<Vec<CertReminder>> {
    let certs: Vec<Certificate> = sqlx::query_as("SELECT * FROM certificates")
        .fetch_all(&state.pool)
        .await?;
    let mut reminders = Vec::new();
    for cert in certs {
        let expiry = nginx_ops::read_cert_expiry(&cert.cert_path).await?;
        if let Some((expires_at, days_left)) = expiry {
            let _ = sqlx::query("UPDATE certificates SET expires_at = ? WHERE id = ?")
                .bind(&expires_at)
                .bind(cert.id)
                .execute(&state.pool)
                .await;
            if days_left <= within_days {
                reminders.push(CertReminder {
                    domain: cert.domain,
                    days_left,
                    expires_at,
                });
            }
        }
    }
    reminders.sort_by_key(|r| r.days_left);
    Ok(reminders)
}

async fn write_cert_files(
    cert_path: &str,
    cert_bytes: Vec<u8>,
    key_path: &str,
    key_bytes: Vec<u8>,
) -> anyhow::Result<()> {
    tokio::fs::write(cert_path, cert_bytes)
        .await
        .with_context(|| format!("写入文件失败：{cert_path}"))?;
    tokio::fs::write(key_path, key_bytes)
        .await
        .with_context(|| format!("写入文件失败：{key_path}"))?;
    Ok(())
}

fn nav() -> &'static str {
    r#"
    <nav>
        <a href="/">仪表盘</a>
        <a href="/sites">站点管理</a>
        <a href="/certs">证书管理</a>
        <a href="/service">服务控制</a>
        <a href="/settings">系统设置</a>
        <a href="/logs">操作日志</a>
        <form method="post" action="/logout" style="display:inline"><button type="submit">退出登录</button></form>
    </nav>
    "#
}

fn normalize_opt(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn action_to_cn(action: &str) -> &str {
    match action {
        "login" => "登录",
        "create_site" => "创建站点",
        "update_site" => "更新站点",
        "delete_site" => "删除站点",
        "upload_cert" => "上传证书",
        "request_cert" => "申请证书",
        "change_password" => "修改密码",
        other => other,
    }
}

fn layout(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{}</title>
  <style>
    :root {{ --bg:#f5f7fb; --card:#ffffff; --text:#202733; --accent:#1456d1; --line:#d9e0ef; }}
    body {{ margin:0; font-family: "Segoe UI", sans-serif; background:linear-gradient(160deg,#eef3ff,#f8fbff); color:var(--text); }}
    nav {{ display:flex; gap:12px; align-items:center; padding:12px 18px; background:var(--card); border-bottom:1px solid var(--line); }}
    nav a {{ color:var(--accent); text-decoration:none; font-weight:600; }}
    main {{ max-width:1080px; margin:20px auto; padding:0 14px; }}
    .cards {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:12px; }}
    .card {{ background:var(--card); border:1px solid var(--line); border-radius:14px; padding:14px; }}
    form {{ margin:12px 0; background:var(--card); border:1px solid var(--line); border-radius:14px; padding:12px; }}
    .form-grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(220px,1fr)); gap:10px; }}
    label {{ display:flex; flex-direction:column; gap:6px; font-size:14px; }}
    input, select, button {{ border:1px solid var(--line); border-radius:8px; padding:8px; }}
    button {{ background:var(--accent); color:white; border:none; cursor:pointer; }}
    table {{ width:100%; border-collapse:collapse; background:var(--card); border:1px solid var(--line); border-radius:14px; overflow:hidden; }}
    th, td {{ border-bottom:1px solid var(--line); padding:10px; text-align:left; font-size:14px; }}
  </style>
</head>
<body>
<main>
{}
</main>
</body>
</html>"#,
        title, body
    )
}
