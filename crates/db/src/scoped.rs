//! Scoped transaction helpers для tenant-isolated DB доступа.
//!
//! - [`ReadScope`] — guard для read-only запросов (acquire → explicit `finish()`)
//! - [`with_tenant_write`] — closure-based write TX для event handlers
//!
//! Гарантируют, что `SET LOCAL app.tenant_id` всегда выполняется внутри транзакции.

use std::future::Future;
use std::pin::Pin;

use kernel::types::TenantId;
use kernel::{AppError, IntoInternal};

use crate::pool::PgPool;
use crate::rls::set_tenant_context;

/// Guard для read-only запросов с tenant isolation.
///
/// `BEGIN READ ONLY` + `SET LOCAL tenant_id` при acquire.
/// Read-only TX завершается через [`ReadScope::finish`].
/// Если scope потерян до `finish()`, pool recycle выполнит cleanup соединения.
///
/// ```ignore
/// let read = ReadScope::acquire(&pool, ctx.tenant_id).await?;
/// let repo = InventoryRepo::new(read.client(), ctx.tenant_id);
/// let row = repo.get_balance(&sku).await?;
/// read.finish().await?;
/// ```
pub struct ReadScope {
    client: deadpool_postgres::Client,
}

impl ReadScope {
    /// Открыть read-only TX с tenant context.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при ошибке checkout'а, `BEGIN` или `SET LOCAL`.
    pub async fn acquire(pool: &PgPool, tenant_id: TenantId) -> Result<Self, AppError> {
        let client = pool.get().await.internal("pool checkout")?;
        client
            .batch_execute("BEGIN READ ONLY")
            .await
            .internal("begin read")?;

        if let Err(e) = set_tenant_context(&**client, tenant_id).await {
            let _ = client.batch_execute("ROLLBACK").await;
            return Err(AppError::Internal(format!("set tenant: {e}")));
        }

        Ok(Self { client })
    }

    /// PostgreSQL-клиент внутри read-only TX с tenant context.
    pub fn client(&self) -> &deadpool_postgres::Client {
        &self.client
    }

    /// Завершить read-only TX (COMMIT). Вызывать после всех запросов.
    ///
    /// Если не вызвать — соединение вернётся в пул с открытой TX,
    /// `RecyclingMethod::Clean` выполнит `DISCARD ALL` при recycle.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при ошибке COMMIT (маловероятно для read-only TX).
    pub async fn finish(self) -> Result<(), AppError> {
        self.client
            .batch_execute("COMMIT")
            .await
            .internal("read scope commit")
    }
}

/// Read-write transaction с tenant isolation.
///
/// `BEGIN` → `SET LOCAL tenant_id` → closure → `COMMIT`/`ROLLBACK`.
///
/// Используется в event handler'ах и других write-операциях
/// вне `CommandPipeline` (который использует `PgUnitOfWork`).
///
/// # Errors
///
/// Возвращает ошибку closure или ошибку БД при проблемах с транзакцией.
pub async fn with_tenant_write<T: Send>(
    pool: &PgPool,
    tenant_id: TenantId,
    f: impl for<'a> FnOnce(
        &'a deadpool_postgres::Client,
    ) -> Pin<Box<dyn Future<Output = Result<T, anyhow::Error>> + Send + 'a>>,
) -> Result<T, anyhow::Error> {
    let client = pool.get().await?;
    client.batch_execute("BEGIN").await?;

    if let Err(e) = set_tenant_context(&**client, tenant_id).await {
        let _ = client.batch_execute("ROLLBACK").await;
        return Err(e);
    }

    match f(&client).await {
        Ok(v) => {
            client.batch_execute("COMMIT").await?;
            Ok(v)
        }
        Err(e) => {
            let _ = client.batch_execute("ROLLBACK").await;
            Err(e)
        }
    }
}
