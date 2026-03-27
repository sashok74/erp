//! `WarehouseModule` — регистрация Warehouse BC в приложении.

use std::sync::Arc;

use event_bus::{EventBus, EventHandlerAdapter};

use crate::infrastructure::event_handlers::ProductCreatedHandler;

/// Warehouse Bounded Context module.
pub struct WarehouseModule;

impl WarehouseModule {
    /// Имя модуля.
    #[must_use]
    pub fn name() -> &'static str {
        "warehouse"
    }

    /// Зарегистрировать event handler'ы Warehouse BC на шине.
    pub async fn register_handlers(bus: &dyn EventBus, pool: Arc<db::PgPool>) {
        let handler = ProductCreatedHandler::new(pool);
        let adapter = Arc::new(EventHandlerAdapter::new(handler));
        bus.subscribe("erp.catalog.product_created.v1", adapter)
            .await;
    }
}
