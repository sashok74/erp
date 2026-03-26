//! Контракт Bounded Context модуля.
//!
//! Каждый BC регистрируется как модуль: подписывает event handler'ы на шину.
//! HTTP-маршруты строятся в gateway (delivery-слой), а не в runtime.

use async_trait::async_trait;
use event_bus::traits::EventBus;

/// Модуль Bounded Context — точка регистрации BC в системе.
///
/// Каждый BC (Warehouse, Finance, ...) реализует этот trait.
/// HTTP-маршрутизация — ответственность gateway (`BcRouter`).
#[async_trait]
pub trait BoundedContextModule: Send + Sync + 'static {
    /// Имя BC: `"warehouse"`, `"finance"`, `"audit"`.
    fn name(&self) -> &'static str;

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
