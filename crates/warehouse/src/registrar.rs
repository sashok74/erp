//! Warehouse BC permission manifest.
//!
//! Declares warehouse-specific roles, actions (commands + queries),
//! and role-to-action grants including platform role `viewer`.

use kernel::security::{
    PermissionDef, PermissionManifest, PermissionRegistrar, RoleDef, RoleGrant,
};

/// Warehouse BC permission registrar.
pub struct WarehousePermissions;

impl PermissionRegistrar for WarehousePermissions {
    fn permission_manifest(&self) -> PermissionManifest {
        PermissionManifest {
            bc_code: "warehouse".into(),
            roles: vec![
                RoleDef {
                    code: "warehouse_manager".into(),
                    display_name_ru: "Менеджер склада".into(),
                    display_name_en: Some("Warehouse Manager".into()),
                    is_superadmin: false,
                    security_level: 2,
                },
                RoleDef {
                    code: "warehouse_operator".into(),
                    display_name_ru: "Кладовщик".into(),
                    display_name_en: Some("Warehouse Operator".into()),
                    is_superadmin: false,
                    security_level: 1,
                },
            ],
            permissions: vec![
                // ── Commands ──
                PermissionDef {
                    command: "warehouse.receive_goods".into(),
                    display_name_ru: "Приёмка товара".into(),
                    display_name_en: Some("Receive Goods".into()),
                    category: Some("Складские операции".into()),
                },
                // ── Queries ──
                PermissionDef {
                    command: "warehouse.get_balance".into(),
                    display_name_ru: "Просмотр остатков".into(),
                    display_name_en: Some("Get Balance".into()),
                    category: Some("Запросы".into()),
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
                // Platform role grants: viewer gets read-only queries
                RoleGrant {
                    role_code: "viewer".into(),
                    commands: vec!["warehouse.get_balance".into()],
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::security::platform_roles;

    #[test]
    fn bc_code_correct() {
        let m = WarehousePermissions.permission_manifest();
        assert_eq!(m.bc_code, "warehouse");
    }

    #[test]
    fn all_actions_start_with_namespace() {
        let m = WarehousePermissions.permission_manifest();
        for perm in &m.permissions {
            assert!(
                perm.command.starts_with("warehouse."),
                "action '{}' must start with 'warehouse.'",
                perm.command
            );
        }
    }

    #[test]
    fn viewer_grants_are_queries_only() {
        let m = WarehousePermissions.permission_manifest();
        let viewer_grant = m
            .grants
            .iter()
            .find(|g| g.role_code == platform_roles::VIEWER)
            .expect("viewer grant must exist");

        // All viewer grants should be query actions (get_*)
        for cmd in &viewer_grant.commands {
            assert!(
                cmd.contains(".get_"),
                "viewer grant '{}' should be a query action",
                cmd
            );
        }
    }

    #[test]
    fn all_handler_actions_present() {
        let m = WarehousePermissions.permission_manifest();
        let actions: Vec<&str> = m.permissions.iter().map(|p| p.command.as_str()).collect();

        // Must match actual command_name()/query_name() in handlers
        assert!(actions.contains(&"warehouse.receive_goods"));
        assert!(actions.contains(&"warehouse.get_balance"));
    }
}
