//! `WarehouseModule` — регистрация Warehouse BC в приложении.

use std::sync::Arc;

use crate::application::commands::receive_goods::ReceiveGoodsHandler;
use crate::application::queries::get_balance::GetBalanceHandler;
use crate::infrastructure::routes::{WarehouseState, warehouse_routes};

/// Warehouse Bounded Context module.
pub struct WarehouseModule;

impl WarehouseModule {
    /// Имя модуля.
    #[must_use]
    pub fn name() -> &'static str {
        "warehouse"
    }

    /// Создать axum Router с маршрутами warehouse.
    pub fn routes<UF: runtime::ports::UnitOfWorkFactory + 'static>(
        pipeline: Arc<runtime::pipeline::CommandPipeline<UF>>,
        pool: Arc<db::PgPool>,
    ) -> axum::Router {
        let state = Arc::new(WarehouseState {
            pipeline,
            receive_handler: Arc::new(ReceiveGoodsHandler::new(pool.clone())),
            balance_handler: Arc::new(GetBalanceHandler::new(pool)),
        });
        warehouse_routes(state)
    }
}
