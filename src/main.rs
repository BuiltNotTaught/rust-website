//! A minimal Rust website template: Axum + Tera + SQLite.
//!
//! Everything site-specific is either in `templates/` or comes from environment
//! variables — you should not need to edit this file to launch, only to add pages.
//!
//! Quick start:
//!   cargo run                 # then open http://localhost:3000
//!
//! Configuration (all optional, sensible defaults):
//!   PORT                  port to listen on            (default 3000)
//!   TEMPLATE_DIR          where the templates live     (default "templates")
//!   STATIC_DIR            where static files live      (default "static")
//!   DATABASE_PATH         SQLite file                  (default "data.db")
//!   SITE_URL              absolute URL, used in robots.txt
//!   TURNSTILE_SITE_KEY    enables the contact-form captcha widget
//!   TURNSTILE_SECRET_KEY  enables server-side captcha verification
//!   SENDGRID_API_KEY      enables emailing contact submissions
//!   CONTACT_TO_EMAIL      where contact mail is sent
//!   CONTACT_FROM_EMAIL    verified sender address

mod db;
mod models;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware,
    response::Html,
    routing::get,
    Router,
};
use std::sync::Arc;
use tera::Tera;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

/// The 404 page. It is a plain HTML file (not a Tera template) so that it can
/// be served even if template loading is broken.
const NOT_FOUND_HTML: &str = include_str!("../static/404.html");

#[derive(Clone)]
struct Config {
    site_url: String,
    turnstile_site_key: String,
    turnstile_secret_key: String,
    sendgrid_api_key: String,
    contact_to: String,
    contact_from: String,
}

impl Config {
    fn from_env() -> Self {
        let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            site_url: get("SITE_URL", "https://example.com"),
            turnstile_site_key: get("TURNSTILE_SITE_KEY", ""),
            turnstile_secret_key: get("TURNSTILE_SECRET_KEY", ""),
            sendgrid_api_key: get("SENDGRID_API_KEY", ""),
            contact_to: get("CONTACT_TO_EMAIL", ""),
            contact_from: get("CONTACT_FROM_EMAIL", ""),
        }
    }
}

type AppState = (
    Arc<sqlx::SqlitePool>,
    Arc<Tera>,
    Arc<reqwest::Client>,
    Arc<Config>,
);

/// Adds caching and security headers to every response.
async fn headers_middleware(req: Request<Body>, next: middleware::Next) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let mut res = next.run(req).await;
    let h = res.headers_mut();

    // Static assets cache hard. HTML is deliberately left uncached so edits show
    // up immediately, and so a CDN in front does not serve stale pages.
    if path.ends_with(".css") || path.ends_with(".js") || path.ends_with(".woff2") {
        h.insert(
            "Cache-Control",
            "public, max-age=31536000, immutable".parse().unwrap(),
        );
    }

    // If you add third-party scripts, widen script-src accordingly.
    h.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline' https://challenges.cloudflare.com; \
         style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; \
         frame-src https://challenges.cloudflare.com; object-src 'none'; base-uri 'self'"
            .parse()
            .unwrap(),
    );
    h.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    h.insert("X-Frame-Options", "SAMEORIGIN".parse().unwrap());
    h.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    res
}

/// Renders a template, or returns a real error status.
///
/// This never returns HTTP 200 with an error body — a broken template must
/// surface as a 500 so monitoring actually catches it.
fn render(tera: &Tera, name: &str, ctx: &tera::Context) -> (StatusCode, Html<String>) {
    match tera.render(name, ctx) {
        Ok(html) => (StatusCode::OK, Html(html)),
        Err(e) => {
            tracing::error!("template '{}' failed to render: {:#}", name, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    "<h1>500</h1><p>Template <code>{}</code> failed to render. \
                     Check the server logs.</p>",
                    name
                )),
            )
        }
    }
}

