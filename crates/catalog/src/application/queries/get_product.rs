//! `GetProductQuery` — запрос товара по SKU.
//!
//! Read-only, без транзакции. Использует `PgPool` напрямую.

use std::sync::Arc;

use async_trait::async_trait;
use kernel::{AppError, DomainError, RequestContext};
use runtime::query_handler::QueryHandler;
use serde::Serialize;
use uuid::Uuid;

use crate::infrastructure::repos::PgProductRepo;

/// Запрос товара по SKU.
#[derive(Debug)]
pub struct GetProductQuery {
    pub sku: String,
}

/// Результат запроса товара.
#[derive(Debug, Serialize)]
pub struct ProductResult {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

/// Handler запроса товара.
pub struct GetProductHandler {
    pool: Arc<db::PgPool>,
}

impl GetProductHandler {
    #[must_use]
    pub fn new(pool: Arc<db::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl QueryHandler for GetProductHandler {
    type Query = GetProductQuery;
    type Result = ProductResult;

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

        let row = PgProductRepo::find_by_sku(&**client, ctx.tenant_id, &query.sku)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        match row {
            Some(r) => Ok(ProductResult {
                product_id: r.id,
                sku: r.sku,
                name: r.name,
                category: r.category,
                unit: r.unit,
            }),
            None => Err(AppError::Domain(DomainError::NotFound(format!(
                "Product with SKU '{}'",
                query.sku
            )))),
        }
    }
}
