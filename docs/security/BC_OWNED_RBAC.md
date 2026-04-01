# BC-Owned Roles & Permissions — Дизайн-документ

> **Дата:** 2026-03-31
> **Статус:** Proposal
> **Связи:** ADR (auth, Hybrid B+C), AUT (Cedar), SD_PHASES.md, auth crate

---

## 1. Проблема

Текущий auth crate нарушает BC autonomy:

```
auth/claims.rs    → enum Role { Admin, WarehouseManager, WarehouseOperator, ... }
auth/rbac.rs      → map.grant(Role::WarehouseOperator, &["warehouse.receive_goods", ...])
```

- **Auth знает про все BC.** Добавление нового BC (Procurement, Manufacturing) требует правки auth crate.
- **Роли глобальны.** `WarehouseOperator` определён в auth, хотя это знание Warehouse BC.
- **Маппинг централизован.** `default_erp_permissions()` содержит бизнес-логику всех BC.

Целевая модель: **BC приходит в платформу со своими ролями, командами и маппингами.**

---

## 2. Целевая архитектура

```
┌─────────────────────────────────────────────────────────────┐
│  BC crate (warehouse, catalog, procurement, ...)            │
│                                                             │
│  impl PermissionRegistrar for WarehouseBc {                 │
│      fn permission_manifest() → PermissionManifest {        │
│          roles: [warehouse_manager, warehouse_operator]     │
│          permissions: [receive_goods, ship_goods, ...]      │
│          grants: [operator → {receive, ship, transfer}]     │
│      }                                                      │
│  }                                                          │
├─────────────────────────────────────────────────────────────┤
│  kernel crate (traits + types, zero deps)                   │
│                                                             │
│  trait PermissionRegistrar { fn permission_manifest(); }     │
│  struct PermissionManifest { bc_code, roles, perms, grants }│
├─────────────────────────────────────────────────────────────┤
│  auth crate (infrastructure, enforcement)                   │
│                                                             │
│  PermissionRegistry ← собирает манифесты всех BC           │
│  JwtPermissionChecker ← использует Registry                │
│  JwtService ← issue/verify (без изменений)                  │
│  auth_middleware ← без изменений                             │
├─────────────────────────────────────────────────────────────┤
│  gateway crate (startup orchestration)                      │
│                                                             │
│  1. Собрать manifests от всех BC                            │
│  2. Построить PermissionRegistry                            │
│  3. Передать в JwtPermissionChecker                         │
│  4. (Phase DB) Persist manifests → iam.* для Admin UI       │
└─────────────────────────────────────────────────────────────┘
```

### Принцип: BC defines, Platform enforces

| Что | Кто определяет | Кто хранит runtime | Кто проверяет |
|-----|----------------|-------------------|---------------|
| Роли BC | BC (manifest) | PermissionRegistry (memory) + iam.* (DB, позже) | — |
| Команды BC | BC (manifest) | PermissionRegistry | — |
| Role→Permission | BC (manifest) | PermissionRegistry | JwtPermissionChecker |
| User→Role | Admin (UI/API) | iam.user_role_assignments (DB) | JWT claims |
| SD containers | BC (SdManifest) | authorization.* (DB) | SecurityDescriptorChecker |
| SD entities | BC (SdManifest) | authorization.* (DB) | SecurityDescriptorChecker |

---

## 3. Новые типы в kernel

### kernel/src/security/mod.rs (рядом с SD traits)

