//! Контракт Bounded Context модуля.
//!
//! Каждый BC регистрируется как модуль: предоставляет метаданные,
//! подписывает event handler'ы на шину, указывает путь к миграциям.
//! HTTP-маршруты строятся в gateway (delivery-слой), а не в runtime.

use async_trait::async_trait;
use event_bus::traits::EventBus;

/// Модуль Bounded Context — точка регистрации BC в системе.
///
/// Каждый BC (Warehouse, Finance, ...) реализует этот trait.
/// Зависимости (pool, etc.) хранятся в struct, а не передаются в методы.
/// HTTP-маршрутизация — ответственность gateway (`AppBuilder`).
#[async_trait]
pub trait BoundedContextModule: Send + Sync + 'static {
    /// Имя BC: `"warehouse"`, `"finance"`, `"catalog"`.
    fn name(&self) -> &'static str;

    /// Путь к директории миграций: `"migrations/warehouse"`.
    fn migrations_dir(&self) -> &'static str;

    /// Зарегистрировать event handler'ы на шине.
    /// Вызывается при старте приложения.
    async fn register_handlers(&self, bus: &dyn EventBus);
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;

    struct StubModule;

    #[async_trait]
    impl BoundedContextModule for StubModule {
        fn name(&self) -> &'static str {
            "stub"
        }

        fn migrations_dir(&self) -> &'static str {
            "migrations/stub"
        }

        async fn register_handlers(&self, _bus: &dyn EventBus) {}
    }

    fn assert_object_safe(_: Box<dyn BoundedContextModule>) {}

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn bounded_context_module_is_object_safe_and_send_sync() {
        assert_object_safe(Box::new(StubModule));
        assert_send_sync::<Box<dyn BoundedContextModule>>();
    }
}
