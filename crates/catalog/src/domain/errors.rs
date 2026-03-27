//! Ошибки доменного слоя Catalog BC.

use kernel::{AppError, DomainError};
use thiserror::Error;

/// Доменные ошибки каталога товаров.
#[derive(Debug, Clone, Error)]
pub enum CatalogDomainError {
    /// SKU невалиден (пустой или слишком длинный).
    #[error("SKU невалиден: {0}")]
    InvalidSku(String),

    /// Наименование невалидно (пустое или слишком длинное).
    #[error("Наименование невалидно: {0}")]
    InvalidName(String),

    /// Товар с таким SKU уже существует.
    #[error("Товар с SKU '{0}' уже существует")]
    DuplicateSku(String),
}

impl From<CatalogDomainError> for AppError {
    fn from(e: CatalogDomainError) -> Self {
        match e {
            CatalogDomainError::InvalidSku(_) | CatalogDomainError::InvalidName(_) => {
                AppError::Validation(e.to_string())
            }
            CatalogDomainError::DuplicateSku(_) => {
                AppError::Domain(DomainError::BusinessRule(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_sku_converts_to_validation() {
        let err: AppError = CatalogDomainError::InvalidSku("empty".into()).into();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn invalid_name_converts_to_validation() {
        let err: AppError = CatalogDomainError::InvalidName("empty".into()).into();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn duplicate_sku_converts_to_domain() {
        let err: AppError = CatalogDomainError::DuplicateSku("BOLT-42".into()).into();
        assert!(matches!(
            err,
            AppError::Domain(DomainError::BusinessRule(_))
        ));
    }
}
