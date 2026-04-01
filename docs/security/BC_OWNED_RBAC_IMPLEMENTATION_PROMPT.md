# Промпт реализации: BC-Owned RBAC (Phase A, refined)

> Дата: 2026-03-31
> Управляющие документы:
> - `docs/security/BC_OWNED_RBAC.md`
> - `docs/security/AUTHORIZATION_ACCEPTANCE_PLAN.md`
> - `docs/EXECUTION_PLAN.md`
> - `docs/engineering_invariants.md`
>
> Цель: перейти от hardcoded `enum Role` + `PermissionMap` к BC-owned manifests + `PermissionRegistry`.
> После выполнения добавление нового BC не требует правки auth crate.

---

## 1. Что должен гарантировать результат

Итоговая реализация должна гарантировать:

1. Роли и permissions определяются BC через manifests.
2. `auth` не знает заранее о BC-specific ролях.
3. `CommandPipeline` и `QueryPipeline` используют одну и ту же policy model.
4. `viewer` не имеет implicit read-access. Query-доступ дается только через явные grants.
5. Unknown role никогда не расширяет доступ.
6. Gateway валидирует manifests fail-fast при старте.
7. `test_support` и `/dev/token` работают в новой модели без legacy `Role` enum.

---

## 2. Жесткие инварианты

1. **Существующие тесты — source of truth.** Все workspace tests должны проходить после завершения работы.
2. **Core runtime contracts не менять:** `PermissionChecker` trait signature, `CommandPipeline`, `QueryPipeline`, `CommandHandler`, `UnitOfWork`, `EventBus`.
3. **Unknown role policy фиксирована заранее:**
   - runtime checker: `deny-by-default`
   - `/dev/token`: unknown role = `400 Bad Request`
   - запрещено silently ignore/filter roles
4. **Viewer policy фиксирована заранее:**
   - `viewer` не superadmin
   - `viewer` не получает implicit queries
   - `viewer` может получать только явно зарегистрированные read/query actions
5. **Cross-grant rule ограничена namespace owner'ом:**
   - BC может выдавать grants на platform roles (`viewer`, при необходимости `admin`)
   - но только на собственные actions своего namespace
   - BC не может регистрировать или grant'ить foreign actions
6. **Validation должна быть строгой:** startup fail-fast на duplicate codes, namespace mismatch, invalid wildcard, unknown action target, platform role collision.
7. **Тесты query authorization обязаны идти через `QueryPipeline`, а не прямым вызовом handler.**
8. **Clorinde SQL policy не ухудшать.** Не добавлять новый production inline SQL.

---

## 3. Архитектурные решения, которые нельзя оставлять двусмысленными

### 3.1 Permissions = actions

В Phase A поле `PermissionDef.command` семантически хранит `action name`.
Это включает:

- commands: `warehouse.receive_goods`
- queries: `warehouse.get_balance`

Поле можно не переименовывать ради минимального diff, но реализация должна обращаться с ним как с action identifier.

### 3.2 Query authorization обязательна

Текущий runtime уже вызывает `PermissionChecker` из `QueryPipeline`.
Поэтому реализация обязана:

- регистрировать query actions в manifests
- выдавать grants на query actions явно
- покрыть это integration tests через `QueryPipeline`

### 3.3 Unknown roles

Выбранное поведение для реализации:

- `Claims.to_request_context()` просто копирует `roles: Vec<String>`
- `JwtPermissionChecker` не делает никаких special-cases и просто вызывает registry
- неизвестная роль не матчится ни на одно grant правило и therefore denied
- `/dev/token` валидирует входные роли и отклоняет unknown role с `400`

---

## 4. Обязательные изменения сверх базового плана

Помимо основного рефакторинга, обязательно сделать следующее.

### 4.1 Обновить `crates/test_support`

Нужно заменить все использования `default_erp_permissions()` на helper, который строит `PermissionRegistry` из реальных manifests warehouse + catalog.

Обязательно обновить:

- `command_pipeline()`
- `query_pipeline()`

Новая shared helper функция должна быть примерно такой:

```rust
pub fn test_permission_registry() -> Arc<auth::PermissionRegistry> {
    use kernel::security::PermissionRegistrar;

    let wh = warehouse::...permission_manifest();
    let cat = catalog::...permission_manifest();

    Arc::new(
        auth::PermissionRegistry::from_manifests_validated(vec![wh, cat])
            .expect("test manifests must be valid")
    )
}
```

### 4.2 Обновить `/dev/token`

Нельзя больше делать `filter_map(Role::from_str_opt)`.

Нужно:

1. собрать registry или использовать уже собранный registry
2. считать known roles = platform roles + registry roles
3. если в body есть unknown role, вернуть `400 Bad Request`
4. если все роли валидны, выпустить JWT с `Vec<String>`

### 4.3 Проверить покрытие всех используемых actions

После добавления manifests нужно убедиться, что все реально используемые action names присутствуют в registry:

- все `command_name()` из warehouse/catalog
- все `query_name()` из warehouse/catalog

Если action используется в runtime, но отсутствует в manifest, это bug.

---

## 5. Рекомендуемая последовательность реализации

### Шаг 1. kernel

Добавить `crates/kernel/src/security.rs` или `security/mod.rs` и re-export в `lib.rs`.

Обязательные типы:

- `PermissionRegistrar`
- `PermissionManifest`
- `RoleDef`
- `PermissionDef`
- `RoleGrant`
- `platform_roles::{ADMIN, VIEWER}`

### Шаг 2. auth registry

Создать `crates/auth/src/registry.rs`.

Обязательные API:

