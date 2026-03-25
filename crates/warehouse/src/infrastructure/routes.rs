//! Axum HTTP handlers для Warehouse BC.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use bigdecimal::BigDecimal;
use kernel::RequestContext;
use runtime::pipeline::CommandPipeline;
use runtime::query_handler::QueryHandler;
use serde::{Deserialize, Serialize};

use crate::application::commands::receive_goods::{
    ReceiveGoodsCommand, ReceiveGoodsHandler, ReceiveGoodsResult,
};
use crate::application::queries::get_balance::{BalanceResult, GetBalanceHandler, GetBalanceQuery};

/// Shared state для warehouse routes.
pub struct WarehouseState<UF: runtime::ports::UnitOfWorkFactory> {
    pub pipeline: Arc<CommandPipeline<UF>>,
    pub receive_handler: Arc<ReceiveGoodsHandler>,
    pub balance_handler: Arc<GetBalanceHandler>,
}

/// JSON body для POST /receive.
#[derive(Debug, Deserialize)]
pub struct ReceiveGoodsBody {
    pub sku: String,
    pub quantity: BigDecimal,
}

/// JSON response для POST /receive.
#[derive(Debug, Serialize)]
pub struct ReceiveGoodsResponse {
    pub item_id: String,
    pub movement_id: String,
    pub new_balance: String,
    pub doc_number: String,
}

impl From<ReceiveGoodsResult> for ReceiveGoodsResponse {
    fn from(r: ReceiveGoodsResult) -> Self {
        Self {
            item_id: r.item_id.to_string(),
            movement_id: r.movement_id.to_string(),
            new_balance: r.new_balance.to_string(),
            doc_number: r.doc_number,
        }
    }
}

/// Query params для GET /balance.
#[derive(Debug, Deserialize)]
pub struct BalanceQueryParams {
    pub sku: String,
}

/// JSON response для GET /balance.
#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub sku: String,
    pub balance: String,
    pub item_id: Option<String>,
}

impl From<BalanceResult> for BalanceResponse {
    fn from(r: BalanceResult) -> Self {
        Self {
            sku: r.sku,
            balance: r.balance.to_string(),
            item_id: r.item_id.map(|id| id.to_string()),
        }
    }
}

/// POST /receive — приёмка товара.
async fn receive_goods<UF: runtime::ports::UnitOfWorkFactory>(
    State(state): State<Arc<WarehouseState<UF>>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<ReceiveGoodsBody>,
) -> impl IntoResponse {
    let cmd = ReceiveGoodsCommand {
        sku: body.sku,
        quantity: body.quantity,
    };

    match state.pipeline.execute(&*state.receive_handler, &cmd, &ctx).await {
        Ok(result) => {
            let resp: ReceiveGoodsResponse = result.into();
            (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response()
        }
        Err(e) => error_response(&e),
    }
}

/// GET /balance — запрос остатков.
async fn get_balance<UF: runtime::ports::UnitOfWorkFactory>(
    State(state): State<Arc<WarehouseState<UF>>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<BalanceQueryParams>,
) -> impl IntoResponse {
    let query = GetBalanceQuery { sku: params.sku };

    match state.balance_handler.handle(&query, &ctx).await {
        Ok(result) => {
            let resp = BalanceResponse::from(result);
            (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response()
        }
        Err(e) => error_response(&e),
    }
}

fn error_response(e: &kernel::AppError) -> axum::response::Response {
    use kernel::AppError;

    let (status, msg) = match e {
        AppError::Unauthorized(_) => (StatusCode::FORBIDDEN, e.to_string()),
        AppError::Validation(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        AppError::Domain(_) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string()),
    };

    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

/// Создать `Router` для Warehouse BC.
pub fn warehouse_routes<UF: runtime::ports::UnitOfWorkFactory + 'static>(
    state: Arc<WarehouseState<UF>>,
) -> Router {
    Router::new()
        .route("/receive", routing::post(receive_goods::<UF>))
        .route("/balance", routing::get(get_balance::<UF>))
        .with_state(state)
}
