//! JWT issue/verify service.
//!
//! Uses HS256 (symmetric key). For production consider RS256/ES256,
//! but HS256 is sufficient for a modular monolith.

use chrono::Duration;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use kernel::AppError;
use kernel::types::{TenantId, UserId};

use crate::claims::Claims;

/// JWT token issue/verify service.
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    token_ttl: Duration,
}

impl JwtService {
    /// Create service with a symmetric key and token TTL.
    #[must_use]
    pub fn new(secret: &str, token_ttl: Duration) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            token_ttl,
        }
    }

    /// Issue a JWT token for a user.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` if serialization/signing fails.
    pub fn issue(
        &self,
        user_id: &UserId,
        tenant_id: &TenantId,
        roles: Vec<String>,
    ) -> Result<String, AppError> {
        let now = chrono::Utc::now();
        let exp = now + self.token_ttl;

        // JWT spec: iat/exp are positive Unix timestamps, safe to cast
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let claims = Claims {
            sub: user_id.as_uuid().to_string(),
            tenant_id: tenant_id.as_uuid().to_string(),
            roles,
            iat: now.timestamp() as usize,
            exp: exp.timestamp() as usize,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::Internal(format!("JWT encode error: {e}")))
    }

    /// Verify and decode a JWT token.
    ///
    /// # Errors
    ///
    /// `AppError::Unauthorized` if the token is invalid, expired, or the signature doesn't match.
    pub fn verify(&self, token: &str) -> Result<Claims, AppError> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| AppError::Unauthorized(format!("invalid token: {e}")))?;
        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> JwtService {
        JwtService::new("test-secret-key-at-least-32-bytes!", Duration::hours(1))
    }

    #[test]
    fn issue_verify_round_trip() {
        let svc = test_service();
        let user_id = UserId::new();
        let tenant_id = TenantId::new();
        let roles = vec!["admin".to_string(), "warehouse_operator".to_string()];

        let token = svc.issue(&user_id, &tenant_id, roles.clone()).unwrap();
        let claims = svc.verify(&token).unwrap();

        assert_eq!(claims.sub, user_id.as_uuid().to_string());
        assert_eq!(claims.tenant_id, tenant_id.as_uuid().to_string());
        assert_eq!(claims.roles, roles);
    }

    #[test]
    fn verify_garbage_token_returns_unauthorized() {
        let svc = test_service();
        let err = svc.verify("not.a.jwt").unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn verify_wrong_secret_returns_unauthorized() {
        let svc1 = JwtService::new("secret-one-must-be-long-enough!!", Duration::hours(1));
        let svc2 = JwtService::new("secret-two-must-be-long-enough!!", Duration::hours(1));

        let token = svc1
            .issue(&UserId::new(), &TenantId::new(), vec!["viewer".to_string()])
            .unwrap();

        let err = svc2.verify(&token).unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn verify_expired_token_returns_unauthorized() {
        let svc = JwtService::new("test-secret-key-at-least-32-bytes!", Duration::hours(-1));

        let token = svc.issue(&UserId::new(), &TenantId::new(), vec![]).unwrap();

        let err = svc.verify(&token).unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }
}
