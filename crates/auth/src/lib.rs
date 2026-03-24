#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Auth — JWT issue/verify, RBAC, axum middleware.
//!
//! Первая реальная реализация порта `PermissionChecker` из runtime.
//! `JwtPermissionChecker` подставляется в `CommandPipeline` вместо `NoopPermissionChecker`.
//!
//! БД не нужна — роли в JWT claims, RBAC маппинг статический.

pub mod checker;
pub mod claims;
pub mod jwt;
pub mod middleware;
pub mod rbac;

// Re-exports для удобства: `use auth::JwtService`
pub use checker::JwtPermissionChecker;
pub use claims::{Claims, Role};
pub use jwt::JwtService;
pub use middleware::{AppErrorResponse, auth_middleware};
pub use rbac::{PermissionMap, default_erp_permissions};
