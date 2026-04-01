#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Catalog BC — справочник товаров.
//!
//! - Domain: `Product` (aggregate), `ProductCreated` (event), `Sku`/`ProductName` (value objects)
//! - Application: `CreateProductCommand` + handler, `GetProductQuery` + handler
//! - Infrastructure: `PgProductRepo`, axum routes, `CatalogModule`

pub mod application;
pub mod db;
pub mod domain;
pub mod infrastructure;
pub mod module;
pub mod registrar;
