//! `CreateProductCommand` — создание товара в каталоге.
//!
//! Полный canonical write path:
//! validate VOs → check duplicate → create aggregate → persist → history → outbox.

use std::sync::Arc;

use async_trait::async_trait;
use db::PgCommandContext;
use kernel::types::EntityId;
use kernel::{AppError, Command, IntoInternal, RequestContext};
use runtime::command_handler::CommandHandler;
use runtime::ports::UnitOfWork;
use serde::Serialize;
use uuid::Uuid;

use crate::domain::aggregates::Product;
use crate::domain::errors::CatalogDomainError;
use crate::domain::value_objects::{ProductName, Sku};
use crate::infrastructure::repos::PgProductRepo;

/// Команда создания товара.
#[derive(Debug)]
pub struct CreateProductCommand {
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

impl Command for CreateProductCommand {
    fn command_name(&self) -> &'static str {
        "catalog.create_product"
    }
}

/// Результат создания товара.
#[derive(Debug, Serialize)]
pub struct CreateProductResult {
    pub product_id: Uuid,
}

/// Handler создания товара.
pub struct CreateProductHandler {
    #[allow(dead_code)]
    pool: Arc<db::PgPool>,
}

impl CreateProductHandler {
    #[must_use]
    pub fn new(pool: Arc<db::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CommandHandler for CreateProductHandler {
    type Cmd = CreateProductCommand;
    type Result = CreateProductResult;

    async fn handle(
        &self,
        cmd: &Self::Cmd,
        ctx: &RequestContext,
        uow: &mut dyn UnitOfWork,
    ) -> Result<Self::Result, AppError> {
        // 1. Validate value objects
        let sku = Sku::new(&cmd.sku)?;
        let name = ProductName::new(&cmd.name)?;

        // 2. Downcast UoW → PgCommandContext
        let mut db = PgCommandContext::from_uow(uow)?;

        // 3. Check duplicate
        if PgProductRepo::find_by_sku(db.client(), ctx.tenant_id, sku.as_str())
            .await
            .internal("find_by_sku")?
            .is_some()
        {
            return Err(CatalogDomainError::DuplicateSku(cmd.sku.clone()).into());
        }

        // 4. Create aggregate
        let product_id = EntityId::new();
        let mut product = Product::create(
            product_id,
            ctx.tenant_id,
            sku.clone(),
            name,
            cmd.category.clone(),
            cmd.unit.clone(),
        );

        // 5. Persist
        PgProductRepo::create_product(
            db.client(),
            ctx.tenant_id,
            *product_id.as_uuid(),
            sku.as_str(),
            product.name().as_str(),
            product.category(),
            product.unit(),
        )
        .await
        .internal("create_product")?;

        // 6. Domain history
        let new_state = serde_json::json!({
            "sku": sku.as_str(),
            "name": product.name().as_str(),
            "category": product.category(),
            "unit": product.unit(),
        });
        audit::DomainHistoryWriter::record_change(
            db.client(),
            ctx,
            "product",
            *product_id.as_uuid(),
            "erp.catalog.product_created.v1",
            None::<&serde_json::Value>,
            Some(&new_state),
        )
        .await?;

        // 7. Emit events to outbox
        db.emit_events(&mut product, ctx, "catalog")?;

        Ok(CreateProductResult {
            product_id: *product_id.as_uuid(),
        })
    }
}
