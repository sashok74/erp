# Layer 4a — Auth: JWT + RBAC (без БД)
> Подробное ТЗ | ERP Pilot on Rust
> Дата: 2026-03-23 | Привязка: ADR v1 (Simple JWT auth на старте), BCR (Command Pipeline)
> Предусловие: Layer 1 (Kernel), Layer 3a (Event Bus), Layer 5a (Runtime + Pipeline) выполнены

---

## Зачем этот слой

Auth — первая **реальная реализация порта** из Layer 5a. В runtime мы определили `PermissionChecker` trait и тестировали Pipeline с `NoopPermissionChecker` (всё разрешено). Теперь подставляем настоящую реализацию: JWT для аутентификации, RBAC для авторизации.

Что делает этот слой:

1. **JWT Service** — выпуск и проверка токенов. Токен содержит: user_id, tenant_id, roles, expiration
2. **RBAC** — проверка «может ли пользователь с ролью X выполнить команду Y». Enum-based, без БД
3. **PermissionChecker** — реализация порта из runtime. Подставляется в Pipeline вместо Noop
4. **Axum middleware** — извлечение JWT из HTTP header'а → `RequestContext`. Пригодится в Layer 7 (Gateway)

### Почему без БД

Роли хранятся в JWT claims, не в БД. Таблица permissions не нужна: маппинг role → allowed commands — enum + HashSet в коде. Для MVP этого достаточно. Позже (если понадобится dynamic RBAC) — таблица в PostgreSQL, но архитектура не изменится: `PermissionChecker` trait останется тем же.

---

## Что мы изучим в этом слое (Rust)

| Концепция | Где применяется | Зачем в Rust |
|-----------|----------------|--------------|
| `jsonwebtoken` crate | JWT issue/verify | HMAC-SHA256, claims, expiration |
| `enum` + `#[derive]` | Role, Permission | Перечисления как first-class citizens |
| `HashSet` / `HashMap` | Role → Permissions mapping | Быстрый lookup O(1) |
| `impl Trait for Struct` | JwtPermissionChecker impl PermissionChecker | Подстановка реализации в Pipeline |
| `chrono` для времени | JWT exp/iat claims | Unix timestamps |
| Axum extractors | `TypedHeader<Authorization<Bearer>>` | Извлечение данных из HTTP request |
| Middleware pattern | `axum::middleware::from_fn` | Цепочка обработки до route handler |
| `Extension` / `State` | RequestContext в request extensions | Передача данных через middleware chain |
| `#[cfg(test)]` conditional | Тестовый хелпер для генерации токенов | Код только для тестов |

---

## Структура файлов после выполнения

```
crates/auth/src/
├── lib.rs              ← pub mod + re-exports
├── jwt.rs              ← JwtService: issue + verify
├── claims.rs           ← Claims struct, Role enum
├── rbac.rs             ← PermissionMap: role → commands
├── checker.rs          ← JwtPermissionChecker (impl PermissionChecker)
└── middleware.rs        ← Axum middleware: header → RequestContext
```

---

## Задача 4a.1 — Claims + Role enum

### Зачем в ERP

JWT токен несёт claims — утверждения о пользователе. Для ERP минимальный набор: кто (user_id), в каком тенанте (tenant_id), какие роли (roles), когда истекает (exp). Роли — enum, не строки. Компилятор ловит опечатки: `Role::Warehouseman` vs `"warehoseman"`.

### Зачем в Rust (что учим)

**Enum как closed set** — `Role` содержит все возможные роли. Добавление новой роли = добавление варианта в enum + обработка во всех `match`. Компилятор предупредит о необработанных вариантах (exhaustive matching).

**`#[serde(rename_all = "snake_case")]`** — в JWT claims роли сериализуются как строки: `"warehouse_manager"`, не `"WarehouseManager"`. serde делает маппинг автоматически.

### Требования к коду

**Файл: `crates/auth/src/claims.rs`**

1. **`Role`** — enum ролей ERP:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum Role {
       Admin,              // полный доступ
       WarehouseManager,   // управление складом
       WarehouseOperator,  // операции на складе (приёмка, отгрузка)
       Accountant,         // финансы, GL
       SalesManager,       // продажи
       Viewer,             // только чтение
   }
   ```

2. **`Claims`** — содержимое JWT:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Claims {
       pub sub: String,            // user_id (UUID as string)
       pub tenant_id: String,      // tenant_id (UUID as string)
       pub roles: Vec<Role>,
       pub exp: usize,             // expiration (unix timestamp)
       pub iat: usize,             // issued at
   }
   ```
   - `to_request_context(&self) -> Result<RequestContext, AppError>` — парсит sub/tenant_id в UUID, создаёт RequestContext

