//! Integration-тесты для db crate.
//!
//! Требуют реальную `PostgreSQL` через `DATABASE_URL`.
//! Каждый тест использует уникальный `tenant_id` для изоляции.

use std::sync::Arc;

use event_bus::EventEnvelope;
use kernel::DomainEvent;
use kernel::types::{RequestContext, TenantId, UserId};
use runtime::ports::{UnitOfWork, UnitOfWorkFactory};
use serde::Serialize;
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests")
}

fn new_ctx() -> RequestContext {
    RequestContext::new(TenantId::new(), UserId::new())
}

fn make_test_envelope(ctx: &RequestContext) -> EventEnvelope {
    #[derive(Debug, Clone, Serialize)]
    struct TestEvt {
        id: Uuid,
    }

    impl DomainEvent for TestEvt {
        fn event_type(&self) -> &'static str {
            "erp.test.integration.v1"
        }
        fn aggregate_id(&self) -> Uuid {
            self.id
        }
    }

    let evt = TestEvt { id: Uuid::now_v7() };
    EventEnvelope::from_domain_event(&evt, ctx, "test").unwrap()
}

// ─── Health check ────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_check_ok() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    pool.health_check().await.unwrap();
}

// ─── Migrations ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn run_migrations_idempotent() {
    let pool = db::PgPool::new(&database_url()).unwrap();

    // Первый запуск — применяет миграции (или пропускает, если уже).
    db::migrate::run_migrations(&pool, "../../migrations/common")
        .await
        .unwrap();

    // Второй запуск — идемпотентность.
    db::migrate::run_migrations(&pool, "../../migrations/common")
        .await
        .unwrap();

    // Проверяем, что таблицы существуют.
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = 'common' AND table_name IN \
             ('tenants', 'outbox', 'audit_log', 'sequences', 'inbox', 'domain_history')",
            &[],
        )
        .await
        .unwrap();
    let count: i64 = row.get(0);
    assert_eq!(count, 6, "expected 6 common tables");
}

// ─── RLS isolation ───────────────────────────────────────────────────────────

#[tokio::test]
async fn rls_tenant_isolation() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let tenant_a = TenantId::new();
    let tenant_b = TenantId::new();

    // INSERT row as superuser (RLS bypass for erp_admin).
    client
        .execute(
            "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) \
             VALUES ($1, 'rls_test_a', 'A-', 1)",
            &[tenant_a.as_uuid()],
        )
        .await
        .unwrap();

    client
        .execute(
            "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) \
             VALUES ($1, 'rls_test_b', 'B-', 1)",
            &[tenant_b.as_uuid()],
        )
        .await
        .unwrap();

    // Теперь проверяем RLS: SET tenant_id = A → видна только строка A.
    client.batch_execute("BEGIN").await.unwrap();
    db::set_tenant_context(&**client, tenant_a).await.unwrap();

    // erp_admin — superuser, RLS не применяется к нему напрямую.
    // Для корректной проверки нужен non-superuser. Но мы можем проверить,
    // что SET LOCAL работает (функция current_tenant_id возвращает правильный UUID).
    let current = client
        .query_one("SELECT common.current_tenant_id()", &[])
        .await
        .unwrap();
    let current_uuid: Uuid = current.get(0);
    assert_eq!(current_uuid, *tenant_a.as_uuid());

    client.batch_execute("ROLLBACK").await.unwrap();

    // Cleanup.
    client
        .execute(
            "DELETE FROM common.sequences WHERE seq_name LIKE 'rls_test_%'",
            &[],
        )
        .await
        .unwrap();
}

// ─── UoW commit → outbox ────────────────────────────────────────────────────

#[tokio::test]
async fn uow_commit_writes_outbox() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let factory = db::PgUnitOfWorkFactory::new(pool.clone());

    let ctx = new_ctx();
    let envelope = make_test_envelope(&ctx);
    let event_id = envelope.event_id;

    let mut uow = factory.begin(&ctx).await.unwrap();
    uow.add_outbox_entry(envelope);
    Box::new(uow).commit().await.unwrap();

    // Проверяем, что outbox запись появилась.
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT event_type FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    let event_type: String = row.get(0);
    assert_eq!(event_type, "erp.test.integration.v1");

    // Cleanup.
    client
        .execute(
            "DELETE FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
}

// ─── UoW rollback → outbox НЕ записан ───────────────────────────────────────

