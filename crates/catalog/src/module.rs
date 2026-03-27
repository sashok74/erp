//! `CatalogModule` — регистрация Catalog BC в приложении.

use async_trait::async_trait;
use event_bus::EventBus;
use runtime::BoundedContextModule;

/// Catalog Bounded Context module.
pub struct CatalogModule;

#[async_trait]
impl BoundedContextModule for CatalogModule {
    fn name(&self) -> &'static str {
        "catalog"
    }

    fn migrations_dir(&self) -> &'static str {
        "migrations/catalog"
    }

    async fn register_handlers(&self, _bus: &dyn EventBus) {
        // Catalog не подписывается на чужие события (пока)
    }
}
