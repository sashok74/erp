//! `ReceiveGoodsCommand` — приёмка товара на склад.
//!
//! Полный canonical write path:
//! validate → find/create item → receive → movement + balance → history → seq → outbox.

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use db::PgCommandContext;
use kernel::types::EntityId;
use kernel::{AppError, Command, IntoInternal, RequestContext};
use runtime::command_handler::CommandHandler;
use runtime::ports::UnitOfWork;
use serde::Serialize;
use uuid::Uuid;

use crate::application::ports::InventoryRepo;
use crate::domain::aggregates::InventoryItem;
use crate::domain::value_objects::{Quantity, Sku};

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
#[derive(Default)]
pub struct ReceiveGoodsHandler;

impl ReceiveGoodsHandler {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
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

        // 2. Downcast UoW → PgCommandContext
        let mut db = PgCommandContext::from_uow(uow)?;

        // 3-6. Repo operations (scoped to drop repo before db.record_change)
        let repo = InventoryRepo::new(db.client(), ctx.tenant_id);

        let (item_id, old_balance) = if let Some((id, balance)) =
            repo.find_by_sku(sku.as_str()).await?
        {
            (id, balance)
        } else {
            let new_id = EntityId::new();
            repo.create_item(*new_id.as_uuid(), sku.as_str()).await?;
            (*new_id.as_uuid(), BigDecimal::from(0))
        };

        // 4. Domain: item.receive()
        let old_balance_qty = Quantity::new(old_balance.clone()).internal("balance")?;
        let mut item =
            InventoryItem::from_state(EntityId::from_uuid(item_id), sku.clone(), old_balance_qty);

        // 5. SeqGen: номер документа
        let doc_number = seq_gen::PgSequenceGenerator::next_value(
            db.client(),
            ctx.tenant_id,
            "warehouse.receipt",
            "ПРХ-",
        )
        .await
        .internal("seq_gen")?;

        let event = item.receive(&qty, doc_number.clone())?;
        let new_balance = event.new_balance.clone();
        let movement_id = Uuid::now_v7();

        // 6. Repo: save movement + upsert balance
        repo.save_movement(
            movement_id,
            item_id,
            "goods_received",
            &event.quantity,
            &event.new_balance,
            &doc_number,
            ctx.correlation_id,
            *ctx.user_id.as_uuid(),
        )
        .await?;

        repo.upsert_balance(item_id, sku.as_str(), &event.new_balance, movement_id)
            .await?;

        // 7. Domain history: old → new state (deferred — flush в commit)
        let old_state = serde_json::json!({ "balance": old_balance.to_string() });
        let new_state = serde_json::json!({ "balance": new_balance.to_string() });
        db.record_change(
            ctx,
            "inventory_item",
            item_id,
            "erp.warehouse.goods_received.v1",
            Some(&old_state),
            Some(&new_state),
        )?;

        // 8. Emit events to outbox
        db.emit_events(&mut item, ctx, "warehouse")?;

        Ok(ReceiveGoodsResult {
            item_id,
            movement_id,
            new_balance,
            doc_number,
        })
    }
}
