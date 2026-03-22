//! Контракт команд (CQRS) и конверт для передачи через pipeline.
//!
//! Команда — намерение изменить состояние системы.
//! `CommandEnvelope` оборачивает команду контекстом запроса.

use crate::types::RequestContext;

/// Контракт для всех команд в системе.
///
/// Каждая команда — отдельная структура с данными, реализующая этот trait.
/// Bounds `Send + Sync + 'static` необходимы для передачи между потоками
/// tokio runtime и хранения в dispatch-таблицах.
pub trait Command: Send + Sync + 'static {
    /// Имя команды для routing, аудита и логирования.
    ///
    /// Формат: `"bc_name.command_name"`, например `"warehouse.receive_goods"`.
    fn command_name(&self) -> &'static str;
}

/// Конверт команды — команда + контекст запроса.
///
/// Pipeline (Layer 5) работает с envelope'ами, а не с голыми командами,
/// чтобы всегда иметь доступ к `tenant_id`, `user_id`, `correlation_id`.
#[derive(Debug)]
pub struct CommandEnvelope<C: Command> {
    /// Команда для выполнения.
    pub command: C,
    /// Контекст запроса (кто, когда, какой tenant).
    pub context: RequestContext,
}

impl<C: Command> CommandEnvelope<C> {
    /// Создать конверт из команды и контекста.
    #[must_use]
    pub fn new(command: C, context: RequestContext) -> Self {
        Self { command, context }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TenantId, UserId};

    #[derive(Debug)]
    struct DummyCommand {
        value: i32,
    }

    impl Command for DummyCommand {
        fn command_name(&self) -> &'static str {
            "test.do_something"
        }
    }

    #[test]
    fn dummy_command_name() {
        let cmd = DummyCommand { value: 42 };
        assert_eq!(cmd.command_name(), "test.do_something");
    }

    #[test]
    fn command_envelope_new_fields_accessible() {
        let tenant_id = TenantId::new();
        let user_id = UserId::new();
        let ctx = RequestContext::new(tenant_id, user_id);
        let cmd = DummyCommand { value: 99 };

        let envelope = CommandEnvelope::new(cmd, ctx);

        assert_eq!(envelope.command.value, 99);
        assert_eq!(envelope.context.tenant_id, tenant_id);
        assert_eq!(envelope.context.user_id, user_id);
    }
}
