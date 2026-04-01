//! `PermissionRegistry` — unified role/action registry built from BC manifests.
//!
//! Replaces the old `PermissionMap` + `default_erp_permissions()`.
//! Immutable after construction. Thread-safe via `Arc<PermissionRegistry>`.

use std::collections::{HashMap, HashSet};

use kernel::security::{PermissionDef, PermissionManifest, RoleDef, platform_roles};

/// Registry of all roles and actions, assembled from BC manifests.
#[derive(Debug)]
pub struct PermissionRegistry {
    /// `role_code` -> granted action patterns (exact or wildcard).
    grants: HashMap<String, Vec<String>>,
    /// `role_codes` with `is_superadmin` = true (+ platform `admin`).
    superadmin_roles: HashSet<String>,
    /// All registered roles (BC-owned only, not platform).
    known_roles: HashMap<String, RoleDef>,
    /// All registered actions.
    known_permissions: HashMap<String, PermissionDef>,
}

impl PermissionRegistry {
    /// Build registry from BC manifests (no validation).
    #[must_use]
    pub fn from_manifests(manifests: Vec<PermissionManifest>) -> Self {
        let mut grants: HashMap<String, Vec<String>> = HashMap::new();
        let mut superadmin_roles = HashSet::new();
        let mut known_roles = HashMap::new();
        let mut known_permissions = HashMap::new();

        // Platform admin is always superadmin
        superadmin_roles.insert(platform_roles::ADMIN.to_string());

        for manifest in manifests {
            for role in &manifest.roles {
                if role.is_superadmin {
                    superadmin_roles.insert(role.code.clone());
                }
                known_roles.insert(role.code.clone(), role.clone());
            }

            for perm in &manifest.permissions {
                known_permissions.insert(perm.command.clone(), perm.clone());
            }

            for grant in &manifest.grants {
                grants
                    .entry(grant.role_code.clone())
                    .or_default()
                    .extend(grant.commands.clone());
            }
        }

        Self {
            grants,
            superadmin_roles,
            known_roles,
            known_permissions,
        }
    }

    /// Build registry with full validation. Recommended for production startup.
    ///
    /// # Errors
    ///
    /// Returns list of validation errors if manifests are invalid.
    pub fn from_manifests_validated(
        manifests: Vec<PermissionManifest>,
    ) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut seen_roles: HashMap<String, String> = HashMap::new();
        let mut seen_perms: HashMap<String, String> = HashMap::new();

