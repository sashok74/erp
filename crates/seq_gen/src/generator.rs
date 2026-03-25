//! `PgSequenceGenerator` — gap-free per-tenant номера документов.
//!
//! `SELECT FOR UPDATE` внутри TX обеспечивает сериализацию:
//! конкурентные запросы блокируются, пока первый не завершит TX.
//!
//! Результат: `format!("{}{:06}", prefix, next_value)` → `"ПРХ-000001"`.

use clorinde_gen::client::GenericClient;
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
        client: &impl GenericClient,
        tenant_id: TenantId,
        seq_name: &str,
        default_prefix: &str,
    ) -> Result<String, anyhow::Error> {
        let tid = *tenant_id.as_uuid();

        // 1. Ensure sequence exists.
        clorinde_gen::queries::common::sequences::ensure_sequence()
            .bind(client, &tid, &seq_name, &default_prefix)
            .await?;

        // 2. Lock + read.
        let val = clorinde_gen::queries::common::sequences::next_value()
            .bind(client, &tid, &seq_name)
            .one()
            .await?;

        let prefix = val.prefix;
        let next_value = val.next_value;

        // 3. Increment.
        clorinde_gen::queries::common::sequences::increment_sequence()
            .bind(client, &tid, &seq_name)
            .await?;

        Ok(format!("{prefix}{next_value:06}"))
    }
}
