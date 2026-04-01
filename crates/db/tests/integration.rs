//! Integration-тесты для db crate.
//!
//! Требуют реальную `PostgreSQL` через `DATABASE_URL`.
//! Каждый тест использует уникальный `tenant_id` для изоляции.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use event_bus::InProcessBus;
use event_bus::registry::ErasedEventHandler;
use event_bus::traits::{EventBus, EventHandler};
use event_bus::{EventEnvelope, EventHandlerAdapter};
use kernel::{DomainEvent, IntoInternal};
use kernel::types::{RequestContext, TenantId, UserId};
use runtime::ports::{UnitOfWork, UnitOfWorkFactory};
use serde::{Deserialize, Serialize};
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
    let now = chrono::Utc::now().fixed_offset();
    let payload = serde_json::json!({"item": "BOLT-42", "qty": 100});
    let correlation_id = Uuid::now_v7();
    let causation_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();

    // Insert.
    let id = clorinde_gen::queries::common::outbox::insert_outbox_entry()
        .bind(
            &client,
            &tenant_id,
            &event_id,
            &"erp.warehouse.goods_received.v1",
            &"warehouse",
            &payload,
            &correlation_id,
            &causation_id,
            &user_id,
            &now,
        )
        .one()
        .await
        .unwrap();
    assert!(id > 0);

    // Mark published.
    let affected = clorinde_gen::queries::common::outbox::mark_published()
        .bind(&client, &id)
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
    clorinde_gen::queries::common::sequences::ensure_sequence()
        .bind(&client, &tenant_id, &seq_name, &"CT-")
        .await
        .unwrap();

    // Begin TX for FOR UPDATE.
    client.batch_execute("BEGIN").await.unwrap();

    // Get next_value.
    let val = clorinde_gen::queries::common::sequences::next_value()
        .bind(&client, &tenant_id, &seq_name)
        .one()
        .await
        .unwrap();
    assert_eq!(val.prefix, "CT-");
    assert_eq!(val.next_value, 1);

    // Increment.
    clorinde_gen::queries::common::sequences::increment_sequence()
        .bind(&client, &tenant_id, &seq_name)
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

    let handler_name = "test_handler";

    // First insert → 1 row.
    let inserted = clorinde_gen::queries::common::inbox::try_insert_inbox()
        .bind(&client, &event_id, &"erp.test.inbox.v1", &"test", &handler_name)
        .await
        .unwrap();
    assert_eq!(inserted, 1, "first insert should succeed");

    // Duplicate (same event_id + handler_name) → 0 rows (idempotent).
    let dup = clorinde_gen::queries::common::inbox::try_insert_inbox()
        .bind(&client, &event_id, &"erp.test.inbox.v1", &"test", &handler_name)
        .await
        .unwrap();
    assert_eq!(dup, 0, "duplicate insert should be no-op");

    // Different handler_name → 1 row (independent).
    let other_handler = clorinde_gen::queries::common::inbox::try_insert_inbox()
        .bind(&client, &event_id, &"erp.test.inbox.v1", &"test", &"other_handler")
        .await
        .unwrap();
    assert_eq!(other_handler, 1, "different handler should succeed");

    // check_processed → true.
    let processed = clorinde_gen::queries::common::inbox::check_processed()
        .bind(&client, &event_id, &handler_name)
        .opt()
        .await
        .unwrap();
    assert!(processed.is_some(), "event should be marked as processed");

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
    let row = clorinde_gen::queries::common::tenants::create_tenant()
        .bind(&client, &id, &name, &slug)
        .one()
        .await
        .unwrap();
    assert_eq!(row.id, id);
    assert_eq!(row.name, name);
    assert_eq!(&row.slug, slug);
    assert!(row.is_active);

    // Get by id.
    let found = clorinde_gen::queries::common::tenants::get_tenant()
        .bind(&client, &id)
        .opt()
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

    let now = chrono::Utc::now().fixed_offset();
    let result = serde_json::json!({"status": "ok"});
    let tenant_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let correlation_id = Uuid::now_v7();
    let causation_id = Uuid::now_v7();

    let id = clorinde_gen::queries::common::audit::insert_audit_log()
        .bind(
            &client,
            &tenant_id,
            &user_id,
            &"warehouse::receive_goods",
            &result,
            &correlation_id,
            &causation_id,
            &now,
        )
        .one()
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

    let now = chrono::Utc::now().fixed_offset();
    let old_state = serde_json::json!({"qty": 0});
    let new_state = serde_json::json!({"qty": 100});
    let tenant_id = Uuid::now_v7();
    let entity_id = Uuid::now_v7();
    let correlation_id = Uuid::now_v7();
    let causation_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();

    let id = clorinde_gen::queries::common::domain_history::insert_domain_history()
        .bind(
            &client,
            &tenant_id,
            &"warehouse::inventory_item",
            &entity_id,
            &"erp.warehouse.goods_received.v1",
            &old_state,
            &new_state,
            &correlation_id,
            &causation_id,
            &user_id,
            &now,
        )
        .one()
        .await
        .unwrap();
    assert!(id > 0);

    // Cleanup.
    client
        .execute("DELETE FROM common.domain_history WHERE id = $1", &[&id])
        .await
        .unwrap();
}