```rust
// ══════════════════════════════════════════════════════════
// RBAC Registration — BC declares its roles and permissions
// ══════════════════════════════════════════════════════════

/// BC регистрирует свои роли и разрешения при старте.
/// Аналог SdRegistrar, но для action-level RBAC (commands + queries).
pub trait PermissionRegistrar: Send + Sync {
    fn permission_manifest(&self) -> PermissionManifest;
}

/// Манифест RBAC-конфигурации одного BC.
#[derive(Debug, Clone)]
pub struct PermissionManifest {
    pub bc_code: String,

    /// Роли, определённые этим BC
    pub roles: Vec<RoleDef>,

    /// Действия (permissions), определённые этим BC.
    /// В Phase A сюда входят и commands, и queries, потому что
    /// текущий QueryPipeline тоже вызывает PermissionChecker.
    pub permissions: Vec<PermissionDef>,

    /// Маппинг: роль → список разрешённых действий
    /// Действия могут содержать wildcard: "warehouse.*"
    pub grants: Vec<RoleGrant>,
}

/// Определение роли
#[derive(Debug, Clone)]
pub struct RoleDef {
    /// Уникальный код: "warehouse_operator"
    /// Convention: "{bc_code}_{role_name}" для BC-specific ролей
    pub code: String,
    pub display_name_ru: String,
    pub display_name_en: Option<String>,
    /// Роль с полным доступом ко всем командам всех BC
    pub is_superadmin: bool,
    /// Security level (0-3, ERPNext-style)
    /// Определяет видимость полей с security_level <= role.level
    pub security_level: u8,
}

/// Определение действия (permission)
#[derive(Debug, Clone)]
pub struct PermissionDef {
    /// Полное имя действия: "warehouse.receive_goods" или "warehouse.get_balance"
    /// Convention: "{bc_code}.{action_name}"
    pub command: String,
    pub display_name_ru: String,
    pub display_name_en: Option<String>,
    /// Категория для группировки в Admin UI.
    /// Например: "Commands", "Queries", "Inventory"
    pub category: Option<String>,
}

/// Маппинг роли на действия
#[derive(Debug, Clone)]
pub struct RoleGrant {
    /// Код роли (должен быть в roles)
    pub role_code: String,
    /// Действия (exact или wildcard "warehouse.*")
    pub commands: Vec<String>,
}
```

### Платформенные роли (не BC-owned)

```rust
/// Platform-level roles живут в kernel как constants, не в auth.
/// Их не нужно регистрировать — они known at compile time.
pub mod platform_roles {
    pub const ADMIN: &str = "admin";
    pub const VIEWER: &str = "viewer";
}
```

`admin` и `viewer` — платформенные роли, они не принадлежат ни одному BC. `admin` = superadmin (доступ ко всему). `viewer` = read-only (нет доступа к командам, только queries).

Важно: в текущем runtime `QueryPipeline` тоже вызывает `PermissionChecker`, поэтому read-only доступ не может быть "неявным". Query-действия должны быть явно зарегистрированы в manifests и явно выданы через grants.

### Cross-grant rule: BC → platform role

BC может давать grants платформенным ролям (`viewer`, `admin`) на свои собственные actions. Это необходимо, потому что только BC знает какие queries у него есть и какие из них безопасны для read-only доступа.

Правила:
- BC может ссылаться в `RoleGrant.role_code` на platform role (`viewer`, `admin`)
- BC **не может** ссылаться на роли чужих BC (namespace enforcement)
- `validate()` не ругается на platform roles в grants, даже если они не в `known_roles` этого BC
- `admin` grant от BC избыточен (superadmin и так имеет доступ), но не является ошибкой
- Grants от нескольких BC для одной platform role объединяются (union)

---

## 4. Пример: Warehouse BC manifest

