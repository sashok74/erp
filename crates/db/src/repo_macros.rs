//! Макросы для генерации adapter-методов поверх clorinde.
//!
//! Два паттерна:
//! - [`repo_exec!`] — write-запрос без результата (`INSERT`/`UPDATE`/`DELETE`)
//! - [`repo_opt!`] — read-запрос с `Option<T>` результатом (`SELECT ... LIMIT 1`)
//!
//! Конверсии типов — через [`crate::conversions`] helpers:
//! `tid()`, `uid()`, `dec_str()`, `parse_dec()`.

/// Генерирует exec-метод репозитория: `.bind(...).await?; Ok(())`.
///
/// ```ignore
/// repo_exec! {
///     /// Создать запись inventory_item.
///     pub async fn create_item(
///         client: &impl GenericClient,
///         tenant_id: TenantId,
///         item_id: Uuid,
///         sku: &str,
///     ) via clorinde_gen::queries::warehouse::inventory::create_item;
///     bind = [&tid(tenant_id), &item_id, &sku];
/// }
/// ```
#[macro_export]
macro_rules! repo_exec {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $client:ident : &impl GenericClient
            $(, $param:ident : $pty:ty)* $(,)?
        ) via $query:path;
        bind = [$($bind:expr),* $(,)?];
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// Returns error on SQL query failure.
        #[allow(clippy::too_many_arguments)]
        $vis async fn $name(
            $client: &impl ::clorinde_gen::client::GenericClient,
            $($param: $pty,)*
        ) -> ::anyhow::Result<()> {
            $query()
                .bind($client, $($bind,)*)
                .await?;
            Ok(())
        }
    };
}

/// Генерирует opt-метод репозитория: `.bind(...).opt().await?` + row mapping.
///
/// Блок `map` получает clorinde row struct и возвращает domain DTO.
/// Внутри блока можно использовать `?` для fallible-конверсий (например `parse_dec`).
///
/// ```ignore
/// repo_opt! {
///     /// Получить баланс по SKU.
///     pub async fn get_balance(
///         client: &impl GenericClient,
///         tenant_id: TenantId,
///         sku: &str,
///     ) -> Option<BalanceRow>
///     via clorinde_gen::queries::warehouse::balances::get_balance;
///     bind = [&tid(tenant_id), &sku];
///     map = |r| {
///         BalanceRow {
///             item_id: r.item_id,
///             sku: r.sku,
///             balance: parse_dec(&r.balance)?,
///         }
///     };
/// }
/// ```
#[macro_export]
macro_rules! repo_opt {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $client:ident : &impl GenericClient
            $(, $param:ident : $pty:ty)* $(,)?
        ) -> Option<$ret:ty>
        via $query:path;
        bind = [$($bind:expr),* $(,)?];
        map = |$row:ident| $body:block $(;)?
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// Returns error on SQL query failure or data conversion.
        $vis async fn $name(
            $client: &impl ::clorinde_gen::client::GenericClient,
            $($param: $pty,)*
        ) -> ::anyhow::Result<Option<$ret>> {
            let row = $query()
                .bind($client, $($bind,)*)
                .opt()
                .await?;
            match row {
                Some($row) => Ok(Some($body)),
                None => Ok(None),
            }
        }
    };
}
