//! `GetProductQuery` — запрос товара по SKU.
//!
//! Read-only, внутри `BEGIN READ ONLY` + RLS через `ReadScope`.

use std::sync::Arc;

use async_trait::async_trait;
use kernel::{AppError, DomainError, Query, RequestContext};
use runtime::query_handler::QueryHandler;
use serde::Serialize;
use uuid::Uuid;

use crate::db::CatalogDb;

/// Запрос товара по SKU.
#[derive(Debug)]
pub struct GetProductQuery {
    pub sku: String,
}

impl Query for GetProductQuery {
    fn query_name(&self) -> &'static str {
        "catalog.get_product"
    }
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
        let read = db::ReadScope::acquire(&self.pool, ctx.tenant_id).await?;
        let cat = CatalogDb::new(read.client(), ctx.tenant_id);

        let row = cat.products.find_by_sku(&query.sku).await?;
        read.finish().await?;

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
