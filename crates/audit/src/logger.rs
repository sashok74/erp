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
        let tenant_id = ctx.tenant_id;
        let user_id = *ctx.user_id.as_uuid();
        let correlation_id = ctx.correlation_id;
        let causation_id = ctx.causation_id;
        let command_name = command_name.to_string();
        let result = result.clone();

        db::with_tenant_write(&self.pool, tenant_id, |client| {
            Box::pin(async move {
                let now = chrono::Utc::now().fixed_offset();
                let tid = *tenant_id.as_uuid();
                clorinde_gen::queries::common::audit::insert_audit_log()
                    .bind(
                        client,
                        &tid,
                        &user_id,
                        &command_name.as_str(),
                        &result,
                        &correlation_id,
                        &causation_id,
                        &now,
                    )
                    .one()
                    .await?;
                Ok(())
            })
        })
        .await
    }
}