/// A page with no data behind it, just a title.
fn simple_page(tera: &Tera, template: &str, title: &str) -> (StatusCode, Html<String>) {
    let mut ctx = tera::Context::new();
    ctx.insert("title", title);
    render(tera, template, &ctx)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt::init();
    let config = Config::from_env();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "data.db".to_string());
    let pool = db::init_db(&db_path)
        .await
        .expect("failed to initialise database");

    // Templates. Unlike a bare Tera::new(), this refuses to start when the glob
    // matched nothing: Tera treats "no files found" as success, which otherwise
    // yields a server that returns an error page for every single route.
    let template_dir = std::env::var("TEMPLATE_DIR").unwrap_or_else(|_| "templates".to_string());
    let glob = format!("{}/**/*.html", template_dir);
    let mut tera = Tera::new(&glob)
        .unwrap_or_else(|e| panic!("could not parse templates in '{}': {}", template_dir, e));
    if tera.get_template_names().count() == 0 {
        panic!(
            "no templates found in '{}'. Set TEMPLATE_DIR, or run from the project root.",
            template_dir
        );
    }
    tera.autoescape_on(vec![".html"]);
    println!(
        "loaded {} templates from '{}'",
        tera.get_template_names().count(),
        template_dir
    );

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());

    let state: AppState = (
        Arc::new(pool),
        Arc::new(tera),
        Arc::new(reqwest::Client::new()),
        Arc::new(config),
    );

    let app = Router::new()
        .route("/", get(home))
        .route("/about", get(about))
        .route("/work", get(work))
        .route("/blog", get(blog_index))
        .route("/blog/:slug", get(blog_post))
        .route("/contact", get(contact_form).post(contact_submit))
        // ---- PROJECT PAGES -------------------------------------------------
        // Add one line per project page. The template path is relative to
        // TEMPLATE_DIR, so "projects/example.html" means
        // templates/projects/example.html
        .route("/projects/example", get(project_example))
        // --------------------------------------------------------------------
        .route("/robots.txt", get(robots))
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn(headers_middleware))
        .with_state(state)
        .fallback(not_found);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("could not bind {}: {}", addr, e));

    println!("listening on http://{}", addr);
    axum::serve(listener, app).await.expect("server error");
}

// ---------------------------------------------------------------------------
// Static pages
// ---------------------------------------------------------------------------

async fn home(State((_, tera, _, _)): State<AppState>) -> (StatusCode, Html<String>) {
    simple_page(&tera, "home.html", "Home")
}

async fn about(State((_, tera, _, _)): State<AppState>) -> (StatusCode, Html<String>) {
    simple_page(&tera, "about.html", "About")
}

async fn work(State((_, tera, _, _)): State<AppState>) -> (StatusCode, Html<String>) {
    simple_page(&tera, "work.html", "Work")
}

async fn project_example(State((_, tera, _, _)): State<AppState>) -> (StatusCode, Html<String>) {
    simple_page(&tera, "projects/example.html", "Example Project")
}

// ---------------------------------------------------------------------------
// Blog
// ---------------------------------------------------------------------------

async fn blog_index(State((pool, tera, _, _)): State<AppState>) -> (StatusCode, Html<String>) {
    let posts: Vec<models::Post> = sqlx::query_as("SELECT * FROM posts ORDER BY created_at DESC")
        .fetch_all(pool.as_ref())
        .await
        .unwrap_or_default();

    let mut ctx = tera::Context::new();
    ctx.insert("title", "Blog");
    ctx.insert("posts", &posts);
    render(&tera, "blog.html", &ctx)
}

