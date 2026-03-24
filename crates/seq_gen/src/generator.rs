//! `PgSequenceGenerator` — gap-free per-tenant номера документов.
//!
//! `SELECT FOR UPDATE` внутри TX обеспечивает сериализацию:
//! конкурентные запросы блокируются, пока первый не завершит TX.
//!
//! Результат: `format!("{}{:06}", prefix, next_value)` → `"ПРХ-000001"`.

use kernel::types::TenantId;

/// Gap-free sequence generator. Вызывается внутри `UoW` TX.
pub struct PgSequenceGenerator;

impl PgSequenceGenerator {
    /// Получить следующий номер в последовательности.
    ///
    /// Три шага в одной TX:
    /// 1. Ensure sequence exists (idempotent)
    /// 2. Lock + read (`FOR UPDATE`)
    /// 3. Increment
    ///
    /// # Errors
    ///
    /// `anyhow::Error` при ошибке SQL или если sequence не найдена.
    pub async fn next_value(
        client: &impl tokio_postgres::GenericClient,
        tenant_id: TenantId,
        seq_name: &str,
        default_prefix: &str,
    ) -> Result<String, anyhow::Error> {
        // 1. Ensure sequence exists.
        client
            .execute(
                "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) \
                 VALUES ($1, $2, $3, 1) \
                 ON CONFLICT (tenant_id, seq_name) DO NOTHING",
                &[tenant_id.as_uuid(), &seq_name, &default_prefix],
            )
            .await?;

        // 2. Lock + read.
        let row = client
            .query_one(
                "SELECT prefix, next_value \
                 FROM common.sequences \
                 WHERE tenant_id = $1 AND seq_name = $2 \
                 FOR UPDATE",
                &[tenant_id.as_uuid(), &seq_name],
            )
            .await?;

        let prefix: String = row.get(0);
        let next_value: i64 = row.get(1);

        // 3. Increment.
        client
            .execute(
                "UPDATE common.sequences \
                 SET next_value = next_value + 1 \
                 WHERE tenant_id = $1 AND seq_name = $2",
                &[tenant_id.as_uuid(), &seq_name],
            )
            .await?;

        Ok(format!("{prefix}{next_value:06}"))
    }
}
