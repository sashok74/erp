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
use kernel::types::{RequestContext, TenantId, UserId};
use runtime::pipeline::CommandPipeline;
use runtime::stubs::NoopExtensionHooks;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use warehouse::application::commands::receive_goods::{ReceiveGoodsCommand, ReceiveGoodsHandler};
use warehouse::application::queries::get_balance::{GetBalanceHandler, GetBalanceQuery};

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests")
}

fn ctx_with_roles(roles: &[&str]) -> RequestContext {
    let mut ctx = RequestContext::new(TenantId::new(), UserId::new());
    ctx.roles = roles.iter().map(|s| (*s).to_string()).collect();
    ctx
}

fn operator_ctx() -> RequestContext {
    ctx_with_roles(&["warehouse_operator"])
}

/// Shared pool with migrations applied once.
static POOL: tokio::sync::OnceCell<Arc<db::PgPool>> = tokio::sync::OnceCell::const_new();

async fn setup_pool() -> Arc<db::PgPool> {
    POOL.get_or_init(|| async {
        let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());

        // Ensure migrations applied (idempotent).
        db::migrate::run_migrations(&pool, "../../migrations/common")
            .await
            .unwrap();
        db::migrate::run_migrations(&pool, "../../migrations/warehouse")
            .await
            .unwrap();

        pool
    })
    .await
    .clone()
}

fn make_pipeline(
    pool: Arc<db::PgPool>,
    bus: Arc<InProcessBus>,
) -> CommandPipeline<db::PgUnitOfWorkFactory> {
    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));
    let perm_map = auth::rbac::default_erp_permissions();
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(perm_map));
    let audit = Arc::new(audit::PgAuditLog::new(pool));

    CommandPipeline::new(uow_factory, bus, checker, Arc::new(NoopExtensionHooks), audit)
}