async fn blog_post(
    State((pool, tera, _, _)): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> (StatusCode, Html<String>) {
    let post: Option<models::Post> = sqlx::query_as("SELECT * FROM posts WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(pool.as_ref())
        .await
        .unwrap_or(None);

    match post {
        Some(p) => {
            let mut ctx = tera::Context::new();
            ctx.insert("title", &p.title);
            ctx.insert("post", &p);
            render(&tera, "post.html", &ctx)
        }
        // A missing post is a genuine 404, not a 200 with an apology on it.
        None => (StatusCode::NOT_FOUND, Html(NOT_FOUND_HTML.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Contact form
// ---------------------------------------------------------------------------

async fn contact_form(State((_, tera, _, cfg)): State<AppState>) -> (StatusCode, Html<String>) {
    let mut ctx = tera::Context::new();
    ctx.insert("title", "Contact");
    ctx.insert("turnstile_site_key", &cfg.turnstile_site_key);
    render(&tera, "contact.html", &ctx)
}

/// Verifies a Cloudflare Turnstile token. Returns true when verification is
/// disabled, so the form still works before you configure it.
async fn turnstile_ok(client: &reqwest::Client, cfg: &Config, token: &str) -> bool {
    if cfg.turnstile_secret_key.is_empty() {
        return true;
    }
    let res = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&[
            ("secret", cfg.turnstile_secret_key.as_str()),
            ("response", token),
        ])
        .send()
        .await;

    match res {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["success"].as_bool())
            .unwrap_or(false),
        Err(e) => {
            tracing::error!("turnstile verification failed: {}", e);
            false
        }
    }
}

async fn contact_submit(
    State((pool, tera, client, cfg)): State<AppState>,
    axum::Form(form): axum::Form<std::collections::HashMap<String, String>>,
) -> (StatusCode, Html<String>) {
    let field = |k: &str| form.get(k).cloned().unwrap_or_default();
    let (name, email, message) = (field("name"), field("email"), field("message"));

    if message.trim().is_empty() || message.len() > 1000 {
        return (
            StatusCode::BAD_REQUEST,
            Html(
                "<h1>400</h1><p>Message must be 1-1000 characters.</p><a href='/contact'>back</a>"
                    .to_string(),
            ),
        );
    }

    // Spam check runs before anything is stored or sent.
    if !turnstile_ok(&client, &cfg, &field("cf-turnstile-response")).await {
        return (
            StatusCode::BAD_REQUEST,
            Html(
                "<h1>400</h1><p>Captcha verification failed.</p><a href='/contact'>back</a>"
                    .to_string(),
            ),
        );
    }

    if let Err(e) = sqlx::query("INSERT INTO notes (body) VALUES (?)")
        .bind(format!(
            "Name: {}\nEmail: {}\nMessage: {}",
            name, email, message
        ))
        .execute(pool.as_ref())
        .await
    {
        tracing::error!("could not store contact message: {}", e);
    }

    // Email delivery is optional and never blocks the response.
    if !cfg.sendgrid_api_key.is_empty() && !cfg.contact_to.is_empty() {
        let (client, cfg) = (client.clone(), cfg.clone());
        let (n, em, msg) = (name.clone(), email.clone(), message.clone());
        tokio::spawn(async move {
            let body = serde_json::json!({
                "personalizations": [{ "to": [{"email": cfg.contact_to}] }],
                "from": { "email": cfg.contact_from },
                "subject": format!("Contact form: {}", n),
                "content": [{ "type": "text/plain",
                    "value": format!("Name: {}\nEmail: {}\n\n{}", n, em, msg) }]
            });
            if let Err(e) = client
                .post("https://api.sendgrid.com/v3/mail/send")
                .header("Authorization", format!("Bearer {}", cfg.sendgrid_api_key))
                .json(&body)
                .send()
                .await
            {
                tracing::error!("sendgrid error: {}", e);
            }
        });
    }

    simple_page(&tera, "thanks.html", "Thanks")
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

async fn robots(
    State((_, _, _, cfg)): State<AppState>,
) -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain")],
        format!(
            "User-agent: *\nAllow: /\n\nSitemap: {}/sitemap.xml\n",
            cfg.site_url
        ),
    )
}

/// Real 404 status. Returning 200 here ("soft 404") makes search engines index
/// every mistyped URL on the site.
async fn not_found() -> (StatusCode, Html<&'static str>) {
    (StatusCode::NOT_FOUND, Html(NOT_FOUND_HTML))
}
