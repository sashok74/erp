//! HTTP-маршруты Catalog BC.
//!
//! BC сам описывает свои routes через [`bc_http::BcRouter`].
//! Gateway монтирует результат через `.nest("/catalog", routes(...))`.

use std::sync::Arc;

use axum::Router;
use axum::http::{Method, StatusCode};

use bc_http::BcRouter;
use db::PgUnitOfWorkFactory;
use runtime::{CommandPipeline, QueryPipeline};

use crate::application::commands::create_product::CreateProductHandler;
use crate::application::queries::get_product::GetProductHandler;

/// Построить axum `Router` для Catalog BC.
pub fn routes(
    pipeline: Arc<CommandPipeline<PgUnitOfWorkFactory>>,
    query_pipeline: Arc<QueryPipeline>,
    pool: Arc<db::PgPool>,
) -> Router {
    BcRouter::new(pipeline, query_pipeline)
        .command_with_status(&Method::POST, "/products", StatusCode::CREATED, {
            let pool = pool.clone();
            move || CreateProductHandler::new(pool.clone())
        })
        .query(&Method::GET, "/products", {
            move || GetProductHandler::new(pool.clone())
        })
        .build()
}