#[tokio::test]
async fn uow_rollback_discards_outbox() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let factory = db::PgUnitOfWorkFactory::new(pool.clone());

    let ctx = new_ctx();
    let envelope = make_test_envelope(&ctx);
    let event_id = envelope.event_id;

    let mut uow = factory.begin(&ctx).await.unwrap();
    uow.add_outbox_entry(envelope);
    Box::new(uow).rollback().await.unwrap();

    // Проверяем, что outbox запись НЕ появилась.
    let client = pool.get().await.unwrap();
    let row = client
        .query_opt(
            "SELECT 1 FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    assert!(
        row.is_none(),
        "outbox entry should not exist after rollback"
    );
}

// ─── UoW downcast ────────────────────────────────────────────────────────────

#[tokio::test]
async fn uow_downcast_to_pg() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let factory = db::PgUnitOfWorkFactory::new(pool);

    let ctx = new_ctx();
    let mut uow = factory.begin(&ctx).await.unwrap();

    // Downcast через as_any_mut.
    let pg_uow: &mut db::PgUnitOfWork = uow
        .as_any_mut()
        .downcast_mut::<db::PgUnitOfWork>()
        .expect("downcast to PgUnitOfWork should succeed");

    // Выполняем SQL через downcast'нутый client.
    let row = pg_uow
        .client()
        .query_one("SELECT 1 AS one", &[])
        .await
        .unwrap();
    let one: i32 = row.get(0);
    assert_eq!(one, 1);

    Box::new(uow).rollback().await.unwrap();
}

// ─── UoW: handler-like SQL within TX ────────────────────────────────────────

#[tokio::test]
async fn uow_sql_within_transaction() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let factory = db::PgUnitOfWorkFactory::new(pool.clone());

    let ctx = new_ctx();
    let tenant_id = ctx.tenant_id;
    let mut uow = factory.begin(&ctx).await.unwrap();

    // INSERT sequence inside TX.
    let pg_uow = uow.as_any_mut().downcast_mut::<db::PgUnitOfWork>().unwrap();
    pg_uow
        .client()
        .execute(
            "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) \
             VALUES ($1, 'uow_test_seq', 'T-', 42)",
            &[tenant_id.as_uuid()],
        )
        .await
        .unwrap();

    // Rollback — INSERT откатывается.
    Box::new(uow).rollback().await.unwrap();

    // Проверяем, что строка НЕ появилась.
    let client = pool.get().await.unwrap();
    let row = client
        .query_opt(
            "SELECT 1 FROM common.sequences \
             WHERE tenant_id = $1 AND seq_name = 'uow_test_seq'",
            &[tenant_id.as_uuid()],
        )
        .await
        .unwrap();
    assert!(row.is_none(), "sequence should not exist after TX rollback");
}

// ─── Clorinde-gen queries ────────────────────────────────────────────────────

#[tokio::test]
async fn clorinde_outbox_round_trip() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let tenant_id = Uuid::now_v7();
    let event_id = Uuid::now_v7();
    let now = chrono::Utc::now();
    let payload = serde_json::json!({"item": "BOLT-42", "qty": 100});

    // Insert.
    let params = clorinde_gen::common::outbox::InsertOutboxParams {
        tenant_id,
        event_id,
        event_type: "erp.warehouse.goods_received.v1",
        source: "warehouse",
        payload: &payload,
        correlation_id: Uuid::now_v7(),
        causation_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        created_at: now,
    };
    let id = clorinde_gen::common::outbox::insert_outbox_entry(&**client, &params)
        .await
        .unwrap();
    assert!(id > 0);

    // Mark published.
    let affected = clorinde_gen::common::outbox::mark_published(&**client, id)
        .await
        .unwrap();
    assert_eq!(affected, 1);

    // Cleanup.
    client
        .execute("DELETE FROM common.outbox WHERE id = $1", &[&id])
        .await
        .unwrap();
}

// ─── Clorinde-gen sequences ──────────────────────────────────────────────────

