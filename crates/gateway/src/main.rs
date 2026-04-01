#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Gateway — composition root assembling all Bounded Contexts.
//!
//! Single binary in the system. Run: `cargo run -p gateway`.
//! Gateway doesn't know about specific BC handlers — it mounts
//! modular entrypoints via [`AppBuilder`](app_builder::AppBuilder).

mod app_builder;
mod config;
mod dev_endpoints;

use std::sync::Arc;
use std::time::Duration;

use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use kernel::security::PermissionRegistrar;
use tokio_util::sync::CancellationToken;
use tracing::info;

use app_builder::AppBuilder;
use config::AppConfig;
use dev_endpoints::{DevTokenState, dev_token_router};

#[tokio::main]
async fn main() {
    // 1. Tracing
    tracing_subscriber::fmt::init();

    // 2. Config
    let config = AppConfig::from_env();
    info!(addr = %config.listen_addr, "starting ERP Gateway");

    // 3. Database pool
    let pool = Arc::new(db::PgPool::new(&config.database_url).expect("PgPool creation failed"));
    pool.health_check().await.expect("DB health check failed");
    info!("database connection OK");

    // 4. Infrastructure services
    let bus = Arc::new(event_bus::InProcessBus::new());
    let jwt_service = Arc::new(auth::JwtService::new(
        &config.jwt_secret,
        chrono::Duration::hours(8),
    ));

    // 5. BC-owned RBAC: collect manifests -> validated registry -> checker
    let wh_perms = warehouse::registrar::WarehousePermissions;
    let cat_perms = catalog::registrar::CatalogPermissions;

    let registry = Arc::new(
        auth::PermissionRegistry::from_manifests_validated(vec![
            wh_perms.permission_manifest(),
            cat_perms.permission_manifest(),
        ])
        .expect("RBAC manifest validation failed"),
    );
    info!(
        "permission registry built: {} roles, {} actions",
        registry.roles().len(),
        registry.permissions().len()
    );

    let checker = Arc::new(auth::JwtPermissionChecker::new(registry.clone()));
    let audit_log = Arc::new(audit::PgAuditLog::new(pool.clone()));
    let extensions = Arc::new(runtime::stubs::NoopExtensionHooks);
    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));

    // 6. Command Pipeline + Query Pipeline
    let pipeline = Arc::new(runtime::CommandPipeline::new(
        uow_factory,
        bus.clone(),
        checker.clone(),
        extensions.clone(),
        audit_log.clone(),
    ));

    let query_pipeline = Arc::new(runtime::QueryPipeline::new(checker, extensions, audit_log));

    // 7. Register Bounded Contexts (migrations + handlers + routes)
    let wh = warehouse::module::WarehouseModule::new(pool.clone());
    let cat = catalog::module::CatalogModule;

    let mut builder = AppBuilder::new(pool.clone(), bus.clone(), pipeline, query_pipeline).await;
    builder
        .register(&wh, warehouse::infrastructure::http::routes)
        .await;
    builder
        .register(&cat, catalog::infrastructure::http::routes)
        .await;

    // 8. Graceful shutdown token
    let cancel = CancellationToken::new();

    // 9. Outbox Relay (background task) — uses inbox_bus for enforced dedup
    let inbox_bus_for_relay = builder.inbox_bus().clone();
    let relay = db::OutboxRelay::new(
        pool.clone(),
        inbox_bus_for_relay as Arc<dyn event_bus::traits::EventBus>,
        Duration::from_millis(config.relay_poll_ms),
        config.relay_batch_size,
        cancel.child_token(),
    );
    tokio::spawn(async move {
        relay.run().await.ok();
    });

    // 10. Router
    let inbox_bus = builder.inbox_bus().clone();
    let app = build_router(builder.into_api(), jwt_service, &inbox_bus, registry, &pool);

    // 11. Serve with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("bind failed");
    info!(addr = %config.listen_addr, "ERP Gateway listening");

    let cancel_for_signal = cancel.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel_for_signal))
        .await
        .expect("server error");

    info!("gateway shut down");
}

/// Ожидает SIGINT или SIGTERM, затем отменяет `CancellationToken`.
async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
    info!("shutdown signal received, stopping...");
    cancel.cancel();
}

/// Build Router: /health + /ready + /api with auth middleware + optionally /dev/*.
fn build_router(
    api: Router,
    jwt_service: Arc<auth::JwtService>,
    inbox_bus: &Arc<db::InboxBusDecorator>,
    registry: Arc<auth::PermissionRegistry>,
    pool: &Arc<db::PgPool>,
) -> Router {
    // API routes — all BCs under /api/{bc_name}, protected by JWT
    let jwt_for_middleware = jwt_service.clone();
    let api = api.layer(axum::middleware::from_fn(move |req, next| {
        let svc = jwt_for_middleware.clone();
        async move { auth::auth_middleware(req, next, svc).await }
    }));

    // Root router
    let pool_for_ready = pool.clone();
    let mut root = Router::new()
        .nest("/api", api)
        .route("/health", routing::get(health))
        .route(
            "/ready",
            routing::get(move || {
                let pool = pool_for_ready.clone();
                async move { readiness(pool).await }
            }),
        );

    // Dev endpoints (only when DEV_MODE is set)
    if std::env::var("DEV_MODE").is_ok() {
        info!("DEV_MODE enabled: POST /dev/token, GET /dev/events available");
        let dev = dev_token_router(DevTokenState {
            jwt: jwt_service,
            registry,
        });
        root = root.merge(dev);

        let inbox_bus_for_route = inbox_bus.clone();
        root = root.route(
            "/dev/events",
            routing::get(move || {
                let bus = inbox_bus_for_route.clone();
                async move {
                    let entries = bus.event_map().await;
                    Json(serde_json::json!({
                        "events": entries,
                        "total_subscriptions": entries.len(),
                    }))
                }
            }),
        );
    }

    // Global layers: body size limit + request timeout
    root.layer(tower_http::limit::RequestBodyLimitLayer::new(1_048_576)) // 1 MB
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(30),
        ))
}

/// Liveness probe — процесс жив, всегда 200.
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Readiness probe — проверяет доступность БД. 200 / 503.
async fn readiness(pool: Arc<db::PgPool>) -> impl IntoResponse {
    match pool.health_check().await {
        Ok(()) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({ "status": "ready" })),
        ),
        Err(e) => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "not ready", "error": e.to_string() })),
        ),
    }
}
