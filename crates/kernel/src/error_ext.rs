//! Extension trait для преобразования произвольных ошибок в `AppError::Internal`.
//!
//! Заменяет повторяющийся `.map_err(|e| AppError::Internal(format!("ctx: {e}")))`.
//!
//! ```ignore
//! // Было:
//! repo.find_by_sku(client, tid, sku).await
//!     .map_err(|e| AppError::Internal(format!("find_by_sku: {e}")))?;
//!
//! // Стало:
//! repo.find_by_sku(client, tid, sku).await.internal("find_by_sku")?;
//! ```

use crate::errors::AppError;

/// Преобразование `Result<T, E>` в `Result<T, AppError::Internal>` с контекстом.
pub trait IntoInternal<T> {
    /// Маппит ошибку в `AppError::Internal("{context}: {original_error}")`.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` с контекстом и исходной ошибкой.
    fn internal(self, context: &str) -> Result<T, AppError>;
}

impl<T, E: std::fmt::Display> IntoInternal<T> for Result<T, E> {
    fn internal(self, context: &str) -> Result<T, AppError> {
        self.map_err(|e| AppError::Internal(format!("{context}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_io_error_converts_with_context() {
        let result: Result<i32, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused"));
        let err = result.internal("db_connect").unwrap_err();
        match err {
            AppError::Internal(msg) => assert!(msg.contains("db_connect: connection refused")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn string_error_converts_with_context() {
        let result: Result<(), String> = Err("bad input".to_string());
        let err = result.internal("parse").unwrap_err();
        assert!(matches!(err, AppError::Internal(msg) if msg == "parse: bad input"));
    }

    #[test]
    fn ok_passes_through() {
        let result: Result<i32, String> = Ok(42);
        assert_eq!(result.internal("ctx").unwrap(), 42);
    }
}
