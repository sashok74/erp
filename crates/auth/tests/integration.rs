//! Integration-тесты для auth crate.
//!
//! Проверяют взаимодействие JwtPermissionChecker с CommandPipeline.

use std::sync::Arc;

use kernel::types::{RequestContext, TenantId, UserId};
use kernel::{AppError, Command};
use runtime::command_handler::CommandHandler;
use runtime::pipeline::CommandPipeline;
use runtime::stubs::{InMemoryUnitOfWorkFactory, NoopAuditLog, NoopExtensionHooks};

use auth::checker::JwtPermissionChecker;
use auth::PermissionRegistry;
use kernel::security::{PermissionDef, PermissionManifest, RoleDef, RoleGrant};

fn test_registry() -> Arc<PermissionRegistry> {
    let wh = PermissionManifest {
        bc_code: "warehouse".into(),
        roles: vec![
            RoleDef {
                code: "warehouse_manager".into(),
                display_name_ru: "Менеджер склада".into(),
                display_name_en: None,
                is_superadmin: false,
                security_level: 2,
            },
            RoleDef {
                code: "warehouse_operator".into(),
                display_name_ru: "Кладовщик".into(),
                display_name_en: None,
                is_superadmin: false,
                security_level: 1,
            },
        ],
        permissions: vec![
            PermissionDef {
                command: "warehouse.receive_goods".into(),
                display_name_ru: "Приёмка".into(),
                display_name_en: None,
                category: None,
            },
            PermissionDef {
                command: "warehouse.get_balance".into(),
                display_name_ru: "Остатки".into(),
                display_name_en: None,
                category: None,
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
                    "warehouse.get_balance".into(),
                ],
            },
            RoleGrant {
                role_code: "viewer".into(),
                commands: vec!["warehouse.get_balance".into()],
            },
        ],
    };
    Arc::new(PermissionRegistry::from_manifests_validated(vec![wh]).unwrap())
}

fn ctx_with_roles(roles: &[&str]) -> RequestContext {
    let mut ctx = RequestContext::new(TenantId::new(), UserId::new());
    ctx.roles = roles.iter().map(|s| (*s).to_string()).collect();
    ctx
}

#[derive(Debug)]
struct TestCmd;
impl Command for TestCmd {
    fn command_name(&self) -> &'static str {
        "warehouse.receive_goods"
    }
}

#[derive(Debug, serde::Serialize)]
struct TestResult {
    ok: bool,
}

struct TestHandler;

#[async_trait::async_trait]
impl CommandHandler for TestHandler {
    type Cmd = TestCmd;
    type Result = TestResult;

    async fn handle(
        &self,
        _cmd: &Self::Cmd,
        _ctx: &RequestContext,
        _uow: &mut dyn runtime::ports::UnitOfWork,
    ) -> Result<Self::Result, AppError> {
        Ok(TestResult { ok: true })
    }
}

#[tokio::test]
async fn checker_integrates_with_pipeline() {
    let uow_factory = Arc::new(InMemoryUnitOfWorkFactory::new());
    let bus = Arc::new(event_bus::InProcessBus::new());
    let checker = Arc::new(JwtPermissionChecker::new(test_registry()));

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
        .execute(&TestHandler, &TestCmd, &unauthorized_ctx)
        .await;
    assert!(matches!(result, Err(AppError::Unauthorized(_))));

    // Authorized user → Ok
    let authorized_ctx = ctx_with_roles(&["warehouse_operator"]);
    let result = pipeline
        .execute(&TestHandler, &TestCmd, &authorized_ctx)
        .await;
    assert!(result.is_ok());
}
