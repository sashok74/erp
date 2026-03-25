//! `GetBalanceQuery` — запрос текущего баланса по SKU.
//!
//! Read-only, без транзакции. Использует `PgPool` напрямую.

use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use kernel::{AppError, RequestContext};
use runtime::query_handler::QueryHandler;
use serde::Serialize;
use uuid::Uuid;

use crate::infrastructure::repos::PgInventoryRepo;

/// Запрос баланса по SKU.
#[derive(Debug)]
pub struct GetBalanceQuery {
    pub sku: String,
}

/// Результат запроса баланса.
#[derive(Debug, Serialize)]
pub struct BalanceResult {
    pub sku: String,
    pub balance: BigDecimal,
    pub item_id: Option<Uuid>,
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
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("pool checkout: {e}")))?;

        // Set tenant context for RLS
        db::rls::set_tenant_context(&**client, ctx.tenant_id)
            .await
            .map_err(|e| AppError::Internal(format!("set tenant: {e}")))?;

        let row = PgInventoryRepo::get_balance(&**client, ctx.tenant_id, &query.sku)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        match row {
            Some(r) => Ok(BalanceResult {
                sku: r.sku,
                balance: r.balance,
                item_id: Some(r.item_id),
            }),
            None => Ok(BalanceResult {
                sku: query.sku.clone(),
                balance: BigDecimal::from(0),
                item_id: None,
            }),
        }
    }
}
