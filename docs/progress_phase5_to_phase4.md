# ERP Pilot — Прогресс: Phase 5 → Phase 4

> Дата: 2026-03-25
> Контекст: Phase 0–3, 5 завершены. Следующий — Phase 4.

---

## Phase 5 — Gateway Assembly (✅ завершена)

### Что сделано

Собрали все crate'ы в работающий HTTP-сервер. Первый `cargo run`, первый `curl`.

### Компоненты

```
main.rs
  ├── AppConfig::from_env()          ← DATABASE_URL, JWT_SECRET, LISTEN_ADDR
  ├── PgPool::new() + health_check   ← connection pool
  ├── run_migrations(common/)         ← идемпотентные миграции при старте
  ├── run_migrations(warehouse/)
  ├── InProcessBus::new()             ← in-process event bus
  ├── JwtService::new()               ← HS256, TTL 8h
  ├── JwtPermissionChecker::new()     ← RBAC из static map
  ├── PgAuditLog::new()               ← audit после commit
  ├── NoopExtensionHooks              ← заглушка Lua/WASM
  ├── PgUnitOfWorkFactory::new()      ← BEGIN + RLS + COMMIT
  ├── CommandPipeline::new()           ← auth → hooks → TX → handler → commit → audit
  ├── OutboxRelay → tokio::spawn()    ← фоновый poll outbox → bus
  └── axum::serve(Router)             ← HTTP сервер
```

### Маршруты

| Метод | Путь | Auth | Описание |
|-------|------|------|----------|
| GET | `/health` | Нет | `{"status":"ok"}` |
| POST | `/dev/token` | Нет | Выдать JWT (только DEV_MODE) |
| POST | `/api/warehouse/receive` | JWT | Приёмка товара |
| GET | `/api/warehouse/balance?sku=...` | JWT | Запрос остатков |

### Проверки

- `cargo run -p gateway` → "ERP Gateway listening on 0.0.0.0:3000"
- Postman: полный flow (token → receive → balance)
- Без JWT → 401
- Viewer role → 403
- Operator role → 200
- Relay в фоне публикует events

### Архитектурные решения Phase 5

| Решение | Обоснование |
|---------|-------------|
| Config из env, не из файла | 12-factor app, просто для MVP |
| Миграции при старте (idempotent) | Не нужен отдельный migration tool |
| `POST /dev/token` за флагом DEV_MODE | Удобство разработки без отдельного auth-сервера |
| NoopExtensionHooks | Lua/WASM = Phase 7, заглушка не мешает |
| Один pipeline на все BC | Modular monolith — pipeline shared, handlers разные |

---

## Phase 4 — Catalog BC + Cross-Context Projection (🔶 следующий)

### Зачем

До Phase 4 — один BC. Архитектура не доказана для inter-BC взаимодействия. Phase 4 добавляет:

1. **Второй BC** по тому же шаблону → валидирует воспроизводимость паттерна
2. **Golden record + local projection** → Catalog владеет product, warehouse хранит копию
3. **Integration events через outbox** → ProductCreated из Catalog доходит до Warehouse
4. **Inbox dedup** → повторная доставка = idempotent upsert

### Сквозной сценарий

```
┌──────────┐    outbox     ┌───────┐    bus     ┌───────────┐
│ Catalog  │──────────────→│ Relay │───────────→│ Warehouse │
│          │  ProductCreated│       │            │ subscriber│
│ products │               └───────┘            │           │
│ (golden  │                                    │ product_  │
│  record) │                                    │ projections│
└──────────┘                                    └───────────┘
```

**Пользовательский flow:**

```
1. POST /api/catalog/products
   {"sku":"BOLT-42", "name":"Болт М8", "category":"Метизы", "unit":"шт"}
   → 200 {"product_id":"..."}
   → outbox: ProductCreated event

2. (relay poll, ~500ms)
   → bus.publish(ProductCreated)
   → warehouse ProductCreatedHandler
   → upsert warehouse.product_projections

3. POST /api/warehouse/receive
   {"sku":"BOLT-42", "quantity":100}
   → 200 {"new_balance":"100", "doc_number":"ПРХ-000001"}

4. GET /api/warehouse/balance?sku=BOLT-42
   → 200 {"sku":"BOLT-42", "balance":"100", "product_name":"Болт М8"}
```

