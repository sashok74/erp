//! `JwtPermissionChecker` — `PermissionChecker` implementation backed by `PermissionRegistry`.
//!
//! Extracts role strings from `RequestContext.roles`, checks against the registry.
//! Unknown roles are denied by default (no match in grants).

use std::sync::Arc;

use async_trait::async_trait;
use kernel::{AppError, RequestContext};
use runtime::ports::PermissionChecker;

use crate::registry::PermissionRegistry;

/// RBAC authorization via `PermissionRegistry`.
///
/// Plugs into `CommandPipeline` and `QueryPipeline` as `Arc<dyn PermissionChecker>`.
pub struct JwtPermissionChecker {
    registry: Arc<PermissionRegistry>,
}

impl JwtPermissionChecker {
    /// Create checker backed by a permission registry.
    #[must_use]
    pub fn new(registry: Arc<PermissionRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl PermissionChecker for JwtPermissionChecker {
    async fn check_permission(
        &self,
        ctx: &RequestContext,
        command_name: &str,
    ) -> Result<(), AppError> {
        if self.registry.is_allowed(&ctx.roles, command_name) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!(
                "no permission for command '{command_name}'"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::security::{PermissionDef, PermissionManifest, RoleDef, RoleGrant};
    use kernel::types::{TenantId, UserId};

    fn test_registry() -> Arc<PermissionRegistry> {
        let wh = PermissionManifest {
            bc_code: "warehouse".into(),
            roles: vec![
                RoleDef {
                    code: "warehouse_manager".into(),
                    display_name_ru: "Менеджер склада".into(),
                    display_name_en: None,
                    is_superadmin: false,
                    security_level: 2,
                },
                RoleDef {
                    code: "warehouse_operator".into(),
                    display_name_ru: "Кладовщик".into(),
                    display_name_en: None,
                    is_superadmin: false,
                    security_level: 1,
                },
            ],
            permissions: vec![
                PermissionDef {
                    command: "warehouse.receive_goods".into(),
                    display_name_ru: "Приёмка".into(),
                    display_name_en: None,
                    category: None,
                },
                PermissionDef {
                    command: "warehouse.get_balance".into(),
                    display_name_ru: "Остатки".into(),
                    display_name_en: None,
                    category: None,
                },
            ],
            grants: vec![
                RoleGrant {
                    role_code: "warehouse_manager".into(),
                    commands: vec!["warehouse.*".into()],
                },
                RoleGrant {
                    role_code: "warehouse_operator".into(),
                    commands: vec![
                        "warehouse.receive_goods".into(),
                        "warehouse.get_balance".into(),
                    ],
                },
                RoleGrant {
                    role_code: "viewer".into(),
                    commands: vec!["warehouse.get_balance".into()],
                },
            ],
        };
        Arc::new(
            PermissionRegistry::from_manifests_validated(vec![wh]).unwrap(),
        )
    }

    fn ctx_with_roles(roles: &[&str]) -> RequestContext {
        let mut ctx = RequestContext::new(TenantId::new(), UserId::new());
        ctx.roles = roles.iter().map(|s| (*s).to_string()).collect();
        ctx
    }

    #[tokio::test]
    async fn admin_allowed() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["admin"]);
        assert!(
            checker
                .check_permission(&ctx, "warehouse.receive_goods")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn operator_allowed_warehouse() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["warehouse_operator"]);
        assert!(
            checker
                .check_permission(&ctx, "warehouse.receive_goods")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn operator_denied_finance() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["warehouse_operator"]);
        let err = checker
            .check_permission(&ctx, "finance.post_journal")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[tokio::test]
    async fn unknown_role_denied() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["unknown_role"]);
        let err = checker
            .check_permission(&ctx, "warehouse.receive_goods")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[tokio::test]
    async fn viewer_query_allowed() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["viewer"]);
        assert!(
            checker
                .check_permission(&ctx, "warehouse.get_balance")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn viewer_command_denied() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["viewer"]);
        let err = checker
            .check_permission(&ctx, "warehouse.receive_goods")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[tokio::test]
    async fn multiple_roles_union() {
        let checker = JwtPermissionChecker::new(test_registry());
        let ctx = ctx_with_roles(&["viewer", "warehouse_operator"]);
        assert!(
            checker
                .check_permission(&ctx, "warehouse.receive_goods")
                .await
                .is_ok()
        );
    }
}
