//! Integration-тесты для audit crate.
//!
//! Требуют реальную PostgreSQL через DATABASE_URL.

use std::sync::Arc;

use kernel::types::{RequestContext, TenantId, UserId};
use runtime::ports::AuditLog;
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests")
}

fn new_ctx() -> RequestContext {
    RequestContext::new(TenantId::new(), UserId::new())
}

// ─── PgAuditLog ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_audit_log_writes_row() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let audit = audit::PgAuditLog::new(pool.clone());

    let ctx = new_ctx();
    let result = serde_json::json!({"status": "ok", "count": 42});

    audit.log(&ctx, "warehouse.receive_goods", &result).await;

    // Verify row exists.
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT command_name, correlation_id, user_id \
             FROM common.audit_log \
             WHERE tenant_id = $1 AND correlation_id = $2",
            &[ctx.tenant_id.as_uuid(), &ctx.correlation_id],
        )
        .await
        .unwrap();

    let command_name: String = row.get(0);
    let correlation_id: Uuid = row.get(1);
    let user_id: Uuid = row.get(2);

    assert_eq!(command_name, "warehouse.receive_goods");
    assert_eq!(correlation_id, ctx.correlation_id);
    assert_eq!(user_id, *ctx.user_id.as_uuid());

    // Cleanup.
    client
        .execute(
            "DELETE FROM common.audit_log WHERE correlation_id = $1",
            &[&ctx.correlation_id],
        )
        .await
        .unwrap();
}

// ─── DomainHistoryWriter ────────────────────────────────────────────────────

#[tokio::test]
async fn domain_history_writer_records_change() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());

    let ctx = new_ctx();
    let entity_id = Uuid::now_v7();
    let old_state = serde_json::json!({"qty": 0});
    let new_state = serde_json::json!({"qty": 100});

    // Use a direct connection (simulating inside-TX call).
    let client = pool.get().await.unwrap();
    let id = audit::DomainHistoryWriter::record(
        &**client,
        &ctx,
        "inventory_item",
        entity_id,
        "erp.warehouse.goods_received.v1",
        Some(&old_state),
        Some(&new_state),
    )
    .await
    .unwrap();
    assert!(id > 0);

    // Verify.
    let row = client
        .query_one(
            "SELECT entity_type, entity_id, event_type, correlation_id \
             FROM common.domain_history WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();
    let entity_type: String = row.get(0);
    let stored_entity_id: Uuid = row.get(1);
    let event_type: String = row.get(2);
    let correlation_id: Uuid = row.get(3);

    assert_eq!(entity_type, "inventory_item");
    assert_eq!(stored_entity_id, entity_id);
    assert_eq!(event_type, "erp.warehouse.goods_received.v1");
    assert_eq!(correlation_id, ctx.correlation_id);

    // Cleanup.
    client
        .execute("DELETE FROM common.domain_history WHERE id = $1", &[&id])
        .await
        .unwrap();
}

#[tokio::test]
async fn domain_history_writer_null_old_state() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());

    let ctx = new_ctx();
    let new_state = serde_json::json!({"name": "Bolt M8"});

    let client = pool.get().await.unwrap();
    let id = audit::DomainHistoryWriter::record(
        &**client,
        &ctx,
        "product",
        Uuid::now_v7(),
        "erp.catalog.product_created.v1",
        None,
        Some(&new_state),
    )
    .await
    .unwrap();
    assert!(id > 0);

    // Cleanup.
    client
        .execute("DELETE FROM common.domain_history WHERE id = $1", &[&id])
        .await
        .unwrap();
}
