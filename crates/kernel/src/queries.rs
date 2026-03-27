//! Контракт запросов (CQRS) — read side identity.
//!
//! `Query` — зеркало [`Command`](crate::commands::Command) для read-операций.
//! Предоставляет стабильный identity key (`query_name`) для авторизации,
//! аудита и routing'а через `QueryPipeline`.

/// Контракт для всех запросов в системе.
///
/// Каждый запрос — отдельная структура, реализующая этот trait.
/// Bounds `Send + Sync + 'static` необходимы для передачи между потоками.
pub trait Query: Send + Sync + 'static {
    /// Имя запроса для routing, аудита и логирования.
    ///
    /// Формат: `"bc_name.query_name"`, например `"warehouse.get_balance"`.
    fn query_name(&self) -> &'static str;
}
