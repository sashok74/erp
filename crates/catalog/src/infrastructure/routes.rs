//! Axum HTTP handlers для Catalog BC.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use kernel::RequestContext;
use runtime::pipeline::CommandPipeline;
use runtime::query_handler::QueryHandler;
use serde::{Deserialize, Serialize};

use crate::application::commands::create_product::{
    CreateProductCommand, CreateProductHandler, CreateProductResult,
};
use crate::application::queries::get_product::{GetProductHandler, GetProductQuery, ProductResult};

/// Shared state для catalog routes.
pub struct CatalogState<UF: runtime::ports::UnitOfWorkFactory> {
    pub pipeline: Arc<CommandPipeline<UF>>,
    pub create_handler: Arc<CreateProductHandler>,
    pub get_handler: Arc<GetProductHandler>,
}

/// JSON body для POST /products.
#[derive(Debug, Deserialize)]
pub struct CreateProductBody {
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

/// JSON response для POST /products.
#[derive(Debug, Serialize)]
pub struct CreateProductResponse {
    pub product_id: String,
}

impl From<CreateProductResult> for CreateProductResponse {
    fn from(r: CreateProductResult) -> Self {
        Self {
            product_id: r.product_id.to_string(),
        }
    }
}

/// Query params для GET /products.
#[derive(Debug, Deserialize)]
pub struct ProductQueryParams {
    pub sku: String,
}

/// JSON response для GET /products.
#[derive(Debug, Serialize)]
pub struct ProductResponse {
    pub product_id: String,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

impl From<ProductResult> for ProductResponse {
    fn from(r: ProductResult) -> Self {
        Self {
            product_id: r.product_id.to_string(),
            sku: r.sku,
            name: r.name,
            category: r.category,
            unit: r.unit,
        }
    }
}

/// POST /products — создать товар.
async fn create_product<UF: runtime::ports::UnitOfWorkFactory>(
    State(state): State<Arc<CatalogState<UF>>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<CreateProductBody>,
) -> impl IntoResponse {
    let cmd = CreateProductCommand {
        sku: body.sku,
        name: body.name,
        category: body.category,
        unit: body.unit,
    };

    match state
        .pipeline
        .execute(&*state.create_handler, &cmd, &ctx)
        .await
    {
        Ok(result) => {
            let resp: CreateProductResponse = result.into();
            (
                StatusCode::CREATED,
                Json(serde_json::to_value(resp).unwrap()),
            )
                .into_response()
        }
        Err(e) => error_response(&e),
    }
}

/// GET /products — получить товар по SKU.
async fn get_product<UF: runtime::ports::UnitOfWorkFactory>(
    State(state): State<Arc<CatalogState<UF>>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<ProductQueryParams>,
) -> impl IntoResponse {
    let query = GetProductQuery { sku: params.sku };

    match state.get_handler.handle(&query, &ctx).await {
        Ok(result) => {
            let resp = ProductResponse::from(result);
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

/// Создать `Router` для Catalog BC.
pub fn catalog_routes<UF: runtime::ports::UnitOfWorkFactory + 'static>(
    state: Arc<CatalogState<UF>>,
) -> Router {
    Router::new()
        .route("/products", routing::post(create_product::<UF>))
        .route("/products", routing::get(get_product::<UF>))
        .with_state(state)
}
