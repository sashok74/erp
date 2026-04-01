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
    let handler = CreateProductHandler::new();

    let ctx = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "BOLT-42".into(),
        name: "Болт М8".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };

    let result = pipeline.execute(&handler, &cmd, &ctx).await.unwrap();
    assert!(!result.product_id.is_nil());

    // Verify DB: product, outbox, audit — all in a single tenant-scoped read TX
    let product_id = result.product_id;
    let corr_id = ctx.correlation_id;
    let (sku, name, event_type, cmd_name) =
        test_support::tenant_query(&pool, ctx.tenant_id, |client| Box::pin(async move {
            let row = client
                .query_one(
                    "SELECT sku, name FROM catalog.products WHERE tenant_id = $1 AND id = $2",
                    &[ctx.tenant_id.as_uuid(), &product_id],
                )
                .await
                .unwrap();
            let sku: String = row.get(0);
            let name: String = row.get(1);

            let outbox = client
                .query_one(
                    "SELECT event_type FROM common.outbox \
                     WHERE tenant_id = $1 AND correlation_id = $2",
                    &[ctx.tenant_id.as_uuid(), &corr_id],
                )
                .await
                .unwrap();
            let event_type: String = outbox.get(0);

            let audit = client
                .query_one(
                    "SELECT command_name FROM common.audit_log \
                     WHERE tenant_id = $1 AND correlation_id = $2",
                    &[ctx.tenant_id.as_uuid(), &corr_id],
                )
                .await
                .unwrap();
            let cmd_name: String = audit.get(0);

            (sku, name, event_type, cmd_name)
        })).await;

    assert_eq!(sku, "BOLT-42");
    assert_eq!(name, "Болт М8");
    assert_eq!(event_type, "erp.catalog.product_created.v1");
    assert_eq!(cmd_name, "catalog.create_product");

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 2: Duplicate SKU → error ──────────────────────────────────────────

#[tokio::test]
async fn duplicate_sku_rejected() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let handler = CreateProductHandler::new();

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
    let handler = CreateProductHandler::new();

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

// ─── Test 4: GetProduct query via QueryPipeline ─────────────────────────────

