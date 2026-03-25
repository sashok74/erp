//! Value Objects для Catalog BC: `Sku`, `ProductName`.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::errors::CatalogDomainError;

/// SKU — артикул товара.
///
/// Непустой, <= 50 символов.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sku(String);

impl Sku {
    /// Создать SKU с валидацией.
    ///
    /// # Errors
    ///
    /// `InvalidSku` — пустой или длиннее 50 символов.
    pub fn new(value: impl Into<String>) -> Result<Self, CatalogDomainError> {
        let s = value.into();
        if s.is_empty() {
            return Err(CatalogDomainError::InvalidSku(
                "SKU не может быть пустым".into(),
            ));
        }
        if s.len() > 50 {
            return Err(CatalogDomainError::InvalidSku(format!(
                "SKU не может быть длиннее 50 символов: {}",
                s.len()
            )));
        }
        Ok(Self(s))
    }

    /// Строковое представление.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sku {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Наименование товара.
///
/// Непустое, <= 200 символов.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProductName(String);

impl ProductName {
    /// Создать наименование с валидацией.
    ///
    /// # Errors
    ///
    /// `InvalidName` — пустое или длиннее 200 символов.
    pub fn new(value: impl Into<String>) -> Result<Self, CatalogDomainError> {
        let s = value.into();
        if s.is_empty() {
            return Err(CatalogDomainError::InvalidName(
                "Наименование не может быть пустым".into(),
            ));
        }
        if s.len() > 200 {
            return Err(CatalogDomainError::InvalidName(format!(
                "Наименование не может быть длиннее 200 символов: {}",
                s.len()
            )));
        }
        Ok(Self(s))
    }

    /// Строковое представление.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProductName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Sku ────────────────────────────────────────────────────────────

    #[test]
    fn sku_valid() {
        let sku = Sku::new("BOLT-42").unwrap();
        assert_eq!(sku.as_str(), "BOLT-42");
    }

    #[test]
    fn sku_empty_rejected() {
        assert!(Sku::new("").is_err());
    }

    #[test]
    fn sku_too_long_rejected() {
        let long = "A".repeat(51);
        assert!(Sku::new(long).is_err());
    }

    #[test]
    fn sku_max_length_ok() {
        let max = "A".repeat(50);
        assert!(Sku::new(max).is_ok());
    }

    #[test]
    fn sku_serde_round_trip() {
        let sku = Sku::new("NUT-7").unwrap();
        let json = serde_json::to_string(&sku).unwrap();
        let restored: Sku = serde_json::from_str(&json).unwrap();
        assert_eq!(sku, restored);
    }

    // ─── ProductName ──────────────────────────────────────────────────────

    #[test]
    fn name_valid() {
        let name = ProductName::new("Болт M10x50").unwrap();
        assert_eq!(name.as_str(), "Болт M10x50");
    }

    #[test]
    fn name_empty_rejected() {
        assert!(ProductName::new("").is_err());
    }

    #[test]
    fn name_too_long_rejected() {
        let long = "A".repeat(201);
        assert!(ProductName::new(long).is_err());
    }

    #[test]
    fn name_max_length_ok() {
        let max = "A".repeat(200);
        assert!(ProductName::new(max).is_ok());
    }

    #[test]
    fn name_serde_round_trip() {
        let name = ProductName::new("Гайка M10").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        let restored: ProductName = serde_json::from_str(&json).unwrap();
        assert_eq!(name, restored);
    }
}