        for manifest in &manifests {
            // Namespace enforcement: permissions must start with bc_code
            for perm in &manifest.permissions {
                let expected = format!("{}.", manifest.bc_code);
                if !perm.command.starts_with(&expected) {
                    errors.push(format!(
                        "BC '{}' registers foreign permission '{}' (must start with '{expected}')",
                        manifest.bc_code, perm.command
                    ));
                }
                if let Some(prev_bc) = seen_perms.get(&perm.command) {
                    errors.push(format!(
                        "Duplicate permission '{}': registered by '{}' and '{}'",
                        perm.command, prev_bc, manifest.bc_code
                    ));
                } else {
                    seen_perms.insert(perm.command.clone(), manifest.bc_code.clone());
                }
            }

            // Duplicate roles + platform role collision
            for role in &manifest.roles {
                if let Some(prev_bc) = seen_roles.get(&role.code) {
                    errors.push(format!(
                        "Duplicate role '{}': registered by '{}' and '{}'",
                        role.code, prev_bc, manifest.bc_code
                    ));
                } else {
                    seen_roles.insert(role.code.clone(), manifest.bc_code.clone());
                }
                if platform_roles::ALL.contains(&role.code.as_str()) {
                    errors.push(format!(
                        "BC '{}' defines role '{}' which conflicts with platform role",
                        manifest.bc_code, role.code
                    ));
                }
            }

            // Grants: namespace enforcement for platform role grants
            for grant in &manifest.grants {
                let is_platform = platform_roles::ALL.contains(&grant.role_code.as_str());
                let is_own = manifest.roles.iter().any(|r| r.code == grant.role_code);
                if !is_platform && !is_own {
                    // Could be a role from another BC — checked later in validate()
                }
                // BC can only grant actions in its own namespace
                for cmd in &grant.commands {
                    let action_ns = cmd.split('.').next().unwrap_or("");
                    if action_ns != manifest.bc_code && !cmd.ends_with('*') {
                        errors.push(format!(
                            "BC '{}' grants action '{}' outside its namespace",
                            manifest.bc_code, cmd
                        ));
                    }
                    // For wildcards, check prefix
                    if cmd.ends_with(".*") {
                        let wc_ns = cmd.strip_suffix(".*").unwrap_or("");
                        if wc_ns != manifest.bc_code {
                            errors.push(format!(
                                "BC '{}' grants wildcard '{}' outside its namespace",
                                manifest.bc_code, cmd
                            ));
                        }
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let registry = Self::from_manifests(manifests);
        registry.validate()?;
        Ok(registry)
    }

    /// Validate internal consistency of the assembled registry.
    ///
    /// # Errors
    ///
    /// Returns list of validation errors.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let platform: HashSet<&str> = platform_roles::ALL.iter().copied().collect();

        // 1. Grants reference unknown roles
        for role_code in self.grants.keys() {
            if !self.known_roles.contains_key(role_code)
                && !self.superadmin_roles.contains(role_code)
                && !platform.contains(role_code.as_str())
            {
                errors.push(format!("Grant references unknown role: '{role_code}'"));
            }
        }

        // 2. Grants reference unknown actions (non-wildcard only)
        for (role_code, commands) in &self.grants {
            for cmd in commands {
                if !cmd.ends_with('*') && !self.known_permissions.contains_key(cmd) {
                    errors.push(format!("Role '{role_code}' grants unknown action: '{cmd}'"));
                }
            }
        }

        // 3. Wildcard format: must end with ".*"
        for (role_code, commands) in &self.grants {
            for cmd in commands {
                if cmd.contains('*') && !cmd.ends_with(".*") {
                    errors.push(format!(
                        "Role '{role_code}': invalid wildcard '{cmd}' (must end with '.*')"
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check if an action is allowed for the given set of roles.
    #[must_use]
    pub fn is_allowed(&self, roles: &[String], action_name: &str) -> bool {
        for role in roles {
            if self.superadmin_roles.contains(role) {
                return true;
            }
            if let Some(commands) = self.grants.get(role) {
                for pattern in commands {
                    if let Some(prefix) = pattern.strip_suffix('*') {
                        if action_name.starts_with(prefix) {
                            return true;
                        }
                    } else if pattern == action_name {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// All registered BC-owned roles (for Admin UI).
    #[must_use]
    pub fn roles(&self) -> &HashMap<String, RoleDef> {
        &self.known_roles
    }

    /// All registered actions (for Admin UI).
    #[must_use]
    pub fn permissions(&self) -> &HashMap<String, PermissionDef> {
        &self.known_permissions
    }

    /// Check if a role code is known (platform or BC-owned).
    #[must_use]
    pub fn is_known_role(&self, role_code: &str) -> bool {
        self.known_roles.contains_key(role_code) || platform_roles::ALL.contains(&role_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::security::{PermissionDef, PermissionManifest, RoleDef, RoleGrant};

    fn wh_manifest() -> PermissionManifest {
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
                PermissionDef {
                    command: "warehouse.receive_goods".into(),
                    display_name_ru: "Приёмка товара".into(),
                    display_name_en: None,
                    category: None,
                },
                PermissionDef {
                    command: "warehouse.ship_goods".into(),
                    display_name_ru: "Отгрузка товара".into(),
                    display_name_en: None,
                    category: None,
                },
                PermissionDef {
                    command: "warehouse.get_balance".into(),
                    display_name_ru: "Просмотр остатков".into(),
                    display_name_en: None,
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
                        "warehouse.ship_goods".into(),
                        "warehouse.get_balance".into(),
                    ],
                },
                RoleGrant {
                    role_code: "viewer".into(),
                    commands: vec!["warehouse.get_balance".into()],
                },
            ],
        }
    }

    fn cat_manifest() -> PermissionManifest {
        PermissionManifest {
            bc_code: "catalog".into(),
            roles: vec![RoleDef {
                code: "catalog_manager".into(),
                display_name_ru: "Менеджер каталога".into(),
                display_name_en: None,
                is_superadmin: false,
                security_level: 1,
            }],
            permissions: vec![
                PermissionDef {
                    command: "catalog.create_product".into(),
                    display_name_ru: "Создание товара".into(),
                    display_name_en: None,
                    category: None,
                },
                PermissionDef {
                    command: "catalog.get_product".into(),
                    display_name_ru: "Просмотр товара".into(),
                    display_name_en: None,
                    category: Some("Запросы".into()),
                },
            ],
            grants: vec![
                RoleGrant {
                    role_code: "catalog_manager".into(),
                    commands: vec!["catalog.*".into()],
                },
                RoleGrant {
                    role_code: "viewer".into(),
                    commands: vec!["catalog.get_product".into()],
                },
            ],
        }
    }

    fn s(val: &str) -> String {
        val.to_string()
    }

    // ─── Basic registry tests ────────────────────────────────────────

    #[test]
    fn single_manifest_grants_built() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest()]);
        assert!(reg.is_allowed(&[s("warehouse_operator")], "warehouse.receive_goods"));
        assert!(!reg.is_allowed(&[s("warehouse_operator")], "warehouse.adjust_inventory"));
    }

    #[test]
    fn two_manifests_merged() {
        let reg = PermissionRegistry::from_manifests_validated(vec![wh_manifest(), cat_manifest()])
            .unwrap();
        assert_eq!(reg.roles().len(), 3); // wh_manager, wh_operator, cat_manager
        assert!(reg.permissions().contains_key("warehouse.receive_goods"));
        assert!(reg.permissions().contains_key("catalog.create_product"));
    }

    #[test]
    fn admin_any_action_allowed() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest()]);
        assert!(reg.is_allowed(&[s("admin")], "warehouse.receive_goods"));
        assert!(reg.is_allowed(&[s("admin")], "anything.at_all"));
    }

    #[test]
    fn viewer_query_allowed_via_bc_grant() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest(), cat_manifest()]);
        assert!(reg.is_allowed(&[s("viewer")], "warehouse.get_balance"));
        assert!(reg.is_allowed(&[s("viewer")], "catalog.get_product"));
    }

    #[test]
    fn viewer_command_denied() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest(), cat_manifest()]);
        assert!(!reg.is_allowed(&[s("viewer")], "warehouse.receive_goods"));
        assert!(!reg.is_allowed(&[s("viewer")], "catalog.create_product"));
    }

