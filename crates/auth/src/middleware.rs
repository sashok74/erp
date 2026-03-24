//! Axum middleware для JWT-аутентификации и маппинг `AppError` → HTTP response.
//!
//! `auth_middleware` извлекает Bearer token из Authorization header,
//! верифицирует через `JwtService`, конвертирует в `RequestContext`
//! и кладёт в request extensions.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use kernel::errors::{AppError, DomainError};

use crate::jwt::JwtService;

/// Newtype-обёртка для обхода orphan rule: `AppError` определён в kernel,
/// а `IntoResponse` — в axum. Обёртка позволяет реализовать trait в auth crate.
pub struct AppErrorResponse(pub AppError);

impl From<AppError> for AppErrorResponse {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for AppErrorResponse {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            AppError::Domain(domain_err) => match domain_err {
                DomainError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
                DomainError::InsufficientStock { .. }
                | DomainError::NegativeBalance
                | DomainError::ConcurrencyConflict { .. }
                | DomainError::BusinessRule(_) => {
                    (StatusCode::UNPROCESSABLE_ENTITY, "DOMAIN_ERROR")
                }
            },
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        let body = serde_json::json!({
            "error": {
                "code": code,
                "message": self.0.to_string(),
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

/// Axum middleware: извлечение JWT → `RequestContext` в extensions.
///
/// # Errors
///
/// Возвращает 401 если нет header'а, токен невалидный или claims некорректны.
pub async fn auth_middleware(
    request: Request<Body>,
    next: Next,
    jwt_service: Arc<JwtService>,
) -> Response {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return AppErrorResponse(AppError::Unauthorized(
                "missing or invalid Authorization header".to_string(),
            ))
            .into_response();
        }
    };

    let claims = match jwt_service.verify(token) {
        Ok(c) => c,
        Err(e) => return AppErrorResponse(e).into_response(),
    };

    let ctx = match claims.to_request_context() {
        Ok(c) => c,
        Err(e) => return AppErrorResponse(e).into_response(),
    };

    let mut request = request;
    request.extensions_mut().insert(ctx);

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::Role;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use chrono::Duration;
    use kernel::types::{RequestContext, TenantId, UserId};
    use tower::ServiceExt;

    fn test_jwt_service() -> Arc<JwtService> {
        Arc::new(JwtService::new(
            "test-secret-key-at-least-32-bytes!",
            Duration::hours(1),
        ))
    }

    fn make_app(jwt_service: Arc<JwtService>) -> Router {
        let svc = jwt_service;
        Router::new()
            .route(
                "/test",
                get(|req: axum::extract::Request| async move {
                    let ctx = req.extensions().get::<RequestContext>();
                    match ctx {
                        Some(c) => {
                            let body = serde_json::json!({
                                "user_id": c.user_id.as_uuid().to_string(),
                                "roles": c.roles,
                            });
                            axum::Json(body).into_response()
                        }
                        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                    }
                }),
            )
            .layer(axum::middleware::from_fn(move |req, next| {
                let svc = Arc::clone(&svc);
                async move { auth_middleware(req, next, svc).await }
            }))
    }

    #[tokio::test]
    async fn valid_jwt_passes_through() {
        let jwt_svc = test_jwt_service();
        let user_id = UserId::new();
        let tenant_id = TenantId::new();
        let token = jwt_svc
            .issue(&user_id, &tenant_id, vec![Role::Admin])
            .unwrap();

        let app = make_app(jwt_svc);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_header_returns_401() {
        let jwt_svc = test_jwt_service();
        let app = make_app(jwt_svc);

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn invalid_token_returns_401() {
        let jwt_svc = test_jwt_service();
        let app = make_app(jwt_svc);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", "Bearer invalid.token.here")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn app_error_status_code_mapping() {
        // Unauthorized → 401
        let resp = AppErrorResponse(AppError::Unauthorized("no".to_string())).into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Validation → 400
        let resp = AppErrorResponse(AppError::Validation("bad input".to_string())).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // NotFound → 404
        let resp = AppErrorResponse(AppError::Domain(DomainError::NotFound("item".to_string())))
            .into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // BusinessRule → 422
        let resp = AppErrorResponse(AppError::Domain(DomainError::BusinessRule(
            "rule".to_string(),
        )))
        .into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // InsufficientStock → 422
        let resp = AppErrorResponse(AppError::Domain(DomainError::InsufficientStock {
            required: "10".to_string(),
            available: "3".to_string(),
        }))
        .into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // Internal → 500
        let resp = AppErrorResponse(AppError::Internal("db down".to_string())).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
