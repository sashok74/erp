//! DTO-маппинг для Warehouse BC: `FromBody` / `FromQueryParams` реализации.
//!
//! Маршрутизация (`BcRouter`) — в `gateway::bc_router`.

use bigdecimal::BigDecimal;
use runtime::dto::{FromBody, FromQueryParams};
use serde::Deserialize;

use crate::application::commands::receive_goods::ReceiveGoodsCommand;
use crate::application::queries::get_balance::GetBalanceQuery;

// ─── FromBody / FromQueryParams ─────────────────────────────────────────────

/// JSON body для POST /receive.
#[derive(Debug, Deserialize)]
pub struct ReceiveGoodsBody {
    pub sku: String,
    pub quantity: BigDecimal,
}

impl FromBody for ReceiveGoodsCommand {
    type Body = ReceiveGoodsBody;

    fn from_body(body: Self::Body) -> Self {
        Self {
            sku: body.sku,
            quantity: body.quantity,
        }
    }
}

/// Query params для GET /balance.
#[derive(Debug, Deserialize)]
pub struct BalanceQueryParams {
    pub sku: String,
}

impl FromQueryParams for GetBalanceQuery {
    type Params = BalanceQueryParams;

    fn from_params(params: Self::Params) -> Self {
        Self { sku: params.sku }
    }
}
