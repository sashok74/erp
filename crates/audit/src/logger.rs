//! `PgAuditLog` — реализация `AuditLog` trait на `PostgreSQL`.
//!
//! Записывает audit log **после commit** основной транзакции.
//! Отдельное соединение из pool — не через `UoW` TX.
//! Best-effort: ошибки логируются, но не ломают бизнес-операцию.

use std::sync::Arc;

use async_trait::async_trait;
use kernel::RequestContext;
use runtime::ports::AuditLog;
use tracing::error;

use db::PgPool;

/// Audit log writer на `PostgreSQL`.
///
/// Подставляется в `CommandPipeline` вместо `NoopAuditLog`.
pub struct PgAuditLog {
    pool: Arc<PgPool>,
}

impl PgAuditLog {
    /// Создать writer с указанным pool.
    #[must_use]
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLog for PgAuditLog {
    async fn log(&self, ctx: &RequestContext, command_name: &str, result: &serde_json::Value) {
        if let Err(e) = self.write_audit(ctx, command_name, result).await {
            error!(
                command = command_name,
                error = %e,
                "audit log write failed (best-effort)"
            );
        }
    }
}

impl PgAuditLog {
    async fn write_audit(
        &self,
        ctx: &RequestContext,
        command_name: &str,
        result: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let client = self.pool.get().await?;
        let now = chrono::Utc::now().fixed_offset();
        let tenant_id = *ctx.tenant_id.as_uuid();
        let user_id = *ctx.user_id.as_uuid();
        clorinde_gen::queries::common::audit::insert_audit_log()
            .bind(
                &client,
                &tenant_id,
                &user_id,
                &command_name,
                result,
                &ctx.correlation_id,
                &ctx.causation_id,
                &now,
            )
            .one()
            .await?;
        Ok(())
    }
}
