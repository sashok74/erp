//! `GetBalanceQuery` — запрос текущего баланса по SKU.
//!
//! Read-only, внутри `BEGIN READ ONLY` + RLS через `ReadScope`.

use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use kernel::{AppError, Query, RequestContext};
use runtime::query_handler::QueryHandler;
use serde::Serialize;
use uuid::Uuid;

use crate::application::repos::InventoryRepo;

/// Запрос баланса по SKU.
#[derive(Debug)]
pub struct GetBalanceQuery {
    pub sku: String,
}

impl Query for GetBalanceQuery {
    fn query_name(&self) -> &'static str {
        "warehouse.get_balance"
    }
}

/// Результат запроса баланса.
#[derive(Debug, Serialize)]
pub struct BalanceResult {
    pub sku: String,
    pub balance: BigDecimal,
    pub item_id: Option<Uuid>,
    pub product_name: Option<String>,
}

/// Handler запроса баланса.
pub struct GetBalanceHandler {
    pool: Arc<db::PgPool>,
}

impl GetBalanceHandler {
    #[must_use]
    pub fn new(pool: Arc<db::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl QueryHandler for GetBalanceHandler {
    type Query = GetBalanceQuery;
    type Result = BalanceResult;

    async fn handle(
        &self,
        query: &Self::Query,
        ctx: &RequestContext,
    ) -> Result<Self::Result, AppError> {
        let read = db::ReadScope::acquire(&self.pool, ctx.tenant_id).await?;
        let repo = InventoryRepo::new(read.client(), ctx.tenant_id);

        let row = repo.get_balance(&query.sku).await?;
        let projection = repo.get_product_projection(&query.sku).await?;
        let product_name = projection.map(|p| p.name);

        match row {
            Some(r) => Ok(BalanceResult {
                sku: r.sku,
                balance: r.balance,
                item_id: Some(r.item_id),
                product_name,
            }),
            None => Ok(BalanceResult {
                sku: query.sku.clone(),
                balance: BigDecimal::from(0),
                item_id: None,
                product_name,
            }),
        }
    }
}
