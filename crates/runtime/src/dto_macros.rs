//! Макросы для генерации `FromBody` / `FromQueryParams` boilerplate.
//!
//! Генерируют DTO-struct + trait impl для 1:1 маппинга полей.
//! При нестандартном маппинге (переименование, конвертация) — писать вручную.

/// Генерирует `#[derive(Deserialize)]` body struct + `impl FromBody` для команды.
///
/// Используется когда поля body совпадают с полями команды 1:1.
///
/// ```ignore
/// from_body! {
///     CreateProductBody -> CreateProductCommand {
///         sku: String,
///         name: String,
///         category: String,
///         unit: String,
///     }
/// }
/// ```
#[macro_export]
macro_rules! from_body {
    (
        $body:ident -> $cmd:ty {
            $($field:ident : $fty:ty),* $(,)?
        }
    ) => {
        #[derive(Debug, ::serde::Deserialize)]
        pub struct $body {
            $(pub $field: $fty,)*
        }

        impl $crate::dto::FromBody for $cmd {
            type Body = $body;

            fn from_body(body: Self::Body) -> Self {
                Self {
                    $($field: body.$field,)*
                }
            }
        }
    };
}

/// Генерирует `#[derive(Deserialize)]` params struct + `impl FromQueryParams` для запроса.
///
/// Используется когда поля params совпадают с полями запроса 1:1.
///
/// ```ignore
/// from_query_params! {
///     BalanceQueryParams -> GetBalanceQuery {
///         sku: String,
///     }
/// }
/// ```
#[macro_export]
macro_rules! from_query_params {
    (
        $params:ident -> $query:ty {
            $($field:ident : $fty:ty),* $(,)?
        }
    ) => {
        #[derive(Debug, ::serde::Deserialize)]
        pub struct $params {
            $(pub $field: $fty,)*
        }

        impl $crate::dto::FromQueryParams for $query {
            type Params = $params;

            fn from_params(params: Self::Params) -> Self {
                Self {
                    $($field: params.$field,)*
                }
            }
        }
    };
}
