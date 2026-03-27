use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use event_bus::traits::EventBus;
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

pub fn command_pipeline(
    pool: Arc<db::PgPool>,
    bus: Arc<dyn EventBus>,
) -> CommandPipeline<db::PgUnitOfWorkFactory> {
    let uow_factory = Arc::new(db::PgUnitOfWorkFactory::new(pool.clone()));
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(
        auth::rbac::default_erp_permissions(),
    ));
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
    let checker = Arc::new(auth::checker::JwtPermissionChecker::new(
        auth::rbac::default_erp_permissions(),
    ));
    let audit = Arc::new(audit::PgAuditLog::new(pool));

    QueryPipeline::new(checker, Arc::new(NoopExtensionHooks), audit)
}

pub async fn cleanup_tenant(pool: &db::PgPool, tenant_id: TenantId, tables: &[&str]) {
    let client = pool.get().await.expect("failed to get DB client");
    let tenant_uuid = tenant_id.as_uuid();

    for table in tables {
        client
            .execute(
                &format!("DELETE FROM {table} WHERE tenant_id = $1"),
                &[tenant_uuid],
            )
            .await
            .unwrap_or_else(|err| panic!("failed to cleanup tenant data in {table}: {err}"));
    }
}
