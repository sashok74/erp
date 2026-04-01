#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Kernel — Platform SDK для Bounded Contexts.
//!
//! Определяет контракты (трейты), идентификаторы, ошибки и формат событий.
//! Не содержит бизнес-примитивов (Value Objects) — они живут в каждом BC.
//!
//! Нулевые зависимости от инфраструктуры.

pub mod commands;
pub mod entity;
pub mod error_ext;
pub mod errors;
pub mod events;
pub mod queries;
pub mod security;
pub mod types;

// Re-exports для удобства: `use kernel::TenantId` вместо `use kernel::types::TenantId`
pub use commands::{Command, CommandEnvelope};
pub use entity::{AggregateRoot, Entity};
pub use error_ext::IntoInternal;
pub use errors::{AppError, DomainError};
pub use events::{CloudEvent, DomainEvent};
pub use queries::Query;
pub use types::{EntityId, RequestContext, TenantId, UserId};
