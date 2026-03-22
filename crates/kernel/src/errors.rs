//! Иерархия ошибок: доменные и прикладные.
//!
//! `DomainError` — нарушение бизнес-правил (не баг, а нормальная ситуация).
//! `AppError` — ошибки уровня приложения, включают доменные через `#[from]`.

use thiserror::Error;

/// Ошибки доменного слоя — нарушения бизнес-правил.
///
/// Эти ошибки ожидаемы и обрабатываемы. Например, попытка отгрузить
/// больше товара, чем есть на складе — не баг, а бизнес-ситуация.
#[derive(Debug, Clone, Error)]
pub enum DomainError {
    /// Недостаточно остатков для выполнения операции.
    #[error("Недостаточно остатков: требуется {required}, доступно {available}")]
    InsufficientStock { required: String, available: String },

    /// Баланс не может быть отрицательным.
    #[error("Баланс не может быть отрицательным")]
    NegativeBalance,

    /// Сущность не найдена.
    #[error("Не найдено: {0}")]
    NotFound(String),

    /// Конфликт конкурентного доступа (optimistic locking).
    #[error("Конфликт версий: ожидалась {expected}, получена {actual}")]
    ConcurrencyConflict { expected: i64, actual: i64 },

    /// Нарушение произвольного бизнес-правила.
    #[error("Бизнес-правило: {0}")]
    BusinessRule(String),
}

/// Ошибки прикладного слоя.
///
/// Объединяют доменные ошибки с инфраструктурными и авторизационными.
/// `DomainError` конвертируется автоматически через `?`-operator.
#[derive(Debug, Error)]
pub enum AppError {
    /// Доменная ошибка (бизнес-правило).
    #[error("{0}")]
    Domain(#[from] DomainError),

    /// Пользователь не авторизован или не имеет прав.
    #[error("Нет доступа: {0}")]
    Unauthorized(String),

    /// Ошибка валидации входных данных.
    #[error("Ошибка валидации: {0}")]
    Validation(String),

    /// Внутренняя ошибка (инфраструктура, БД, сеть).
    #[error("Внутренняя ошибка: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn domain_error_insufficient_stock_display() {
        let err = DomainError::InsufficientStock {
            required: "10".to_string(),
            available: "3".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Недостаточно остатков: требуется 10, доступно 3"
        );
    }

    #[test]
    fn domain_error_concurrency_conflict_display() {
        let err = DomainError::ConcurrencyConflict {
            expected: 5,
            actual: 3,
        };
        assert_eq!(err.to_string(), "Конфликт версий: ожидалась 5, получена 3");
    }

    #[test]
    fn domain_error_converts_to_app_error_via_from() {
        fn fallible() -> Result<(), AppError> {
            Err(DomainError::NegativeBalance)?
        }

        let err = fallible().unwrap_err();
        assert!(matches!(
            err,
            AppError::Domain(DomainError::NegativeBalance)
        ));
    }

    #[test]
    fn app_error_domain_source_returns_original() {
        let domain_err = DomainError::NotFound("Item #42".to_string());
        let app_err = AppError::Domain(domain_err);

        // source() returns the wrapped DomainError
        let source = app_err.source().expect("source should be present");
        let downcasted = source.downcast_ref::<DomainError>().unwrap();
        assert!(matches!(downcasted, DomainError::NotFound(msg) if msg == "Item #42"));
    }
}
