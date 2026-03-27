//! Query Pipeline — лёгкий конвейер обработки запросов.
//!
//! В отличие от [`CommandPipeline`](crate::pipeline::CommandPipeline), работает
//! без транзакции и `UnitOfWork`. Запрос проходит:
//! auth → `before_query` → handler → `after_query` (fire-and-forget) → audit.

use std::sync::Arc;

use kernel::{AppError, Query, RequestContext};
use tracing::{error, info};

use crate::ports::{AuditLog, ExtensionHooks, PermissionChecker};
use crate::query_handler::QueryHandler;

/// Лёгкий конвейер обработки запросов.
///
/// Содержит auth + hooks + audit как `Arc<dyn Trait>`.
/// Без `UnitOfWork`, без `EventBus`, без транзакции.
pub struct QueryPipeline {
    auth: Arc<dyn PermissionChecker>,
    extensions: Arc<dyn ExtensionHooks>,
    audit: Arc<dyn AuditLog>,
}

impl QueryPipeline {
    /// Создать pipeline со всеми зависимостями.
    #[must_use]
    pub fn new(
        auth: Arc<dyn PermissionChecker>,
        extensions: Arc<dyn ExtensionHooks>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            auth,
            extensions,
            audit,
        }
    }

    /// Выполнить запрос через конвейер.
    ///
    /// 1. `auth.check_permission()` → Err = прерывание
    /// 2. `extensions.before_query()` → Err = прерывание
    /// 3. `handler.handle(query, ctx)` → Err = прерывание
    /// 4. `tokio::spawn(extensions.after_query())` → fire-and-forget
    /// 5. `audit.log()` → запись
    /// 6. Return result
    ///
    /// # Errors
    ///
    /// `AppError` — ошибка авторизации, хука или handler'а.
    pub async fn execute<H: QueryHandler>(
        &self,
        handler: &H,
        query: &H::Query,
        ctx: &RequestContext,
    ) -> Result<H::Result, AppError> {
        let query_name = query.query_name();

        // 1. Авторизация
        self.auth.check_permission(ctx, query_name).await?;

        // 2. Before-hook (может отменить запрос)
        self.extensions.before_query(query_name, ctx).await?;

        // 3. Handler
        let result = handler.handle(query, ctx).await?;

        // 4. After-hook (fire-and-forget)
        let ext = Arc::clone(&self.extensions);
        let qn = query_name.to_string();
        let after_ctx = ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = ext.after_query(&qn, &after_ctx).await {
                error!(
                    query = qn,
                    error = %e,
                    "after_query hook failed"
                );
            }
        });

        // 5. Audit
        let audit_value = serde_json::to_value(&result)
            .unwrap_or_else(|_| serde_json::Value::String("ok".to_string()));
        self.audit.log(ctx, query_name, &audit_value).await;

        info!(query = query_name, "query executed successfully");

        // 6. Return
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_handler::QueryHandler;
    use crate::stubs::{
        NoopAuditLog, NoopExtensionHooks, NoopPermissionChecker, SpyAuditLog, SpyPermissionChecker,
    };
    use async_trait::async_trait;
    use kernel::types::{TenantId, UserId};
    use serde::Serialize;
    use std::sync::atomic::Ordering;

    // ─── Test fixtures ───────────────────────────────────────────────────

    struct PingQuery;

    impl Query for PingQuery {
        fn query_name(&self) -> &'static str {
            "test.ping"
        }
    }

    #[derive(Debug, Serialize, PartialEq)]
    struct PongResult {
        pong: bool,
    }

    struct PingHandler;

    #[async_trait]
    impl QueryHandler for PingHandler {
        type Query = PingQuery;
        type Result = PongResult;

        async fn handle(
            &self,
            _query: &Self::Query,
            _ctx: &RequestContext,
        ) -> Result<Self::Result, AppError> {
            Ok(PongResult { pong: true })
        }
    }

    struct FailingQueryHandler;

    #[async_trait]
    impl QueryHandler for FailingQueryHandler {
        type Query = PingQuery;
        type Result = PongResult;

        async fn handle(
            &self,
            _query: &Self::Query,
            _ctx: &RequestContext,
        ) -> Result<Self::Result, AppError> {
            Err(AppError::Domain(kernel::DomainError::NotFound(
                "not found".to_string(),
            )))
        }
    }

    fn test_ctx() -> RequestContext {
        RequestContext::new(TenantId::new(), UserId::new())
    }

    fn make_pipeline(
        auth: Arc<dyn PermissionChecker>,
        extensions: Arc<dyn ExtensionHooks>,
        audit: Arc<dyn AuditLog>,
    ) -> QueryPipeline {
        QueryPipeline::new(auth, extensions, audit)
    }

    // ─── Tests ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn happy_path_query_returns_result() {
        let pipeline = make_pipeline(
            Arc::new(NoopPermissionChecker),
            Arc::new(NoopExtensionHooks),
            Arc::new(NoopAuditLog),
        );

        let result = pipeline
            .execute(&PingHandler, &PingQuery, &test_ctx())
            .await
            .unwrap();
        assert_eq!(result, PongResult { pong: true });
    }

    #[tokio::test]
    async fn auth_reject_handler_not_called() {
        let spy_auth = SpyPermissionChecker::denying("no access");
        let called = Arc::clone(&spy_auth.called);

        let pipeline = make_pipeline(
            Arc::new(spy_auth),
            Arc::new(NoopExtensionHooks),
            Arc::new(NoopAuditLog),
        );

        let result = pipeline
            .execute(&PingHandler, &PingQuery, &test_ctx())
            .await;

        assert!(matches!(result, Err(AppError::Unauthorized(_))));
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn audit_log_records_query_on_success() {
        let spy_audit = Arc::new(SpyAuditLog::new());
        let recorded = Arc::clone(&spy_audit.recorded);

        let pipeline = make_pipeline(
            Arc::new(NoopPermissionChecker),
            Arc::new(NoopExtensionHooks),
            spy_audit,
        );

        pipeline
            .execute(&PingHandler, &PingQuery, &test_ctx())
            .await
            .unwrap();

        let names = recorded.lock().unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "test.ping");
    }

    #[tokio::test]
    async fn handler_error_no_audit() {
        let spy_audit = Arc::new(SpyAuditLog::new());
        let recorded = Arc::clone(&spy_audit.recorded);

        let pipeline = make_pipeline(
            Arc::new(NoopPermissionChecker),
            Arc::new(NoopExtensionHooks),
            spy_audit,
        );

        let result = pipeline
            .execute(&FailingQueryHandler, &PingQuery, &test_ctx())
            .await;

        assert!(result.is_err());
        let names = recorded.lock().unwrap();
        assert!(names.is_empty());
    }
}