- `from_manifests()`
- `from_manifests_validated()`
- `validate()`
- `is_allowed()`
- `roles()`
- `permissions()`

`validate()` обязана проверять:

1. duplicate role code across manifests
2. duplicate permission/action code across manifests
3. grant references unknown role
4. grant references unknown action
5. invalid wildcard format
6. namespace mismatch
7. platform role collision (`admin`, `viewer` cannot be defined by BC)
8. grants на platform roles допустимы, но только для action своего BC namespace

### Шаг 3. claims + jwt

- удалить `enum Role`
- `Claims.roles -> Vec<String>`
- `JwtService::issue(..., Vec<String>)`

### Шаг 4. checker

- перевести `JwtPermissionChecker` на `Arc<PermissionRegistry>`
- убрать все `from_str_opt` / enum conversions
- использовать `registry.is_allowed(&ctx.roles, action_name)`

### Шаг 5. удалить legacy RBAC

- удалить `crates/auth/src/rbac.rs`
- обновить `auth/lib.rs`
- обновить middleware tests

### Шаг 6. manifests в BC

#### Warehouse

Manifest обязан включать:

- roles: `warehouse_manager`, `warehouse_operator`
- command actions: `warehouse.receive_goods`, ...
- query actions: минимум `warehouse.get_balance`
- grants:
  - `warehouse_manager` -> `warehouse.*`
  - `warehouse_operator` -> explicit warehouse commands + queries
  - `viewer` -> warehouse queries only

#### Catalog

Manifest обязан включать:

- roles: `catalog_manager`
- command actions: `catalog.create_product`
- query actions: минимум `catalog.get_product`
- grants:
  - `catalog_manager` -> `catalog.*`
  - `viewer` -> catalog queries only

### Шаг 7. gateway

В startup:

1. собрать manifests
2. вызвать `PermissionRegistry::from_manifests_validated(...)`
3. создать checker
4. передать checker в `CommandPipeline`
5. передать checker в `QueryPipeline`
6. использовать ту же policy model для `/dev/token`

### Шаг 8. test_support

Обязательный отдельный шаг. Не оставлять legacy wiring.

### Шаг 9. integration + e2e verification

Сначала validation tests, потом pipeline/query tests, затем workspace sweep.

---

## 6. Обязательные тесты

### 6.1 Registry tests

Минимальный набор:

1. single manifest -> grants built correctly
2. two manifests -> merged correctly
3. admin -> any action allowed
4. viewer + BC grants -> query allowed
5. viewer -> command denied
6. BC role -> own namespace allowed
7. BC role -> foreign namespace denied
8. multiple roles -> union semantics
9. unknown role -> denied
10. empty roles -> denied
11. wildcard valid match
12. wildcard foreign namespace mismatch
13. validate unknown role
14. validate unknown action
15. validate invalid wildcard
16. validate duplicate role code
17. validate duplicate action code
18. validate namespace mismatch
19. validate platform role collision
20. valid manifests -> Ok

### 6.2 Checker / pipeline tests

Минимальный набор:

1. `admin` command allowed
2. `warehouse_operator` own command allowed
3. `warehouse_operator` foreign command denied
4. `viewer` command denied
5. `viewer` own query allowed through `QueryPipeline`
6. `viewer` foreign query denied through `QueryPipeline` if not granted
7. multiple roles union works
8. unknown role denied

### 6.3 `/dev/token` tests

Минимальный набор:

1. valid roles -> token issued
2. unknown role -> `400`
3. mixed valid + unknown roles -> `400`

### 6.4 Manifest tests in BC crates

Для каждого BC:

- `bc_code` correct
- all actions start with correct namespace
- viewer grants contain only query/read actions
- all actions used by public handlers are present in manifest

### 6.5 test_support tests or verification

Нужно проверить, что shared test wiring использует registry from manifests, а не hardcoded RBAC map.

---

## 7. Обязательные grep / verification checks

После реализации должны выполняться все проверки.

### Legacy removal

```bash
grep -rn "enum Role" crates/ --include="*.rs"
grep -rn "default_erp_permissions" crates/ --include="*.rs"
grep -rn "from_str_opt" crates/ --include="*.rs"
test ! -f crates/auth/src/rbac.rs
```

Ожидаемый результат:

- 0 matches для grep
- `rbac.rs` не существует

### Coverage checks

```bash
rg -n "fn command_name\(|fn query_name\(" crates/warehouse crates/catalog
rg -n "warehouse\.|catalog\." crates/warehouse/src/registrar.rs crates/catalog/src/registrar.rs
```

Ожидаемый результат:

- все runtime action names покрыты manifests

### Final build and test

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

---

## 8. Что запрещено делать в реализации

Запрещено:

- возвращать legacy `Role` enum в любой форме
- оставлять `default_erp_permissions()` даже как compatibility layer
- special-case'ить `viewer` в checker как implicit read-all
- silently ignore unknown roles в `/dev/token`
- тестировать query auth прямым вызовом query handler вместо `QueryPipeline`
- оставлять `test_support` на старом hardcoded RBAC wiring
- разрешать BC определять `admin` или `viewer` как свои роли
- разрешать BC регистрировать чужой namespace

---

## 9. Definition of Done

Задача считается выполненной только если:

1. `enum Role` удален полностью
2. `rbac.rs` удален полностью
3. registry строится из manifests BC
4. gateway стартует через validated manifests
5. `CommandPipeline` и `QueryPipeline` используют одну policy model
6. `viewer` работает только через explicit query grants
7. unknown roles denied в runtime и rejected в `/dev/token`
8. `test_support` использует registry from manifests
9. все workspace tests проходят
10. clippy проходит без warnings