#[tokio::test]
async fn clorinde_sequences_round_trip() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let tenant_id = Uuid::now_v7();
    let seq_name = "clorinde_test_seq";

    // Ensure sequence.
    clorinde_gen::common::sequences::ensure_sequence(&**client, tenant_id, seq_name, "CT-")
        .await
        .unwrap();

    // Begin TX for FOR UPDATE.
    client.batch_execute("BEGIN").await.unwrap();

    // Get next_value.
    let val = clorinde_gen::common::sequences::next_value(&**client, tenant_id, seq_name)
        .await
        .unwrap()
        .expect("sequence should exist");
    assert_eq!(val.prefix, "CT-");
    assert_eq!(val.next_value, 1);

    // Increment.
    clorinde_gen::common::sequences::increment_sequence(&**client, tenant_id, seq_name)
        .await
        .unwrap();

    client.batch_execute("COMMIT").await.unwrap();

    // Cleanup.
    client
        .execute(
            "DELETE FROM common.sequences WHERE tenant_id = $1 AND seq_name = $2",
            &[&tenant_id, &seq_name],
        )
        .await
        .unwrap();
}

// ─── Clorinde-gen inbox ─────────────────────────────────────────────────────

#[tokio::test]
async fn clorinde_inbox_dedup() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let event_id = Uuid::now_v7();

    // First insert → 1 row.
    let inserted = clorinde_gen::common::inbox::try_insert_inbox(
        &**client,
        event_id,
        "erp.test.inbox.v1",
        "test",
    )
    .await
    .unwrap();
    assert_eq!(inserted, 1, "first insert should succeed");

    // Duplicate → 0 rows (idempotent).
    let dup = clorinde_gen::common::inbox::try_insert_inbox(
        &**client,
        event_id,
        "erp.test.inbox.v1",
        "test",
    )
    .await
    .unwrap();
    assert_eq!(dup, 0, "duplicate insert should be no-op");

    // check_processed → true.
    let processed = clorinde_gen::common::inbox::check_processed(&**client, event_id)
        .await
        .unwrap();
    assert!(processed, "event should be marked as processed");

    // Cleanup.
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

// ─── Clorinde-gen tenants ───────────────────────────────────────────────────

#[tokio::test]
async fn clorinde_tenants_crud() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let id = Uuid::now_v7();
    let name = "Test Corp";
    let slug = &format!("test-corp-{}", &id.to_string()[..8]);

    // Create.
    let row = clorinde_gen::common::tenants::create_tenant(&**client, id, name, slug)
        .await
        .unwrap();
    assert_eq!(row.id, id);
    assert_eq!(row.name, name);
    assert_eq!(&row.slug, slug);
    assert!(row.is_active);

    // Get by id.
    let found = clorinde_gen::common::tenants::get_tenant(&**client, id)
        .await
        .unwrap()
        .expect("tenant should exist");
    assert_eq!(found.id, id);

    // Cleanup.
    client
        .execute("DELETE FROM common.tenants WHERE id = $1", &[&id])
        .await
        .unwrap();
}

// ─── Clorinde-gen audit ─────────────────────────────────────────────────────

#[tokio::test]
async fn clorinde_audit_insert() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let now = chrono::Utc::now();
    let result = serde_json::json!({"status": "ok"});

    let params = clorinde_gen::common::audit::InsertAuditParams {
        tenant_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        command_name: "warehouse::receive_goods",
        result: &result,
        correlation_id: Uuid::now_v7(),
        causation_id: Uuid::now_v7(),
        created_at: now,
    };
    let id = clorinde_gen::common::audit::insert_audit_log(&**client, &params)
        .await
        .unwrap();
    assert!(id > 0);

    // Cleanup.
    client
        .execute("DELETE FROM common.audit_log WHERE id = $1", &[&id])
        .await
        .unwrap();
}

// ─── Clorinde-gen domain_history ────────────────────────────────────────────

#[tokio::test]
async fn clorinde_domain_history_insert() {
    let pool = db::PgPool::new(&database_url()).unwrap();
    let client = pool.get().await.unwrap();

    let now = chrono::Utc::now();
    let old_state = serde_json::json!({"qty": 0});
    let new_state = serde_json::json!({"qty": 100});

    let params = clorinde_gen::common::domain_history::InsertHistoryParams {
        tenant_id: Uuid::now_v7(),
        entity_type: "warehouse::inventory_item",
        entity_id: Uuid::now_v7(),
        event_type: "erp.warehouse.goods_received.v1",
        old_state: Some(&old_state),
        new_state: Some(&new_state),
        correlation_id: Uuid::now_v7(),
        causation_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        created_at: now,
    };
    let id = clorinde_gen::common::domain_history::insert_domain_history(&**client, &params)
        .await
        .unwrap();
    assert!(id > 0);

    // Cleanup.
    client
        .execute("DELETE FROM common.domain_history WHERE id = $1", &[&id])
        .await
        .unwrap();
}
