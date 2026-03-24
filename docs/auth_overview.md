# Как работает авторизация в ERP

> Layer 4a — JWT + RBAC, без базы данных

---

## Общая схема

```
HTTP-запрос                   Pipeline (runtime)
     │                              │
     ▼                              │
┌─────────────┐                     │
│ middleware.rs│  ← извлекает JWT    │
│  из header  │     из Authorization│
└─────┬───────┘                     │
      │ Bearer token                │
      ▼                             │
┌─────────────┐                     │
│  jwt.rs     │  ← verify()         │
│  JwtService │    проверяет подпись │
│             │    + срок годности   │
└─────┬───────┘                     │
      │ Claims                      │
      ▼                             │
┌─────────────┐                     │
│  claims.rs  │  ← to_request_     │
│  Claims →   │    context()        │
│  Req.Context│    парсит UUID      │
└─────┬───────┘                     │
      │ RequestContext              │
      │ (с roles: Vec<String>)      │
      ▼                             ▼
  extensions_mut()          ┌───────────────┐
  кладёт в request          │  checker.rs   │
                            │  JwtPermission│
                            │  Checker      │
                            │               │
                            │ ctx.roles →   │
                            │ Role enum →   │
                            │ rbac.rs       │
                            └───────┬───────┘
                                    │
                                    ▼
                            ┌───────────────┐
                            │  rbac.rs      │
                            │  PermissionMap│
                            │               │
                            │ role + cmd →  │
                            │ allowed?      │
                            └───────────────┘
```

---

## Компоненты

### 1. Role enum (`claims.rs`)

Закрытый набор ролей ERP:

| Role | Назначение |
|------|-----------|
| `Admin` | Полный доступ ко всем командам |
| `WarehouseManager` | Все складские операции (`warehouse.*`) |
| `WarehouseOperator` | Только базовые складские операции (приём, отгрузка, перемещение, резерв) |
| `Accountant` | Финансовые операции (`finance.*`) |
| `SalesManager` | Продажи (`sales.*`) |
| `Viewer` | Только чтение — команды запрещены |

Роли сериализуются в snake_case (`WarehouseManager` → `"warehouse_manager"`) через `#[serde(rename_all = "snake_case")]`.

В kernel (`RequestContext`) роли хранятся как `Vec<String>` — kernel не знает о конкретных ролях. Конвертация `String → Role` происходит через `Role::from_str_opt()` в checker.

### 2. Claims — содержимое JWT (`claims.rs`)

```rust
pub struct Claims {
    pub sub: String,         // user_id (UUID)
    pub tenant_id: String,   // tenant_id (UUID)
    pub roles: Vec<Role>,    // роли
    pub exp: usize,          // expiration (Unix timestamp)
    pub iat: usize,          // issued at
}
```

Метод `to_request_context()` конвертирует Claims в `RequestContext`:
- Парсит `sub` и `tenant_id` из строки в UUID
- Конвертирует `Vec<Role>` в `Vec<String>` (через serde)
- При невалидном UUID → `AppError::Unauthorized`

### 3. JwtService — выдача и проверка токенов (`jwt.rs`)

```rust
pub struct JwtService {
    encoding_key: EncodingKey,   // для подписи
    decoding_key: DecodingKey,   // для проверки
    token_ttl: Duration,         // время жизни
}
```

Два метода:

- **`issue(user_id, tenant_id, roles) → String`** — создаёт JWT, подписывает HMAC-SHA256
- **`verify(token) → Claims`** — проверяет подпись + expiration, возвращает Claims

Алгоритм: HS256 (симметричный ключ). Ключ передаётся при создании `JwtService::new(secret, ttl)`.

Ошибки verify: невалидная подпись, истёкший токен, битый формат → `AppError::Unauthorized`.

### 4. PermissionMap — RBAC маппинг (`rbac.rs`)

```rust
pub struct PermissionMap {
    grants: HashMap<Role, HashSet<&'static str>>,  // роль → команды
    admin_roles: HashSet<Role>,                     // роли-суперадмины
}
```

Логика проверки `is_allowed(roles, command_name)`:

1. Если хотя бы одна роль в `admin_roles` → **разрешено** (любая команда)
2. Для каждой роли проверяем `grants`:
   - Точное совпадение: `"warehouse.receive_goods"` == `"warehouse.receive_goods"`
   - Wildcard: `"warehouse.*"` → `command_name.starts_with("warehouse.")` → **разрешено**
