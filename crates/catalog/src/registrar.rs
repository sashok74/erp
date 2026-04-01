//! Catalog BC permission manifest.
//!
//! Declares catalog-specific roles, actions (commands + queries),
//! and role-to-action grants including platform role `viewer`.

use kernel::security::{
    PermissionDef, PermissionManifest, PermissionRegistrar, RoleDef, RoleGrant,
};

/// Catalog BC permission registrar.
pub struct CatalogPermissions;

impl PermissionRegistrar for CatalogPermissions {
    fn permission_manifest(&self) -> PermissionManifest {
        PermissionManifest {
            bc_code: "catalog".into(),
            roles: vec![RoleDef {
                code: "catalog_manager".into(),
                display_name_ru: "Менеджер каталога".into(),
                display_name_en: Some("Catalog Manager".into()),
                is_superadmin: false,
                security_level: 1,
            }],
            permissions: vec![
                // ── Commands ──
                PermissionDef {
                    command: "catalog.create_product".into(),
                    display_name_ru: "Создание товара".into(),
                    display_name_en: Some("Create Product".into()),
                    category: Some("Каталог".into()),
                },
                // ── Queries ──
                PermissionDef {
                    command: "catalog.get_product".into(),
                    display_name_ru: "Просмотр товара".into(),
                    display_name_en: Some("Get Product".into()),
                    category: Some("Запросы".into()),
                },
            ],
            grants: vec![
                RoleGrant {
                    role_code: "catalog_manager".into(),
                    commands: vec!["catalog.*".into()],
                },
                // Platform role grants: viewer gets read-only queries
                RoleGrant {
                    role_code: "viewer".into(),
                    commands: vec!["catalog.get_product".into()],
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
        let m = CatalogPermissions.permission_manifest();
        assert_eq!(m.bc_code, "catalog");
    }

    #[test]
    fn all_actions_start_with_namespace() {
        let m = CatalogPermissions.permission_manifest();
        for perm in &m.permissions {
            assert!(
                perm.command.starts_with("catalog."),
                "action '{}' must start with 'catalog.'",
                perm.command
            );
        }
    }

    #[test]
    fn viewer_grants_are_queries_only() {
        let m = CatalogPermissions.permission_manifest();
        let viewer_grant = m
            .grants
            .iter()
            .find(|g| g.role_code == platform_roles::VIEWER)
            .expect("viewer grant must exist");

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
        let m = CatalogPermissions.permission_manifest();
        let actions: Vec<&str> = m.permissions.iter().map(|p| p.command.as_str()).collect();

        assert!(actions.contains(&"catalog.create_product"));
        assert!(actions.contains(&"catalog.get_product"));
    }
}
