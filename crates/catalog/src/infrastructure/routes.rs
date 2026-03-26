//! DTO-маппинг для Catalog BC: `FromBody` / `FromQueryParams` реализации.
//!
//! Маршрутизация (`BcRouter`) — в `gateway::bc_router`.

use runtime::dto::{FromBody, FromQueryParams};
use serde::Deserialize;

use crate::application::commands::create_product::CreateProductCommand;
use crate::application::queries::get_product::GetProductQuery;

// ─── FromBody / FromQueryParams ─────────────────────────────────────────────

/// JSON body для POST /products.
#[derive(Debug, Deserialize)]
pub struct CreateProductBody {
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

impl FromBody for CreateProductCommand {
    type Body = CreateProductBody;

    fn from_body(body: Self::Body) -> Self {
        Self {
            sku: body.sku,
            name: body.name,
            category: body.category,
            unit: body.unit,
        }
    }
}

/// Query params для GET /products.
#[derive(Debug, Deserialize)]
pub struct ProductQueryParams {
    pub sku: String,
}

impl FromQueryParams for GetProductQuery {
    type Params = ProductQueryParams;

    fn from_params(params: Self::Params) -> Self {
        Self { sku: params.sku }
    }
}
