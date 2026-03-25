//! `CreateProductCommand` — создание товара в каталоге.
//!
//! Полный canonical write path:
//! validate VOs → check duplicate → create aggregate → persist → history → outbox.

use std::sync::Arc;

use async_trait::async_trait;
use event_bus::EventEnvelope;
use kernel::entity::AggregateRoot;
use kernel::types::EntityId;
use kernel::{AppError, Command, RequestContext};
use runtime::command_handler::CommandHandler;
use runtime::ports::UnitOfWork;
use serde::Serialize;
use tokio_postgres::Client;
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

        // Scope the PgUnitOfWork borrow — collect envelopes, then add to uow after.
        let (result, envelopes) = {
            // 2. Downcast UoW → PgUnitOfWork → client
            let pg = uow
                .as_any_mut()
                .downcast_mut::<db::PgUnitOfWork>()
                .ok_or_else(|| AppError::Internal("expected PgUnitOfWork".into()))?;
            let client: &Client = pg.client();

            // 3. Check duplicate
            if PgProductRepo::find_by_sku(client, ctx.tenant_id, sku.as_str())
                .await
                .map_err(|e| AppError::Internal(format!("find_by_sku: {e}")))?
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
                client,
                ctx.tenant_id,
                *product_id.as_uuid(),
                sku.as_str(),
                product.name().as_str(),
                product.category(),
                product.unit(),
            )
            .await
            .map_err(|e| AppError::Internal(format!("create_product: {e}")))?;

            // 6. Domain history
            let new_state = serde_json::json!({
                "sku": sku.as_str(),
                "name": product.name().as_str(),
                "category": product.category(),
                "unit": product.unit(),
            });
            audit::DomainHistoryWriter::record(
                client,
                ctx,
                "product",
                *product_id.as_uuid(),
                "erp.catalog.product_created.v1",
                None,
                Some(&new_state),
            )
            .await
            .map_err(|e| AppError::Internal(format!("domain_history: {e}")))?;

            // 7. Collect outbox envelopes
            let events = product.take_events();
            let mut envelopes = Vec::with_capacity(events.len());
            for evt in &events {
                let envelope = EventEnvelope::from_domain_event(evt, ctx, "catalog")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                envelopes.push(envelope);
            }

            (
                CreateProductResult {
                    product_id: *product_id.as_uuid(),
                },
                envelopes,
            )
        };
        // pg/client dropped — borrow released

        // 8. Add outbox entries
        for envelope in envelopes {
            uow.add_outbox_entry(envelope);
        }

        Ok(result)
    }
}