// ─── Outbox Relay ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelayTestEvt {
    id: Uuid,
}

impl DomainEvent for RelayTestEvt {
    fn event_type(&self) -> &'static str {
        "erp.test.relay.v1"
    }
    fn aggregate_id(&self) -> Uuid {
        self.id
    }
}

struct FlagHandler {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl EventHandler for FlagHandler {
    type Event = RelayTestEvt;

    async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
        self.called.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn handled_event_type(&self) -> &'static str {
        "erp.test.relay.v1"
    }
}

#[tokio::test]
async fn relay_publishes_outbox_entry() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let bus = Arc::new(InProcessBus::new());

    let called = Arc::new(AtomicBool::new(false));
    let handler = Arc::new(EventHandlerAdapter::new(FlagHandler {
        called: called.clone(),
    }));
    bus.subscribe("erp.test.relay.v1", handler).await;

    // Insert outbox entry directly.
    let ctx = new_ctx();
    let envelope = {
        let evt = RelayTestEvt { id: Uuid::now_v7() };
        EventEnvelope::from_domain_event(&evt, &ctx, "test").unwrap()
    };
    let event_id = envelope.event_id;

    let client = pool.get().await.unwrap();
    let tenant_id = *ctx.tenant_id.as_uuid();
    let user_id = *ctx.user_id.as_uuid();
    let created_at = envelope.timestamp.fixed_offset();
    clorinde_gen::queries::common::outbox::insert_outbox_entry()
        .bind(
            &client,
            &tenant_id,
            &envelope.event_id,
            &envelope.event_type,
            &envelope.source,
            &envelope.payload,
            &envelope.correlation_id,
            &envelope.causation_id,
            &user_id,
            &created_at,
        )
        .one()
        .await
        .unwrap();
    drop(client);

    // Run relay — may publish more than 1 entry if other tests left data.
    // Relay now uses publish_and_wait (synchronous), so no sleep needed.
    let relay = db::OutboxRelay::new(pool.clone(), bus.clone(), Duration::from_millis(50), 100, tokio_util::sync::CancellationToken::new());
    let published = relay.poll_and_publish().await.unwrap();
    assert!(published >= 1, "at least our entry should be published");

    assert!(
        called.load(Ordering::SeqCst),
        "handler should have been called"
    );

    // Verify published_at is set.
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT published FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    let published_flag: bool = row.get(0);
    assert!(published_flag, "outbox entry should be marked published");

    // Cleanup.
    client
        .execute(
            "DELETE FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn relay_increments_retry_on_handler_error() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let bus = Arc::new(InProcessBus::new());

    // publish_and_wait is not used by relay — relay uses publish() which is fire-and-forget.
    // But InProcessBus.publish() spawns handler and catches error internally.
    // For testing retry, we need a bus that returns Err from publish().
    // Let's test via direct outbox row inspection instead: insert, run relay,
    // check that published_at remains NULL when no handler subscribed but bus returns Ok.
    //
    // Actually, InProcessBus.publish() always returns Ok (errors are caught in spawned task).
    // So to test retry_count increment, we need a custom bus impl.

    struct FailingBus;

    #[async_trait]
    impl EventBus for FailingBus {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<(), anyhow::Error> {
            anyhow::bail!("bus publish error")
        }
        async fn publish_and_wait(&self, _envelope: EventEnvelope) -> Result<(), anyhow::Error> {
            anyhow::bail!("bus publish error")
        }
        async fn subscribe(
            &self,
            _event_type: &'static str,
            _handler: Arc<dyn event_bus::ErasedEventHandler>,
        ) {
        }
    }

    let failing_bus: Arc<dyn EventBus> = Arc::new(FailingBus);

    let ctx = new_ctx();
    let envelope = {
        let evt = RelayTestEvt { id: Uuid::now_v7() };
        EventEnvelope::from_domain_event(&evt, &ctx, "test").unwrap()
    };
    let event_id = envelope.event_id;

    let client = pool.get().await.unwrap();

    // Mark any existing unpublished entries to avoid interference.
    client
        .execute(
            "UPDATE common.outbox SET published = true, published_at = now() \
             WHERE published = false",
            &[],
        )
        .await
        .unwrap();

    let tenant_id = *ctx.tenant_id.as_uuid();
    let user_id = *ctx.user_id.as_uuid();
    let created_at = envelope.timestamp.fixed_offset();
    clorinde_gen::queries::common::outbox::insert_outbox_entry()
        .bind(
            &client,
            &tenant_id,
            &envelope.event_id,
            &envelope.event_type,
            &envelope.source,
            &envelope.payload,
            &envelope.correlation_id,
            &envelope.causation_id,
            &user_id,
            &created_at,
        )
        .one()
        .await
        .unwrap();
    drop(client);

    // Run relay once — publish will fail, retry_count incremented.
    let relay = db::OutboxRelay::new(pool.clone(), failing_bus, Duration::from_millis(50), 100, tokio_util::sync::CancellationToken::new());
    relay.poll_and_publish().await.unwrap();

    // Verify retry_count incremented.
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT retry_count, published FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    let retry_count: i32 = row.get(0);
    let published_flag: bool = row.get(1);
    assert!(retry_count >= 1, "retry_count should be at least 1");
    assert!(!published_flag, "should not be published after failure");

    // Test max-retry → DLQ: set retry_count to MAX_RETRIES, verify relay moves to dead_letters.
    client
        .execute(
            "UPDATE common.outbox SET retry_count = 3, published = false WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();

    let published = relay.poll_and_publish().await.unwrap();

    // Outbox entry should be marked as published (removed from relay scope).
    let row = client
        .query_one(
            "SELECT published FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    assert!(
        row.get::<_, bool>(0),
        "entry at max retries should be marked published after DLQ move"
    );

    // Dead letter should exist.
    let dlq_row = client
        .query_one(
            "SELECT event_type, last_error FROM common.dead_letters WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    let dlq_event_type: String = dlq_row.get(0);
    assert_eq!(dlq_event_type, envelope.event_type);

    // Cleanup.
    client
        .execute(
            "DELETE FROM common.dead_letters WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    client
        .execute(
            "DELETE FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();

    let _ = bus;
    let _ = published;
}

// ─── InboxGuard ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn inbox_guard_dedup() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inbox = db::InboxGuard::new(pool.clone());

    let event_id = Uuid::now_v7();
    let handler = "test_handler";

    // First → true (new).
    let first = inbox
        .mark_processed(event_id, "erp.test.inbox_guard.v1", "test", handler)
        .await
        .unwrap();
    assert!(first, "first call should return true");

    // is_processed → true.
    assert!(inbox.is_processed(event_id, handler).await.unwrap());

    // Second mark → false (duplicate).
    let second = inbox
        .mark_processed(event_id, "erp.test.inbox_guard.v1", "test", handler)
        .await
        .unwrap();
    assert!(!second, "second call should return false");

    // Different handler_name, same event_id → true (independent).
    let other_handler = inbox
        .mark_processed(event_id, "erp.test.inbox_guard.v1", "test", "other_handler")
        .await
        .unwrap();
    assert!(other_handler, "different handler should return true");

    // Different event_id → true.
    let other = inbox
        .mark_processed(Uuid::now_v7(), "erp.test.inbox_guard.v1", "test", handler)
        .await
        .unwrap();
    assert!(other, "different event_id should return true");

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

// ─── InboxAwareHandler ─────────────────────────────────────────────────────

use std::sync::atomic::AtomicUsize;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InboxTestEvt {
    id: Uuid,
}

impl DomainEvent for InboxTestEvt {
    fn event_type(&self) -> &'static str {
        "erp.test.inbox_aware.v1"
    }
    fn aggregate_id(&self) -> Uuid {
        self.id
    }
}

struct CountingTestHandler {
    count: Arc<AtomicUsize>,
}

#[async_trait]
impl EventHandler for CountingTestHandler {
    type Event = InboxTestEvt;

    async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn handled_event_type(&self) -> &'static str {
        "erp.test.inbox_aware.v1"
    }
}

struct FailingTestHandler;

#[async_trait]
impl EventHandler for FailingTestHandler {
    type Event = InboxTestEvt;

    async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
        anyhow::bail!("handler error")
    }

    fn handled_event_type(&self) -> &'static str {
        "erp.test.inbox_aware.v1"
    }
}

