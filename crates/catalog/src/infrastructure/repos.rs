//! Реализации persistence для `ProductRepo` через `clorinde_gen`.
//!
//! Split impl: struct определён в `application::repos`,
//! методы реализованы здесь через clorinde SQL.

use db::{repo_exec, repo_opt};
use uuid::Uuid;

use crate::application::repos::{ProductRepo, ProductRow};

impl ProductRepo<'_> {
    repo_opt! {
        /// Найти товар по SKU.
        pub async fn find_by_sku(
            sku: &str,
        ) -> Option<ProductRow>
        via clorinde_gen::queries::catalog::products::find_by_sku;
        bind = [&sku];
        map = |r| {
            ProductRow {
                id: r.id,
                sku: r.sku,
                name: r.name,
                category: r.category,
                unit: r.unit,
            }
        };
    }

    repo_exec! {
        /// Создать товар в каталоге.
        pub async fn create_product(
            id: Uuid,
            sku: &str,
            name: &str,
            category: &str,
            unit: &str,
        ) via clorinde_gen::queries::catalog::products::create_product;
        bind = [&id, &sku, &name, &category, &unit];
    }
}
