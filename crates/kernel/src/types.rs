//! Идентификаторы и контекст запроса.
//!
//! Newtype-обёртки над UUID обеспечивают type safety: компилятор не даст
//! перепутать `TenantId` и `UserId`, хотя внутри оба — UUID v7.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Идентификатор tenant'а (арендатора).
///
/// Каждый tenant — изолированная организация в мультитенантной ERP.
/// Используется в RLS-политиках `PostgreSQL` для фильтрации данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TenantId(Uuid);

impl TenantId {
    /// Создать новый `TenantId` с UUID v7 (time-ordered).
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Создать из существующего UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Доступ к внутреннему UUID.
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Идентификатор пользователя.
///
/// Привязан к конкретному tenant'у. Используется в аудите,
/// авторизации и трассировке запросов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(Uuid);

impl UserId {
    /// Создать новый `UserId` с UUID v7 (time-ordered).
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Создать из существующего UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Доступ к внутреннему UUID.
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Универсальный идентификатор доменной сущности.
///
/// Используется как ID агрегата внутри Bounded Context.
/// Каждый BC может иметь свои именованные ID-типы, но `EntityId` —
/// общий контракт для kernel-уровня.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct EntityId(Uuid);

impl EntityId {
    /// Создать новый `EntityId` с UUID v7 (time-ordered).
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Создать из существующего UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Доступ к внутреннему UUID.
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Контекст запроса — проходит через весь pipeline обработки.
///
/// Создаётся на входе (API Gateway) и передаётся в каждый слой.
/// Содержит информацию о tenant'е, пользователе и трассировочные ID.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Tenant, от имени которого выполняется запрос.
    pub tenant_id: TenantId,
    /// Пользователь, инициировавший запрос.
    pub user_id: UserId,
    /// Сквозной ID для трассировки через цепочку BC.
    pub correlation_id: Uuid,
    /// ID команды/события, породившего текущую операцию.
    pub causation_id: Uuid,
    /// Время создания контекста.
    pub timestamp: DateTime<Utc>,
    /// Роли пользователя (строки — kernel не знает о конкретных ролях).
    pub roles: Vec<String>,
}

impl RequestContext {
    /// Создать новый контекст запроса.
    ///
    /// `correlation_id` и `causation_id` генерируются как UUID v7,
    /// `timestamp` — текущее время UTC. Роли — пустые по умолчанию.
    #[must_use]
    pub fn new(tenant_id: TenantId, user_id: UserId) -> Self {
        Self {
            tenant_id,
            user_id,
            correlation_id: Uuid::now_v7(),
            causation_id: Uuid::now_v7(),
            timestamp: Utc::now(),
            roles: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_id_new_returns_valid_uuid_v7() {
        let id = TenantId::new();
        // UUID v7 has version bits set to 7
        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn tenant_id_serde_round_trip() {
        let id = TenantId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: TenantId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn tenant_id_display_shows_uuid() {
        let uuid = Uuid::now_v7();
        let id = TenantId::from_uuid(uuid);
        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn request_context_new_fills_all_fields() {
        let tenant_id = TenantId::new();
        let user_id = UserId::new();
        let ctx = RequestContext::new(tenant_id, user_id);

        assert_eq!(ctx.tenant_id, tenant_id);
        assert_eq!(ctx.user_id, user_id);
        // correlation_id and causation_id are UUID v7
        assert_eq!(ctx.correlation_id.get_version_num(), 7);
        assert_eq!(ctx.causation_id.get_version_num(), 7);
        // timestamp is recent (within last second)
        let elapsed = Utc::now() - ctx.timestamp;
        assert!(elapsed.num_seconds() < 1);
    }
}
