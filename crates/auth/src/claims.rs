//! JWT Claims.
//!
//! `Claims` — the JWT token payload. Roles are plain strings defined by BC manifests.
//! Kernel's `RequestContext.roles` stores the same strings.

use kernel::AppError;
use kernel::types::{RequestContext, TenantId, UserId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT Claims — token payload.
///
/// Used by `jsonwebtoken` for issue/verify.
/// `to_request_context()` converts claims into kernel-compatible `RequestContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — `UserId` as UUID string.
    pub sub: String,
    /// Tenant ID as UUID string.
    pub tenant_id: String,
    /// User roles (plain strings: `"warehouse_operator"`, `"admin"`, etc.).
    pub roles: Vec<String>,
    /// Expiration time (Unix timestamp).
    pub exp: usize,
    /// Issued at time (Unix timestamp).
    pub iat: usize,
}

impl Claims {
    /// Convert claims into `RequestContext`.
    ///
    /// # Errors
    ///
    /// `AppError::Unauthorized` if `sub` or `tenant_id` is not a valid UUID.
    pub fn to_request_context(&self) -> Result<RequestContext, AppError> {
        let user_uuid = Uuid::parse_str(&self.sub)
            .map_err(|e| AppError::Unauthorized(format!("invalid user_id in token: {e}")))?;
        let tenant_uuid = Uuid::parse_str(&self.tenant_id)
            .map_err(|e| AppError::Unauthorized(format!("invalid tenant_id in token: {e}")))?;

        let mut ctx = RequestContext::new(
            TenantId::from_uuid(tenant_uuid),
            UserId::from_uuid(user_uuid),
        );
        ctx.roles.clone_from(&self.roles);
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_to_request_context_valid_uuids() {
        let user_id = Uuid::now_v7();
        let tenant_id = Uuid::now_v7();
        let claims = Claims {
            sub: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            roles: vec!["admin".to_string(), "warehouse_operator".to_string()],
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
    fn claims_roles_round_trip_json() {
        let claims = Claims {
            sub: Uuid::now_v7().to_string(),
            tenant_id: Uuid::now_v7().to_string(),
            roles: vec!["warehouse_operator".to_string(), "viewer".to_string()],
            exp: 9_999_999_999,
            iat: 1_000_000_000,
        };

        let json = serde_json::to_string(&claims).unwrap();
        let restored: Claims = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.roles, vec!["warehouse_operator", "viewer"]);
    }
}