### Новые таблицы

| Schema | Таблица | Назначение | Owner BC |
|--------|---------|-----------|----------|
| catalog | products | Golden record: sku, name, category, unit | Catalog |
| warehouse | product_projections | Read-only копия из Catalog | Warehouse |

### Новые SQL-файлы

| Файл | Запросы |
|------|---------|
| `queries/catalog/products.sql` | create_product, find_by_sku, find_by_id |
| `queries/warehouse/projections.sql` | upsert_product_projection, get_projection_by_sku |

### Новый crate: catalog

```
crates/catalog/
├── domain/
│   ├── aggregates.rs    ← Product (sku, name, category, unit)
│   ├── events.rs        ← ProductCreated
│   ├── value_objects.rs ← ProductName, Category
│   └── errors.rs        ← CatalogDomainError
├── application/
│   ├── commands/create_product.rs
│   └── queries/get_product.rs
├── infrastructure/
│   ├── repos.rs         ← PgProductRepo (через clorinde-gen)
│   └── routes.rs
└── module.rs
```

### Изменения в существующих crate'ах

| Crate | Файл | Изменение |
|-------|------|-----------|
| **auth** | `claims.rs` | Добавить `CatalogManager` role |
| **auth** | `rbac.rs` | `map.grant(CatalogManager, &["catalog.*"])` |
| **warehouse** | `infrastructure/event_handlers.rs` | НОВЫЙ: `ProductCreatedHandler` |
| **warehouse** | `infrastructure/repos.rs` | Добавить: projection upsert/get |
| **warehouse** | `application/queries/get_balance.rs` | Обогатить ответ `product_name` |
| **gateway** | `main.rs` | Подключить Catalog routes + subscribe handler |

### Маршруты после Phase 4

| Метод | Путь | Auth | BC |
|-------|------|------|----|
| GET | `/health` | Нет | — |
| POST | `/dev/token` | Нет | — |
| POST | `/api/catalog/products` | JWT | Catalog |
| GET | `/api/catalog/products?sku=...` | JWT | Catalog |
| POST | `/api/warehouse/receive` | JWT | Warehouse |
| GET | `/api/warehouse/balance?sku=...` | JWT | Warehouse |

### Архитектурные решения Phase 4

| Решение | Обоснование |
|---------|-------------|
| tenant_id в event payload | EventHandler trait получает `&Event`, не envelope. Pragmatic для MVP — все events tenant-scoped |
| Projection = eventual consistency | Warehouse может получить ReceiveGoods до ProductCreated. GetBalance вернёт `product_name: null` — ожидаемо |
| Свой Sku VO в каждом BC | BC не делят value objects. Catalog.Sku и Warehouse.Sku — разные типы |
| Upsert для projection | Idempotent by design. Повторная доставка ProductCreated не ломает данные |
| Warehouse НЕ вызывает Catalog API | Только через events + local projection. Ноль прямых зависимостей |

### Тесты

| Тест | Что проверяет |
|------|--------------|
| Catalog: happy path | CreateProduct → product в БД + outbox |
| Catalog: duplicate sku | → DuplicateSku error |
| Catalog: unauthorized | viewer → 403 |
| Catalog: GetProduct | correct data |
| **Cross-context E2E** | CreateProduct → relay → warehouse projection upserted |
| **GetBalance enriched** | balance + product_name из проекции |

---

## Сводка прогресса

```
Phase 0  ✅  kernel, event_bus, runtime, auth        76 unit tests
Phase 1  ✅  db, clorinde-gen                         PostgreSQL connected
Phase 2  ✅  relay, inbox, audit, seq_gen             Write flow closed
Phase 3  ✅  warehouse BC                             First business code, 7 integration tests
Phase 5  ✅  gateway                                  First cargo run, Postman tests
Phase 4  🔶  catalog BC + cross-context               ← СЛЕДУЮЩИЙ
Phase 6  ⬜  BC template extraction
Phase 7  ⬜  extensions (Lua/WASM) + thin UI
```

**Итого после Phase 5:** 10 crate'ов, 76+ unit тестов, 10+ integration тестов, работающий HTTP API.

**После Phase 4:** 11 crate'ов, 2 BC, первый inter-BC flow через events, projection pattern доказан.
