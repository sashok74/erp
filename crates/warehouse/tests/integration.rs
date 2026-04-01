//! Integration-тесты для Warehouse BC.
//!
//! Требуют реальную PostgreSQL через `DATABASE_URL`.
//! E2E canonical write path: command → handler → DB → outbox → event.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use event_bus::traits::{EventBus, EventHandler};
use event_bus::{EventHandlerAdapter, InProcessBus};
use kernel::types::{RequestContext, TenantId};
use serde::{Deserialize, Serialize};
use test_support::{
    cleanup_tenant as cleanup_tenant_rows, command_pipeline, request_context, shared_pool,
};
use uuid::Uuid;

use warehouse::application::commands::receive_goods::{ReceiveGoodsCommand, ReceiveGoodsHandler};
use warehouse::application::queries::get_balance::{GetBalanceHandler, GetBalanceQuery};

fn ctx_with_roles(roles: &[&str]) -> RequestContext {
    request_context(roles)
}

fn operator_ctx() -> RequestContext {
    ctx_with_roles(&["warehouse_operator"])
}

const WAREHOUSE_TABLES: &[&str] = &[
    "warehouse.inventory_balances",
    "warehouse.stock_movements",
    "warehouse.inventory_items",
    "common.outbox",
    "common.audit_log",
    "common.domain_history",
    "common.sequences",
];

async fn setup_pool() -> Arc<db::PgPool> {
    shared_pool(&["../../migrations/common", "../../migrations/warehouse"]).await
}

/// Cleanup helper: delete warehouse data for a specific tenant.
async fn cleanup_tenant(pool: &db::PgPool, tenant_id: TenantId) {
    cleanup_tenant_rows(pool, tenant_id, WAREHOUSE_TABLES).await;
}

// ─── Test 1: Happy path — receive goods ─────────────────────────────────────

#[tokio::test]
async fn happy_path_receive_goods() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = command_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new();

    let ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-42".into(),
        quantity: BigDecimal::from(100),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await.unwrap();

    // Assert result
    assert_eq!(result.new_balance, BigDecimal::from(100));
    assert!(result.doc_number.starts_with("ПРХ-"));

    // Assert DB: all checks in a single tenant-scoped read TX
    let item_id = result.item_id;
    let corr_id = ctx.correlation_id;
    let (balance, count, event_type, command_name, entity_type, history_event_type) =
        test_support::tenant_query(&pool, ctx.tenant_id, |client| {
            Box::pin(async move {
                let row = client
                    .query_one(
                        "SELECT balance::TEXT FROM warehouse.inventory_balances \
                     WHERE tenant_id = $1 AND item_id = $2",
                        &[ctx.tenant_id.as_uuid(), &item_id],
                    )
                    .await
                    .unwrap();
                let balance: String = row.get(0);

                let count: i64 = client
                    .query_one(
                        "SELECT COUNT(*) FROM warehouse.stock_movements \
                     WHERE tenant_id = $1 AND item_id = $2",
                        &[ctx.tenant_id.as_uuid(), &item_id],
                    )
                    .await
                    .unwrap()
                    .get(0);

                let outbox_row = client
                    .query_one(
                        "SELECT event_type FROM common.outbox \
                     WHERE tenant_id = $1 AND correlation_id = $2",
                        &[ctx.tenant_id.as_uuid(), &corr_id],
                    )
                    .await
                    .unwrap();
                let event_type: String = outbox_row.get(0);

                let audit_row = client
                    .query_one(
                        "SELECT command_name FROM common.audit_log \
                     WHERE tenant_id = $1 AND correlation_id = $2",
                        &[ctx.tenant_id.as_uuid(), &corr_id],
                    )
                    .await
                    .unwrap();
                let command_name: String = audit_row.get(0);

                let history_row = client
                    .query_one(
                        "SELECT entity_type, event_type FROM common.domain_history \
                     WHERE tenant_id = $1 AND correlation_id = $2",
                        &[ctx.tenant_id.as_uuid(), &corr_id],
                    )
                    .await
                    .unwrap();
                let entity_type: String = history_row.get(0);
                let history_event_type: String = history_row.get(1);

                (
                    balance,
                    count,
                    event_type,
                    command_name,
                    entity_type,
                    history_event_type,
                )
            })
        })
        .await;

    assert_eq!(balance, "100.0000");
    assert_eq!(count, 1);
    assert_eq!(event_type, "erp.warehouse.goods_received.v1");
    assert_eq!(command_name, "warehouse.receive_goods");
    assert_eq!(entity_type, "inventory_item");
    assert_eq!(history_event_type, "erp.warehouse.goods_received.v1");

    // Cleanup
    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 2: Second receive — cumulative ────────────────────────────────────

