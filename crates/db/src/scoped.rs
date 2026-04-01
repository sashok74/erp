//! Scoped transaction helpers — closure-based `BEGIN` + `SET LOCAL` + `COMMIT`/`ROLLBACK`.
//!
//! Гарантируют, что `SET LOCAL app.tenant_id` всегда выполняется внутри транзакции.
//! Разработчику не нужно думать о `BEGIN`/`COMMIT` — обёртка делает всё сама.
//!
//! ```ignore
//! db::with_tenant_read(&pool, ctx.tenant_id, |client| Box::pin(async move {
//!     let row = Repo::find(client, ctx.tenant_id, &sku).await?;
//!     Ok(row)
//! })).await
//! ```

use std::future::Future;
use std::pin::Pin;

use kernel::types::TenantId;
use kernel::{AppError, IntoInternal};

use crate::pool::PgPool;
use crate::rls::set_tenant_context;

/// Read-only transaction с tenant isolation.
///
/// `BEGIN READ ONLY` → `SET LOCAL tenant_id` → closure → `COMMIT`/`ROLLBACK`.
///
/// Используется в query handler'ах. `READ ONLY` предотвращает случайные writes
/// и позволяет `PostgreSQL` применять read-only оптимизации.
///
/// # Errors
///
/// Возвращает ошибку closure или `AppError::Internal` при проблемах с БД.
pub async fn with_tenant_read<T>(
    pool: &PgPool,
    tenant_id: TenantId,
    f: impl for<'a> FnOnce(
        &'a deadpool_postgres::Client,
    ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'a>>,
) -> Result<T, AppError> {
    let client = pool.get().await.internal("pool checkout")?;
    client
        .batch_execute("BEGIN READ ONLY")
        .await
        .internal("begin read")?;

    if let Err(e) = set_tenant_context(&**client, tenant_id).await {
        let _ = client.batch_execute("ROLLBACK").await;
        return Err(AppError::Internal(format!("set tenant: {e}")));
    }

    match f(&client).await {
        Ok(v) => {
            let _ = client.batch_execute("COMMIT").await;
            Ok(v)
        }
        Err(e) => {
            let _ = client.batch_execute("ROLLBACK").await;
            Err(e)
        }
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
    )
        -> Pin<Box<dyn Future<Output = Result<T, anyhow::Error>> + Send + 'a>>,
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