### Тесты

- Role сериализуется: `WarehouseManager` → `"warehouse_manager"`
- Role десериализуется обратно
- Claims::to_request_context() — валидные UUID → Ok
- Claims::to_request_context() — невалидный UUID → Err

---

## Задача 4a.2 — JwtService: выпуск и проверка токенов

### Зачем в ERP

JWT Service — центральный компонент аутентификации. `issue()` — создать токен при логине. `verify()` — проверить токен при каждом запросе. Токен подписывается HMAC-SHA256 секретным ключом. Без ключа подделать невозможно.

### Зачем в Rust (что учим)

**`jsonwebtoken` crate** — стандартная библиотека для JWT в Rust. `encode()` создаёт токен, `decode()` проверяет подпись и парсит claims. Если подпись невалидна или токен истёк — `Err`.

**Secret management** — ключ передаётся через конфигурацию (env var), не хардкодится. `JwtService` хранит `EncodingKey` и `DecodingKey`, созданные из секрета.

**`Result<Claims, AppError>`** — verify может вернуть: expired, invalid signature, malformed. Все маппятся в `AppError::Unauthorized`.

### Требования к коду

**Файл: `crates/auth/src/jwt.rs`**

```rust
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    token_ttl: chrono::Duration,  // время жизни токена
}
```

Методы:
- `new(secret: &str, token_ttl: chrono::Duration) -> Self`
- `issue(&self, user_id: &UserId, tenant_id: &TenantId, roles: Vec<Role>) -> Result<String, AppError>` — создаёт JWT с claims, подписывает HMAC-SHA256
- `verify(&self, token: &str) -> Result<Claims, AppError>` — проверяет подпись, expiration, парсит claims. Ошибки → `AppError::Unauthorized`

### Тесты

- issue → verify round-trip: claims совпадают
- verify с невалидным токеном → Err(Unauthorized)
- verify с другим секретом → Err(Unauthorized)
- verify с истёкшим токеном → Err(Unauthorized) (создать токен с ttl = 0 или negative)

---

## Задача 4a.3 — RBAC: маппинг Role → разрешённые команды

### Зачем в ERP

RBAC (Role-Based Access Control): каждая роль имеет набор разрешённых команд. `WarehouseOperator` может `warehouse.receive_goods` и `warehouse.ship_goods`, но не `warehouse.adjust_inventory` (это для Manager). Admin может всё.

Маппинг статический — в коде, не в БД. Для MVP достаточно. Для production — dynamic RBAC из БД, но trait `PermissionChecker` не изменится.

### Зачем в Rust (что учим)

**`HashMap<Role, HashSet<&'static str>>`** — каждая роль → набор command_name строк. Lookup: `map.get(&role).map(|perms| perms.contains(command_name))`.

**`&'static str`** — command names известны при компиляции (`"warehouse.receive_goods"`). `'static` — живут всю программу, не нужно аллоцировать String.

**Builder pattern** — `PermissionMap::builder().role(Admin).allow_all().role(Operator).allow(&["cmd1", "cmd2"]).build()` — удобная конструкция маппинга.

### Требования к коду

**Файл: `crates/auth/src/rbac.rs`**

```rust
pub struct PermissionMap {
    rules: HashMap<Role, HashSet<&'static str>>,
    admin_roles: HashSet<Role>,  // роли с полным доступом
}
```

Методы:
- `new() -> Self` — пустой маппинг
- `grant(role: Role, commands: &[&'static str]) -> &mut Self` — добавить разрешения для роли
- `grant_all(role: Role) -> &mut Self` — роль-администратор (может всё)
- `is_allowed(&self, roles: &[Role], command_name: &str) -> bool` — проверить хотя бы одна роль имеет доступ

Также: **`default_erp_permissions() -> PermissionMap`** — преднастроенный маппинг для ERP:

| Роль | Разрешённые команды |
|------|-------------------|
| Admin | всё |
| WarehouseManager | warehouse.* |
| WarehouseOperator | warehouse.receive_goods, warehouse.ship_goods, warehouse.transfer_stock, warehouse.reserve_stock, warehouse.release_reservation |
| Accountant | finance.* |
| SalesManager | sales.* |
| Viewer | (нет команд — только queries, которые не проходят через PermissionChecker) |

Wildcard `"warehouse.*"` — если command_name начинается с `"warehouse."`, разрешено.

### Тесты

- Admin → is_allowed для любой команды = true
- WarehouseOperator → warehouse.receive_goods = true
- WarehouseOperator → warehouse.adjust_inventory = false
- WarehouseOperator → finance.post_journal = false
- Viewer → warehouse.receive_goods = false
- Множественные роли: [Viewer, WarehouseOperator] → warehouse.receive_goods = true (хотя бы одна разрешает)