```rust
impl PermissionRegistrar for WarehouseBc {
    fn permission_manifest(&self) -> PermissionManifest {
        PermissionManifest {
            bc_code: "warehouse".into(),
            roles: vec![
                RoleDef {
                    code: "warehouse_manager".into(),
                    display_name_ru: "Менеджер склада".into(),
                    display_name_en: Some("Warehouse Manager".into()),
                    is_superadmin: false,
                    security_level: 2,
                },
                RoleDef {
                    code: "warehouse_operator".into(),
                    display_name_ru: "Кладовщик".into(),
                    display_name_en: Some("Warehouse Operator".into()),
                    is_superadmin: false,
                    security_level: 1,
                },
            ],
            permissions: vec![
                PermissionDef {
                    command: "warehouse.receive_goods".into(),
                    display_name_ru: "Приёмка товара".into(),
                    display_name_en: Some("Receive Goods".into()),
                    category: Some("Складские операции".into()),
                },
                PermissionDef {
                    command: "warehouse.ship_goods".into(),
                    display_name_ru: "Отгрузка товара".into(),
                    display_name_en: Some("Ship Goods".into()),
                    category: Some("Складские операции".into()),
                },
                PermissionDef {
                    command: "warehouse.transfer_stock".into(),
                    display_name_ru: "Перемещение".into(),
                    display_name_en: Some("Transfer Stock".into()),
                    category: Some("Складские операции".into()),
                },
                PermissionDef {
                    command: "warehouse.adjust_inventory".into(),
                    display_name_ru: "Корректировка остатков".into(),
                    display_name_en: Some("Adjust Inventory".into()),
                    category: Some("Инвентаризация".into()),
                },
                PermissionDef {
                    command: "warehouse.reserve_stock".into(),
                    display_name_ru: "Резервирование".into(),
                    display_name_en: Some("Reserve Stock".into()),
                    category: Some("Резервирование".into()),
                },
                PermissionDef {
                    command: "warehouse.release_reservation".into(),
                    display_name_ru: "Снятие резерва".into(),
                    display_name_en: Some("Release Reservation".into()),
                    category: Some("Резервирование".into()),
                },
                // ── Query actions ──────────────────────────────
                PermissionDef {
                    command: "warehouse.get_balance".into(),
                    display_name_ru: "Просмотр остатков".into(),
                    display_name_en: Some("Get Balance".into()),
                    category: Some("Запросы".into()),
                },
                PermissionDef {
                    command: "warehouse.get_movements".into(),
                    display_name_ru: "Просмотр движений".into(),
                    display_name_en: Some("Get Movements".into()),
                    category: Some("Запросы".into()),
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
                        "warehouse.ship_goods".into(),
                        "warehouse.transfer_stock".into(),
                        "warehouse.reserve_stock".into(),
                        "warehouse.release_reservation".into(),
                        "warehouse.get_balance".into(),
                        "warehouse.get_movements".into(),
                    ],
                },
                // ── Platform role grants ──────────────────────
                // BC может давать grants платформенным ролям на свои actions.
                // viewer — платформенная роль, но только BC знает какие queries
                // у него есть и какие из них доступны для read-only.
                RoleGrant {
                    role_code: "viewer".into(),
                    commands: vec![
                        "warehouse.get_balance".into(),
                        "warehouse.get_movements".into(),
                    ],
                },
            ],
        }
    }
}
```

---

## 5. Эволюция auth crate

### 5.1 Что удаляется

| Файл | Что | Почему |
|------|-----|--------|
| `claims.rs` | `enum Role` | Роли теперь строки, определяются BC |
| `claims.rs` | `Role::from_str_opt()` | Не нужен — роли уже строки |
| `rbac.rs` | `default_erp_permissions()` | Заменяется на `PermissionRegistry::from_manifests()` |

### 5.2 Что остаётся без изменений

| Файл | Что | Почему |
|------|-----|--------|
| `jwt.rs` | `JwtService` | Issue/verify не зависит от типов ролей |
| `middleware.rs` | `auth_middleware` | Уже работает со строками через `RequestContext.roles` |
| `middleware.rs` | `AppErrorResponse` | Маппинг ошибок не меняется |

### 5.3 Что меняется

#### Claims (jwt payload)

```rust
// БЫЛО:
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    pub roles: Vec<Role>,      // ← enum
    pub exp: usize,
    pub iat: usize,
}

// СТАЛО:
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    pub roles: Vec<String>,    // ← строки: ["warehouse_operator", "catalog_manager"]
    pub exp: usize,
    pub iat: usize,
}
```

Политика для unknown roles фиксируется явно: неизвестная роль в JWT не должна silently расширять доступ. Для Phase A допустимы только два варианта: reject token на входе либо deny-by-default в checker. Для dev-token endpoint неизвестные роли должны приводить к ошибке 4xx, а не молча отбрасываться.

