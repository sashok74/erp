//! Transport adapters: domain types → clorinde wire format.
//!
//! Wrappers that implement clorinde marker traits ([`clorinde_gen::StringSql`], etc.)
//! so that repo code can pass domain values directly to `.bind()` without manual
//! string conversion.
//!
//! # Write path
//!
//! ```ignore
//! use db::transport::DecStr;
//! bind = [&tid(tenant_id), &item_id, &DecStr(balance)];
//! ```
//!
//! # Read path
//!
//! Use [`crate::conversions::parse_dec`] in `map` closures — clorinde returns
//! `String`/`&str` for `::TEXT` casts, which must be parsed back.
//!
//! # Extending
//!
//! To add a new transport adapter (e.g. for `Money`):
//! 1. Define `MoneyStr<'a>(&'a Money)` here
//! 2. Implement [`postgres_types::ToSql`] (serialize via `Display` or custom logic)
//! 3. Implement [`clorinde_gen::StringSql`] (marker trait, no methods)
//! 4. Use `MoneyStr(&amount)` in bind lists

use bigdecimal::BigDecimal;
use postgres_types::{IsNull, ToSql, Type, private::BytesMut, to_sql_checked};
use std::error::Error;

/// Zero-copy wrapper: `&BigDecimal` → TEXT for clorinde `StringSql` params.
///
/// Implements [`ToSql`] by serializing `BigDecimal` as a TEXT string,
/// matching the `::TEXT::NUMERIC` cast pattern in SQL queries.
///
/// # Example
///
/// ```ignore
/// use db::transport::DecStr;
///
/// repo_exec! {
///     pub async fn upsert_balance(
///         client: &impl GenericClient,
///         tenant_id: TenantId,
///         balance: &BigDecimal,
///     ) via clorinde_gen::queries::warehouse::balances::upsert_balance;
///     bind = [&tid(tenant_id), &DecStr(balance)];
/// }
/// ```
#[derive(Clone, Copy)]
pub struct DecStr<'a>(pub &'a BigDecimal);

impl std::fmt::Debug for DecStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecStr({})", self.0)
    }
}

impl ToSql for DecStr<'_> {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let s = self.0.to_string();
        s.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        <String as ToSql>::accepts(ty)
    }

    to_sql_checked!();
}

impl clorinde_gen::StringSql for DecStr<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn debug_shows_value() {
        let d = BigDecimal::from_str("42.5000").unwrap();
        assert_eq!(format!("{:?}", DecStr(&d)), "DecStr(42.5000)");
    }

    #[test]
    fn accepts_text_and_varchar() {
        assert!(DecStr::accepts(&Type::TEXT));
        assert!(DecStr::accepts(&Type::VARCHAR));
    }

    #[test]
    fn rejects_int4() {
        assert!(!DecStr::accepts(&Type::INT4));
    }

    #[test]
    fn is_sync_and_send() {
        fn assert_sync<T: Sync>() {}
        fn assert_send<T: Send>() {}
        assert_sync::<DecStr<'_>>();
        assert_send::<DecStr<'_>>();
    }

    #[test]
    fn preserves_scale() {
        // 1.0000 must keep trailing zeros — important for NUMERIC(18,4)
        let d = BigDecimal::from_str("1.0000").unwrap();
        let wrapper = DecStr(&d);
        assert_eq!(format!("{}", wrapper.0), "1.0000");
    }

    #[test]
    fn round_trip_with_parse_dec() {
        let original = BigDecimal::from_str("99999999999999.9999").unwrap();
        let s = original.to_string();
        let parsed = crate::conversions::parse_dec(&s).unwrap();
        assert_eq!(original, parsed);
    }
}