fn make_inbox_envelope(ctx: &RequestContext) -> EventEnvelope {
    let evt = InboxTestEvt { id: Uuid::now_v7() };
    EventEnvelope::from_domain_event(&evt, ctx, "test").unwrap()
}

#[tokio::test]
async fn inbox_aware_handler_first_call_succeeds() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inbox = Arc::new(db::InboxGuard::new(pool.clone()));
    let count = Arc::new(AtomicUsize::new(0));

    let inner = Arc::new(EventHandlerAdapter::new(CountingTestHandler {
        count: count.clone(),
    }));
    let handler = db::inbox::InboxAwareHandler::new(inner, inbox);

    let ctx = new_ctx();
    let envelope = make_inbox_envelope(&ctx);
    let event_id = envelope.event_id;

    // First call → inner handler called, inbox recorded.
    handler.handle_envelope(&envelope).await.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

#[tokio::test]
async fn inbox_aware_handler_duplicate_skipped() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inbox = Arc::new(db::InboxGuard::new(pool.clone()));
    let count = Arc::new(AtomicUsize::new(0));

    let inner = Arc::new(EventHandlerAdapter::new(CountingTestHandler {
        count: count.clone(),
    }));
    let handler = db::inbox::InboxAwareHandler::new(inner, inbox);

    let ctx = new_ctx();
    let envelope = make_inbox_envelope(&ctx);
    let event_id = envelope.event_id;

    // First call.
    handler.handle_envelope(&envelope).await.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Second call with same event_id → skipped.
    handler.handle_envelope(&envelope).await.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "duplicate should be skipped");

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