`to_request_context()` упрощается — роли копируются напрямую, без serde round-trip:

```rust
impl Claims {
    pub fn to_request_context(&self) -> Result<RequestContext, AppError> {
        let user_uuid = Uuid::parse_str(&self.sub)
            .map_err(|e| AppError::Unauthorized(format!("invalid user_id: {e}")))?;
        let tenant_uuid = Uuid::parse_str(&self.tenant_id)
            .map_err(|e| AppError::Unauthorized(format!("invalid tenant_id: {e}")))?;

        let mut ctx = RequestContext::new(
            TenantId::from_uuid(tenant_uuid),
            UserId::from_uuid(user_uuid),
        );
        ctx.roles = self.roles.clone();
        Ok(ctx)
    }
}
```

#### JwtService::issue()

```rust
// БЫЛО:
pub fn issue(&self, user_id: &UserId, tenant_id: &TenantId, roles: Vec<Role>) -> Result<String, AppError>

// СТАЛО:
pub fn issue(&self, user_id: &UserId, tenant_id: &TenantId, roles: Vec<String>) -> Result<String, AppError>
```

#### PermissionRegistry (замена PermissionMap)

```rust
/// Реестр всех ролей и разрешений, собранный из BC-манифестов.
/// Immutable после инициализации. Thread-safe (Arc<PermissionRegistry>).
pub struct PermissionRegistry {
    /// role_code → набор разрешённых действий (exact + wildcards)
    grants: HashMap<String, Vec<String>>,
    /// role_codes с is_superadmin = true
    superadmin_roles: HashSet<String>,
    /// Все зарегистрированные роли
    known_roles: HashMap<String, RoleDef>,
    /// Все зарегистрированные действия (для Admin UI discovery)
    known_permissions: HashMap<String, PermissionDef>,
}

impl PermissionRegistry {
    /// Построить реестр из манифестов всех BC.
    pub fn from_manifests(manifests: Vec<PermissionManifest>) -> Self {
        let mut grants: HashMap<String, Vec<String>> = HashMap::new();
        let mut superadmin_roles = HashSet::new();
        let mut known_roles = HashMap::new();
        let mut known_permissions = HashMap::new();

        // Platform roles
        superadmin_roles.insert("admin".into());

        for manifest in manifests {
            // Register roles
            for role in &manifest.roles {
                if role.is_superadmin {
                    superadmin_roles.insert(role.code.clone());
                }
                known_roles.insert(role.code.clone(), role.clone());
            }

            // Register permissions (actions)
            for perm in &manifest.permissions {
                known_permissions.insert(perm.command.clone(), perm.clone());
            }

            // Register grants
            for grant in &manifest.grants {
                grants
                    .entry(grant.role_code.clone())
                    .or_default()
                    .extend(grant.commands.clone());
            }
        }

        Self { grants, superadmin_roles, known_roles, known_permissions }
    }

    /// Проверить, разрешено ли действие для набора ролей.
    pub fn is_allowed(&self, roles: &[String], action_name: &str) -> bool {
        for role in roles {
            if self.superadmin_roles.contains(role) {
                return true;
            }
            if let Some(commands) = self.grants.get(role) {
                for pattern in commands {
                    if let Some(prefix) = pattern.strip_suffix('*') {
                        if action_name.starts_with(prefix) {
                            return true;
                        }
                    } else if pattern == action_name {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Все зарегистрированные роли (для Admin UI).
    pub fn roles(&self) -> &HashMap<String, RoleDef> {
        &self.known_roles
    }

    /// Все зарегистрированные действия (для Admin UI).
    pub fn permissions(&self) -> &HashMap<String, PermissionDef> {
        &self.known_permissions
    }

    /// Валидация манифестов при старте. Fail-fast: startup должен упасть
    /// если манифесты некорректны. Без enum compiler больше не ловит
    /// опечатки — runtime validation компенсирует.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 1. Grants reference unknown roles
        //    Platform roles (admin, viewer) допустимы — BC может давать им grants
        let platform_roles: HashSet<&str> = ["admin", "viewer"].into_iter().collect();
        for role_code in self.grants.keys() {
            if !self.known_roles.contains_key(role_code)
                && !self.superadmin_roles.contains(role_code)
                && !platform_roles.contains(role_code.as_str())
            {
                errors.push(format!("Grant references unknown role: '{role_code}'"));
            }
        }

        // 2. Grants reference unknown actions (non-wildcard only)
        for (role_code, commands) in &self.grants {
            for cmd in commands {
                if !cmd.ends_with('*') && !self.known_permissions.contains_key(cmd) {
                    errors.push(format!(
                        "Role '{role_code}' grants unknown action: '{cmd}'"
                    ));
                }
            }
        }

        // 3. Namespace enforcement: permission prefix must match bc_code
        //    "warehouse.receive_goods" is valid only in a manifest with bc_code="warehouse"
        //    Checked during from_manifests, but we store bc_code per permission for this:
        for (cmd, perm) in &self.known_permissions {
            let expected_prefix = format!("{}.", cmd.split('.').next().unwrap_or(""));
            if !cmd.starts_with(&expected_prefix) || expected_prefix.len() <= 1 {
                errors.push(format!(
                    "Permission '{cmd}' has invalid format (expected 'bc_code.action_name')"
                ));
            }
        }

        // 4. Duplicate detection is handled by HashMap::insert in from_manifests
        //    (last-write-wins), but we track and report it explicitly:
        //    see from_manifests_validated() below

        // 5. Wildcard format: must be "prefix.*", not "**" or "prefix*"
        for (role_code, commands) in &self.grants {
            for cmd in commands {
                if cmd.contains('*') && !cmd.ends_with(".*") {
                    errors.push(format!(
                        "Role '{role_code}': invalid wildcard '{cmd}' (must end with '.*')"
                    ));
                }
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// Построить реестр с duplicate detection. Рекомендуется вместо from_manifests()
    /// для production startup.
    pub fn from_manifests_validated(
        manifests: Vec<PermissionManifest>,
    ) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut seen_roles: HashMap<String, String> = HashMap::new(); // code → bc_code
        let mut seen_perms: HashMap<String, String> = HashMap::new(); // command → bc_code

        for manifest in &manifests {
            // Check namespace enforcement: permissions must start with bc_code
            for perm in &manifest.permissions {
                let expected = format!("{}.", manifest.bc_code);
                if !perm.command.starts_with(&expected) {
                    errors.push(format!(
                        "BC '{}' registers foreign permission '{}' (must start with '{}')",
                        manifest.bc_code, perm.command, expected
                    ));
                }
                if let Some(prev_bc) = seen_perms.get(&perm.command) {
                    errors.push(format!(
                        "Duplicate permission '{}': registered by '{}' and '{}'",
                        perm.command, prev_bc, manifest.bc_code
                    ));
                } else {
                    seen_perms.insert(perm.command.clone(), manifest.bc_code.clone());
                }
            }

            // Check duplicate roles
            for role in &manifest.roles {
                if let Some(prev_bc) = seen_roles.get(&role.code) {
                    errors.push(format!(
                        "Duplicate role '{}': registered by '{}' and '{}'",
                        role.code, prev_bc, manifest.bc_code
                    ));
                } else {
                    seen_roles.insert(role.code.clone(), manifest.bc_code.clone());
                }
                // Platform role collision
                if role.code == "admin" || role.code == "viewer" {
                    errors.push(format!(
                        "BC '{}' defines role '{}' which conflicts with platform role",
                        manifest.bc_code, role.code
                    ));
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let registry = Self::from_manifests(manifests);
        registry.validate()?;
        Ok(registry)
    }
}
```

