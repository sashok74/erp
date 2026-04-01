//! `CreateProductCommand` — создание товара в каталоге.
//!
//! Полный canonical write path:
//! validate VOs → check duplicate → create aggregate → persist → history → outbox.

use async_trait::async_trait;
use db::PgCommandContext;
use kernel::types::EntityId;
use kernel::{AppError, Command, RequestContext};
use runtime::command_handler::CommandHandler;
use runtime::ports::UnitOfWork;
use serde::Serialize;
use uuid::Uuid;

use crate::db::CatalogDb;
use crate::domain::aggregates::Product;
use crate::domain::errors::CatalogDomainError;
use crate::domain::value_objects::{ProductName, Sku};

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
#[derive(Default)]
pub struct CreateProductHandler;

impl CreateProductHandler {
    #[must_use]
    pub fn new() -> Self {
        Self
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
        let cat = CatalogDb::new(db.client(), ctx.tenant_id);

        // 3. Check duplicate
        if cat.products.find_by_sku(sku.as_str()).await?.is_some() {
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
        cat.products
            .create_product(
                product_id.as_uuid(),
                sku.as_str(),
                product.name().as_str(),
                product.category(),
                product.unit(),
            )
            .await?;

        // 6. Domain history (deferred — flush в commit)
        let new_state = serde_json::json!({
            "sku": sku.as_str(),
            "name": product.name().as_str(),
            "category": product.category(),
            "unit": product.unit(),
        });
        db.record_change(
            ctx,
            "product",
            *product_id.as_uuid(),
            "erp.catalog.product_created.v1",
            None::<&serde_json::Value>,
            Some(&new_state),
        )?;

        // 7. Emit events to outbox
        db.emit_events(&mut product, ctx, "catalog")?;

        Ok(CreateProductResult {
            product_id: *product_id.as_uuid(),
        })
    }
}
