//! HTTP-маршруты Warehouse BC.
//!
//! BC сам описывает свои routes через [`bc_http::BcRouter`].
//! Gateway монтирует результат через `.nest("/warehouse", routes(...))`.

use std::sync::Arc;

use axum::Router;
use axum::http::Method;

use bc_http::BcRouter;
use db::PgUnitOfWorkFactory;
use runtime::CommandPipeline;

use crate::application::commands::receive_goods::ReceiveGoodsHandler;
use crate::application::queries::get_balance::GetBalanceHandler;

/// Построить axum `Router` для Warehouse BC.
pub fn routes(
    pipeline: Arc<CommandPipeline<PgUnitOfWorkFactory>>,
    pool: Arc<db::PgPool>,
) -> Router {
    BcRouter::new(pipeline)
        .command(&Method::POST, "/receive", {
            let pool = pool.clone();
            move || ReceiveGoodsHandler::new(pool.clone())
        })
        .query(&Method::GET, "/balance", {
            move || GetBalanceHandler::new(pool.clone())
        })
        .build()
}