Рекомендуется использовать `from_manifests_validated()` при startup — он объединяет сборку и валидацию в один шаг с полным набором проверок.

#### JwtPermissionChecker

```rust
// БЫЛО:
pub struct JwtPermissionChecker {
    permission_map: PermissionMap,
}

// СТАЛО:
pub struct JwtPermissionChecker {
    registry: Arc<PermissionRegistry>,
}

impl JwtPermissionChecker {
    pub fn new(registry: Arc<PermissionRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl PermissionChecker for JwtPermissionChecker {
    async fn check_permission(
        &self,
        ctx: &RequestContext,
        command_name: &str,
    ) -> Result<(), AppError> {
        if self.registry.is_allowed(&ctx.roles, command_name) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!(
                "no permission for command '{command_name}'"
            )))
        }
    }
}
```

Обрати внимание: `from_str_opt()`, конвертация из enum — всё убирается. `ctx.roles` — уже строки, `is_allowed` принимает строки.

---

## 6. Startup flow (gateway)

```rust
// gateway/src/main.rs (или app_builder)

// 1. Каждый BC предоставляет свой манифест
let warehouse_manifest = warehouse_bc.permission_manifest();
let catalog_manifest = catalog_bc.permission_manifest();
// ... будущие BC добавляются сюда

// 2. Собираем реестр с валидацией (fail-fast)
let registry = Arc::new(
    PermissionRegistry::from_manifests_validated(vec![
        warehouse_manifest,
        catalog_manifest,
    ]).expect("RBAC manifest validation failed")
);

// 4. Передаём в checker
let permission_checker = Arc::new(JwtPermissionChecker::new(registry.clone()));

// 5. Pipelines используют checker
let pipeline = CommandPipeline::new(
    uow_factory,
    bus,
    permission_checker.clone(),
    extension_hooks.clone(),
    audit_log.clone(),
);

let query_pipeline = QueryPipeline::new(
    permission_checker,
    extension_hooks,
    audit_log,
);

// 6. (Опционально) Registry доступен для Admin UI endpoints
// GET /api/admin/roles → registry.roles()
// GET /api/admin/permissions → registry.permissions()
```

