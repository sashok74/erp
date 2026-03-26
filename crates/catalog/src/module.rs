//! `CatalogModule` — регистрация Catalog BC в приложении.

/// Catalog Bounded Context module.
pub struct CatalogModule;

impl CatalogModule {
    /// Имя модуля.
    #[must_use]
    pub fn name() -> &'static str {
        "catalog"
    }
}
