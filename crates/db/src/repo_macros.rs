//! Макросы для генерации adapter-методов поверх clorinde.
//!
//! Четыре паттерна:
//! - [`repo_exec!`] — write-запрос без результата (`INSERT`/`UPDATE`/`DELETE`)
//! - [`repo_opt!`] — read-запрос с `Option<T>` результатом (`SELECT ... LIMIT 1`)
//! - [`repo_one!`] — read-запрос ровно с одной строкой (ошибка если 0 строк)
//! - [`repo_all!`] — read-запрос с `Vec<T>` результатом (`SELECT ... N rows`)
//!
//! Конверсии типов — через [`crate::conversions`] helpers (`tid()`, `uid()`, `parse_dec()`)
//! и [`crate::transport`] adapters ([`crate::transport::DecStr`] для decimal bind).

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

/// Генерирует one-метод репозитория: `.bind(...).opt().await?` → ровно одна строка.
///
/// При 0 строк возвращает `anyhow::bail!`. Используем `.opt()` вместо `.one()`
/// для различимой ошибки (clorinde `.one()` возвращает `tokio_postgres::Error`).
///
/// Обёртка `One<T>` нужна из-за ограничения Rust macro: после `ty` fragment
/// допускаются только `{`, `[`, `=>`, `,`, `>`, `=`, `:`, `;`, `|`, `as`, `where`.
///
/// ```ignore
/// repo_one! {
///     /// Получить товар по ID (обязательно существует).
///     pub async fn get_by_id(
///         client: &impl GenericClient,
///         tenant_id: TenantId,
///         id: Uuid,
///     ) -> One<ProductRow>
///     via clorinde_gen::queries::catalog::products::get_by_id;
///     bind = [&tid(tenant_id), &id];
///     map = |r| {
///         ProductRow { id: r.id, name: r.name }
///     };
/// }
/// ```
#[macro_export]
macro_rules! repo_one {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $client:ident : &impl GenericClient
            $(, $param:ident : $pty:ty)* $(,)?
        ) -> One<$ret:ty>
        via $query:path;
        bind = [$($bind:expr),* $(,)?];
        map = |$row:ident| $body:block $(;)?
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// Returns error if no row found or on SQL query failure.
        #[allow(clippy::too_many_arguments)]
        $vis async fn $name(
            $client: &impl ::clorinde_gen::client::GenericClient,
            $($param: $pty,)*
        ) -> ::anyhow::Result<$ret> {
            let row = $query()
                .bind($client, $($bind,)*)
                .opt()
                .await?;
            match row {
                Some($row) => Ok($body),
                None => ::anyhow::bail!("{}: expected exactly one row", stringify!($name)),
            }
        }
    };
}

/// Генерирует all-метод репозитория: `.bind(...).all().await?` → `Vec<T>`.
///
/// ```ignore
/// repo_all! {
///     /// Получить все товары tenant'а.
///     pub async fn list_all(
///         client: &impl GenericClient,
///         tenant_id: TenantId,
///     ) -> Vec<ProductRow>
///     via clorinde_gen::queries::catalog::products::list_all;
///     bind = [&tid(tenant_id)];
///     map = |r| {
///         ProductRow { id: r.id, name: r.name }
///     };
/// }
/// ```
#[macro_export]
macro_rules! repo_all {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $client:ident : &impl GenericClient
            $(, $param:ident : $pty:ty)* $(,)?
        ) -> Vec<$ret:ty>
        via $query:path;
        bind = [$($bind:expr),* $(,)?];
        map = |$row:ident| $body:block $(;)?
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// Returns error on SQL query failure or data conversion.
        #[allow(clippy::too_many_arguments)]
        $vis async fn $name(
            $client: &impl ::clorinde_gen::client::GenericClient,
            $($param: $pty,)*
        ) -> ::anyhow::Result<Vec<$ret>> {
            let rows = $query()
                .bind($client, $($bind,)*)
                .all()
                .await?;
            rows.into_iter()
                .map(|$row| Ok($body))
                .collect::<::anyhow::Result<Vec<_>>>()
        }
    };
}