---

## 7. Связь с SD: единый паттерн регистрации

Оба trait'а следуют одному шаблону:

```
BC implements trait → manifest → platform collects → platform enforces
```

| Trait | Что регистрирует | Где enforcement |
|-------|-----------------|-----------------|
| `PermissionRegistrar` | Роли, actions (commands + queries), role→action | `PermissionRegistry` → `JwtPermissionChecker` |
| `SdRegistrar` | Контейнеры, SD-entities, mask bits | `authorization.*` → `SecurityDescriptorChecker` |

BC может реализовать оба:

```rust
impl PermissionRegistrar for WarehouseBc {
    fn permission_manifest(&self) -> PermissionManifest { ... }
}

impl SdRegistrar for WarehouseBc {
    fn sd_manifest(&self) -> SdManifest { ... }
}
```

Gateway при старте собирает оба типа манифестов:

```rust
// RBAC
let rbac_manifests = vec![
    warehouse_bc.permission_manifest(),
    catalog_bc.permission_manifest(),
];
let registry = PermissionRegistry::from_manifests(rbac_manifests);

// SD
let sd_manifests = vec![
    warehouse_bc.sd_manifest(),
    catalog_bc.sd_manifest(),
];
// → persist to authorization.* tables
```

---

## 8. Фазы миграции auth crate

### Phase A: BC-owned RBAC (без ломки JWT формата)

**Шаги:**

1. Добавить в kernel: `PermissionRegistrar`, `PermissionManifest`, `RoleDef`, `PermissionDef`, `RoleGrant`, `platform_roles`
2. Зафиксировать, что `permissions` в Phase A покрывают и commands, и queries
3. Реализовать `PermissionRegistrar` для Warehouse BC и Catalog BC, включая query-actions и viewer grants
4. Создать `PermissionRegistry` в auth crate с `from_manifests_validated()`
5. Заменить `PermissionMap` → `PermissionRegistry` в `JwtPermissionChecker`
6. `Claims.roles: Vec<Role>` → `Vec<String>`
7. `JwtService::issue()` принимает `Vec<String>` вместо `Vec<Role>`
8. Удалить `enum Role`, `Role::from_str_opt()`, `default_erp_permissions()`
9. Обновить тесты: использовать строки вместо enum и прогонять auth через `CommandPipeline` и `QueryPipeline`
10. Gateway: `from_manifests_validated()` при старте, передавать registry в оба pipeline

