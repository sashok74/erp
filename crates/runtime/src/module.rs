//! Контракт Bounded Context модуля.
//!
//! Gateway не знает конкретные BC. Каждый BC регистрируется как модуль:
//! отдаёт HTTP routes и подписывает event handler'ы на шину.

use async_trait::async_trait;
use event_bus::traits::EventBus;

/// Модуль Bounded Context — точка регистрации BC в системе.
///
/// Каждый BC (Warehouse, Finance, ...) реализует этот trait.
/// Gateway собирает модули и объединяет их routes в единый HTTP-сервер.
#[async_trait]
pub trait BoundedContextModule: Send + Sync + 'static {
    /// Имя BC: `"warehouse"`, `"finance"`, `"audit"`.
    fn name(&self) -> &'static str;

    /// HTTP-маршруты модуля. Монтируются в общий Router gateway'ем.
    fn routes(&self) -> axum::Router;

    /// Зарегистрировать event handler'ы на шине.
    /// Вызывается при старте приложения.
    async fn register_handlers(&self, bus: &dyn EventBus);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Object safety: Box<dyn BoundedContextModule> должен компилироваться
    fn _assert_object_safe(_: Box<dyn BoundedContextModule>) {}

    fn _assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn bounded_context_module_is_object_safe_and_send_sync() {
        _assert_send_sync::<Box<dyn BoundedContextModule>>();
    }
}
