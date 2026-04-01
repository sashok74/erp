#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Auth — JWT issue/verify, BC-owned RBAC via `PermissionRegistry`, axum middleware.
//!
//! `JwtPermissionChecker` backed by `PermissionRegistry` (built from BC manifests)
//! plugs into `CommandPipeline` and `QueryPipeline` as `PermissionChecker`.

pub mod checker;
pub mod claims;
pub mod jwt;
pub mod middleware;
pub mod registry;

// Re-exports
pub use checker::JwtPermissionChecker;
pub use claims::Claims;
pub use jwt::JwtService;
pub use middleware::{AppErrorResponse, auth_middleware};
pub use registry::PermissionRegistry;
