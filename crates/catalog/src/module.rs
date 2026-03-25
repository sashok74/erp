//! `CatalogModule` — регистрация Catalog BC в приложении.

use std::sync::Arc;

use crate::application::commands::create_product::CreateProductHandler;
use crate::application::queries::get_product::GetProductHandler;
use crate::infrastructure::routes::{CatalogState, catalog_routes};

/// Catalog Bounded Context module.
pub struct CatalogModule;

impl CatalogModule {
    /// Имя модуля.
    #[must_use]
    pub fn name() -> &'static str {
        "catalog"
    }

    /// Создать axum Router с маршрутами catalog.
    pub fn routes<UF: runtime::ports::UnitOfWorkFactory + 'static>(
        pipeline: Arc<runtime::pipeline::CommandPipeline<UF>>,
        pool: Arc<db::PgPool>,
    ) -> axum::Router {
        let state = Arc::new(CatalogState {
            pipeline,
            create_handler: Arc::new(CreateProductHandler::new(pool.clone())),
            get_handler: Arc::new(GetProductHandler::new(pool)),
        });
        catalog_routes(state)
    }
}
