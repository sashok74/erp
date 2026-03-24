//! Stub-реализации портов для тестирования без инфраструктуры.
//!
//! Два вида:
//! - **Noop** — ничего не делают, всегда `Ok`. Для простых тестов.
//! - **Spy** — записывают вызовы. Для проверки, что Pipeline вызвал нужные шаги.

use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use event_bus::EventEnvelope;
use kernel::{AppError, RequestContext};

use crate::ports::{AuditLog, ExtensionHooks, PermissionChecker, UnitOfWork, UnitOfWorkFactory};

// ─── Noop stubs ──────────────────────────────────────────────────────────────

/// Всегда разрешает доступ. Для тестов без авторизации.
pub struct NoopPermissionChecker;

#[async_trait]
impl PermissionChecker for NoopPermissionChecker {
    async fn check_permission(
        &self,
        _ctx: &RequestContext,
        _command_name: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

/// Ничего не записывает. Для тестов без аудита.
pub struct NoopAuditLog;

#[async_trait]
impl AuditLog for NoopAuditLog {
    async fn log(&self, _ctx: &RequestContext, _command_name: &str, _result: &serde_json::Value) {}
}

/// Хуки-заглушки: before/after всегда Ok.
pub struct NoopExtensionHooks;

#[async_trait]
impl ExtensionHooks for NoopExtensionHooks {
    async fn before_command(
        &self,
        _command_name: &str,
        _ctx: &RequestContext,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn after_command(
        &self,
        _command_name: &str,
        _ctx: &RequestContext,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

// ─── InMemory UoW ────────────────────────────────────────────────────────────

/// In-memory `UoW` фабрика. Создаёт `InMemoryUnitOfWork` без реальной транзакции.
#[derive(Clone)]
pub struct InMemoryUnitOfWorkFactory {
    /// Shared-флаг: был ли вызван commit на последнем `UoW`.
    pub committed: Arc<AtomicBool>,
    /// Shared-флаг: был ли вызван rollback на последнем `UoW`.
    pub rolled_back: Arc<AtomicBool>,
}

impl InMemoryUnitOfWorkFactory {
    /// Создать фабрику с отслеживанием commit/rollback.
    #[must_use]
    pub fn new() -> Self {
        Self {
            committed: Arc::new(AtomicBool::new(false)),
            rolled_back: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for InMemoryUnitOfWorkFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnitOfWorkFactory for InMemoryUnitOfWorkFactory {
    type UoW = InMemoryUnitOfWork;

    async fn begin(&self, _ctx: &RequestContext) -> Result<Self::UoW, AppError> {
        // Сбрасываем флаги при каждом begin
        self.committed.store(false, Ordering::SeqCst);
        self.rolled_back.store(false, Ordering::SeqCst);
        Ok(InMemoryUnitOfWork {
            entries: Vec::new(),
            committed: Arc::clone(&self.committed),
            rolled_back: Arc::clone(&self.rolled_back),
        })
    }
}

/// In-memory Unit of Work. Хранит outbox-записи в `Vec`, commit/rollback ставят флаги.
pub struct InMemoryUnitOfWork {
    /// Outbox-записи, добавленные handler'ом.
    pub entries: Vec<EventEnvelope>,
    /// Shared-флаг commit'а — доступен через фабрику.
    committed: Arc<AtomicBool>,
    /// Shared-флаг rollback'а — доступен через фабрику.
    rolled_back: Arc<AtomicBool>,
}

#[async_trait]
impl UnitOfWork for InMemoryUnitOfWork {
    fn add_outbox_entry(&mut self, envelope: EventEnvelope) {
        self.entries.push(envelope);
    }

    async fn commit(self: Box<Self>) -> Result<(), AppError> {
        self.committed.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> Result<(), AppError> {
        self.rolled_back.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ─── Spy stubs ───────────────────────────────────────────────────────────────

/// Spy-авторизация: записывает факт вызова + можно настроить результат.
pub struct SpyPermissionChecker {
    /// Был ли вызван `check_permission`.
    pub called: Arc<AtomicBool>,
    /// `None` = разрешить, `Some(reason)` = отказать с `Unauthorized`.
    deny_reason: Option<String>,
}

impl SpyPermissionChecker {
    /// Создать spy, который разрешает доступ.
    #[must_use]
    pub fn allowing() -> Self {
        Self {
            called: Arc::new(AtomicBool::new(false)),
            deny_reason: None,
        }
    }

    /// Создать spy, который запрещает доступ.
    #[must_use]
    pub fn denying(reason: &str) -> Self {
        Self {
            called: Arc::new(AtomicBool::new(false)),
            deny_reason: Some(reason.to_string()),
        }
    }
}

#[async_trait]
impl PermissionChecker for SpyPermissionChecker {
    async fn check_permission(
        &self,
        _ctx: &RequestContext,
        _command_name: &str,
    ) -> Result<(), AppError> {
        self.called.store(true, Ordering::SeqCst);
        match &self.deny_reason {
            None => Ok(()),
            Some(reason) => Err(AppError::Unauthorized(reason.clone())),
        }
    }
}

/// Spy-аудит: записывает имена команд для проверки.
pub struct SpyAuditLog {
    /// Записанные имена команд.
    pub recorded: Arc<Mutex<Vec<String>>>,
}

impl SpyAuditLog {
    /// Создать spy с пустым журналом.
    #[must_use]
    pub fn new() -> Self {
        Self {
            recorded: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for SpyAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuditLog for SpyAuditLog {
    async fn log(&self, _ctx: &RequestContext, command_name: &str, _result: &serde_json::Value) {
        self.recorded
            .lock()
            .expect("SpyAuditLog mutex poisoned")
            .push(command_name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::types::{TenantId, UserId};

    fn test_ctx() -> RequestContext {
        RequestContext::new(TenantId::new(), UserId::new())
    }

    #[tokio::test]
    async fn noop_permission_checker_always_ok() {
        let checker = NoopPermissionChecker;
        let result = checker.check_permission(&test_ctx(), "test.cmd").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn noop_extension_hooks_always_ok() {
        let hooks = NoopExtensionHooks;
        let ctx = test_ctx();
        assert!(hooks.before_command("test.cmd", &ctx).await.is_ok());
        assert!(hooks.after_command("test.cmd", &ctx).await.is_ok());
    }

    #[tokio::test]
    async fn in_memory_uow_add_outbox_entry_grows_vec() {
        let factory = InMemoryUnitOfWorkFactory::new();
        let mut uow = factory.begin(&test_ctx()).await.unwrap();

        assert_eq!(uow.entries.len(), 0);

        let envelope = make_test_envelope();
        uow.add_outbox_entry(envelope);
        assert_eq!(uow.entries.len(), 1);

        let envelope2 = make_test_envelope();
        uow.add_outbox_entry(envelope2);
        assert_eq!(uow.entries.len(), 2);
    }

    #[tokio::test]
    async fn in_memory_uow_commit_sets_flag() {
        let factory = InMemoryUnitOfWorkFactory::new();
        let uow = factory.begin(&test_ctx()).await.unwrap();

        assert!(!factory.committed.load(Ordering::SeqCst));
        Box::new(uow).commit().await.unwrap();
        assert!(factory.committed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn in_memory_uow_rollback_sets_flag() {
        let factory = InMemoryUnitOfWorkFactory::new();
        let uow = factory.begin(&test_ctx()).await.unwrap();

        assert!(!factory.rolled_back.load(Ordering::SeqCst));
        Box::new(uow).rollback().await.unwrap();
        assert!(factory.rolled_back.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn spy_permission_checker_allowing_records_call() {
        let spy = SpyPermissionChecker::allowing();
        let called = Arc::clone(&spy.called);

        assert!(!called.load(Ordering::SeqCst));
        let result = spy.check_permission(&test_ctx(), "test.cmd").await;
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn spy_permission_checker_denying_returns_error() {
        let spy = SpyPermissionChecker::denying("forbidden");
        let result = spy.check_permission(&test_ctx(), "test.cmd").await;
        assert!(matches!(result, Err(AppError::Unauthorized(_))));
    }

    #[tokio::test]
    async fn spy_audit_log_records_command_names() {
        let spy = SpyAuditLog::new();
        let recorded = Arc::clone(&spy.recorded);

        spy.log(&test_ctx(), "warehouse.receive", &serde_json::json!({}))
            .await;
        spy.log(&test_ctx(), "warehouse.ship", &serde_json::json!({}))
            .await;

        let names = recorded.lock().unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "warehouse.receive");
        assert_eq!(names[1], "warehouse.ship");
    }

    fn make_test_envelope() -> EventEnvelope {
        use kernel::DomainEvent;
        use serde::Serialize;
        use uuid::Uuid;

        #[derive(Debug, Clone, Serialize)]
        struct Evt {
            id: Uuid,
        }

        impl DomainEvent for Evt {
            fn event_type(&self) -> &'static str {
                "test.evt.v1"
            }
            fn aggregate_id(&self) -> Uuid {
                self.id
            }
        }

        let evt = Evt { id: Uuid::now_v7() };
        let ctx = test_ctx();
        EventEnvelope::from_domain_event(&evt, &ctx, "test").unwrap()
    }
}
