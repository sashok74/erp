//! Integration-тесты для Catalog BC.
//!
//! Требуют реальную PostgreSQL через `DATABASE_URL`.

use std::sync::Arc;

use kernel::types::{RequestContext, TenantId, UserId};
use runtime::pipeline::CommandPipeline;
use runtime::stubs::NoopExtensionHooks;
use test_support::{
    cleanup_tenant as cleanup_tenant_rows, command_pipeline, request_context, shared_pool,
};

use catalog::application::commands::create_product::{CreateProductCommand, CreateProductHandler};
use catalog::application::queries::get_product::{GetProductHandler, GetProductQuery};

fn ctx_with_roles(roles: &[&str]) -> RequestContext {
    request_context(roles)
}

fn catalog_manager_ctx() -> RequestContext {
    ctx_with_roles(&["catalog_manager"])
}

const CATALOG_TABLES: &[&str] = &[
    "catalog.products",
    "common.outbox",
    "common.audit_log",
    "common.domain_history",
];

async fn setup_pool() -> Arc<db::PgPool> {
    shared_pool(&["../../migrations/common", "../../migrations/catalog"]).await
}

async fn cleanup_tenant(pool: &db::PgPool, tenant_id: TenantId) {
    cleanup_tenant_rows(pool, tenant_id, CATALOG_TABLES).await;
}

// ─── Test 1: Happy path — create product ────────────────────────────────────

#[tokio::test]
async fn happy_path_create_product() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let handler = CreateProductHandler::new(pool.clone());

    let ctx = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "BOLT-42".into(),
        name: "Болт М8".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await.unwrap();
    assert!(!result.product_id.is_nil());

    // Verify DB: product exists
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT sku, name FROM catalog.products WHERE tenant_id = $1 AND id = $2",
            &[ctx.tenant_id.as_uuid(), &result.product_id],
        )
        .await
        .unwrap();
    let sku: String = row.get(0);
    let name: String = row.get(1);
    assert_eq!(sku, "BOLT-42");
    assert_eq!(name, "Болт М8");

    // Verify outbox
    let outbox = client
        .query_one(
            "SELECT event_type FROM common.outbox \
             WHERE tenant_id = $1 AND correlation_id = $2",
            &[ctx.tenant_id.as_uuid(), &ctx.correlation_id],
        )
        .await
        .unwrap();
    let event_type: String = outbox.get(0);
    assert_eq!(event_type, "erp.catalog.product_created.v1");

    // Verify audit
    let audit = client
        .query_one(
            "SELECT command_name FROM common.audit_log \
             WHERE tenant_id = $1 AND correlation_id = $2",
            &[ctx.tenant_id.as_uuid(), &ctx.correlation_id],
        )
        .await
        .unwrap();
    let cmd_name: String = audit.get(0);
    assert_eq!(cmd_name, "catalog.create_product");

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 2: Duplicate SKU → error ──────────────────────────────────────────

#[tokio::test]
async fn duplicate_sku_rejected() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let handler = CreateProductHandler::new(pool.clone());

    let ctx = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "DUP-SKU".into(),
        name: "First".into(),
        category: "Cat".into(),
        unit: "шт".into(),
    };

    pipeline.execute(&handler, &cmd, &ctx).await.unwrap();

    // Second create with same SKU → error
    let cmd2 = CreateProductCommand {
        sku: "DUP-SKU".into(),
        name: "Second".into(),
        category: "Cat".into(),
        unit: "шт".into(),
    };
    let result = pipeline.execute(&handler, &cmd2, &ctx).await;
    assert!(result.is_err());

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 3: Unauthorized → 403 ────────────────────────────────────────────

#[tokio::test]
async fn unauthorized_rejected() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let handler = CreateProductHandler::new(pool.clone());

    let ctx = ctx_with_roles(&["viewer"]);
    let cmd = CreateProductCommand {
        sku: "UNAUTH-SKU".into(),
        name: "Test".into(),
        category: "".into(),
        unit: "шт".into(),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4: GetProduct query ───────────────────────────────────────────────

#[tokio::test]
async fn get_product_after_create() {
    use runtime::query_handler::QueryHandler;

    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let create_handler = CreateProductHandler::new(pool.clone());
    let query_handler = GetProductHandler::new(pool.clone());

    let ctx = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "QRY-BOLT".into(),
        name: "Болт запросный".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    pipeline.execute(&create_handler, &cmd, &ctx).await.unwrap();

    let query = GetProductQuery {
        sku: "QRY-BOLT".into(),
    };
    let result = query_handler.handle(&query, &ctx).await.unwrap();
    assert_eq!(result.sku, "QRY-BOLT");
    assert_eq!(result.name, "Болт запросный");
    assert!(!result.product_id.is_nil());

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 5: Cross-context E2E — ProductCreated → warehouse projection ──────

#[tokio::test]
async fn cross_context_product_projection() {
    use event_bus::EventBus;
    use std::time::Duration;

    let pool = setup_pool().await;

    // Also need warehouse migrations for projection table
    db::migrate::run_migrations(&pool, "../../migrations/warehouse")
        .await
        .unwrap();

    let bus = Arc::new(event_bus::InProcessBus::new());

    // Register warehouse handler for ProductCreated
    let wh_handler =
        warehouse::infrastructure::event_handlers::ProductCreatedHandler::new(pool.clone());
    let adapter = Arc::new(event_bus::EventHandlerAdapter::new(wh_handler));
    bus.subscribe("erp.catalog.product_created.v1", adapter)
        .await;

    // Build pipeline with this bus
    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));
    let perm_map = auth::rbac::default_erp_permissions();
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(perm_map));
    let audit_log = Arc::new(audit::PgAuditLog::new(pool.clone()));
    let pipeline = CommandPipeline::new(
        uow_factory,
        bus.clone(),
        checker,
        Arc::new(NoopExtensionHooks),
        audit_log,
    );

    let create_handler = CreateProductHandler::new(pool.clone());
    let ctx = catalog_manager_ctx();

    // 1. Create product in catalog
    let cmd = CreateProductCommand {
        sku: "CROSS-BOLT".into(),
        name: "Болт кросс-контекст".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    pipeline.execute(&create_handler, &cmd, &ctx).await.unwrap();

    // 2. Run outbox relay → publish_and_wait → warehouse handler upserts projection
    let relay = db::OutboxRelay::new(pool.clone(), bus, Duration::from_millis(100), 10);
    let _ = relay.poll_and_publish().await;

    // 3. Verify projection exists in warehouse schema
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT name FROM warehouse.product_projections \
             WHERE tenant_id = $1 AND sku = $2",
            &[ctx.tenant_id.as_uuid(), &"CROSS-BOLT"],
        )
        .await
        .unwrap();
    let name: String = row.get(0);
    assert_eq!(name, "Болт кросс-контекст");

    // Cleanup
    client
        .execute(
            "DELETE FROM warehouse.product_projections WHERE tenant_id = $1",
            &[ctx.tenant_id.as_uuid()],
        )
        .await
        .ok();
    client
        .execute("DELETE FROM common.inbox WHERE event_id IN (SELECT event_id FROM common.outbox WHERE tenant_id = $1)", &[ctx.tenant_id.as_uuid()])
        .await
        .ok();
    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 6: GetBalance enriched with product_name ──────────────────────────

