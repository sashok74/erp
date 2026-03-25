//! `ReceiveGoodsCommand` — приёмка товара на склад.
//!
//! Полный canonical write path:
//! validate → find/create item → receive → movement + balance → history → seq → outbox.

use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use event_bus::EventEnvelope;
use kernel::entity::AggregateRoot;
use kernel::types::EntityId;
use kernel::{AppError, Command, RequestContext};
use runtime::command_handler::CommandHandler;
use runtime::ports::UnitOfWork;
use serde::Serialize;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::domain::aggregates::InventoryItem;
use crate::domain::value_objects::{Quantity, Sku};
use crate::infrastructure::repos::PgInventoryRepo;

/// Команда приёмки товара.
#[derive(Debug)]
pub struct ReceiveGoodsCommand {
    pub sku: String,
    pub quantity: BigDecimal,
}

impl Command for ReceiveGoodsCommand {
    fn command_name(&self) -> &'static str {
        "warehouse.receive_goods"
    }
}

/// Результат приёмки.
#[derive(Debug, Serialize)]
pub struct ReceiveGoodsResult {
    pub item_id: Uuid,
    pub movement_id: Uuid,
    pub new_balance: BigDecimal,
    pub doc_number: String,
}

/// Handler приёмки товара.
pub struct ReceiveGoodsHandler {
    #[allow(dead_code)]
    pool: Arc<db::PgPool>,
}

impl ReceiveGoodsHandler {
    #[must_use]
    pub fn new(pool: Arc<db::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
#[allow(clippy::too_many_lines)]
impl CommandHandler for ReceiveGoodsHandler {
    type Cmd = ReceiveGoodsCommand;
    type Result = ReceiveGoodsResult;

    async fn handle(
        &self,
        cmd: &Self::Cmd,
        ctx: &RequestContext,
        uow: &mut dyn UnitOfWork,
    ) -> Result<Self::Result, AppError> {
        // 1. Validate value objects
        let sku = Sku::new(&cmd.sku)?;
        let qty = Quantity::new(cmd.quantity.clone())?;

        // Scope the PgUnitOfWork borrow — collect envelopes, then add to uow after.
        let (result, envelopes) = {
            // 2. Downcast UoW → PgUnitOfWork → client
            let pg = uow
                .as_any_mut()
                .downcast_mut::<db::PgUnitOfWork>()
                .ok_or_else(|| AppError::Internal("expected PgUnitOfWork".into()))?;
            // Explicit type: deref chain Object → ClientWrapper → Client
            let client: &Client = pg.client();

            // 3. Find or create InventoryItem
            let (item_id, old_balance) =
                if let Some((id, balance)) =
                    PgInventoryRepo::find_by_sku(client, ctx.tenant_id, sku.as_str())
                        .await
                        .map_err(|e| AppError::Internal(format!("find_by_sku: {e}")))?
                {
                    (id, balance)
                } else {
                    let new_id = EntityId::new();
                    PgInventoryRepo::create_item(
                        client,
                        ctx.tenant_id,
                        *new_id.as_uuid(),
                        sku.as_str(),
                    )
                    .await
                    .map_err(|e| AppError::Internal(format!("create_item: {e}")))?;
                    (*new_id.as_uuid(), BigDecimal::from(0))
                };

            // 4. Domain: item.receive()
            let old_balance_qty = Quantity::new(old_balance.clone())
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let mut item = InventoryItem::from_state(
                EntityId::from_uuid(item_id),
                sku.clone(),
                old_balance_qty,
            );

            // 5. SeqGen: номер документа
            let doc_number = seq_gen::PgSequenceGenerator::next_value(
                client,
                ctx.tenant_id,
                "warehouse.receipt",
                "ПРХ-",
            )
            .await
            .map_err(|e| AppError::Internal(format!("seq_gen: {e}")))?;

            let event = item.receive(&qty, doc_number.clone())?;
            let new_balance = event.new_balance.clone();
            let movement_id = Uuid::now_v7();

            // 6. Repo: save movement + upsert balance
            PgInventoryRepo::save_movement(
                client,
                ctx.tenant_id,
                movement_id,
                item_id,
                "goods_received",
                &event.quantity,
                &event.new_balance,
                &doc_number,
                ctx.correlation_id,
                *ctx.user_id.as_uuid(),
            )
            .await
            .map_err(|e| AppError::Internal(format!("save_movement: {e}")))?;

            PgInventoryRepo::upsert_balance(
                client,
                ctx.tenant_id,
                item_id,
                sku.as_str(),
                &event.new_balance,
                movement_id,
            )
            .await
            .map_err(|e| AppError::Internal(format!("upsert_balance: {e}")))?;

            // 7. Domain history: old → new state
            let old_state = serde_json::json!({ "balance": old_balance.to_string() });
            let new_state = serde_json::json!({ "balance": new_balance.to_string() });
            audit::DomainHistoryWriter::record(
                client,
                ctx,
                "inventory_item",
                item_id,
                "erp.warehouse.goods_received.v1",
                Some(&old_state),
                Some(&new_state),
            )
            .await
            .map_err(|e| AppError::Internal(format!("domain_history: {e}")))?;

            // 8. Collect outbox envelopes
            let events = item.take_events();
            let mut envelopes = Vec::with_capacity(events.len());
            for evt in &events {
                let envelope = EventEnvelope::from_domain_event(evt, ctx, "warehouse")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                envelopes.push(envelope);
            }

            (
                ReceiveGoodsResult {
                    item_id,
                    movement_id,
                    new_balance,
                    doc_number,
                },
                envelopes,
            )
        };
        // pg/client dropped — borrow released

        // 9. Add outbox entries
        for envelope in envelopes {
            uow.add_outbox_entry(envelope);
        }

        Ok(result)
    }
}
