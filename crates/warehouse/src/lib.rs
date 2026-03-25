#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Warehouse BC — MVP domain: складской учёт.
//!
//! - Domain: `InventoryItem` (aggregate), `GoodsReceived` (event), `Sku`/`Quantity` (value objects)
//! - Application: `ReceiveGoodsCommand` + handler, `GetBalanceQuery` + handler
//! - Infrastructure: `PgInventoryRepo`, axum routes, `WarehouseModule`

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod module;