---

## Задача 4a.4 — JwtPermissionChecker: реализация порта

### Зачем в ERP

Это ключевой момент: мы **подставляем реальную реализацию** вместо `NoopPermissionChecker`. Pipeline не изменился — он по-прежнему вызывает `self.auth.check_permission(ctx, cmd_name)`. Но теперь вместо «всё разрешено» — проверка JWT claims + RBAC маппинг.

### Зачем в Rust (что учим)

**`impl Trait for Struct`** — `JwtPermissionChecker` реализует `PermissionChecker` из runtime::ports. Это Dependency Inversion в действии: runtime определил порт, auth предоставляет адаптер.

**Trait object substitution** — в Pipeline: `Arc<dyn PermissionChecker>`. Подставляем `Arc::new(JwtPermissionChecker::new(...))` вместо `Arc::new(NoopPermissionChecker)`. Тип Pipeline не изменился.

### Требования к коду

**Файл: `crates/auth/src/checker.rs`**

```rust
/// Реализация PermissionChecker из runtime::ports.
/// Проверяет роли из RequestContext против PermissionMap.
///
/// RequestContext.user_id уже извлечён из JWT middleware'ом.
/// Здесь проверяем: имеет ли пользователь роль, разрешающую команду.
pub struct JwtPermissionChecker {
    permission_map: PermissionMap,
    jwt_service: Arc<JwtService>,
}
```

Реализация `PermissionChecker`:
- `check_permission(ctx, command_name)`:
  1. Получить roles из ctx (нужно расширить RequestContext или передавать roles отдельно)
  2. `permission_map.is_allowed(&roles, command_name)`
  3. false → `Err(AppError::Unauthorized(format!("No permission for {command_name}")))`

**Вопрос дизайна: где хранить roles?**

Два варианта:

**Вариант A:** Расширить `RequestContext` в kernel, добавив `roles: Vec<Role>`. Минус: kernel зависит от auth (Role enum).

**Вариант B (рекомендуемый):** Хранить roles в отдельной структуре `AuthContext`, которая передаётся через axum Extensions рядом с RequestContext. Checker получает оба из request.

Для текущего Layer (без axum request): checker принимает roles как параметр или хранит маппинг `UserId → Roles` (in-memory, заполняется из JWT при каждом запросе).

**Прагматичное решение для MVP:** добавить `pub roles: Vec<String>` в `RequestContext` (строки, не enum — kernel не знает о конкретных ролях). Checker конвертирует строки в `Role` enum.

### Тесты

- Checker с Admin в roles → check_permission для любой команды = Ok
- Checker с WarehouseOperator → warehouse.receive_goods = Ok
- Checker с WarehouseOperator → finance.post_journal = Err(Unauthorized)
- Checker с пустыми roles → Err(Unauthorized)
- **Integration с Pipeline:** заменить NoopPermissionChecker на JwtPermissionChecker → Pipeline с unauthorized user → Err, handler не вызван

---

## Задача 4a.5 — Axum middleware: JWT header → RequestContext

### Зачем в ERP

HTTP запрос приходит с заголовком `Authorization: Bearer <token>`. Middleware:
1. Извлекает токен из header
2. Вызывает `jwt_service.verify(token)` → Claims
3. Создаёт `RequestContext` из Claims
4. Кладёт в axum Extensions — доступен всем route handler'ам

Без middleware каждый handler должен был бы парсить JWT сам. Middleware делает это один раз, централизованно.

### Зачем в Rust (что учим)

**Axum middleware** — `axum::middleware::from_fn_with_state`. Функция принимает `Request`, возвращает `Response`. Между ними — проверка JWT. Если токен невалиден — 401 без вызова handler'а.

**`Request::extensions_mut()`** — axum позволяет прикреплять произвольные данные к request. Middleware кладёт `RequestContext`, handler извлекает через `Extension<RequestContext>`.

**`TypedHeader<Authorization<Bearer>>`** — типизированное извлечение Authorization header. axum-extra парсит header за нас.

### Требования к коду

**Файл: `crates/auth/src/middleware.rs`**

```rust
/// Axum middleware: извлекает JWT из Authorization header,
/// проверяет подпись, создаёт RequestContext.
///
/// Используется в Layer 7 (Gateway). Определяем здесь,
/// потому что логика — часть auth, не gateway.
pub async fn auth_middleware(
    State(jwt_service): State<Arc<JwtService>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 1. Извлечь Bearer token из header
    // 2. jwt_service.verify(token) → Claims
    // 3. Claims → RequestContext
    // 4. req.extensions_mut().insert(ctx)
    // 5. next.run(req).await
    // При ошибке: 401 Unauthorized
}
```

