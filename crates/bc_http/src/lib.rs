#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! `bc_http` — HTTP DSL для Bounded Contexts.
//!
//! Типобезопасный маршрутизатор [`BcRouter`]: десериализация, вызов pipeline/query,
//! маппинг ошибок. BC описывает маршруты декларативно через closure-фабрики:
//!
//! ```ignore
//! BcRouter::new(pipeline)
//!     .command(&Method::POST, "/receive", {
//!         let pool = pool.clone();
//!         move || ReceiveGoodsHandler::new(pool.clone())
//!     })
//!     .query(&Method::GET, "/balance", {
//!         let pool = pool.clone();
//!         move || GetBalanceHandler::new(pool.clone())
//!     })
//!     .build()
//! ```

use std::sync::Arc;

use axum::extract::{Extension, Query};
use axum::http::{Method, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use serde::Serialize;

use kernel::{AppError, DomainError, RequestContext};

use runtime::command_handler::CommandHandler;
use runtime::dto::{FromBody, FromQueryParams};
use runtime::pipeline::CommandPipeline;
use runtime::ports::UnitOfWorkFactory;
use runtime::query_handler::QueryHandler;

/// Типобезопасный маршрутизатор для Bounded Context.
///
/// Строит axum `Router` с автоматической десериализацией,
/// вызовом pipeline/query handler и маппингом ошибок.
/// Использует closure-фабрики вместо trait `HandlerFactory` —
/// типобезопасное создание handler'ов без `dyn Any` + downcast.
pub struct BcRouter<UF: UnitOfWorkFactory> {
    pipeline: Arc<CommandPipeline<UF>>,
    router: Router,
}

impl<UF: UnitOfWorkFactory + 'static> BcRouter<UF> {
    /// Создать новый маршрутизатор.
    #[must_use]
    pub fn new(pipeline: Arc<CommandPipeline<UF>>) -> Self {
        Self {
            pipeline,
            router: Router::new(),
        }
    }

    /// Зарегистрировать command handler по HTTP-методу и пути.
    ///
    /// Handler создаётся через closure-фабрику `factory`.
    /// Запрос десериализуется из JSON body через `FromBody`.
    /// Результат сериализуется в JSON, ошибки маппятся в HTTP-статусы.
    #[must_use]
    pub fn command<H, F>(self, method: &Method, path: &str, factory: F) -> Self
    where
        H: CommandHandler,
        H::Cmd: FromBody,
        H::Result: Serialize + Send + 'static,
        F: Fn() -> H + Clone + Send + Sync + 'static,
    {
        self.command_with_status::<H, F>(method, path, StatusCode::OK, factory)
    }

    /// Зарегистрировать command handler с кастомным HTTP status code при успехе.
    #[must_use]
    pub fn command_with_status<H, F>(
        mut self,
        method: &Method,
        path: &str,
        success_status: StatusCode,
        factory: F,
    ) -> Self
    where
        H: CommandHandler,
        H::Cmd: FromBody,
        H::Result: Serialize + Send + 'static,
        F: Fn() -> H + Clone + Send + Sync + 'static,
    {
        let handler = Arc::new((factory)());
        let pipeline = self.pipeline.clone();

        let method_router = routing::on(
            method_to_filter(method),
            move |Extension(ctx): Extension<RequestContext>,
                  Json(body): Json<<H::Cmd as FromBody>::Body>| {
                let handler = handler.clone();
                let pipeline = pipeline.clone();
                async move {
                    let cmd = <H::Cmd as FromBody>::from_body(body);
                    match pipeline.execute(&*handler, &cmd, &ctx).await {
                        Ok(result) => {
                            let json = serde_json::to_value(result)
                                .unwrap_or_else(|_| serde_json::json!("ok"));
                            (success_status, Json(json)).into_response()
                        }
                        Err(e) => error_to_response(&e),
                    }
                }
            },
        );

        self.router = self.router.route(path, method_router);
        self
    }

    /// Зарегистрировать query handler по HTTP-методу и пути.
    ///
    /// Handler создаётся через closure-фабрику `factory`.
    /// Параметры десериализуются из query string через `FromQueryParams`.
    #[must_use]
    pub fn query<H, F>(mut self, method: &Method, path: &str, factory: F) -> Self
    where
        H: QueryHandler,
        H::Query: FromQueryParams,
        H::Result: Serialize + Send + 'static,
        F: Fn() -> H + Clone + Send + Sync + 'static,
    {
        let handler = Arc::new((factory)());

        let method_router = routing::on(
            method_to_filter(method),
            move |Extension(ctx): Extension<RequestContext>,
                  Query(params): Query<<H::Query as FromQueryParams>::Params>| {
                let handler = handler.clone();
                async move {
                    let query = <H::Query as FromQueryParams>::from_params(params);
                    match handler.handle(&query, &ctx).await {
                        Ok(result) => {
                            let json = serde_json::to_value(result)
                                .unwrap_or_else(|_| serde_json::json!("ok"));
                            (StatusCode::OK, Json(json)).into_response()
                        }
                        Err(e) => error_to_response(&e),
                    }
                }
            },
        );

        self.router = self.router.route(path, method_router);
        self
    }

    /// Завершить построение и вернуть axum `Router`.
    pub fn build(self) -> Router {
        self.router
    }
}

/// Маппинг `AppError` -> HTTP response.
fn error_to_response(e: &AppError) -> axum::response::Response {
    let (status, code) = match e {
        AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
        AppError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
        AppError::Domain(domain_err) => match domain_err {
            DomainError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            _ => (StatusCode::UNPROCESSABLE_ENTITY, "DOMAIN_ERROR"),
        },
        AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
    };

    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": e.to_string(),
        }
    });

    (status, Json(body)).into_response()
}

/// Конвертация `Method` в axum `MethodFilter`.
fn method_to_filter(method: &Method) -> axum::routing::MethodFilter {
    match *method {
        Method::POST => axum::routing::MethodFilter::POST,
        Method::PUT => axum::routing::MethodFilter::PUT,
        Method::DELETE => axum::routing::MethodFilter::DELETE,
        Method::PATCH => axum::routing::MethodFilter::PATCH,
        // GET and all other methods default to GET
        _ => axum::routing::MethodFilter::GET,
    }
}