**Что НЕ меняется:**
- `PermissionChecker` trait в runtime (тот же интерфейс)
- `CommandPipeline` и `QueryPipeline` как интерфейсы runtime
- `auth_middleware` (работает с `RequestContext.roles` — уже строки)
- `AppErrorResponse` (маппинг ошибок)
- Формат JWT на проводе (roles были snake_case строки и остаются)

**Breaking changes:**
- `JwtService::issue()` signature — но это внутренний API gateway
- `enum Role` удаляется — потребители переходят на строки

### Phase B: DB-backed roles (для Admin UI)

После Phase A, когда потребуется Admin UI для управления ролями:

```sql
-- iam.role_definitions — populated from manifests, editable by admin
CREATE TABLE iam.role_definitions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    code            VARCHAR(100) NOT NULL,
    bc_code         VARCHAR(50) NOT NULL,
    display_name_ru VARCHAR(200) NOT NULL,
    display_name_en VARCHAR(200),
    is_superadmin   BOOLEAN NOT NULL DEFAULT false,
    security_level  SMALLINT NOT NULL DEFAULT 0,
    source          VARCHAR(20) NOT NULL DEFAULT 'manifest'
                    CHECK (source IN ('manifest', 'admin', 'system')),
    is_active       BOOLEAN NOT NULL DEFAULT true,
    CONSTRAINT uq_role_code UNIQUE (tenant_id, code)
);

-- iam.permission_definitions — populated from manifests
CREATE TABLE iam.permission_definitions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bc_code         VARCHAR(50) NOT NULL,
    command         VARCHAR(200) NOT NULL,
    display_name_ru VARCHAR(200) NOT NULL,
    display_name_en VARCHAR(200),
    category        VARCHAR(100),
    CONSTRAINT uq_permission UNIQUE (bc_code, command)
);

-- iam.role_permissions — from manifests + admin overrides
CREATE TABLE iam.role_permissions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL,
    role_code   VARCHAR(100) NOT NULL,
    command     VARCHAR(200) NOT NULL,  -- exact or "warehouse.*"
    source      VARCHAR(20) NOT NULL DEFAULT 'manifest'
                CHECK (source IN ('manifest', 'admin')),
    CONSTRAINT uq_role_perm UNIQUE (tenant_id, role_code, command)
);

-- iam.user_role_assignments — admin manages
CREATE TABLE iam.user_role_assignments (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL,
    user_id     UUID NOT NULL,
    role_code   VARCHAR(100) NOT NULL,
    scope_type  VARCHAR(50),   -- NULL = global, "warehouse" = scoped
    scope_id    UUID,          -- NULL = global, UUID = specific container
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    assigned_by UUID NOT NULL,
    CONSTRAINT uq_user_role UNIQUE (tenant_id, user_id, role_code, scope_id)
);
```

**Scope** — ключевое добавление Phase B:
- `scope_type = NULL, scope_id = NULL` → роль на всю систему
- `scope_type = "warehouse", scope_id = WH-1` → роль только на складе WH-1

Это соединяет RBAC с SD: назначение роли на контейнер = ACE на SD контейнера.

#### Scoped roles в JWT

При наличии scoped assignments JWT payload расширяется:

```json
{
  "sub": "user-uuid",
  "tenant_id": "tenant-uuid",
  "roles": ["warehouse_operator", "catalog_manager"],
  "scoped_roles": [
    {"role": "warehouse_operator", "scope_type": "warehouse", "scope_id": "WH-1-uuid"},
    {"role": "warehouse_operator", "scope_type": "warehouse", "scope_id": "WH-2-uuid"}
  ],
  "exp": 9999999999
}
```

