//! Command Pipeline — центральный конвейер обработки команд.
//!
//! Каждая команда проходит одинаковый путь:
//! auth → hooks → begin TX → handler → commit → after-hook → audit.
//!
//! Разработчик BC пишет только `CommandHandler`. Всё остальное — runtime.

use std::sync::Arc;

use event_bus::traits::EventBus;
use kernel::{AppError, Command, RequestContext};
use tracing::{error, info};

use crate::command_handler::CommandHandler;
use crate::ports::{AuditLog, ExtensionHooks, PermissionChecker, UnitOfWork, UnitOfWorkFactory};

/// Конвейер обработки команд.
///
/// Содержит все зависимости как `Arc<dyn Trait>` — dependency injection.
/// Параметризован `UF: UnitOfWorkFactory` для type-safe доступа к `UoW`.
pub struct CommandPipeline<UF: UnitOfWorkFactory> {
    uow_factory: Arc<UF>,
    #[allow(dead_code)]
    bus: Arc<dyn EventBus>,
    auth: Arc<dyn PermissionChecker>,
    extensions: Arc<dyn ExtensionHooks>,
    audit: Arc<dyn AuditLog>,
}

impl<UF: UnitOfWorkFactory> CommandPipeline<UF> {
    /// Создать pipeline со всеми зависимостями.
    #[must_use]
    pub fn new(
        uow_factory: Arc<UF>,
        bus: Arc<dyn EventBus>,
        auth: Arc<dyn PermissionChecker>,
        extensions: Arc<dyn ExtensionHooks>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            uow_factory,
            bus,
            auth,
            extensions,
            audit,
        }
    }

    /// Выполнить команду через полный конвейер.
    ///
    /// 1. `auth.check_permission()` → Err = прерывание
    /// 2. `extensions.before_command()` → Err = прерывание
    /// 3. `uow_factory.begin(ctx)` → BEGIN
    /// 4. `handler.handle(cmd, ctx, &mut uow)` → Err = rollback
    /// 5. `uow.commit()` → COMMIT
    /// 6. `tokio::spawn(extensions.after_command())` → fire-and-forget
    /// 7. `audit.log()` → запись
    /// 8. Return result
    ///
    /// # Errors
    ///
    /// `AppError` — ошибка авторизации, хука, handler'а или commit'а.
    pub async fn execute<H: CommandHandler>(
        &self,
        handler: &H,
        cmd: &H::Cmd,
        ctx: &RequestContext,
    ) -> Result<H::Result, AppError> {
        let command_name = cmd.command_name();

        // 1. Авторизация
        self.auth.check_permission(ctx, command_name).await?;

        // 2. Before-hook (может отменить команду)
        self.extensions.before_command(command_name, ctx).await?;

        // 3. Начинаем транзакцию
        let mut uow = self.uow_factory.begin(ctx).await?;

        // 4. Вызываем handler
        let result = handler.handle(cmd, ctx, &mut uow).await;

        match result {
            Ok(value) => {
                // 5. Commit
                Box::new(uow).commit().await?;

                // 6. After-hook (fire-and-forget)
                let ext = Arc::clone(&self.extensions);
                let cmd_name = command_name.to_string();
                let after_ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = ext.after_command(&cmd_name, &after_ctx).await {
                        error!(
                            command = cmd_name,
                            error = %e,
                            "after_command hook failed"
                        );
                    }
                });

                // 7. Audit
                let audit_value = serde_json::to_value(&value)
                    .unwrap_or_else(|_| serde_json::Value::String("ok".to_string()));
                self.audit.log(ctx, command_name, &audit_value).await;

                info!(command = command_name, "command executed successfully");

                // 8. Return
                Ok(value)
            }
            Err(e) => {
                // Rollback при ошибке handler'а
                if let Err(rb_err) = Box::new(uow).rollback().await {
                    error!(
                        command = command_name,
                        error = %rb_err,
                        "rollback failed"
                    );
                }
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::UnitOfWork;
    use crate::stubs::{
        InMemoryUnitOfWorkFactory, NoopAuditLog, NoopExtensionHooks, NoopPermissionChecker,
        SpyAuditLog, SpyPermissionChecker,
    };
    use async_trait::async_trait;
    use event_bus::InProcessBus;
    use kernel::DomainError;
    use kernel::types::{TenantId, UserId};
    use serde::Serialize;
    use std::sync::atomic::Ordering;

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

    /// Handler, который всегда возвращает ошибку.
    struct FailingHandler;

    #[async_trait]
    impl CommandHandler for FailingHandler {
        type Cmd = EchoCommand;
        type Result = EchoResult;

        async fn handle(
            &self,
            _cmd: &Self::Cmd,
            _ctx: &RequestContext,
            _uow: &mut dyn UnitOfWork,
        ) -> Result<Self::Result, AppError> {
            Err(AppError::Domain(DomainError::BusinessRule(
                "always fails".to_string(),
            )))
        }
    }

    /// Spy-hook: записывает вызовы before/after.
    struct SpyExtensionHooks {
        before_called: Arc<std::sync::atomic::AtomicBool>,
        reject: bool,
    }

    impl SpyExtensionHooks {
        fn accepting() -> Self {
            Self {
                before_called: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                reject: false,
            }
        }

        fn rejecting() -> Self {
            Self {
                before_called: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                reject: true,
            }
        }
    }

    #[async_trait]
    impl ExtensionHooks for SpyExtensionHooks {
        async fn before_command(
            &self,
            _command_name: &str,
            _ctx: &RequestContext,
        ) -> Result<(), AppError> {
            self.before_called.store(true, Ordering::SeqCst);
            if self.reject {
                Err(AppError::Validation("hook rejected".to_string()))
            } else {
                Ok(())
            }
        }

        async fn after_command(
            &self,
            _command_name: &str,
            _ctx: &RequestContext,
        ) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    fn test_ctx() -> RequestContext {
        RequestContext::new(TenantId::new(), UserId::new())
    }

    fn make_pipeline(
        uow_factory: Arc<InMemoryUnitOfWorkFactory>,
        auth: Arc<dyn PermissionChecker>,
        extensions: Arc<dyn ExtensionHooks>,
        audit: Arc<dyn AuditLog>,
    ) -> CommandPipeline<InMemoryUnitOfWorkFactory> {
        let bus = Arc::new(InProcessBus::new());
        CommandPipeline::new(uow_factory, bus, auth, extensions, audit)
    }

    // ─── Tests ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn happy_path_echo_handler_returns_result() {
        let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
        let pipeline = make_pipeline(
            Arc::clone(&uow_factory),
            Arc::new(NoopPermissionChecker),
            Arc::new(NoopExtensionHooks),
            Arc::new(NoopAuditLog),
        );

        let cmd = EchoCommand {
            message: "hello".to_string(),
        };
        let ctx = test_ctx();

        let result = pipeline.execute(&EchoHandler, &cmd, &ctx).await.unwrap();
        assert_eq!(
            result,
            EchoResult {
                echoed: "hello".to_string()
            }
        );
        assert!(uow_factory.committed.load(Ordering::SeqCst));
        assert!(!uow_factory.rolled_back.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn auth_reject_handler_not_called() {
        let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
        let spy_auth = SpyPermissionChecker::denying("no access");
        let called = Arc::clone(&spy_auth.called);

        let pipeline = make_pipeline(
            Arc::clone(&uow_factory),
            Arc::new(spy_auth),
            Arc::new(NoopExtensionHooks),
            Arc::new(NoopAuditLog),
        );

        let cmd = EchoCommand {
            message: "secret".to_string(),
        };
        let result = pipeline.execute(&EchoHandler, &cmd, &test_ctx()).await;

        assert!(matches!(result, Err(AppError::Unauthorized(_))));
        assert!(called.load(Ordering::SeqCst));
        // UoW не должен был начаться
        assert!(!uow_factory.committed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn hook_reject_handler_not_called() {
        let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
        let pipeline = make_pipeline(
            Arc::clone(&uow_factory),
            Arc::new(NoopPermissionChecker),
            Arc::new(SpyExtensionHooks::rejecting()),
            Arc::new(NoopAuditLog),
        );

        let cmd = EchoCommand {
            message: "blocked".to_string(),
        };
        let result = pipeline.execute(&EchoHandler, &cmd, &test_ctx()).await;

        assert!(matches!(result, Err(AppError::Validation(_))));
        assert!(!uow_factory.committed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn handler_error_triggers_rollback_not_commit() {
        let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
        let pipeline = make_pipeline(
            Arc::clone(&uow_factory),
            Arc::new(NoopPermissionChecker),
            Arc::new(NoopExtensionHooks),
            Arc::new(NoopAuditLog),
        );

        let cmd = EchoCommand {
            message: "fail".to_string(),
        };
        let result = pipeline.execute(&FailingHandler, &cmd, &test_ctx()).await;

        assert!(result.is_err());
        assert!(!uow_factory.committed.load(Ordering::SeqCst));
        assert!(uow_factory.rolled_back.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn audit_log_records_command_on_success() {
        let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
        let spy_audit = Arc::new(SpyAuditLog::new());
        let recorded = Arc::clone(&spy_audit.recorded);

        let pipeline = make_pipeline(
            uow_factory,
            Arc::new(NoopPermissionChecker),
            Arc::new(NoopExtensionHooks),
            spy_audit,
        );

        let cmd = EchoCommand {
            message: "audited".to_string(),
        };
        pipeline
            .execute(&EchoHandler, &cmd, &test_ctx())
            .await
            .unwrap();

        let names = recorded.lock().unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "test.echo");
    }
}
