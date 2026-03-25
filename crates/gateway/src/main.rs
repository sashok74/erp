#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Gateway — HTTP-сервер, собирающий все Bounded Contexts.
//!
//! Единственный binary в системе. Запуск: `cargo run -p gateway`.

mod config;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use serde::Deserialize;
use event_bus::EventBus;
use tracing::info;

use config::AppConfig;

#[tokio::main]
async fn main() {
    // 1. Tracing
    tracing_subscriber::fmt::init();

    // 2. Config
    let config = AppConfig::from_env();
    info!(addr = %config.listen_addr, "starting ERP Gateway");

    // 3. Database pool
    let pool = Arc::new(
        db::PgPool::new(&config.database_url).expect("PgPool creation failed"),
    );
    pool.health_check().await.expect("DB health check failed");
    info!("database connection OK");

    // 4. Migrations (idempotent)
    db::migrate::run_migrations(&pool, "migrations/common")
        .await
        .expect("common migrations failed");
    db::migrate::run_migrations(&pool, "migrations/warehouse")
        .await
        .expect("warehouse migrations failed");
    db::migrate::run_migrations(&pool, "migrations/catalog")
        .await
        .expect("catalog migrations failed");

    // 5. Infrastructure services
    let bus = Arc::new(event_bus::InProcessBus::new());
    let jwt_service = Arc::new(auth::JwtService::new(
        &config.jwt_secret,
        chrono::Duration::hours(8),
    ));
    let perm_map = auth::rbac::default_erp_permissions();
    let checker = Arc::new(auth::JwtPermissionChecker::new(perm_map));
    let audit_log = Arc::new(audit::PgAuditLog::new(pool.clone()));
    let extensions = Arc::new(runtime::stubs::NoopExtensionHooks);
    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));

    // 6. Command Pipeline
    let pipeline = Arc::new(runtime::CommandPipeline::new(
        uow_factory,
        bus.clone(),
        checker,
        extensions,
        audit_log,
    ));

    // 6a. Register event handlers
    let product_handler =
        warehouse::infrastructure::event_handlers::ProductCreatedHandler::new(pool.clone());
    let adapter = Arc::new(event_bus::EventHandlerAdapter::new(product_handler));
    bus.subscribe("erp.catalog.product_created.v1", adapter)
        .await;

    // 7. Outbox Relay (background task)
    let relay = db::OutboxRelay::new(
        pool.clone(),
        bus,
        Duration::from_millis(config.relay_poll_ms),
        config.relay_batch_size,
    );
    tokio::spawn(async move {
        relay.run().await.ok();
    });

    // 8. Router
    let app = build_router(pipeline, pool, jwt_service);

    // 9. Serve
    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("bind failed");
    info!(addr = %config.listen_addr, "ERP Gateway listening");
    axum::serve(listener, app).await.expect("server error");
}

/// Собрать Router: /health + /api/{bc} с auth middleware + опционально /dev/token.
fn build_router(
    pipeline: Arc<runtime::CommandPipeline<db::PgUnitOfWorkFactory>>,
    pool: Arc<db::PgPool>,
    jwt_service: Arc<auth::JwtService>,
) -> Router {
    // BC routes
    let warehouse = warehouse::module::WarehouseModule::routes(pipeline.clone(), pool.clone());
    let catalog = catalog::module::CatalogModule::routes(pipeline, pool);

    // API routes — все BC под /api/{bc_name}, protected by JWT
    let jwt_for_middleware = jwt_service.clone();
    let api = Router::new()
        .nest("/warehouse", warehouse)
        .nest("/catalog", catalog)
        .layer(axum::middleware::from_fn(move |req, next| {
            let svc = jwt_for_middleware.clone();
            async move { auth::auth_middleware(req, next, svc).await }
        }));

    // Root router
    let mut root = Router::new()
        .nest("/api", api)
        .route("/health", routing::get(health));

    // Dev token endpoint (only when DEV_MODE is set)
    if std::env::var("DEV_MODE").is_ok() {
        info!("DEV_MODE enabled: POST /dev/token available");
        let dev = Router::new()
            .route("/dev/token", routing::post(dev_issue_token))
            .with_state(jwt_service);
        root = root.merge(dev);
    }

    root
}

/// Health check — без auth, всегда 200.
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ─── Dev token endpoint ─────────────────────────────────────────────────────

/// Тело запроса для dev token.
#[derive(Deserialize)]
struct DevTokenRequest {
    tenant_id: uuid::Uuid,
    roles: Vec<String>,
}

/// POST /dev/token — выдать JWT для тестирования (только `DEV_MODE`).
async fn dev_issue_token(
    State(jwt): State<Arc<auth::JwtService>>,
    Json(body): Json<DevTokenRequest>,
) -> impl IntoResponse {
    let tenant_id = kernel::types::TenantId::from_uuid(body.tenant_id);
    let user_id = kernel::types::UserId::new();
    let roles: Vec<auth::Role> = body
        .roles
        .iter()
        .filter_map(|r| auth::Role::from_str_opt(r))
        .collect();

    match jwt.issue(&user_id, &tenant_id, roles) {
        Ok(token) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({
                "token": token,
                "user_id": user_id.as_uuid().to_string(),
                "tenant_id": tenant_id.as_uuid().to_string(),
            })),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
