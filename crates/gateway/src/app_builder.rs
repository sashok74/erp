//! `AppBuilder` — регистрация Bounded Context модулей при старте приложения.
//!
//! Устраняет линейный рост `main.rs`: каждый BC = один вызов `register()`.
//! `AppBuilder` выполняет для каждого BC:
//! 1. Миграции (idempotent)
//! 2. Регистрация event handler'ов на шине (через `InboxBusDecorator`)
//! 3. Монтирование HTTP-маршрутов под `/api/{bc_name}`

use std::sync::Arc;

use axum::Router;
use event_bus::traits::EventBus;
use tracing::info;

use runtime::BoundedContextModule;

/// Builder для сборки API-роутера из Bounded Context модулей.
pub struct AppBuilder {
    pool: Arc<db::PgPool>,
    inbox_bus: Arc<db::InboxBusDecorator>,
    pipeline: Arc<runtime::CommandPipeline<db::PgUnitOfWorkFactory>>,
    query_pipeline: Arc<runtime::QueryPipeline>,
    api: Router,
}

impl AppBuilder {
    /// Создать builder. Common-миграции запускаются сразу.
    /// Bus оборачивается в `InboxBusDecorator` — enforced consumer dedup.
    pub async fn new(
        pool: Arc<db::PgPool>,
        bus: Arc<event_bus::InProcessBus>,
        pipeline: Arc<runtime::CommandPipeline<db::PgUnitOfWorkFactory>>,
        query_pipeline: Arc<runtime::QueryPipeline>,
    ) -> Self {
        db::migrate::run_migrations(&pool, "migrations/common")
            .await
            .expect("common migrations failed");

        let inbox_bus = Arc::new(
            db::InboxBusDecorator::new(bus as Arc<dyn EventBus>, pool.clone()),
        );

        Self {
            pool,
            inbox_bus,
            pipeline,
            query_pipeline,
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
            Arc<runtime::QueryPipeline>,
            Arc<db::PgPool>,
        ) -> Router,
    ) -> &mut Self {
        let name = module.name();

        // 1. Migrations
        db::migrate::run_migrations(&self.pool, module.migrations_dir())
            .await
            .unwrap_or_else(|e| panic!("{name} migrations failed: {e}"));
        info!(bc = name, "migrations applied");

        // 2. Event handlers — BC получает декорированный bus, НЕ ЗНАЕТ про inbox
        module.register_handlers(&*self.inbox_bus).await;
        info!(bc = name, "event handlers registered");

        // 3. Routes
        let routes = routes_fn(
            self.pipeline.clone(),
            self.query_pipeline.clone(),
            self.pool.clone(),
        );
        self.api = std::mem::take(&mut self.api).nest(&format!("/{name}"), routes);

        self
    }

    /// Доступ к декоратору — для `/dev/events` и relay wiring.
    pub fn inbox_bus(&self) -> &Arc<db::InboxBusDecorator> {
        &self.inbox_bus
    }

    /// Финализировать: вернуть собранный API-роутер.
    pub fn into_api(self) -> Router {
        self.api
    }
}
