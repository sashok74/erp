//! DTO-маппинг для Warehouse BC: `FromBody` / `FromQueryParams` реализации.
//!
//! HTTP-маршрутизация — в [`super::http`] через `bc_http::BcRouter`.

use bigdecimal::BigDecimal;
use runtime::{from_body, from_query_params};

use crate::application::commands::receive_goods::ReceiveGoodsCommand;
use crate::application::queries::get_balance::GetBalanceQuery;

from_body! {
    ReceiveGoodsBody -> ReceiveGoodsCommand {
        sku: String,
        quantity: BigDecimal,
    }
}

from_query_params! {
    BalanceQueryParams -> GetBalanceQuery {
        sku: String,
    }
}