#[tokio::test]
async fn get_product_after_create() {
    let pool = setup_pool().await;
    let cmd_pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let create_handler = CreateProductHandler::new();
    let query_handler = GetProductHandler::new(pool.clone());

    let ctx = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "QRY-BOLT".into(),
        name: "Болт запросный".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    cmd_pipeline
        .execute(&create_handler, &cmd, &ctx)
        .await
        .unwrap();

    let query = GetProductQuery {
        sku: "QRY-BOLT".into(),
    };
    let result = qry_pipeline
        .execute(&query_handler, &query, &ctx)
        .await
        .unwrap();
    assert_eq!(result.sku, "QRY-BOLT");
    assert_eq!(result.name, "Болт запросный");
    assert!(!result.product_id.is_nil());

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4a: Viewer can query catalog via QueryPipeline ────────────────────

#[tokio::test]
async fn viewer_can_query_catalog_product() {
    let pool = setup_pool().await;
    let cmd_pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let create_handler = CreateProductHandler::new();
    let query_handler = GetProductHandler::new(pool.clone());

    // Create as catalog_manager
    let mgr_ctx = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "VIEW-BOLT".into(),
        name: "Болт для просмотра".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    cmd_pipeline
        .execute(&create_handler, &cmd, &mgr_ctx)
        .await
        .unwrap();

    // Query as viewer — should succeed (explicit grant in manifest)
    let mut viewer_ctx = ctx_with_roles(&["viewer"]);
    viewer_ctx.tenant_id = mgr_ctx.tenant_id;

    let query = GetProductQuery {
        sku: "VIEW-BOLT".into(),
    };
    let result = qry_pipeline
        .execute(&query_handler, &query, &viewer_ctx)
        .await
        .unwrap();
    assert_eq!(result.sku, "VIEW-BOLT");

    cleanup_tenant(&pool, mgr_ctx.tenant_id).await;
}

// ─── Test 4b: Viewer denied command via CommandPipeline ─────────────────────

#[tokio::test]
async fn viewer_denied_catalog_command() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let handler = CreateProductHandler::new();

    let ctx = ctx_with_roles(&["viewer"]);
    let cmd = CreateProductCommand {
        sku: "VIEW-DENY".into(),
        name: "Test".into(),
        category: "Cat".into(),
        unit: "шт".into(),
    };
    let result = pipeline.execute(&handler, &cmd, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 4c: Warehouse role denied catalog query ───────────────────────────

#[tokio::test]
async fn warehouse_role_denied_catalog_query() {
    let pool = setup_pool().await;
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let query_handler = GetProductHandler::new(pool.clone());

    let ctx = ctx_with_roles(&["warehouse_operator"]);
    let query = GetProductQuery {
        sku: "CROSS-DENY".into(),
    };
    let result = qry_pipeline.execute(&query_handler, &query, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 5a: Unknown role denied catalog command ──────────────────────────

#[tokio::test]
async fn unknown_role_denied_catalog_command() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let handler = CreateProductHandler::new();

    let ctx = ctx_with_roles(&["nonexistent_role"]);
    let cmd = CreateProductCommand {
        sku: "UNKNOWN-CMD".into(),
        name: "Test".into(),
        category: "Cat".into(),
        unit: "шт".into(),
    };
    let result = pipeline.execute(&handler, &cmd, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 5b: Unknown role denied catalog query ────────────────────────────

#[tokio::test]
async fn unknown_role_denied_catalog_query() {
    let pool = setup_pool().await;
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let query_handler = GetProductHandler::new(pool.clone());

    let ctx = ctx_with_roles(&["nonexistent_role"]);
    let query = GetProductQuery {
        sku: "UNKNOWN-QRY".into(),
    };
    let result = qry_pipeline.execute(&query_handler, &query, &ctx).await;
    assert!(matches!(result, Err(kernel::AppError::Unauthorized(_))));

    cleanup_tenant(&pool, ctx.tenant_id).await;
}

// ─── Test 5c: Catalog query respects tenant isolation ───────────────────────

#[tokio::test]
async fn catalog_query_tenant_isolation() {
    let pool = setup_pool().await;
    let cmd_pipeline = command_pipeline(pool.clone(), Arc::new(event_bus::InProcessBus::new()));
    let qry_pipeline = test_support::query_pipeline(pool.clone());
    let create_handler = CreateProductHandler::new();
    let query_handler = GetProductHandler::new(pool.clone());

    // Tenant A creates a product
    let ctx_a = catalog_manager_ctx();
    let cmd = CreateProductCommand {
        sku: "TENANT-BOLT".into(),
        name: "Товар tenant A".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    cmd_pipeline
        .execute(&create_handler, &cmd, &ctx_a)
        .await
        .unwrap();

    // Tenant B with the same role cannot see tenant A data
    let ctx_b = catalog_manager_ctx();
    let query = GetProductQuery {
        sku: "TENANT-BOLT".into(),
    };
    let result = qry_pipeline.execute(&query_handler, &query, &ctx_b).await;
    assert!(matches!(
        result,
        Err(kernel::AppError::Domain(kernel::DomainError::NotFound(_)))
    ));

    cleanup_tenant(&pool, ctx_a.tenant_id).await;
    cleanup_tenant(&pool, ctx_b.tenant_id).await;
}

// ─── Test 6: Cross-context E2E — ProductCreated → warehouse projection ──────

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
    let registry = test_support::test_permission_registry();
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(registry));
    let audit_log = Arc::new(audit::PgAuditLog::new(pool.clone()));
    let pipeline = CommandPipeline::new(
        uow_factory,
        bus.clone(),
        checker,
        Arc::new(NoopExtensionHooks),
        audit_log,
    );

    let create_handler = CreateProductHandler::new();
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
    let relay = db::OutboxRelay::new(pool.clone(), bus, Duration::from_millis(100), 10, tokio_util::sync::CancellationToken::new());
    let _ = relay.poll_and_publish().await;

    // 3. Verify projection exists in warehouse schema
    let name = test_support::tenant_query(&pool, ctx.tenant_id, |client| Box::pin(async move {
        let row = client
            .query_one(
                "SELECT name FROM warehouse.product_projections \
                 WHERE tenant_id = $1 AND sku = $2",
                &[ctx.tenant_id.as_uuid(), &"CROSS-BOLT"],
            )
            .await
            .unwrap();
        let name: String = row.get(0);
        name
    })).await;
    assert_eq!(name, "Болт кросс-контекст");

    // Cleanup cross-context tables + catalog tables
    cleanup_tenant_rows(
        &pool,
        ctx.tenant_id,
        &["warehouse.product_projections"],
    )
    .await;
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
    let registry = test_support::test_permission_registry();
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(registry));
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
    let create_handler = CreateProductHandler::new();
    let cmd = CreateProductCommand {
        sku: "ENRICH-BOLT".into(),
        name: "Болт обогащённый".into(),
        category: "Метизы".into(),
        unit: "шт".into(),
    };
    pipeline.execute(&create_handler, &cmd, &ctx).await.unwrap();

    // 2. Relay → publish_and_wait → projection upserted
    let relay = db::OutboxRelay::new(pool.clone(), bus, Duration::from_millis(100), 10, tokio_util::sync::CancellationToken::new());
    let _ = relay.poll_and_publish().await;

    // 3. Receive goods in warehouse (need new ctx for fresh correlation_id)
    let mut ctx2 = RequestContext::new(ctx.tenant_id, ctx.user_id);
    ctx2.roles = vec!["admin".to_string()];

    let receive_handler =
        warehouse::application::commands::receive_goods::ReceiveGoodsHandler::new();
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

    // Cleanup cross-context tables + catalog tables
    cleanup_tenant_rows(
        &pool,
        ctx.tenant_id,
        &[
            "warehouse.inventory_balances",
            "warehouse.stock_movements",
            "warehouse.inventory_items",
            "warehouse.product_projections",
            "common.sequences",
        ],
    )
    .await;
    cleanup_tenant(&pool, ctx.tenant_id).await;
}
