//! `JwtPermissionChecker` — реализация `PermissionChecker` из `runtime::ports`.
//!
//! Первая реальная подстановка адаптера вместо `NoopPermissionChecker`.
//! Извлекает роли из `RequestContext.roles` (строки), конвертирует в `Role` enum,
//! проверяет через `PermissionMap`.

use async_trait::async_trait;
use kernel::{AppError, RequestContext};
use runtime::ports::PermissionChecker;

use crate::claims::Role;
use crate::rbac::PermissionMap;

/// RBAC-авторизация через JWT роли.
///
/// Подставляется в `CommandPipeline` вместо `NoopPermissionChecker`.
pub struct JwtPermissionChecker {
    permission_map: PermissionMap,
}

impl JwtPermissionChecker {
    /// Создать checker с указанным маппингом ролей.
    #[must_use]
    pub fn new(permission_map: PermissionMap) -> Self {
        Self { permission_map }
    }
}

#[async_trait]
impl PermissionChecker for JwtPermissionChecker {
    async fn check_permission(
        &self,
        ctx: &RequestContext,
        command_name: &str,
    ) -> Result<(), AppError> {
        let roles: Vec<Role> = ctx
            .roles
            .iter()
            .filter_map(|s| Role::from_str_opt(s))
            .collect();

        if self.permission_map.is_allowed(&roles, command_name) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!(
                "no permission for command '{command_name}'"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::default_erp_permissions;
    use kernel::types::{TenantId, UserId};

    fn ctx_with_roles(roles: &[&str]) -> RequestContext {
        let mut ctx = RequestContext::new(TenantId::new(), UserId::new());
        ctx.roles = roles.iter().map(|s| (*s).to_string()).collect();
        ctx
    }

    #[tokio::test]
    async fn admin_allowed() {
        let checker = JwtPermissionChecker::new(default_erp_permissions());
        let ctx = ctx_with_roles(&["admin"]);
        assert!(
            checker
                .check_permission(&ctx, "warehouse.receive_goods")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn operator_allowed_warehouse() {
        let checker = JwtPermissionChecker::new(default_erp_permissions());
        let ctx = ctx_with_roles(&["warehouse_operator"]);
        assert!(
            checker
                .check_permission(&ctx, "warehouse.receive_goods")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn operator_denied_finance() {
        let checker = JwtPermissionChecker::new(default_erp_permissions());
        let ctx = ctx_with_roles(&["warehouse_operator"]);
        let err = checker
            .check_permission(&ctx, "finance.post_journal")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[tokio::test]
    async fn unknown_role_ignored() {
        let checker = JwtPermissionChecker::new(default_erp_permissions());
        let ctx = ctx_with_roles(&["unknown_role"]);
        let err = checker
            .check_permission(&ctx, "warehouse.receive_goods")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    // ─── Fixtures for integration test ─────────────────────────────

    #[derive(Debug)]
    struct PipelineTestCmd;
    impl kernel::Command for PipelineTestCmd {
        fn command_name(&self) -> &'static str {
            "warehouse.receive_goods"
        }
    }

    #[derive(Debug, serde::Serialize)]
    struct PipelineTestResult {
        ok: bool,
    }

    struct PipelineTestHandler;

    #[async_trait::async_trait]
    impl runtime::command_handler::CommandHandler for PipelineTestHandler {
        type Cmd = PipelineTestCmd;
        type Result = PipelineTestResult;

        async fn handle(
            &self,
            _cmd: &Self::Cmd,
            _ctx: &RequestContext,
            _uow: &mut dyn runtime::ports::UnitOfWork,
        ) -> Result<Self::Result, AppError> {
            Ok(PipelineTestResult { ok: true })
        }
    }

    #[tokio::test]
    async fn integration_with_pipeline() {
        use runtime::pipeline::CommandPipeline;
        use runtime::stubs::{InMemoryUnitOfWorkFactory, NoopAuditLog, NoopExtensionHooks};
        use std::sync::Arc;

        let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
        let bus = Arc::new(event_bus::InProcessBus::new());
        let checker = Arc::new(JwtPermissionChecker::new(default_erp_permissions()));

        let pipeline = CommandPipeline::new(
            uow_factory,
            bus,
            checker,
            Arc::new(NoopExtensionHooks),
            Arc::new(NoopAuditLog),
        );

        // Unauthorized user → Err
        let unauthorized_ctx = ctx_with_roles(&["viewer"]);
        let result = pipeline
            .execute(&PipelineTestHandler, &PipelineTestCmd, &unauthorized_ctx)
            .await;
        assert!(matches!(result, Err(AppError::Unauthorized(_))));

        // Authorized user → Ok
        let authorized_ctx = ctx_with_roles(&["warehouse_operator"]);
        let result = pipeline
            .execute(&PipelineTestHandler, &PipelineTestCmd, &authorized_ctx)
            .await;
        assert!(result.is_ok());
    }
}