/// Cleanup helper: delete warehouse data for a specific tenant.
async fn cleanup_tenant(pool: &db::PgPool, tenant_id: TenantId) {
    let client = pool.get().await.unwrap();
    let tid = tenant_id.as_uuid();

    // Warehouse tables
    client
        .execute(
            "DELETE FROM warehouse.inventory_balances WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
    client
        .execute(
            "DELETE FROM warehouse.stock_movements WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
    client
        .execute(
            "DELETE FROM warehouse.inventory_items WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
    // Common tables
    client
        .execute(
            "DELETE FROM common.outbox WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
    client
        .execute(
            "DELETE FROM common.audit_log WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
    client
        .execute(
            "DELETE FROM common.domain_history WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
    client
        .execute(
            "DELETE FROM common.sequences WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
}

// ─── Test 1: Happy path — receive goods ─────────────────────────────────────

#[tokio::test]
async fn happy_path_receive_goods() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = make_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new(pool.clone());

    let ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-42".into(),
        quantity: BigDecimal::from(100),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await.unwrap();

    // Assert result
    assert_eq!(result.new_balance, BigDecimal::from(100));
    assert!(result.doc_number.starts_with("ПРХ-"));

    // Assert DB: inventory_balances
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT balance::TEXT FROM warehouse.inventory_balances \
             WHERE tenant_id = $1 AND item_id = $2",
            &[ctx.tenant_id.as_uuid(), &result.item_id],
        )
        .await
        .unwrap();
    let balance: String = row.get(0);
    assert_eq!(balance, "100.0000");

    // Assert DB: stock_movements (1 row)
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM warehouse.stock_movements \
             WHERE tenant_id = $1 AND item_id = $2",
            &[ctx.tenant_id.as_uuid(), &result.item_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);

    // Assert DB: common.outbox
    let outbox_row = client
        .query_one(
            "SELECT event_type FROM common.outbox \
             WHERE tenant_id = $1 AND correlation_id = $2",
            &[ctx.tenant_id.as_uuid(), &ctx.correlation_id],
        )
        .await
        .unwrap();
    let event_type: String = outbox_row.get(0);
    assert_eq!(event_type, "erp.warehouse.goods_received.v1");

    // Assert DB: common.audit_log
    let audit_row = client
        .query_one(
            "SELECT command_name FROM common.audit_log \
             WHERE tenant_id = $1 AND correlation_id = $2",
            &[ctx.tenant_id.as_uuid(), &ctx.correlation_id],
        )
        .await
        .unwrap();
    let command_name: String = audit_row.get(0);
    assert_eq!(command_name, "warehouse.receive_goods");

    // Assert DB: common.domain_history
    let history_row = client
        .query_one(
            "SELECT entity_type, event_type FROM common.domain_history \
             WHERE tenant_id = $1 AND correlation_id = $2",
            &[ctx.tenant_id.as_uuid(), &ctx.correlation_id],
        )
        .await
        .unwrap();
    let entity_type: String = history_row.get(0);
    let history_event_type: String = history_row.get(1);
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
    let pipeline = make_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new(pool.clone());

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
    let client = pool.get().await.unwrap();
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM warehouse.stock_movements \
             WHERE tenant_id = $1 AND item_id = $2",
            &[ctx.tenant_id.as_uuid(), &r1.item_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 2);

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 3: Unauthorized → error, no side effects ─────────────────────────

#[tokio::test]
async fn unauthorized_rejected() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = make_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new(pool.clone());

    let ctx = ctx_with_roles(&["viewer"]); // no warehouse permissions
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-UNAUTH".into(),
        quantity: BigDecimal::from(100),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    // No side effects
    let client = pool.get().await.unwrap();
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM warehouse.stock_movements \
             WHERE tenant_id = $1",
            &[ctx.tenant_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0, "no movements should exist for unauthorized request");

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4: GetBalance query ───────────────────────────────────────────────

#[tokio::test]
async fn get_balance_after_receive() {
    use runtime::query_handler::QueryHandler;

    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = make_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new(pool.clone());
    let query_handler = GetBalanceHandler::new(pool.clone());

    let ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-QRY".into(),
        quantity: BigDecimal::from(100),
    };
    pipeline.execute(&handler, &cmd, &ctx).await.unwrap();

    // Query balance
    let query = GetBalanceQuery {
        sku: "BOLT-QRY".into(),
    };
    let balance = query_handler.handle(&query, &ctx).await.unwrap();
    assert_eq!(balance.balance, BigDecimal::from(100));
    assert!(balance.item_id.is_some());

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 5: RLS tenant isolation ───────────────────────────────────────────

#[tokio::test]
async fn rls_tenant_isolation() {
    use runtime::query_handler::QueryHandler;

    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = make_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new(pool.clone());
    let query_handler = GetBalanceHandler::new(pool.clone());

    // Tenant A receives goods
    let ctx_a = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-RLS".into(),
        quantity: BigDecimal::from(100),
    };
    pipeline.execute(&handler, &cmd, &ctx_a).await.unwrap();

    // Tenant B queries same SKU → not found
    let ctx_b = operator_ctx(); // different tenant_id
    let query = GetBalanceQuery {
        sku: "BOLT-RLS".into(),
    };
    let balance = query_handler.handle(&query, &ctx_b).await.unwrap();
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

    let pipeline = make_pipeline(pool.clone(), bus.clone());
    let cmd_handler = ReceiveGoodsHandler::new(pool.clone());

    let ctx = operator_ctx();
    let cmd = ReceiveGoodsCommand {
        sku: "BOLT-RELAY".into(),
        quantity: BigDecimal::from(100),
    };
    pipeline.execute(&cmd_handler, &cmd, &ctx).await.unwrap();

    // Run relay: single poll
    let relay = db::OutboxRelay::new(pool.clone(), bus, Duration::from_millis(100), 10);
    let _ = relay.poll_and_publish().await;

    // Give async handler time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(called.load(Ordering::SeqCst), "subscriber should have been called");

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 7: Doc number sequence gap-free ───────────────────────────────────

#[tokio::test]
async fn doc_number_sequence_gap_free() {
    let pool = setup_pool().await;
    let bus = Arc::new(InProcessBus::new());
    let pipeline = make_pipeline(pool.clone(), bus);
    let handler = ReceiveGoodsHandler::new(pool.clone());

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