Семантика:
- `roles` — глобальные роли (Phase A, backward-compatible)
- `scoped_roles` — новое поле, nullable, Phase B only
- Если `scoped_roles` присутствует для данной роли, `PermissionChecker` передаёт scope в pipeline context
- Финальная scope-проверка выполняется на уровне Cedar ABAC (Phase C) или SD container check (Phase B+)
- Phase A: поле отсутствует, `PermissionChecker` работает как раньше — полная backward-compatibility

Альтернатива (если JWT раздувается): `roles` содержат глобальные назначения, а scoped assignments проверяются через DB lookup в middleware. Trade-off: +1 DB query на request, но JWT не растёт.

### Phase C: Cedar ABAC

Когда простого RBAC недостаточно — условия на атрибутах:

```cedar
permit(
    principal in Role::"warehouse_operator",
    action == Action::"warehouse.ship_goods",
    resource
) when {
    resource.warehouse_id == principal.assigned_warehouse_id
    && resource.status == "approved"
};
```

Cedar читает роли из того же `PermissionRegistry`, но добавляет attribute conditions. `HybridPermissionChecker` из SD дизайна работает поверх:

```
RBAC (PermissionRegistry) → Cedar ABAC → SD (per-record)
```

---

## 9. Матрица: что где и когда

| Слой | Phase A | Phase B | Phase C |
|------|---------|---------|---------|
| **kernel** | PermissionRegistrar trait + types | — | Cedar policy types |
| **auth** | PermissionRegistry (in-memory) | + DB-backed registry | + CedarChecker |
| **BC** | impl PermissionRegistrar | — | .cedar policy files |
| **gateway** | Collect manifests → Registry | + Persist to DB | + Load Cedar policies |
| **iam DB** | — | role_definitions, user_role_assignments | — |
| **Admin UI** | — | Role/perm management | Policy viewer |
| **JWT** | roles: Vec<String> + explicit unknown-role policy | + scoped_roles? | — |

---

## 10. Критерий готовности Phase A

1. ✅ `enum Role` удалён из auth crate
2. ✅ `PermissionRegistrar` trait в kernel
3. ✅ Warehouse и Catalog реализуют `PermissionRegistrar` с commands **и** queries в `permissions`
4. ✅ `PermissionRegistry::from_manifests_validated()` собирает маппинги и проверяет duplicate codes, namespace mismatch, unknown actions, platform role collision
5. ✅ `JwtPermissionChecker` использует `PermissionRegistry` и для command, и для query authorization
6. ✅ `Claims.roles: Vec<String>`
7. ✅ Unknown roles в JWT/dev-token обрабатываются по явному deny/reject правилу
8. ✅ Gateway собирает манифесты при старте и передаёт checker в `CommandPipeline` и `QueryPipeline`
9. ✅ BC манифесты включают grants для platform role `viewer` на query-actions
10. ✅ Все существующие тесты проходят (адаптированы)

---

## Приложение: Diff summary для auth crate

```
crates/auth/src/
├── claims.rs     — Role enum УДАЛЁН, Claims.roles → Vec<String>,
│                   to_request_context() упрощён
├── rbac.rs       — УДАЛЁН ПОЛНОСТЬЮ (PermissionMap, default_erp_permissions)
├── registry.rs   — НОВЫЙ: PermissionRegistry (замена rbac.rs)
├── checker.rs    — JwtPermissionChecker: PermissionMap → PermissionRegistry
├── jwt.rs        — issue(): Vec<Role> → Vec<String>
├── middleware.rs  — БЕЗ ИЗМЕНЕНИЙ
└── lib.rs        — re-exports обновлены

crates/kernel/src/
├── security/
│   ├── mod.rs         — НОВЫЙ: PermissionRegistrar trait + types
│   └── sd.rs          — SD traits (уже спроектировано)
└── lib.rs             — pub mod security

crates/warehouse/src/
└── registrar.rs  — НОВЫЙ: impl PermissionRegistrar for WarehouseBc

crates/catalog/src/
└── registrar.rs  — НОВЫЙ: impl PermissionRegistrar for CatalogBc
```
