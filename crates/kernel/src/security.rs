//! BC-Owned RBAC Registration API.
//!
//! BC declares its roles, actions (commands + queries), and role-to-action grants
//! via [`PermissionRegistrar`]. Platform collects manifests at startup and builds
//! a unified [`PermissionRegistry`](auth crate) for enforcement.
//!
//! Platform roles (`admin`, `viewer`) are defined here as constants.
//! BC must NOT redefine them.

/// Platform-level roles known at compile time.
/// BC must not register roles with these codes.
pub mod platform_roles {
    /// Superadmin: full access to all actions across all BCs.
    pub const ADMIN: &str = "admin";
    /// Read-only: no implicit access. Query grants must be explicit per BC.
    pub const VIEWER: &str = "viewer";

    /// All platform role codes for validation.
    pub const ALL: &[&str] = &[ADMIN, VIEWER];
}

/// BC registers its roles and permissions at startup.
pub trait PermissionRegistrar: Send + Sync {
    /// Return the RBAC manifest for this Bounded Context.
    fn permission_manifest(&self) -> PermissionManifest;
}

/// RBAC configuration manifest for one Bounded Context.
#[derive(Debug, Clone)]
pub struct PermissionManifest {
    /// BC identifier, e.g. `"warehouse"`.
    pub bc_code: String,

    /// Roles defined by this BC.
    pub roles: Vec<RoleDef>,

    /// Actions (commands + queries) defined by this BC.
    /// Convention: `"{bc_code}.{action_name}"`.
    pub permissions: Vec<PermissionDef>,

    /// Role-to-action grants.
    /// Actions may contain wildcard: `"warehouse.*"`.
    pub grants: Vec<RoleGrant>,
}

/// Role definition.
#[derive(Debug, Clone)]
pub struct RoleDef {
    /// Unique code: `"warehouse_operator"`.
    pub code: String,
    pub display_name_ru: String,
    pub display_name_en: Option<String>,
    /// Superadmin: full access to all actions in all BCs.
    pub is_superadmin: bool,
    /// Security level (0-3).
    pub security_level: u8,
}

/// Action (permission) definition.
#[derive(Debug, Clone)]
pub struct PermissionDef {
    /// Full action name: `"warehouse.receive_goods"` or `"warehouse.get_balance"`.
    pub command: String,
    pub display_name_ru: String,
    pub display_name_en: Option<String>,
    /// Category for Admin UI grouping.
    pub category: Option<String>,
}

/// Role-to-action grant mapping.
#[derive(Debug, Clone)]
pub struct RoleGrant {
    /// Role code (must be in manifest roles or a platform role).
    pub role_code: String,
    /// Actions (exact or wildcard `"warehouse.*"`).
    pub commands: Vec<String>,
}