#[tokio::test]
async fn get_balance_enriched_with_product_name() {
    use bigdecimal::BigDecimal;
    use event_bus::EventBus;
    use runtime::query_handler::QueryHandler;
    use std::time::Duration;

    let pool = setup_pool().await;

    db::migrate::run_migrations(&pool, "../../migrations/warehouse")
        .await
        .unwrap();

    let bus = Arc::new(event_bus::InProcessBus::new());

    // Register warehouse handler
    let wh_handler =
        warehouse::infrastructure::event_handlers::ProductCreatedHandler::new(pool.clone());
    let adapter = Arc::new(event_bus::EventHandlerAdapter::new(wh_handler));
    bus.subscribe("erp.catalog.product_created.v1", adapter)
        .await;

    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));
    let perm_map = auth::rbac::default_erp_permissions();
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(perm_map));
    let audit_log = Arc::new(audit::PgAuditLog::new(pool.clone()));
    let pipeline = CommandPipeline::new(
        uow_factory,
        bus.clone(),
        checker,
        Arc::new(NoopExtensionHooks),
        audit_log,
    );

    // Use ctx with both catalog and warehouse permissions
    let mut ctx = RequestContext::new(TenantId::new(), UserId::new());
    ctx.roles = vec!["admin".to_string()];

    // 1. Create product in catalog
    let create_handler = CreateProductHandler::new(pool.clone());
    let cmd = CreateProductCommand {
        sku: "ENRICH-BOLT".into(),
        name: "Болт обогащённый".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    pipeline.execute(&create_handler, &cmd, &ctx).await.unwrap();

    // 2. Relay → publish_and_wait → projection upserted
    let relay = db::OutboxRelay::new(pool.clone(), bus, Duration::from_millis(100), 10);
    let _ = relay.poll_and_publish().await;

    // 3. Receive goods in warehouse (need new ctx for fresh correlation_id)
    let mut ctx2 = RequestContext::new(ctx.tenant_id, ctx.user_id);
    ctx2.roles = vec!["admin".to_string()];

    let receive_handler =
        warehouse::application::commands::receive_goods::ReceiveGoodsHandler::new(pool.clone());
    let receive_cmd = warehouse::application::commands::receive_goods::ReceiveGoodsCommand {
        sku: "ENRICH-BOLT".into(),
        quantity: BigDecimal::from(100),
    };
    pipeline
        .execute(&receive_handler, &receive_cmd, &ctx2)
        .await
        .unwrap();

    // 4. GetBalance → should include product_name
    let balance_handler =
        warehouse::application::queries::get_balance::GetBalanceHandler::new(pool.clone());
    let query = warehouse::application::queries::get_balance::GetBalanceQuery {
        sku: "ENRICH-BOLT".into(),
    };
    let balance = balance_handler.handle(&query, &ctx2).await.unwrap();
    assert_eq!(balance.balance, BigDecimal::from(100));
    assert_eq!(balance.product_name.as_deref(), Some("Болт обогащённый"));

    // Cleanup
    let client = pool.get().await.unwrap();
    let tid = ctx.tenant_id.as_uuid();
    client
        .execute(
            "DELETE FROM warehouse.product_projections WHERE tenant_id = $1",
            &[tid],
        )
        .await
        .ok();
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
    client
        .execute("DELETE FROM common.sequences WHERE tenant_id = $1", &[tid])
        .await
        .ok();
    client
        .execute("DELETE FROM common.inbox WHERE event_id IN (SELECT event_id FROM common.outbox WHERE tenant_id = $1)", &[tid])
        .await
        .ok();
    cleanup_tenant(&pool, ctx.tenant_id).await;
}
