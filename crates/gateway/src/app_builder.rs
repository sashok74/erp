//! `AppBuilder` — регистрация Bounded Context модулей при старте приложения.
//!
//! Устраняет линейный рост `main.rs`: каждый BC = один вызов `register()`.
//! `AppBuilder` выполняет для каждого BC:
//! 1. Миграции (idempotent)
//! 2. Регистрация event handler'ов на шине
//! 3. Монтирование HTTP-маршрутов под `/api/{bc_name}`

use std::sync::Arc;

use axum::Router;
use tracing::info;

use runtime::BoundedContextModule;

/// Builder для сборки API-роутера из Bounded Context модулей.
pub struct AppBuilder {
    pool: Arc<db::PgPool>,
    bus: Arc<event_bus::InProcessBus>,
    pipeline: Arc<runtime::CommandPipeline<db::PgUnitOfWorkFactory>>,
    api: Router,
}

impl AppBuilder {
    /// Создать builder. Common-миграции запускаются сразу.
    pub async fn new(
        pool: Arc<db::PgPool>,
        bus: Arc<event_bus::InProcessBus>,
        pipeline: Arc<runtime::CommandPipeline<db::PgUnitOfWorkFactory>>,
    ) -> Self {
        db::migrate::run_migrations(&pool, "migrations/common")
            .await
            .expect("common migrations failed");

        Self {
            pool,
            bus,
            pipeline,
            api: Router::new(),
        }
    }

    /// Зарегистрировать BC: миграции → event handlers → routes.
    ///
    /// `routes_fn` имеет ту же сигнатуру, что и `{bc}::infrastructure::http::routes`.
    pub async fn register(
        &mut self,
        module: &dyn BoundedContextModule,
        routes_fn: impl FnOnce(
            Arc<runtime::CommandPipeline<db::PgUnitOfWorkFactory>>,
            Arc<db::PgPool>,
        ) -> Router,
    ) -> &mut Self {
        let name = module.name();

        // 1. Migrations
        db::migrate::run_migrations(&self.pool, module.migrations_dir())
            .await
            .unwrap_or_else(|e| panic!("{name} migrations failed: {e}"));
        info!(bc = name, "migrations applied");

        // 2. Event handlers
        module.register_handlers(&*self.bus).await;
        info!(bc = name, "event handlers registered");

        // 3. Routes
        let routes = routes_fn(self.pipeline.clone(), self.pool.clone());
        self.api = std::mem::take(&mut self.api).nest(&format!("/{name}"), routes);

        self
    }

    /// Финализировать: вернуть собранный API-роутер.
    pub fn into_api(self) -> Router {
        self.api
    }
}