3. Ни одна роль не подошла → **запрещено**

Преднастроенный маппинг `default_erp_permissions()`:

| Роль | Разрешения |
|------|-----------|
| Admin | всё (grant_all) |
| WarehouseManager | `warehouse.*` (wildcard) |
| WarehouseOperator | 5 конкретных команд: `receive_goods`, `ship_goods`, `transfer_stock`, `reserve_stock`, `release_reservation` |
| Accountant | `finance.*` |
| SalesManager | `sales.*` |
| Viewer | ничего (queries не проходят через PermissionChecker) |

Маппинг **статический** (в коде, не в БД). Для dynamic RBAC позже — тот же интерфейс `PermissionMap`, но данные из БД.

### 5. JwtPermissionChecker — адаптер для Pipeline (`checker.rs`)

Реализует trait `PermissionChecker` из `runtime::ports`:

```rust
#[async_trait]
impl PermissionChecker for JwtPermissionChecker {
    async fn check_permission(&self, ctx: &RequestContext, command_name: &str) -> Result<(), AppError>;
}
```

Алгоритм:
1. Извлекает `ctx.roles` (Vec<String>)
2. Конвертирует строки → `Role` enum через `Role::from_str_opt()` (неизвестные роли игнорируются)
3. Вызывает `permission_map.is_allowed(&roles, command_name)`
4. `false` → `Err(AppError::Unauthorized("no permission for command '...'"))`

Подставляется в `CommandPipeline` вместо `NoopPermissionChecker`.

### 6. Axum middleware (`middleware.rs`)

```rust
pub async fn auth_middleware(request: Request, next: Next, jwt_service: Arc<JwtService>) -> Response
```

Цепочка:
1. Извлечь `Authorization: Bearer <token>` из header
2. Нет header или не Bearer → 401
3. `jwt_service.verify(token)` → Claims или 401
4. `claims.to_request_context()` → RequestContext или 401
5. `request.extensions_mut().insert(ctx)` — положить в extensions
6. `next.run(request).await` — передать дальше

**AppError → HTTP status mapping** (через `AppErrorResponse` newtype):

| AppError | HTTP Status |
|----------|------------|
| `Unauthorized` | 401 |
| `Validation` | 400 |
| `Domain(NotFound)` | 404 |
| `Domain(InsufficientStock/BusinessRule/...)` | 422 |
| `Internal` | 500 |

Ответ — JSON:
```json
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "..."
  }
}
```

---

## Поток данных: от HTTP до handler

```
1. HTTP: Authorization: Bearer eyJhbGci...
                │
2. middleware:  │→ jwt_service.verify("eyJhbGci...") → Claims
                │→ claims.to_request_context() → RequestContext { roles: ["warehouse_operator"] }
                │→ request.extensions_mut().insert(ctx)
                │
3. Pipeline:   │→ checker.check_permission(ctx, "warehouse.receive_goods")
                │→ ctx.roles → ["warehouse_operator"] → Role::WarehouseOperator
                │→ permission_map.is_allowed([WarehouseOperator], "warehouse.receive_goods")
                │→ exact match found → Ok(())
                │
4. Handler:    │→ вызывается только если авторизация прошла
```

---

## Связь между crate'ами

```
kernel                          runtime
  │ RequestContext                │ PermissionChecker trait (порт)
  │ (roles: Vec<String>)         │ NoopPermissionChecker (заглушка)
  │                               │
  └───────────┬───────────────────┘
              │
              ▼
           auth
  ├── claims.rs     Role enum, Claims struct
  ├── jwt.rs        JwtService (issue/verify)
  ├── rbac.rs       PermissionMap (role → commands)
  ├── checker.rs    JwtPermissionChecker (impl PermissionChecker)
  └── middleware.rs auth_middleware (axum, для Gateway)
```

Ключевой принцип: **Dependency Inversion**. Runtime определяет абстрактный порт `PermissionChecker`. Auth предоставляет конкретную реализацию `JwtPermissionChecker`. Pipeline не знает про JWT, роли или RBAC — он вызывает `checker.check_permission()` через trait object `Arc<dyn PermissionChecker>`.

---

## Изменения в kernel

В `RequestContext` добавлено поле `pub roles: Vec<String>` — список ролей как строки. Kernel не зависит от auth: он не знает о `Role` enum, хранит роли как `Vec<String>`. Конвертация строка ↔ enum происходит на стороне auth.

По умолчанию `roles` = пустой вектор (в `RequestContext::new()`).