Также: **`impl IntoResponse for AppError`** — чтобы AppError автоматически конвертировался в HTTP response с правильным status code:
- `AppError::Unauthorized` → 401
- `AppError::Validation` → 400
- `AppError::Domain(NotFound)` → 404
- `AppError::Domain(_)` → 422 Unprocessable Entity
- `AppError::Internal` → 500

### Тесты

- Запрос с валидным JWT → next вызван, RequestContext в extensions
- Запрос без header → 401
- Запрос с невалидным токеном → 401
- Запрос с истёкшим токеном → 401
- AppError::Unauthorized → status 401
- AppError::Domain(NotFound) → status 404

---

## Задача 4a.6 — Финальная сборка: lib.rs + полная проверка

### Требования к коду

**Файл: `crates/auth/src/lib.rs`**

```rust
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Auth — аутентификация (JWT) и авторизация (RBAC).
//!
//! Реализует PermissionChecker из runtime::ports.
//! JWT claims содержат user_id, tenant_id, roles.
//! RBAC маппинг role → commands — статический, в коде.
//!
//! Middleware для axum: Authorization header → RequestContext.

pub mod checker;
pub mod claims;
pub mod jwt;
pub mod middleware;
pub mod rbac;

pub use checker::JwtPermissionChecker;
pub use claims::{Claims, Role};
pub use jwt::JwtService;
pub use middleware::auth_middleware;
pub use rbac::{default_erp_permissions, PermissionMap};
```

**Обновить `crates/auth/Cargo.toml`:**

```toml
[package]
name = "auth"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
kernel = { workspace = true }
runtime = { workspace = true }        # для PermissionChecker trait
event_bus = { workspace = true }       # EventEnvelope в UnitOfWork (transitive)
jsonwebtoken = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
anyhow = { workspace = true }
axum = { workspace = true }
axum-extra = { workspace = true }
tower = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

### Обновить RequestContext в kernel

Добавить `pub roles: Vec<String>` в `RequestContext` (строки — kernel не знает о конкретных ролях auth). Обновить `RequestContext::new()` — roles = `vec![]` по умолчанию. Это минимальное изменение kernel.

### Финальная проверка

```bash
cargo build --workspace
cargo test -p auth
cargo test -p runtime       # убедиться что Pipeline по-прежнему работает
cargo test -p kernel        # убедиться что RequestContext по-прежнему работает
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
just check
```

---

## Сводка: что получаем после Layer 4a

| Файл | Содержание | Тесты |
|------|------------|-------|
| `claims.rs` | Claims struct, Role enum | serde round-trip, to_request_context |
| `jwt.rs` | JwtService: issue + verify | round-trip, invalid/expired tokens |
| `rbac.rs` | PermissionMap: role → commands | Admin всё, operator частично, viewer ничего |
| `checker.rs` | JwtPermissionChecker (impl PermissionChecker) | integration с Pipeline |
| `middleware.rs` | Axum auth middleware + AppError → Response | valid/invalid/missing JWT, status codes |
| `lib.rs` | Modules + re-exports | integration compile |

### Чему научились (Rust)

- **`jsonwebtoken`** — HMAC-SHA256, claims, encode/decode
- **Enum-based permissions** — Role enum + HashSet для O(1) lookup
- **`impl Trait for Struct`** — подстановка реализации порта
- **Axum middleware** — `from_fn_with_state`, Extensions, TypedHeader
- **`IntoResponse`** — AppError → HTTP status codes
- **`&'static str`** — zero-cost строки для command names
- **Wildcard matching** — `"warehouse.*"` через `starts_with`

### Связь с архитектурой ERP

| Архитектурный элемент | Где реализовано |
|----------------------|-----------------|
| Simple JWT auth (ADR v1) | jwt.rs — HMAC-SHA256 |
| RBAC на старте | rbac.rs — статический маппинг |
| PermissionChecker порт (Layer 5a) | checker.rs — первая реализация |
| Auth middleware (Layer 7 готовность) | middleware.rs — для Gateway |
| Multi-tenancy | tenant_id в JWT claims → RequestContext |

### Что завершено после Layer 4a

**Phase 1 полностью готова.** Все абстракции на месте:
- Kernel (Platform SDK) ✅
- Event Bus (traits + InProcessBus) ✅
- Command Pipeline (traits + stubs) ✅
- Auth (JWT + RBAC + real PermissionChecker) ✅

Pipeline можно тестировать с реальной авторизацией, без БД.

---

## Следующий шаг

Phase 1 завершена → **Phase 2: Layer 2 (Data Access)** — PostgreSQL, Clorinde, миграции, RLS, Unit of Work. Первая связь с реальной БД.
