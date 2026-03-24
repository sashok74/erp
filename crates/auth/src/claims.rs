//! JWT Claims и роли пользователей.
//!
//! `Claims` — содержимое JWT токена. `Role` — перечисление ролей ERP.
//! Kernel не знает о ролях — в `RequestContext.roles` хранятся строки.

use kernel::AppError;
use kernel::types::{RequestContext, TenantId, UserId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Роль пользователя в ERP-системе.
///
/// Определяет набор разрешённых команд через RBAC (`PermissionMap`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Полный доступ ко всем командам.
    Admin,
    /// Управление складом: все операции `warehouse.*`.
    WarehouseManager,
    /// Складские операции: приём, отгрузка, перемещение, резерв.
    WarehouseOperator,
    /// Финансовые операции: `finance.*`.
    Accountant,
    /// Управление продажами: `sales.*`.
    SalesManager,
    /// Только чтение. Нет доступа к командам.
    Viewer,
}

impl Role {
    /// Конвертировать строку в роль. `None` если строка не распознана.
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "warehouse_manager" => Some(Self::WarehouseManager),
            "warehouse_operator" => Some(Self::WarehouseOperator),
            "accountant" => Some(Self::Accountant),
            "sales_manager" => Some(Self::SalesManager),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }
}

/// JWT Claims — содержимое токена.
///
/// Используется `jsonwebtoken` для issue/verify.
/// `to_request_context()` конвертирует claims в kernel-совместимый `RequestContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — `UserId` как строка UUID.
    pub sub: String,
    /// Tenant ID как строка UUID.
    pub tenant_id: String,
    /// Роли пользователя.
    pub roles: Vec<Role>,
    /// Время истечения (Unix timestamp).
    pub exp: usize,
    /// Время выдачи (Unix timestamp).
    pub iat: usize,
}

impl Claims {
    /// Конвертировать claims в `RequestContext`.
    ///
    /// # Errors
    ///
    /// `AppError::Unauthorized` если `sub` или `tenant_id` — невалидный UUID.
    pub fn to_request_context(&self) -> Result<RequestContext, AppError> {
        let user_uuid = Uuid::parse_str(&self.sub)
            .map_err(|e| AppError::Unauthorized(format!("invalid user_id in token: {e}")))?;
        let tenant_uuid = Uuid::parse_str(&self.tenant_id)
            .map_err(|e| AppError::Unauthorized(format!("invalid tenant_id in token: {e}")))?;

        let roles: Vec<String> = self
            .roles
            .iter()
            .map(|r| {
                serde_json::to_value(r)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default()
            })
            .collect();

        let mut ctx = RequestContext::new(
            TenantId::from_uuid(tenant_uuid),
            UserId::from_uuid(user_uuid),
        );
        ctx.roles = roles;
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_serde_round_trip() {
        let role = Role::WarehouseManager;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"warehouse_manager\"");

        let restored: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, Role::WarehouseManager);
    }

    #[test]
    fn role_all_variants_serialize() {
        assert_eq!(serde_json::to_string(&Role::Admin).unwrap(), "\"admin\"");
        assert_eq!(
            serde_json::to_string(&Role::WarehouseOperator).unwrap(),
            "\"warehouse_operator\""
        );
        assert_eq!(
            serde_json::to_string(&Role::Accountant).unwrap(),
            "\"accountant\""
        );
        assert_eq!(
            serde_json::to_string(&Role::SalesManager).unwrap(),
            "\"sales_manager\""
        );
        assert_eq!(serde_json::to_string(&Role::Viewer).unwrap(), "\"viewer\"");
    }

    #[test]
    fn claims_to_request_context_valid_uuids() {
        let user_id = Uuid::now_v7();
        let tenant_id = Uuid::now_v7();
        let claims = Claims {
            sub: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            roles: vec![Role::Admin, Role::WarehouseOperator],
            exp: 9_999_999_999,
            iat: 1_000_000_000,
        };

        let ctx = claims.to_request_context().unwrap();
        assert_eq!(ctx.roles, vec!["admin", "warehouse_operator"]);
    }

    #[test]
    fn claims_to_request_context_invalid_user_id() {
        let claims = Claims {
            sub: "not-a-uuid".to_string(),
            tenant_id: Uuid::now_v7().to_string(),
            roles: vec![],
            exp: 9_999_999_999,
            iat: 1_000_000_000,
        };

        let err = claims.to_request_context().unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn claims_to_request_context_invalid_tenant_id() {
        let claims = Claims {
            sub: Uuid::now_v7().to_string(),
            tenant_id: "bad-tenant".to_string(),
            roles: vec![],
            exp: 9_999_999_999,
            iat: 1_000_000_000,
        };

        let err = claims.to_request_context().unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn role_from_str_opt_works() {
        assert_eq!(Role::from_str_opt("admin"), Some(Role::Admin));
        assert_eq!(
            Role::from_str_opt("warehouse_manager"),
            Some(Role::WarehouseManager)
        );
        assert_eq!(Role::from_str_opt("unknown"), None);
    }
}
