//! Value Objects для Warehouse BC: `Sku`, `Quantity`.
//!
//! Живут в BC, не в kernel (решение Platform SDK).

use std::fmt;
use std::ops::{Add, Sub};

use bigdecimal::{BigDecimal, Zero};
use serde::{Deserialize, Serialize};

use super::errors::WarehouseDomainError;

/// SKU — артикул товара.
///
/// Непустой, ≤50 символов.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sku(String);

impl Sku {
    /// Создать SKU с валидацией.
    ///
    /// # Errors
    ///
    /// `InvalidSku` — пустой или длиннее 50 символов.
    pub fn new(value: impl Into<String>) -> Result<Self, WarehouseDomainError> {
        let s = value.into();
        if s.is_empty() {
            return Err(WarehouseDomainError::InvalidSku(
                "SKU не может быть пустым".into(),
            ));
        }
        if s.len() > 50 {
            return Err(WarehouseDomainError::InvalidSku(format!(
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

/// Количество — неотрицательное число с произвольной точностью.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity(BigDecimal);

impl Quantity {
    /// Создать количество (>=0).
    ///
    /// # Errors
    ///
    /// `NegativeQuantity` — значение < 0.
    pub fn new(value: BigDecimal) -> Result<Self, WarehouseDomainError> {
        if value < BigDecimal::zero() {
            return Err(WarehouseDomainError::NegativeQuantity);
        }
        Ok(Self(value))
    }

    /// Нулевое количество.
    #[must_use]
    pub fn zero() -> Self {
        Self(BigDecimal::zero())
    }

    /// Количество отрицательное?
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0 < BigDecimal::zero()
    }

    /// Количество нулевое?
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Внутреннее значение.
    #[must_use]
    pub fn value(&self) -> &BigDecimal {
        &self.0
    }
}

impl Add for Quantity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Quantity {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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

    // ─── Quantity ───────────────────────────────────────────────────────

    #[test]
    fn quantity_positive_ok() {
        let q = Quantity::new(BigDecimal::from(100)).unwrap();
        assert_eq!(q.to_string(), "100");
    }

    #[test]
    fn quantity_zero_ok() {
        let q = Quantity::new(BigDecimal::from(0)).unwrap();
        assert!(q.is_zero());
    }

    #[test]
    fn quantity_negative_rejected() {
        assert!(Quantity::new(BigDecimal::from(-1)).is_err());
    }

    #[test]
    fn quantity_add() {
        let a = Quantity::new(BigDecimal::from(100)).unwrap();
        let b = Quantity::new(BigDecimal::from(50)).unwrap();
        let sum = a + b;
        assert_eq!(sum.value(), &BigDecimal::from(150));
    }

    #[test]
    fn quantity_sub() {
        let a = Quantity::new(BigDecimal::from(100)).unwrap();
        let b = Quantity::new(BigDecimal::from(30)).unwrap();
        let diff = a - b;
        assert_eq!(diff.value(), &BigDecimal::from(70));
    }

    #[test]
    fn quantity_decimal_precision() {
        let q = Quantity::new(BigDecimal::from_str("100.5000").unwrap()).unwrap();
        assert!(!q.is_zero());
    }

    #[test]
    fn quantity_serde_round_trip() {
        let q = Quantity::new(BigDecimal::from_str("123.4567").unwrap()).unwrap();
        let json = serde_json::to_string(&q).unwrap();
        let restored: Quantity = serde_json::from_str(&json).unwrap();
        assert_eq!(q, restored);
    }
}
