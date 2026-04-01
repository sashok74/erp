use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use event_bus::traits::EventBus;
use kernel::security::PermissionRegistrar;
use kernel::types::{RequestContext, TenantId, UserId};
use runtime::pipeline::CommandPipeline;
use runtime::query_pipeline::QueryPipeline;
use runtime::stubs::NoopExtensionHooks;

#[allow(clippy::type_complexity)]
static POOLS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::OnceCell<Arc<db::PgPool>>>>>> =
    OnceLock::new();

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests")
}

fn pools() -> &'static Mutex<HashMap<String, Arc<tokio::sync::OnceCell<Arc<db::PgPool>>>>> {
    POOLS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn shared_pool(migration_dirs: &[&str]) -> Arc<db::PgPool> {
    let key = migration_dirs.join("|");
    let cell = {
        let mut guard = pools().lock().expect("test pool registry lock poisoned");
        guard
            .entry(key)
            .or_insert_with(|| Arc::new(tokio::sync::OnceCell::const_new()))
            .clone()
    };

    cell.get_or_init(|| async move {
        let pool = Arc::new(db::PgPool::new(&database_url()).expect("failed to create PgPool"));

        for dir in migration_dirs {
            db::migrate::run_migrations(&pool, dir)
                .await
                .unwrap_or_else(|err| panic!("failed to run migrations from {dir}: {err}"));
        }

        pool
    })
    .await
    .clone()
}

pub fn request_context(roles: &[&str]) -> RequestContext {
    let mut ctx = RequestContext::new(TenantId::new(), UserId::new());
    ctx.roles = roles.iter().map(|role| (*role).to_string()).collect();
    ctx
}

/// Build a `PermissionRegistry` from real BC manifests (warehouse + catalog).
pub fn test_permission_registry() -> Arc<auth::PermissionRegistry> {
    let wh = warehouse::registrar::WarehousePermissions.permission_manifest();
    let cat = catalog::registrar::CatalogPermissions.permission_manifest();

    Arc::new(
        auth::PermissionRegistry::from_manifests_validated(vec![wh, cat])
            .expect("test manifests must be valid"),
    )
}

pub fn command_pipeline(
    pool: Arc<db::PgPool>,
    bus: Arc<dyn EventBus>,
) -> CommandPipeline<db::PgUnitOfWorkFactory> {
    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));
    let registry = test_permission_registry();
    let checker = Arc::new(auth::JwtPermissionChecker::new(registry));
    let audit = Arc::new(audit::PgAuditLog::new(pool));

    CommandPipeline::new(
        uow_factory,
        bus,
        checker,
        Arc::new(NoopExtensionHooks),
        audit,
    )
}

pub fn query_pipeline(pool: Arc<db::PgPool>) -> QueryPipeline {
    let registry = test_permission_registry();
    let checker = Arc::new(auth::JwtPermissionChecker::new(registry));
    let audit = Arc::new(audit::PgAuditLog::new(pool));

    QueryPipeline::new(checker, Arc::new(NoopExtensionHooks), audit)
}

/// Run a closure inside a tenant-scoped read TX (for test assertions).
///
/// `BEGIN READ ONLY` → `SET LOCAL tenant_id` → closure → `COMMIT`.
pub async fn tenant_query<T: Send>(
    pool: &db::PgPool,
    tenant_id: TenantId,
    f: impl for<'a> FnOnce(
        &'a deadpool_postgres::Client,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>,
) -> T {
    let client = pool.get().await.expect("pool checkout");
    client
        .batch_execute("BEGIN READ ONLY")
        .await
        .expect("begin");
    db::rls::set_tenant_context(&**client, tenant_id)
        .await
        .expect("set tenant");
    let result = f(&client).await;
    let _ = client.batch_execute("COMMIT").await;
    result
}

/// Delete tenant data from specified tables (inside TX with RLS context).
pub async fn cleanup_tenant(pool: &db::PgPool, tenant_id: TenantId, tables: &[&str]) {
    let client = pool.get().await.expect("pool checkout");
    client.batch_execute("BEGIN").await.expect("begin");
    db::rls::set_tenant_context(&**client, tenant_id)
        .await
        .expect("set tenant");

    for table in tables {
        let sql = format!("DELETE FROM {table} WHERE tenant_id = $1");
        if let Err(e) = client.execute(&sql, &[tenant_id.as_uuid()]).await {
            // If TX is aborted, rollback and retry with fresh TX
            let _ = client.batch_execute("ROLLBACK").await;
            eprintln!("cleanup {table}: {e} (retrying with fresh TX)");
            client.batch_execute("BEGIN").await.expect("begin retry");
            db::rls::set_tenant_context(&**client, tenant_id)
                .await
                .expect("set tenant retry");
            client
                .execute(&sql, &[tenant_id.as_uuid()])
                .await
                .unwrap_or_else(|e2| panic!("cleanup {table} retry: {e2}"));
        }
    }

    client.batch_execute("COMMIT").await.expect("commit");
}
