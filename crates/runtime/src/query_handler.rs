//! Контракт обработчика запросов (CQRS — read side).
//!
//! Query не мутирует состояние, не создаёт событий, не требует транзакции.
//! Может маршрутизироваться на read-реплику, кэшироваться.

use async_trait::async_trait;
use kernel::{AppError, RequestContext};
use serde::Serialize;

/// Обработчик запроса. Read-only операция.
///
/// В отличие от `CommandHandler`, не получает `UnitOfWork` —
/// запросы не участвуют в транзакциях.
#[async_trait]
pub trait QueryHandler: Send + Sync + 'static {
    /// Тип запроса.
    type Query: Send + Sync;

    /// Результат запроса.
    type Result: Serialize + Send;

    /// Выполнить запрос.
    ///
    /// # Errors
    ///
    /// `AppError` — сущность не найдена, ошибка доступа и т.д.
    async fn handle(
        &self,
        query: &Self::Query,
        ctx: &RequestContext,
    ) -> Result<Self::Result, AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::types::{TenantId, UserId};

    // ─── Test fixtures ───────────────────────────────────────────────────

    struct PingQuery;

    #[derive(Debug, Serialize, PartialEq)]
    struct PongResult {
        pong: bool,
    }

    struct PingQueryHandler;

    #[async_trait]
    impl QueryHandler for PingQueryHandler {
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

    #[tokio::test]
    async fn ping_query_handler_returns_pong() {
        let handler = PingQueryHandler;
        let ctx = RequestContext::new(TenantId::new(), UserId::new());

        let result = handler.handle(&PingQuery, &ctx).await.unwrap();
        assert_eq!(result, PongResult { pong: true });
    }
}
