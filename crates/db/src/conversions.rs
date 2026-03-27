//! Конверсии типов между domain и clorinde (`PostgreSQL`).
//!
//! Централизованные helpers для повторяющихся маппингов:
//! - Newtype ID → `Uuid`
//! - `BigDecimal` ↔ `String` (`PostgreSQL` TEXT)
//!
//! Используются напрямую в repo-методах и в макросах [`repo_exec!`], [`repo_opt!`].

use bigdecimal::BigDecimal;
use kernel::types::{EntityId, TenantId, UserId};
use std::str::FromStr;
use uuid::Uuid;

/// `TenantId` → `Uuid` для `.bind()` в clorinde.
#[inline]
#[must_use]
pub fn tid(t: TenantId) -> Uuid {
    *t.as_uuid()
}

/// `UserId` → `Uuid` для `.bind()` в clorinde.
#[inline]
#[must_use]
pub fn uid(u: UserId) -> Uuid {
    *u.as_uuid()
}

/// `EntityId` → `Uuid` для `.bind()` в clorinde.
#[inline]
#[must_use]
pub fn eid(e: EntityId) -> Uuid {
    *e.as_uuid()
}

/// `&BigDecimal` → `String` для `.bind()` в clorinde (`PostgreSQL` TEXT column).
///
/// **Prefer [`crate::transport::DecStr`]** — zero-copy wrapper that implements
/// `StringSql` directly, eliminating manual string conversion in repo code.
#[inline]
#[must_use]
pub fn dec_str(d: &BigDecimal) -> String {
    d.to_string()
}

/// `&str` (`PostgreSQL` TEXT) → `BigDecimal`.
///
/// # Errors
///
/// Returns error if string cannot be parsed as decimal.
#[inline]
pub fn parse_dec(s: &str) -> anyhow::Result<BigDecimal> {
    BigDecimal::from_str(s).map_err(|e| anyhow::anyhow!("parse decimal: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tid_extracts_uuid() {
        let uuid = Uuid::now_v7();
        let tenant = TenantId::from_uuid(uuid);
        assert_eq!(tid(tenant), uuid);
    }

    #[test]
    fn uid_extracts_uuid() {
        let uuid = Uuid::now_v7();
        let user = UserId::from_uuid(uuid);
        assert_eq!(uid(user), uuid);
    }

    #[test]
    fn dec_str_round_trip() {
        let d = BigDecimal::from_str("123.456").unwrap();
        let s = dec_str(&d);
        let d2 = parse_dec(&s).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn parse_dec_invalid_returns_error() {
        assert!(parse_dec("not_a_number").is_err());
    }
}
