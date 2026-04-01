//! Макросы для генерации `&self`-методов репозитория поверх clorinde.
//!
//! Четыре паттерна:
//! - [`repo_exec!`] — write-запрос без результата (`INSERT`/`UPDATE`/`DELETE`)
//! - [`repo_opt!`] — read-запрос с `Option<T>` результатом (`SELECT ... LIMIT 1`)
//! - [`repo_one!`] — read-запрос ровно с одной строкой (ошибка если 0 строк)
//! - [`repo_all!`] — read-запрос с `Vec<T>` результатом (`SELECT ... N rows`)
//!
//! Все макросы:
//! - генерируют `&self`-методы (repo держит `client` + `tenant_id`)
//! - автоматически prepend `self.client` и `tid(self.tenant_id)` в bind
//! - оборачивают ошибки в `AppError::Internal` с контекстом из имени функции
//!
//! Конверсии типов — через [`crate::conversions`] helpers (`tid()`, `uid()`, `parse_dec()`)
//! и [`crate::transport`] adapters ([`crate::transport::DecStr`] для decimal bind).

/// Генерирует `&self` exec-метод репозитория: `.bind(...).await?; Ok(())`.
///
/// `self.client` и `tid(self.tenant_id)` автоматически добавляются в bind.
///
/// ```ignore
/// repo_exec! {
///     /// Создать запись inventory_item.
///     pub async fn create_item(
///         item_id: Uuid,
///         sku: &str,
///     ) via clorinde_gen::queries::warehouse::inventory::create_item;
///     bind = [&item_id, &sku];
/// }
/// ```
#[macro_export]
macro_rules! repo_exec {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $($param:ident : $pty:ty),* $(,)?
        ) via $query:path;
        bind = [$($bind:expr),* $(,)?];
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// `AppError::Internal` on SQL query failure.
        #[allow(clippy::too_many_arguments)]
        $vis async fn $name(
            &self,
            $($param: $pty,)*
        ) -> ::core::result::Result<(), ::kernel::AppError> {
            use ::kernel::IntoInternal;
            $query()
                .bind(self.client, &::db::conversions::tid(self.tenant_id), $($bind,)*)
                .await
                .internal(stringify!($name))?;
            Ok(())
        }
    };
}

/// Генерирует `&self` opt-метод репозитория: `.bind(...).opt().await?` + row mapping.
///
/// ```ignore
/// repo_opt! {
///     /// Получить баланс по SKU.
///     pub async fn get_balance(
///         sku: &str,
///     ) -> Option<BalanceRow>
///     via clorinde_gen::queries::warehouse::balances::get_balance;
///     bind = [&sku];
///     map = |r| {
///         BalanceRow { item_id: r.item_id, sku: r.sku, balance: parse_dec(&r.balance)? }
///     };
/// }
/// ```
#[macro_export]
macro_rules! repo_opt {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $($param:ident : $pty:ty),* $(,)?
        ) -> Option<$ret:ty>
        via $query:path;
        bind = [$($bind:expr),* $(,)?];
        map = |$row:ident| $body:block $(;)?
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// `AppError::Internal` on SQL query failure or data conversion.
        $vis async fn $name(
            &self,
            $($param: $pty,)*
        ) -> ::core::result::Result<Option<$ret>, ::kernel::AppError> {
            use ::kernel::IntoInternal;
            let row = $query()
                .bind(self.client, &::db::conversions::tid(self.tenant_id), $($bind,)*)
                .opt()
                .await
                .internal(stringify!($name))?;
            match row {
                Some($row) => {
                    let mapped = (|| -> ::core::result::Result<$ret, ::kernel::AppError> {
                        Ok($body)
                    })();
                    mapped.map(Some)
                }
                None => Ok(None),
            }
        }
    };
}

/// Генерирует `&self` one-метод репозитория: ровно одна строка, ошибка если 0.
///
/// Обёртка `One<T>` нужна из-за ограничения Rust macro: после `ty` fragment
/// допускаются только `{`, `[`, `=>`, `,`, `>`, `=`, `:`, `;`, `|`, `as`, `where`.
///
/// ```ignore
/// repo_one! {
///     pub async fn get_by_id(id: Uuid) -> One<ProductRow>
///     via clorinde_gen::queries::catalog::products::get_by_id;
///     bind = [&id];
///     map = |r| { ProductRow { id: r.id, name: r.name } };
/// }
/// ```
#[macro_export]
macro_rules! repo_one {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $($param:ident : $pty:ty),* $(,)?
        ) -> One<$ret:ty>
        via $query:path;
        bind = [$($bind:expr),* $(,)?];
        map = |$row:ident| $body:block $(;)?
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// `AppError::Internal` if no row found or on SQL query failure.
        #[allow(clippy::too_many_arguments)]
        $vis async fn $name(
            &self,
            $($param: $pty,)*
        ) -> ::core::result::Result<$ret, ::kernel::AppError> {
            use ::kernel::IntoInternal;
            let row = $query()
                .bind(self.client, &::db::conversions::tid(self.tenant_id), $($bind,)*)
                .opt()
                .await
                .internal(stringify!($name))?;
            match row {
                Some($row) => Ok($body),
                None => Err(::kernel::AppError::Domain(
                    ::kernel::DomainError::NotFound(stringify!($name).to_string())
                )),
            }
        }
    };
}

/// Генерирует `&self` all-метод репозитория: `Vec<T>`.
///
/// ```ignore
/// repo_all! {
///     pub async fn list_all() -> Vec<ProductRow>
///     via clorinde_gen::queries::catalog::products::list_all;
///     bind = [];
///     map = |r| { ProductRow { id: r.id, name: r.name } };
/// }
/// ```
#[macro_export]
macro_rules! repo_all {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $($param:ident : $pty:ty),* $(,)?
        ) -> Vec<$ret:ty>
        via $query:path;
        bind = [$($bind:expr),* $(,)?];
        map = |$row:ident| $body:block $(;)?
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// `AppError::Internal` on SQL query failure or data conversion.
        #[allow(clippy::too_many_arguments)]
        $vis async fn $name(
            &self,
            $($param: $pty,)*
        ) -> ::core::result::Result<Vec<$ret>, ::kernel::AppError> {
            use ::kernel::IntoInternal;
            let rows = $query()
                .bind(self.client, &::db::conversions::tid(self.tenant_id), $($bind,)*)
                .all()
                .await
                .internal(stringify!($name))?;
            rows.into_iter()
                .map(|$row| Ok($body))
                .collect::<::core::result::Result<Vec<_>, ::kernel::AppError>>()
        }
    };
}
