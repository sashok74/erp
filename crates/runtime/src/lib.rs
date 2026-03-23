#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Runtime — конвейер обработки команд и контракты для Bounded Contexts.
//!
//! Центральные компоненты:
//! - [`CommandPipeline`](pipeline::CommandPipeline) — полный конвейер: auth → hooks → TX → handler → commit → audit
//! - [`CommandHandler`](command_handler::CommandHandler) — trait для обработчиков команд (write)
//! - [`QueryHandler`](query_handler::QueryHandler) — trait для обработчиков запросов (read)
//! - [`BoundedContextModule`](module::BoundedContextModule) — регистрация BC в системе
//!
//! Pipeline зависит от trait-портов ([`ports`]), не от конкретной инфраструктуры.
//! Stub-реализации ([`stubs`]) позволяют тестировать без БД.

pub mod command_handler;
pub mod module;
pub mod pipeline;
pub mod ports;
pub mod query_handler;
pub mod stubs;

// Re-exports для удобства: `use runtime::CommandPipeline`
pub use command_handler::CommandHandler;
pub use module::BoundedContextModule;
pub use pipeline::CommandPipeline;
pub use ports::{AuditLog, ExtensionHooks, PermissionChecker, UnitOfWork, UnitOfWorkFactory};
pub use query_handler::QueryHandler;