struct CountingTestHandlerB {
    count: Arc<AtomicUsize>,
}

#[async_trait]
impl EventHandler for CountingTestHandlerB {
    type Event = InboxTestEvt;

    async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn handled_event_type(&self) -> &'static str {
        "erp.test.inbox_aware.v1"
    }
}

#[tokio::test]
async fn inbox_aware_handler_different_handlers_independent() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inbox = Arc::new(db::InboxGuard::new(pool.clone()));

    let count_a = Arc::new(AtomicUsize::new(0));
    let count_b = Arc::new(AtomicUsize::new(0));

    // Use different handler types → different handler_name via type_name.
    let inner_a = Arc::new(EventHandlerAdapter::new(CountingTestHandler {
        count: count_a.clone(),
    }));
    let inner_b = Arc::new(EventHandlerAdapter::new(CountingTestHandlerB {
        count: count_b.clone(),
    }));

    let handler_a = db::inbox::InboxAwareHandler::new(inner_a, inbox.clone());
    let handler_b = db::inbox::InboxAwareHandler::new(inner_b, inbox);

    let ctx = new_ctx();
    let envelope = make_inbox_envelope(&ctx);
    let event_id = envelope.event_id;

    // Both handlers process same event_id → both called (different handler_name).
    handler_a.handle_envelope(&envelope).await.unwrap();
    handler_b.handle_envelope(&envelope).await.unwrap();
    assert_eq!(count_a.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(count_b.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

#[tokio::test]
async fn inbox_aware_handler_error_allows_retry() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inbox = Arc::new(db::InboxGuard::new(pool.clone()));

    let inner: Arc<dyn event_bus::ErasedEventHandler> =
        Arc::new(EventHandlerAdapter::new(FailingTestHandler));
    let handler = db::inbox::InboxAwareHandler::new(inner, inbox.clone());

    let ctx = new_ctx();
    let envelope = make_inbox_envelope(&ctx);
    let event_id = envelope.event_id;
    let handler_name = "event_bus::registry::EventHandlerAdapter<db::integration::FailingTestHandler>";

    // Handler returns Err → inbox NOT recorded.
    let result = handler.handle_envelope(&envelope).await;
    assert!(result.is_err());

    // Inbox should NOT have a record → retry should work.
    assert!(
        !inbox.is_processed(event_id, handler_name).await.unwrap(),
        "inbox should not be recorded after handler error"
    );

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

// ─── InboxBusDecorator ─────────────────────────────────────────────────────

#[tokio::test]
async fn inbox_bus_decorator_event_map() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inner_bus = Arc::new(InProcessBus::new());
    let decorator = db::InboxBusDecorator::new(
        inner_bus as Arc<dyn EventBus>,
        pool,
    );

    let count = Arc::new(AtomicUsize::new(0));
    let handler = Arc::new(EventHandlerAdapter::new(CountingTestHandler {
        count: count.clone(),
    }));
    decorator
        .subscribe("erp.test.inbox_aware.v1", handler)
        .await;

    let map = decorator.event_map().await;
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].event_type, "erp.test.inbox_aware.v1");
    assert!(!map[0].handler_name.is_empty());
}

