//! Ошибки доменного слоя Warehouse BC.

use kernel::{AppError, DomainError};
use thiserror::Error;

/// Доменные ошибки складского модуля.
#[derive(Debug, Clone, Error)]
pub enum WarehouseDomainError {
    /// SKU невалиден (пустой или слишком длинный).
    #[error("SKU невалиден: {0}")]
    InvalidSku(String),

    /// Количество не может быть отрицательным.
    #[error("Количество не может быть отрицательным")]
    NegativeQuantity,

    /// Количество должно быть положительным для приёмки.
    #[error("Количество должно быть положительным")]
    ZeroQuantity,

    /// Недостаточно остатков для отгрузки.
    #[error("Недостаточно остатков: требуется {required}, доступно {available}")]
    InsufficientStock { required: String, available: String },
}

impl From<WarehouseDomainError> for AppError {
    fn from(e: WarehouseDomainError) -> Self {
        match e {
            WarehouseDomainError::InvalidSku(_)
            | WarehouseDomainError::NegativeQuantity
            | WarehouseDomainError::ZeroQuantity => AppError::Validation(e.to_string()),
            WarehouseDomainError::InsufficientStock {
                ref required,
                ref available,
            } => AppError::Domain(DomainError::InsufficientStock {
                required: required.clone(),
                available: available.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_sku_converts_to_validation() {
        let err: AppError = WarehouseDomainError::InvalidSku("empty".into()).into();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn insufficient_stock_converts_to_domain() {
        let err: AppError = WarehouseDomainError::InsufficientStock {
            required: "10".into(),
            available: "3".into(),
        }
        .into();
        assert!(matches!(
            err,
            AppError::Domain(DomainError::InsufficientStock { .. })
        ));
    }
}
