//! Dev-only endpoints: `/dev/token`, `/dev/events`.
//!
//! Available only when `DEV_MODE` env var is set.
//! Not compiled into production builds conceptually, but gated at runtime.

use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Json, Router, routing};

use serde::Deserialize;

/// Shared state for dev token endpoint.
#[derive(Clone)]
pub(crate) struct DevTokenState {
    pub(crate) jwt: Arc<auth::JwtService>,
    pub(crate) registry: Arc<auth::PermissionRegistry>,
}

/// Request body for dev token.
#[derive(Deserialize)]
struct DevTokenRequest {
    tenant_id: uuid::Uuid,
    roles: Vec<String>,
}

/// Build dev Router with `/dev/token`.
pub(crate) fn dev_token_router(state: DevTokenState) -> Router {
    Router::new()
        .route("/dev/token", routing::post(dev_issue_token))
        .with_state(state)
}

/// POST /dev/token — issue JWT for testing (only `DEV_MODE`).
///
/// Validates that all requested roles are known. Unknown role -> 400 Bad Request.
async fn dev_issue_token(
    State(state): State<DevTokenState>,
    Json(body): Json<DevTokenRequest>,
) -> impl IntoResponse {
    // Validate all roles are known
    let unknown: Vec<&String> = body
        .roles
        .iter()
        .filter(|r| !state.registry.is_known_role(r))
        .collect();

    if !unknown.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("unknown roles: {:?}", unknown),
            })),
        )
            .into_response();
    }

    let tenant_id = kernel::types::TenantId::from_uuid(body.tenant_id);
    let user_id = kernel::types::UserId::new();

    match state.jwt.issue(&user_id, &tenant_id, body.roles) {
        Ok(token) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({
                "token": token,
                "user_id": user_id.as_uuid().to_string(),
                "tenant_id": tenant_id.as_uuid().to_string(),
            })),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use kernel::security::PermissionRegistrar;
    use tower::ServiceExt;

    fn test_state() -> DevTokenState {
        let jwt = Arc::new(auth::JwtService::new(
            "test-secret-key-at-least-32-bytes!",
            chrono::Duration::hours(1),
        ));
        let wh = warehouse::registrar::WarehousePermissions.permission_manifest();
        let cat = catalog::registrar::CatalogPermissions.permission_manifest();
        let registry =
            Arc::new(auth::PermissionRegistry::from_manifests_validated(vec![wh, cat]).unwrap());
        DevTokenState { jwt, registry }
    }

    async fn post_token(app: Router, body: serde_json::Value) -> axum::response::Response {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/dev/token")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn dev_token_valid_roles_returns_200() {
        let app = dev_token_router(test_state());
        let body = serde_json::json!({
            "tenant_id": uuid::Uuid::now_v7(),
            "roles": ["warehouse_operator", "viewer"]
        });
        let resp = post_token(app, body).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn dev_token_unknown_role_returns_400() {
        let app = dev_token_router(test_state());
        let body = serde_json::json!({
            "tenant_id": uuid::Uuid::now_v7(),
            "roles": ["nonexistent_role"]
        });
        let resp = post_token(app, body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn dev_token_mixed_valid_and_unknown_returns_400() {
        let app = dev_token_router(test_state());
        let body = serde_json::json!({
            "tenant_id": uuid::Uuid::now_v7(),
            "roles": ["admin", "ghost_role"]
        });
        let resp = post_token(app, body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn dev_token_admin_role_returns_200() {
        let app = dev_token_router(test_state());
        let body = serde_json::json!({
            "tenant_id": uuid::Uuid::now_v7(),
            "roles": ["admin"]
        });
        let resp = post_token(app, body).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn dev_token_empty_roles_returns_200() {
        let app = dev_token_router(test_state());
        let body = serde_json::json!({
            "tenant_id": uuid::Uuid::now_v7(),
            "roles": []
        });
        let resp = post_token(app, body).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
