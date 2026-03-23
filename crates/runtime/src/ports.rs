//! Порты (трейты зависимостей) для Command Pipeline.
//!
//! Pipeline не зависит от конкретной инфраструктуры — только от этих трейтов.
//! Реализации подключаются позже:
//! - `PermissionChecker` → RBAC + JWT (Layer 4a)
//! - `AuditLog` → `PostgreSQL` writer (Layer 4b)
//! - `ExtensionHooks` → Lua/WASM sandbox (Layer 8)
//! - `UnitOfWorkFactory` → `PostgreSQL` TX + RLS (Layer 2)

use async_trait::async_trait;
use event_bus::EventEnvelope;
use kernel::{AppError, RequestContext};

/// Проверка прав доступа перед выполнением команды.
///
/// Pipeline вызывает `check_permission` первым шагом.
/// Если `Err(AppError::Unauthorized)` — handler не вызывается.
#[async_trait]
pub trait PermissionChecker: Send + Sync + 'static {
    /// Проверить, имеет ли пользователь право выполнить команду.
    ///
    /// # Errors
    ///
    /// `AppError::Unauthorized` если доступ запрещён.
    async fn check_permission(
        &self,
        ctx: &RequestContext,
        command_name: &str,
    ) -> Result<(), AppError>;
}

/// Запись аудит-лога после выполнения команды.
///
/// Pipeline вызывает `log` после commit'а. Ошибки аудита не должны
/// ломать основной flow — они логируются через tracing.
#[async_trait]
pub trait AuditLog: Send + Sync + 'static {
    /// Записать результат выполнения команды в аудит-лог.
    async fn log(&self, ctx: &RequestContext, command_name: &str, result: &serde_json::Value);
}

/// Хуки расширений (Lua/WASM) — вызываются до и после команды.
///
/// `before_command` — валидация, обогащение контекста.
/// `after_command` — fire-and-forget side effects (email, webhook).
#[async_trait]
pub trait ExtensionHooks: Send + Sync + 'static {
    /// Хук перед выполнением команды. Может отменить выполнение.
    ///
    /// # Errors
    ///
    /// `AppError` если хук решил заблокировать команду.
    async fn before_command(
        &self,
        command_name: &str,
        ctx: &RequestContext,
    ) -> Result<(), AppError>;

    /// Хук после commit'а. Fire-and-forget — ошибки логируются.
    async fn after_command(
        &self,
        command_name: &str,
        ctx: &RequestContext,
    ) -> Result<(), anyhow::Error>;
}

/// Фабрика Unit of Work — создаёт транзакцию для каждой команды.
///
/// В modular monolith: `BEGIN` `PostgreSQL` транзакции + SET `tenant_id` (RLS).
/// Сейчас — `InMemoryUnitOfWorkFactory` (stub без БД).
#[async_trait]
pub trait UnitOfWorkFactory: Send + Sync + 'static {
    /// Тип Unit of Work, создаваемый фабрикой.
    type UoW: UnitOfWork;

    /// Начать новую транзакцию.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` если не удалось начать транзакцию.
    async fn begin(&self, ctx: &RequestContext) -> Result<Self::UoW, AppError>;
}

/// Unit of Work — абстракция транзакции.
///
/// Handler добавляет outbox-записи через `add_outbox_entry`.
/// Pipeline вызывает `commit` при успехе или `rollback` при ошибке.
#[async_trait]
pub trait UnitOfWork: Send + 'static {
    /// Добавить событие в outbox (будет отправлено после commit).
    fn add_outbox_entry(&mut self, envelope: EventEnvelope);

    /// Зафиксировать транзакцию. Все outbox-записи сохраняются.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` если commit не удался.
    async fn commit(self: Box<Self>) -> Result<(), AppError>;

    /// Откатить транзакцию. Outbox-записи отбрасываются.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` если rollback не удался.
    async fn rollback(self: Box<Self>) -> Result<(), AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Object safety: все трейты должны работать как trait objects
    fn _assert_permission_checker_object_safe(_: Arc<dyn PermissionChecker>) {}
    fn _assert_audit_log_object_safe(_: Arc<dyn AuditLog>) {}
    fn _assert_extension_hooks_object_safe(_: Arc<dyn ExtensionHooks>) {}
    fn _assert_unit_of_work_object_safe(_: Box<dyn UnitOfWork>) {}

    fn _assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn traits_are_object_safe_and_send_sync() {
        _assert_send_sync::<Arc<dyn PermissionChecker>>();
        _assert_send_sync::<Arc<dyn AuditLog>>();
        _assert_send_sync::<Arc<dyn ExtensionHooks>>();
    }
}
