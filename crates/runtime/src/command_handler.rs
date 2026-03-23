//! Контракт обработчика команд (CQRS — write side).
//!
//! Разработчик BC реализует `CommandHandler` для каждой команды.
//! Pipeline вызывает handler внутри транзакции (`UnitOfWork`).

use async_trait::async_trait;
use kernel::{AppError, Command, RequestContext};
use serde::Serialize;

use crate::ports::UnitOfWork;

/// Обработчик команды. Единственное, что пишет разработчик BC.
///
/// Associated types обеспечивают type safety на этапе компиляции:
/// `ReceiveGoodsHandler` нельзя вызвать с `ShipGoodsCommand`.
///
/// Handler получает `&mut dyn UnitOfWork` для добавления outbox-записей.
/// Commit/rollback выполняет Pipeline, не handler.
#[async_trait]
pub trait CommandHandler: Send + Sync + 'static {
    /// Команда, которую обрабатывает handler.
    type Cmd: Command;

    /// Результат успешного выполнения.
    type Result: Serialize + Send;

    /// Выполнить бизнес-логику команды.
    ///
    /// # Errors
    ///
    /// `AppError` — доменная или прикладная ошибка. Pipeline вызовет rollback.
    async fn handle(
        &self,
        cmd: &Self::Cmd,
        ctx: &RequestContext,
        uow: &mut dyn UnitOfWork,
    ) -> Result<Self::Result, AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::UnitOfWorkFactory;
    use kernel::types::{TenantId, UserId};

    // ─── Test fixtures ───────────────────────────────────────────────────

    #[derive(Debug)]
    struct EchoCommand {
        message: String,
    }

    impl Command for EchoCommand {
        fn command_name(&self) -> &'static str {
            "test.echo"
        }
    }

    #[derive(Debug, Serialize, PartialEq)]
    struct EchoResult {
        echoed: String,
    }

    struct EchoHandler;

    #[async_trait]
    impl CommandHandler for EchoHandler {
        type Cmd = EchoCommand;
        type Result = EchoResult;

        async fn handle(
            &self,
            cmd: &Self::Cmd,
            _ctx: &RequestContext,
            _uow: &mut dyn UnitOfWork,
        ) -> Result<Self::Result, AppError> {
            Ok(EchoResult {
                echoed: cmd.message.clone(),
            })
        }
    }

    #[tokio::test]
    async fn echo_handler_returns_echoed_message() {
        let handler = EchoHandler;
        let cmd = EchoCommand {
            message: "hello".to_string(),
        };
        let ctx = RequestContext::new(TenantId::new(), UserId::new());

        let mut uow = crate::stubs::InMemoryUnitOfWorkFactory::new()
            .begin(&ctx)
            .await
            .unwrap();

        let result = handler.handle(&cmd, &ctx, &mut uow).await.unwrap();
        assert_eq!(
            result,
            EchoResult {
                echoed: "hello".to_string()
            }
        );
    }
}
