//! `WarehouseModule` — регистрация Warehouse BC в приложении.

/// Warehouse Bounded Context module.
pub struct WarehouseModule;

impl WarehouseModule {
    /// Имя модуля.
    #[must_use]
    pub fn name() -> &'static str {
        "warehouse"
    }
}