    #[test]
    fn bc_role_own_namespace_allowed() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest(), cat_manifest()]);
        assert!(reg.is_allowed(&[s("warehouse_operator")], "warehouse.receive_goods"));
    }

    #[test]
    fn bc_role_foreign_namespace_denied() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest(), cat_manifest()]);
        assert!(!reg.is_allowed(&[s("warehouse_operator")], "catalog.create_product"));
    }

    #[test]
    fn multiple_roles_union() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest(), cat_manifest()]);
        let roles = vec![s("warehouse_operator"), s("catalog_manager")];
        assert!(reg.is_allowed(&roles, "warehouse.receive_goods"));
        assert!(reg.is_allowed(&roles, "catalog.create_product"));
    }

    #[test]
    fn unknown_role_denied() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest()]);
        assert!(!reg.is_allowed(&[s("nonexistent_role")], "warehouse.receive_goods"));
    }

    #[test]
    fn empty_roles_denied() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest()]);
        assert!(!reg.is_allowed(&[], "warehouse.receive_goods"));
    }

    #[test]
    fn wildcard_valid_match() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest()]);
        assert!(reg.is_allowed(&[s("warehouse_manager")], "warehouse.receive_goods"));
        assert!(reg.is_allowed(&[s("warehouse_manager")], "warehouse.ship_goods"));
        assert!(reg.is_allowed(&[s("warehouse_manager")], "warehouse.any_future_action"));
    }

    #[test]
    fn wildcard_foreign_namespace_no_match() {
        let reg = PermissionRegistry::from_manifests(vec![wh_manifest()]);
        assert!(!reg.is_allowed(&[s("warehouse_manager")], "catalog.create_product"));
    }

    // ─── Validation tests ────────────────────────────────────────────

    #[test]
    fn validate_unknown_role_in_grant() {
        let mut m = wh_manifest();
        m.grants.push(RoleGrant {
            role_code: "ghost_role".into(),
            commands: vec!["warehouse.receive_goods".into()],
        });
        let reg = PermissionRegistry::from_manifests(vec![m]);
        let errs = reg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("ghost_role")));
    }

    #[test]
    fn validate_unknown_action_in_grant() {
        let mut m = wh_manifest();
        m.grants.push(RoleGrant {
            role_code: "warehouse_operator".into(),
            commands: vec!["warehouse.nonexistent".into()],
        });
        let reg = PermissionRegistry::from_manifests(vec![m]);
        let errs = reg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("warehouse.nonexistent")));
    }

    #[test]
    fn validate_invalid_wildcard() {
        let mut m = wh_manifest();
        m.grants.push(RoleGrant {
            role_code: "warehouse_manager".into(),
            commands: vec!["warehouse*".into()],
        });
        let reg = PermissionRegistry::from_manifests(vec![m]);
        let errs = reg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("invalid wildcard")));
    }

    #[test]
    fn validate_duplicate_role_code() {
        let m1 = wh_manifest();
        let mut m2 = cat_manifest();
        m2.roles.push(RoleDef {
            code: "warehouse_manager".into(),
            display_name_ru: "Дубликат".into(),
            display_name_en: None,
            is_superadmin: false,
            security_level: 0,
        });
        let errs = PermissionRegistry::from_manifests_validated(vec![m1, m2]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Duplicate role")));
    }

    #[test]
    fn validate_duplicate_action_code() {
        let m1 = wh_manifest();
        let m2 = PermissionManifest {
            bc_code: "warehouse".into(),
            roles: vec![],
            permissions: vec![PermissionDef {
                command: "warehouse.receive_goods".into(),
                display_name_ru: "Дубликат".into(),
                display_name_en: None,
                category: None,
            }],
            grants: vec![],
        };
        // Same bc_code to avoid namespace mismatch — tests duplicate detection
        let errs = PermissionRegistry::from_manifests_validated(vec![m1, m2]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Duplicate permission")));
    }

    #[test]
    fn validate_namespace_mismatch() {
        let mut m = cat_manifest();
        m.permissions.push(PermissionDef {
            command: "warehouse.receive_goods".into(),
            display_name_ru: "Чужой".into(),
            display_name_en: None,
            category: None,
        });
        let errs = PermissionRegistry::from_manifests_validated(vec![m]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("foreign permission")));
    }

    #[test]
    fn validate_platform_role_collision() {
        let mut m = wh_manifest();
        m.roles.push(RoleDef {
            code: "admin".into(),
            display_name_ru: "Администратор".into(),
            display_name_en: None,
            is_superadmin: false,
            security_level: 0,
        });
        let errs = PermissionRegistry::from_manifests_validated(vec![m]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("platform role")));
    }

    #[test]
    fn valid_manifests_ok() {
        let result =
            PermissionRegistry::from_manifests_validated(vec![wh_manifest(), cat_manifest()]);
        assert!(result.is_ok());
    }

    #[test]
    fn is_known_role_works() {
        let reg = PermissionRegistry::from_manifests_validated(vec![wh_manifest(), cat_manifest()])
            .unwrap();
        assert!(reg.is_known_role("admin"));
        assert!(reg.is_known_role("viewer"));
        assert!(reg.is_known_role("warehouse_manager"));
        assert!(reg.is_known_role("catalog_manager"));
        assert!(!reg.is_known_role("nonexistent"));
    }
}
