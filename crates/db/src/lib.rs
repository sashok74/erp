#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Data Access — `PostgreSQL` connection pool, RLS, `UnitOfWork`, миграции.
//!
//! Реализует `UnitOfWork` из `runtime::ports` поверх `PostgreSQL`-транзакций.
//! RLS обеспечивает tenant isolation на уровне БД.
//! `clorinde-gen` crate содержит типобезопасные SQL-запросы.

pub mod inbox;
pub mod migrate;
pub mod pool;
pub mod relay;
pub mod rls;
pub mod uow;

pub use inbox::InboxGuard;
pub use pool::PgPool;
pub use relay::OutboxRelay;
pub use rls::set_tenant_context;
pub use uow::{PgUnitOfWork, PgUnitOfWorkFactory};
