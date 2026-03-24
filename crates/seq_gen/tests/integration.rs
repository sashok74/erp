//! Integration-тесты для seq_gen crate.
//!
//! Требуют реальную PostgreSQL через DATABASE_URL.

use std::sync::Arc;

use kernel::types::TenantId;
use seq_gen::PgSequenceGenerator;

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests")
}

#[tokio::test]
async fn next_value_returns_formatted_number() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let client = pool.get().await.unwrap();
    let tenant = TenantId::new();

    // Must be inside TX for FOR UPDATE.
    client.batch_execute("BEGIN").await.unwrap();

    let val = PgSequenceGenerator::next_value(&**client, tenant, "test_receipt", "ПРХ-")
        .await
        .unwrap();
    assert_eq!(val, "ПРХ-000001");

    let val2 = PgSequenceGenerator::next_value(&**client, tenant, "test_receipt", "ПРХ-")
        .await
        .unwrap();
    assert_eq!(val2, "ПРХ-000002");

    client.batch_execute("ROLLBACK").await.unwrap();

    // Cleanup (sequence was created before FOR UPDATE, so ensure persists even after rollback).
    // Actually the INSERT ON CONFLICT is inside the TX so it rolls back too. No cleanup needed.
}

#[tokio::test]
async fn different_tenants_independent() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let client = pool.get().await.unwrap();
    let tenant_a = TenantId::new();
    let tenant_b = TenantId::new();

    client.batch_execute("BEGIN").await.unwrap();

    let a1 = PgSequenceGenerator::next_value(&**client, tenant_a, "test_indep", "A-")
        .await
        .unwrap();
    let b1 = PgSequenceGenerator::next_value(&**client, tenant_b, "test_indep", "B-")
        .await
        .unwrap();

    assert_eq!(a1, "A-000001");
    assert_eq!(b1, "B-000001");

    client.batch_execute("ROLLBACK").await.unwrap();
}

#[tokio::test]
async fn different_seq_names_independent() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let client = pool.get().await.unwrap();
    let tenant = TenantId::new();

    client.batch_execute("BEGIN").await.unwrap();

    let receipt = PgSequenceGenerator::next_value(&**client, tenant, "test_receipt2", "ПРХ-")
        .await
        .unwrap();
    let shipment = PgSequenceGenerator::next_value(&**client, tenant, "test_shipment", "ОТГ-")
        .await
        .unwrap();

    assert_eq!(receipt, "ПРХ-000001");
    assert_eq!(shipment, "ОТГ-000001");

    client.batch_execute("ROLLBACK").await.unwrap();
}

#[tokio::test]
async fn concurrent_calls_gap_free() {
    let pool = Arc::new(db::PgPool::new(&database_url()).unwrap());
    let tenant = TenantId::new();
    let seq_name = "test_concurrent";

    // Pre-create the sequence so all concurrent tasks share the same one.
    {
        let client = pool.get().await.unwrap();
        client
            .execute(
                "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) \
                 VALUES ($1, $2, $3, 1) \
                 ON CONFLICT (tenant_id, seq_name) DO NOTHING",
                &[tenant.as_uuid(), &seq_name, &"C-"],
            )
            .await
            .unwrap();
    }

    // Spawn 10 tasks, each getting next_value inside its own TX.
    let mut handles = Vec::new();
    for _ in 0..10 {
        let pool = pool.clone();
        let tenant = tenant;
        handles.push(tokio::spawn(async move {
            let client = pool.get().await.unwrap();
            client.batch_execute("BEGIN").await.unwrap();
            let val = PgSequenceGenerator::next_value(&**client, tenant, seq_name, "C-")
                .await
                .unwrap();
            client.batch_execute("COMMIT").await.unwrap();
            val
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }

    // Sort results and verify gap-free.
    results.sort();
    let expected: Vec<String> = (1..=10).map(|i| format!("C-{i:06}")).collect();
    assert_eq!(
        results, expected,
        "should have 10 sequential gap-free numbers"
    );

    // Cleanup.
    let client = pool.get().await.unwrap();
    client
        .execute(
            "DELETE FROM common.sequences WHERE tenant_id = $1 AND seq_name = $2",
            &[tenant.as_uuid(), &seq_name],
        )
        .await
        .unwrap();
}