#[tokio::test]
async fn inbox_bus_decorator_dedup_via_publish_and_wait() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inner_bus = Arc::new(InProcessBus::new());
    let decorator = db::InboxBusDecorator::new(
        inner_bus as Arc<dyn EventBus>,
        pool.clone(),
    );

    let count = Arc::new(AtomicUsize::new(0));
    let handler = Arc::new(EventHandlerAdapter::new(CountingTestHandler {
        count: count.clone(),
    }));
    decorator
        .subscribe("erp.test.inbox_aware.v1", handler)
        .await;

    let ctx = new_ctx();
    let envelope = make_inbox_envelope(&ctx);
    let event_id = envelope.event_id;

    // First publish → handler called.
    decorator.publish_and_wait(envelope.clone()).await.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Second publish same event_id → skipped by inbox.
    decorator.publish_and_wait(envelope).await.unwrap();
    assert_eq!(
        count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "duplicate should be skipped"
    );

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
}

// ─── Relay + InboxAwareHandler E2E ─────────────────────────────────────────

#[tokio::test]
async fn relay_handler_error_allows_retry_via_inbox() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let inner_bus = Arc::new(InProcessBus::new());
    let decorator = Arc::new(db::InboxBusDecorator::new(
        inner_bus as Arc<dyn EventBus>,
        pool.clone(),
    ));

    // Subscribe a failing handler.
    let handler: Arc<dyn event_bus::ErasedEventHandler> =
        Arc::new(EventHandlerAdapter::new(FailingTestHandler));
    decorator
        .subscribe("erp.test.inbox_aware.v1", handler)
        .await;

    // Insert outbox entry.
    let ctx = new_ctx();
    let evt = InboxTestEvt { id: Uuid::now_v7() };
    let envelope = EventEnvelope::from_domain_event(&evt, &ctx, "test").unwrap();
    let event_id = envelope.event_id;

    let client = pool.get().await.unwrap();

    // Mark existing unpublished entries to avoid interference.
    client
        .execute(
            "UPDATE common.outbox SET published = true, published_at = now() \
             WHERE published = false",
            &[],
        )
        .await
        .unwrap();

    let tenant_id = *ctx.tenant_id.as_uuid();
    let user_id = *ctx.user_id.as_uuid();
    let created_at = envelope.timestamp.fixed_offset();
    clorinde_gen::queries::common::outbox::insert_outbox_entry()
        .bind(
            &client,
            &tenant_id,
            &envelope.event_id,
            &envelope.event_type,
            &envelope.source,
            &envelope.payload,
            &envelope.correlation_id,
            &envelope.causation_id,
            &user_id,
            &created_at,
        )
        .one()
        .await
        .unwrap();
    drop(client);

    // Run relay — publish_and_wait → handler fails → retry_count incremented.
    let relay = db::OutboxRelay::new(
        pool.clone(),
        decorator as Arc<dyn EventBus>,
        Duration::from_millis(50),
        100,
        tokio_util::sync::CancellationToken::new(),
    );
    relay.poll_and_publish().await.unwrap();

    // Verify retry_count incremented, NOT published.
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT retry_count, published FROM common.outbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    let retry_count: i32 = row.get(0);
    let published_flag: bool = row.get(1);
    assert!(retry_count >= 1, "retry_count should be at least 1");
    assert!(!published_flag, "should not be published after handler failure");

    // Inbox should NOT have a record → retry would re-invoke handler.
    let inbox_row = client
        .query_opt(
            "SELECT 1 FROM common.inbox WHERE event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    assert!(
        inbox_row.is_none(),
        "inbox should not be recorded after handler error"
    );

    // Cleanup.
    client
        .execute("DELETE FROM common.outbox WHERE event_id = $1", &[&event_id])
        .await
        .unwrap();
    client
        .execute("DELETE FROM common.inbox WHERE event_id = $1", &[&event_id])
        .await
        .ok();
}