#[tokio::test]
async fn cumulative_receive() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = command_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new();

    let ctx = operator_ctx();

    // First receive: 100
    let cmd1 = ReceiveGoodsCommand {
        sku: "BOLT-CUM".into(),
        quantity: BigDecimal::from(100),
    };
    let r1 = pipeline.execute(&handler, &cmd1, &ctx).await.unwrap();
    assert_eq!(r1.new_balance, BigDecimal::from(100));

    // Second receive: 50 (same ctx reuses correlation_id, so new ctx needed)
    let ctx2 = operator_ctx();
    // Copy tenant/user from first ctx for same tenant
    let mut ctx2 = ctx2;
    ctx2.tenant_id = ctx.tenant_id;
    ctx2.user_id = ctx.user_id;
    ctx2.roles = ctx.roles.clone();

    let cmd2 = ReceiveGoodsCommand {
        sku: "BOLT-CUM".into(),
        quantity: BigDecimal::from(50),
    };
    let r2 = pipeline.execute(&handler, &cmd2, &ctx2).await.unwrap();
    assert_eq!(r2.new_balance, BigDecimal::from(150));

    // Assert 2 movements
    let item_id = r1.item_id;
    let count = test_support::tenant_query(&pool, ctx.tenant_id, |client| {
        Box::pin(async move {
            let count: i64 = client
                .query_one(
                    "SELECT COUNT(*) FROM warehouse.stock_movements \
                 WHERE tenant_id = $1 AND item_id = $2",
                    &[ctx.tenant_id.as_uuid(), &item_id],
                )
                .await
                .unwrap()
                .get(0);
            count
        })
    })
    .await;
    assert_eq!(count, 2);

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 3: Unauthorized → error, no side effects ─────────────────────────

#[tokio::test]
async fn unauthorized_rejected() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = command_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new();

    let ctx = ctx_with_roles(&["viewer"]); // no warehouse permissions
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-UNAUTH".into(),
        quantity: BigDecimal::from(100),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    // No side effects
    let count = test_support::tenant_query(&pool, ctx.tenant_id, |client| {
        Box::pin(async move {
            let count: i64 = client
                .query_one(
                    "SELECT COUNT(*) FROM warehouse.stock_movements \
                 WHERE tenant_id = $1",
                    &[ctx.tenant_id.as_uuid()],
                )
                .await
                .unwrap()
                .get(0);
            count
        })
    })
    .await;
    assert_eq!(
        count, 0,
        "no movements should exist for unauthorized request"
    );

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4: GetBalance query via QueryPipeline ────────────────────────────

#[tokio::test]
async fn get_balance_after_receive() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let cmd_pipeline = command_pipeline(pool.clone(), bus);
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let handler = ReceiveGoodsHandler::new();
    let query_handler = GetBalanceHandler::new(pool.clone());

    let ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-QRY".into(),
        quantity: BigDecimal::from(100),
    };
    cmd_pipeline.execute(&handler, &cmd, &ctx).await.unwrap();

    // Query balance through QueryPipeline (auth checked)
    let query = GetBalanceQuery {
        sku: "BOLT-QRY".into(),
    };
    let balance = qry_pipeline
        .execute(&query_handler, &query, &ctx)
        .await
        .unwrap();
    assert_eq!(balance.balance, BigDecimal::from(100));
    assert!(balance.item_id.is_some());

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4a: Viewer can query warehouse balance ────────────────────────────

#[tokio::test]
async fn viewer_can_query_warehouse_balance() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let cmd_pipeline = command_pipeline(pool.clone(), bus);
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let handler = ReceiveGoodsHandler::new();
    let query_handler = GetBalanceHandler::new(pool.clone());

    // Create data as operator
    let op_ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-VIEW".into(),
        quantity: BigDecimal::from(50),
    };
    cmd_pipeline.execute(&handler, &cmd, &op_ctx).await.unwrap();

    // Query as viewer — should succeed (explicit grant in manifest)
    let mut viewer_ctx = ctx_with_roles(&["viewer"]);
    viewer_ctx.tenant_id = op_ctx.tenant_id;

    let query = GetBalanceQuery {
        sku: "BOLT-VIEW".into(),
    };
    let balance = qry_pipeline
        .execute(&query_handler, &query, &viewer_ctx)
        .await
        .unwrap();
    assert_eq!(balance.balance, BigDecimal::from(50));

    cleanup_tenant(&pool, op_ctx.tenant_id).await;
}

// ─── Test 4b: Catalog role denied warehouse query ───────────────────────────

