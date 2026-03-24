//! RBAC — маппинг ролей на разрешённые команды.
//!
//! Статический маппинг в коде. Dynamic RBAC (из БД) — позже,
//! через тот же `PermissionMap` интерфейс.

use std::collections::{HashMap, HashSet};

use crate::claims::Role;

/// Маппинг ролей на разрешённые команды.
///
/// Поддерживает wildcard: `"warehouse.*"` разрешает все команды,
/// начинающиеся с `"warehouse."`.
pub struct PermissionMap {
    /// Роль → набор разрешённых команд (точных или с `*`).
    grants: HashMap<Role, HashSet<&'static str>>,
    /// Роли с полным доступом (admin).
    admin_roles: HashSet<Role>,
}

impl PermissionMap {
    /// Создать пустой маппинг.
    #[must_use]
    pub fn new() -> Self {
        Self {
            grants: HashMap::new(),
            admin_roles: HashSet::new(),
        }
    }

    /// Дать роли доступ к списку команд.
    ///
    /// Команда может заканчиваться на `*` — wildcard.
    pub fn grant(&mut self, role: Role, commands: &[&'static str]) -> &mut Self {
        let entry = self.grants.entry(role).or_default();
        for &cmd in commands {
            entry.insert(cmd);
        }
        self
    }

    /// Дать роли полный доступ ко всем командам.
    pub fn grant_all(&mut self, role: Role) -> &mut Self {
        self.admin_roles.insert(role);
        self
    }

    /// Проверить, разрешена ли команда для набора ролей.
    #[must_use]
    pub fn is_allowed(&self, roles: &[Role], command_name: &str) -> bool {
        for role in roles {
            // Admin-роль — доступ ко всему
            if self.admin_roles.contains(role) {
                return true;
            }

            if let Some(commands) = self.grants.get(role) {
                for &pattern in commands {
                    if let Some(prefix) = pattern.strip_suffix('*') {
                        // Wildcard: "warehouse.*" matches "warehouse.receive_goods"
                        if command_name.starts_with(prefix) {
                            return true;
                        }
                    } else if pattern == command_name {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Default for PermissionMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Преднастроенный RBAC для ERP.
#[must_use]
pub fn default_erp_permissions() -> PermissionMap {
    let mut map = PermissionMap::new();

    map.grant_all(Role::Admin);

    map.grant(Role::WarehouseManager, &["warehouse.*"]);

    map.grant(
        Role::WarehouseOperator,
        &[
            "warehouse.receive_goods",
            "warehouse.ship_goods",
            "warehouse.transfer_stock",
            "warehouse.reserve_stock",
            "warehouse.release_reservation",
        ],
    );

    map.grant(Role::Accountant, &["finance.*"]);

    map.grant(Role::SalesManager, &["sales.*"]);

    // Viewer — без доступа к командам (только queries)

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_do_anything() {
        let map = default_erp_permissions();
        assert!(map.is_allowed(&[Role::Admin], "warehouse.receive_goods"));
        assert!(map.is_allowed(&[Role::Admin], "finance.post_journal"));
        assert!(map.is_allowed(&[Role::Admin], "anything.at_all"));
    }

    #[test]
    fn warehouse_operator_allowed_commands() {
        let map = default_erp_permissions();
        let roles = &[Role::WarehouseOperator];
        assert!(map.is_allowed(roles, "warehouse.receive_goods"));
        assert!(map.is_allowed(roles, "warehouse.ship_goods"));
        assert!(map.is_allowed(roles, "warehouse.transfer_stock"));
        assert!(map.is_allowed(roles, "warehouse.reserve_stock"));
        assert!(map.is_allowed(roles, "warehouse.release_reservation"));
    }

    #[test]
    fn warehouse_operator_denied_manager_command() {
        let map = default_erp_permissions();
        // adjust_inventory — не в списке оператора, но в warehouse.*
        assert!(!map.is_allowed(&[Role::WarehouseOperator], "warehouse.adjust_inventory"));
        // Manager может через wildcard
        assert!(map.is_allowed(&[Role::WarehouseManager], "warehouse.adjust_inventory"));
    }

    #[test]
    fn warehouse_operator_denied_finance() {
        let map = default_erp_permissions();
        assert!(!map.is_allowed(&[Role::WarehouseOperator], "finance.post_journal"));
    }

    #[test]
    fn viewer_denied_everything() {
        let map = default_erp_permissions();
        let roles = &[Role::Viewer];
        assert!(!map.is_allowed(roles, "warehouse.receive_goods"));
        assert!(!map.is_allowed(roles, "finance.post_journal"));
        assert!(!map.is_allowed(roles, "sales.create_order"));
    }

    #[test]
    fn multiple_roles_union() {
        let map = default_erp_permissions();
        // Viewer + WarehouseOperator → operator permissions apply
        let roles = &[Role::Viewer, Role::WarehouseOperator];
        assert!(map.is_allowed(roles, "warehouse.receive_goods"));
        assert!(!map.is_allowed(roles, "finance.post_journal"));
    }

    #[test]
    fn wildcard_matching() {
        let mut map = PermissionMap::new();
        map.grant(Role::Accountant, &["finance.*"]);

        assert!(map.is_allowed(&[Role::Accountant], "finance.post_journal"));
        assert!(map.is_allowed(&[Role::Accountant], "finance.close_period"));
        assert!(!map.is_allowed(&[Role::Accountant], "warehouse.ship"));
    }

    #[test]
    fn empty_roles_denied() {
        let map = default_erp_permissions();
        assert!(!map.is_allowed(&[], "warehouse.receive_goods"));
    }
}