// ─── RLS enforcement: FORCE RLS blocks reads without tenant context ────────

/// Regression test for P0 finding (2026-04-01 arch review):
/// FORCE ROW LEVEL SECURITY ensures that even the table owner
/// cannot read tenant data without SET LOCAL app.tenant_id.
///
/// This test:
/// 1. Inserts a row via with_tenant_write (correct path)
/// 2. Tries to read it WITHOUT tenant context → must get 0 rows
/// 3. Reads it WITH tenant context via with_tenant_read → must get 1 row
/// 4. Reads it with WRONG tenant context → must get 0 rows
#[tokio::test]
async fn force_rls_blocks_read_without_tenant_context() {
    let pool = db::PgPool::new(&database_url()).unwrap();

    db::migrate::run_migrations(&pool, "../../migrations/common")
        .await
        .unwrap();
    db::migrate::run_migrations(&pool, "../../migrations/warehouse")
        .await
        .unwrap();

    let tenant_a = TenantId::new();
    let item_id = Uuid::now_v7();
    let sku = "RLS-ENFORCE-TEST";

    // 1. Insert via with_tenant_write (correct tenant context).
    db::with_tenant_write(&pool, tenant_a, |client| {
        Box::pin(async move {
            client
                .execute(
                    "INSERT INTO warehouse.inventory_items (tenant_id, id, sku) VALUES ($1, $2, $3)",
                    &[tenant_a.as_uuid(), &item_id, &sku],
                )
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();

    // 2. Read WITHOUT tenant context (raw connection, no SET LOCAL) → 0 rows.
    //    FORCE RLS means even owner is blocked.
    let client = pool.get().await.unwrap();
    let count_no_ctx: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM warehouse.inventory_items WHERE id = $1",
            &[&item_id],
        )
        .await
        .unwrap()
        .get(0);
    drop(client);
    assert_eq!(
        count_no_ctx, 0,
        "FORCE RLS must block reads without tenant context"
    );

    // 3. Read WITH correct tenant context → 1 row.
    let count_correct: i64 = db::with_tenant_read(&pool, tenant_a, |client| {
        Box::pin(async move {
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM warehouse.inventory_items WHERE id = $1",
                    &[&item_id],
                )
                .await
                .internal("count")?;
            Ok(row.get(0))
        })
    })
    .await
    .unwrap();
    assert_eq!(
        count_correct, 1,
        "with_tenant_read with correct tenant must see the row"
    );

    // 4. Read with WRONG tenant context → 0 rows.
    let tenant_b = TenantId::new();
    let count_wrong: i64 = db::with_tenant_read(&pool, tenant_b, |client| {
        Box::pin(async move {
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM warehouse.inventory_items WHERE id = $1",
                    &[&item_id],
                )
                .await
                .internal("count")?;
            Ok(row.get(0))
        })
    })
    .await
    .unwrap();
    assert_eq!(
        count_wrong, 0,
        "with_tenant_read with wrong tenant must NOT see the row"
    );

    // Cleanup.
    db::with_tenant_write(&pool, tenant_a, |client| {
        Box::pin(async move {
            client
                .execute(
                    "DELETE FROM warehouse.inventory_items WHERE id = $1",
                    &[&item_id],
                )
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
}
