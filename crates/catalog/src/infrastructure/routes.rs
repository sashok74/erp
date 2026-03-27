//! DTO-маппинг для Catalog BC: `FromBody` / `FromQueryParams` реализации.
//!
//! HTTP-маршрутизация — в [`super::http`] через `bc_http::BcRouter`.

use runtime::{from_body, from_query_params};

use crate::application::commands::create_product::CreateProductCommand;
use crate::application::queries::get_product::GetProductQuery;

from_body! {
    CreateProductBody -> CreateProductCommand {
        sku: String,
        name: String,
        category: String,
        unit: String,
    }
}

from_query_params! {
    ProductQueryParams -> GetProductQuery {
        sku: String,
    }
}
