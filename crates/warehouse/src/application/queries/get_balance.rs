//! `GetBalanceQuery` — запрос текущего баланса по SKU.
//!
//! Read-only, внутри `BEGIN READ ONLY` + RLS.

use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use kernel::{AppError, Query, RequestContext};
use runtime::query_handler::QueryHandler;
use serde::Serialize;
use uuid::Uuid;

use crate::application::ports::InventoryRepo;

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
        let tenant_id = ctx.tenant_id;
        let sku = query.sku.clone();

        db::with_tenant_read(&self.pool, tenant_id, |client| {
            Box::pin(async move {
                let repo = InventoryRepo::new(client, tenant_id);
                let row = repo.get_balance(&sku).await?;
                let projection = repo.get_product_projection(&sku).await?;
                let product_name = projection.map(|p| p.name);

                match row {
                    Some(r) => Ok(BalanceResult {
                        sku: r.sku,
                        balance: r.balance,
                        item_id: Some(r.item_id),
                        product_name,
                    }),
                    None => Ok(BalanceResult {
                        sku,
                        balance: BigDecimal::from(0),
                        item_id: None,
                        product_name,
                    }),
                }
            })
        })
        .await
    }
}