#[tokio::test]
async fn catalog_role_denied_warehouse_query() {
    let pool = setup_pool().await;
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let query_handler = GetBalanceHandler::new(pool.clone());

    let ctx = ctx_with_roles(&["catalog_manager"]);
    let query = GetBalanceQuery {
        sku: "BOLT-DENY".into(),
    };
    let result = qry_pipeline.execute(&query_handler, &query, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4c: Unknown role denied warehouse command ─────────────────────────

#[tokio::test]
async fn unknown_role_denied_warehouse_command() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = command_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new();

    let ctx = ctx_with_roles(&["nonexistent_role"]);
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-UNKNOWN".into(),
        quantity: BigDecimal::from(100),
    };
    let result = pipeline.execute(&handler, &cmd, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4d: Unknown role denied warehouse query ───────────────────────────

#[tokio::test]
async fn unknown_role_denied_warehouse_query() {
    let pool = setup_pool().await;
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let query_handler = GetBalanceHandler::new(pool.clone());

    let ctx = ctx_with_roles(&["nonexistent_role"]);
    let query = GetBalanceQuery {
        sku: "BOLT-UNKNOWN".into(),
    };
    let result = qry_pipeline.execute(&query_handler, &query, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 5: RLS tenant isolation via QueryPipeline ─────────────────────────

#[tokio::test]
async fn rls_tenant_isolation() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let cmd_pipeline = command_pipeline(pool.clone(), bus);
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let handler = ReceiveGoodsHandler::new();
    let query_handler = GetBalanceHandler::new(pool.clone());

    // Tenant A receives goods
    let ctx_a = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-RLS".into(),
        quantity: BigDecimal::from(100),
    };
    cmd_pipeline.execute(&handler, &cmd, &ctx_a).await.unwrap();

    // Tenant B queries same SKU → not found
    let ctx_b = operator_ctx(); // different tenant_id
    let query = GetBalanceQuery {
        sku: "BOLT-RLS".into(),
    };
    let balance = qry_pipeline
        .execute(&query_handler, &query, &ctx_b)
        .await
        .unwrap();
    assert!(
        balance.item_id.is_none(),
        "tenant B should not see tenant A's data"
    );
    assert_eq!(balance.balance, BigDecimal::from(0));

    cleanup_tenant(&pool, ctx_a.tenant_id).await;
    cleanup_tenant(&pool, ctx_b.tenant_id).await;
}

// ─── Test 6: Outbox relay → subscriber called ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoodsReceivedTest {
    pub item_id: Uuid,
    pub sku: String,
}

impl kernel::DomainEvent for GoodsReceivedTest {
    fn event_type(&self) -> &'static str {
        "erp.warehouse.goods_received.v1"
    }
    fn aggregate_id(&self) -> Uuid {
        self.item_id
    }
}

struct FlagHandler {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl EventHandler for FlagHandler {
    type Event = GoodsReceivedTest;

    async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
        self.called.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn handled_event_type(&self) -> &'static str {
        "erp.warehouse.goods_received.v1"
    }
}

#[tokio::test]
async fn outbox_relay_delivers_event() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());

    // Subscribe handler
    let called = Arc::new(AtomicBool::new(false));
    let handler = FlagHandler {
        called: called.clone(),
    };
    let adapted = Arc::new(EventHandlerAdapter::new(handler));
    bus.subscribe("erp.warehouse.goods_received.v1", adapted)
        .await;

    let pipeline = command_pipeline(pool.clone(), bus.clone());
    let cmd_handler = ReceiveGoodsHandler::new();

    let ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-RELAY".into(),
        quantity: BigDecimal::from(100),
    };
    pipeline.execute(&cmd_handler, &cmd, &ctx).await.unwrap();

    // Run relay: single poll
    let relay = db::OutboxRelay::new(
        pool.clone(),
        bus,
        Duration::from_millis(100),
        10,
        tokio_util::sync::CancellationToken::new(),
    );
    let _ = relay.poll_and_publish().await;

    // Give async handler time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        called.load(Ordering::SeqCst),
        "subscriber should have been called"
    );

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 7: Doc number sequence gap-free ───────────────────────────────────

#[tokio::test]
async fn doc_number_sequence_gap_free() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = command_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new();

    let ctx = operator_ctx();

    let mut doc_numbers = Vec::new();
    for i in 0..3 {
        let mut cmd_ctx = operator_ctx();
        cmd_ctx.tenant_id = ctx.tenant_id;
        cmd_ctx.user_id = ctx.user_id;
        cmd_ctx.roles = ctx.roles.clone();

        let cmd = ReceiveGoodsCommand {
            sku: format!("BOLT-SEQ-{i}"),
            quantity: BigDecimal::from(10),
        };
        let result = pipeline.execute(&handler, &cmd, &cmd_ctx).await.unwrap();
        doc_numbers.push(result.doc_number);
    }

    assert_eq!(doc_numbers[0], "ПРХ-000001");
    assert_eq!(doc_numbers[1], "ПРХ-000002");
    assert_eq!(doc_numbers[2], "ПРХ-000003");

    cleanup_tenant(&pool, ctx.tenant_id).await;
}
