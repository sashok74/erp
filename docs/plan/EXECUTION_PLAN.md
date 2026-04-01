# ERP Execution Plan
> Единственный управляющий документ для порядка реализации.
> Все остальные docs — справочные. При конфликте — этот документ главный.
> Версия: 1.1 | Дата: 2026-03-24 | Статус: принят

---

## Статус документации

| Документ | Роль | Статус |
|----------|------|--------|
| **docs/EXECUTION_PLAN.md** | Что делать дальше, в каком порядке | **ACTIVE — this file** |
| **docs/engineering_invariants.md** | Правила, которые никогда не ослабляются | **ACTIVE** |
| docs/layer0_spec.md | Спецификация Layer 0 (workspace) | Archived — реализован, код = truth |
| docs/layer1_spec.md | Спецификация Layer 1 (kernel) | Archived — реализован, код = truth |
| docs/layer3a_spec.md | Спецификация Layer 3a (event bus) | Archived — реализован, код = truth |
| docs/layer5a_spec.md | Спецификация Layer 5a (runtime) | Archived — реализован, код = truth |
| docs/event_bus_architecture.md | Архитектурный обзор event bus | Reference — актуален |
| project: erp_architecture_diagrams_mermaid.md | 6 схем архитектуры | Reference — актуален |
| project: project_index.md | Индекс исследований (17 артефактов) | Reference — актуален |

**Правило:** для реализованных слоёв код — source of truth, не spec.
Spec'и сохраняются как историческая документация и учебный материал по Rust.
Новые слои получают spec только если нужен отдельный deep-dive (по запросу).

---

## Принцип: минимум бизнеса, максимум механизмов

Каждый архитектурный механизм работает с первого дня.
Бизнес-логика минимальна — ровно столько, сколько нужно для проверки всех механизмов.
Warehouse BC = reference implementation → шаблон → AI-агент генерирует остальные BC.

**Правило принятия решений:**
- «Пока без партий и резервов» — допустимо (упрощение бизнеса)
- «Пока без audit / без events / без RLS» — недопустимо (упрощение механизма)

---

## Стек данных: Clorinde + tokio-postgres

**Clorinde** (cornucopia-rs/clorinde) — кодогенератор: SQL-запросы в `.sql` файлах →
type-safe Rust-код на этапе сборки. Под капотом — `tokio-postgres` (async, не ORM).

Почему не sqlx:
- SQL явно в файлах, не в макросах — читаемость, ревью, AI-генерация
- Кодогенерация без подключения к БД при компиляции
- Прямой контроль над SQL — никакой магии, никакого query builder
- tokio-postgres под капотом — тот же уровень производительности

Что это меняет в существующем коде:
- Workspace Cargo.toml: `sqlx` → `tokio-postgres` + `deadpool-postgres` + `clorinde`
- Kernel ID types: `#[derive(sqlx::Type)]` → `impl ToSql/FromSql` из `postgres-types`
- Миграции: `refinery` (или кастомный runner), не `sqlx migrate`
- Запросы: `.sql` файлы в `queries/` → clorinde генерирует `mod queries`

---

## Текущее состояние

### Готово (фаза 0)

| Crate | Что реализовано | Тесты |
|-------|----------------|-------|
| kernel | TenantId, UserId, Command, DomainEvent, AggregateRoot, AppError, CloudEvent, RequestContext | 17 |
| event_bus | EventBus trait, InProcessBus, EventEnvelope, ErasedEventHandler, HandlerRegistry | 16 |
| runtime | CommandPipeline (auth→hooks→tx→handler→commit→audit), CommandHandler, QueryHandler, ports, stubs | 21 |
| auth | JwtService (HS256), RBAC PermissionMap (wildcard), JwtPermissionChecker, axum middleware | 22 |
| **Итого** | Все traits определены, pipeline работает со stubs, auth интегрирован | **76** |

### Необходимая правка в kernel перед фазой 1

```rust
// Было (kernel/src/types.rs):
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TenantId(Uuid);

// Станет:
use postgres_types::{ToSql, FromSql};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSql, FromSql)]
#[postgres(transparent)]
pub struct TenantId(Uuid);
```

Kernel Cargo.toml: `sqlx` → `postgres-types = { workspace = true }`.
Три типа (TenantId, UserId, EntityId) — derive swap. Тесты не меняются.

---

## Фаза 1 — PostgreSQL + реальный UnitOfWork

**Статус:** DONE -- PgPool, PgUnitOfWork, RLS, миграции common schema, Clorinde-запросы для платформенных таблиц
**Crate:** `db`
**Зависимости:** kernel, event_bus, tokio-postgres, deadpool-postgres, clorinde, refinery

### Задачи

**1.0** Правка kernel: sqlx::Type → postgres-types
- Cargo.toml: sqlx → postgres-types
- Три derive на ID newtypes
- `cargo test --workspace` — всё зелёное

**1.1** PgPool + конфигурация
- `deadpool_postgres::Pool` (connection pool, config из .env)
- Health check: `pool.get()` → `client.query("SELECT 1", &[])`

**1.2** SQL миграции (common schema)
```
migrations/common/
  V001__create_tenants.sql
  V002__create_outbox.sql
  V003__create_inbox.sql
  V004__create_audit_log.sql
  V005__create_domain_history.sql
  V006__enable_rls.sql
```
Runner: `refinery` с `tokio-postgres` backend.

**1.3** Clorinde-запросы для платформенных таблиц
```
crates/db/queries/
  outbox.sql      -- insert_outbox_entry, poll_unpublished, mark_published
  inbox.sql       -- try_insert_inbox, check_processed
  audit.sql       -- insert_audit_log
  history.sql     -- insert_domain_history
  tenants.sql     -- get_tenant, create_tenant
```

**1.4** PgUnitOfWork
- `PgUnitOfWorkFactory::begin(ctx)` → `pool.get()` → `BEGIN` → `SET LOCAL app.tenant_id`
- `PgUnitOfWork` хранит `tokio_postgres::Transaction<'_>`
  - `add_outbox_entry()` → clorinde `insert_outbox_entry` (в той же TX)
  - `commit()` → `transaction.commit()`
  - `rollback()` → `transaction.rollback()`

**1.5** RLS + тест изоляции
```sql
ALTER TABLE {table} ENABLE ROW LEVEL SECURITY;
ALTER TABLE {table} FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON {table}
  USING (tenant_id = current_setting('app.tenant_id')::uuid);
```

### Критерий готовности
```
begin → insert → commit → select = данные есть
rollback → select = пусто
tenant A insert → tenant B select = 0 rows
pipeline.execute() с PgUoW = OK
```

---

## Фаза 2 — Outbox Relay + Audit Writer + SeqGen

**Статус:** DONE -- Outbox relay, inbox dedup, PgAuditLog, domain history, PgSequenceGenerator, dead letter queue

### Задачи

**2.1** Outbox Relay (background tokio task)
- `clorinde::poll_unpublished(limit)` → deserialize → `EventBus::publish` → `mark_published`
- Retry 3x exponential backoff → `move_to_dlq`

**2.2** Inbox dedup
- `clorinde::try_insert_inbox(event_id)` → ON CONFLICT DO NOTHING → skip if already processed

**2.3** PgAuditLog implements `AuditLog`
- `clorinde::insert_audit_log(...)` → отдельный connection (после commit)

**2.4** Domain History
- `clorinde::insert_domain_history(old_state, new_state, entity_type, ...)`

**2.5** PgSequenceGenerator
- `clorinde::next_sequence_value(tenant_id, prefix)` + advisory lock

### Критерий готовности
```
E2E: command → handler → outbox → COMMIT → relay → bus → subscriber called
     → audit_log row → domain_history row → correlation chain intact
```

---

## Фаза 3 — Warehouse Vertical Slice

**Статус:** DONE -- InventoryItem aggregate, ReceiveGoods command, GetBalance query, HTTP endpoints, integration tests
**Crate:** `warehouse`

### Бизнес (минимальный)

- `InventoryItem` aggregate (id, sku, balance)
- `StockMovement` (append-only, balance_after)
- `Sku(String)`, `Quantity(BigDecimal)`
- `GoodsReceived { sku, quantity, warehouse_id }`
- Rule: balance >= 0
- **НЕТ:** партий, сроков, резервов, FIFO, зон, ячеек

### Структура (будущий шаблон!)

```
crates/warehouse/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── domain/
│   │   ├── aggregates.rs        ← InventoryItem
│   │   ├── events.rs            ← GoodsReceived
│   │   ├── value_objects.rs     ← Sku, Quantity
│   │   └── errors.rs
│   ├── application/
│   │   ├── commands/receive_goods.rs
│   │   └── queries/get_balance.rs
│   ├── infrastructure/
│   │   ├── repos.rs             ← calls clorinde-generated fns
│   │   ├── routes.rs            ← axum handlers
│   │   └── event_handlers.rs    ← подписки на чужие events
│   └── module.rs                ← WarehouseModule
├── queries/                     ← SQL для clorinde
│   ├── inventory.sql            ← upsert_balance, insert_movement, get_balance
│   └── projections.sql          ← upsert_item_projection, get_item_projection
├── migrations/warehouse/
│   ├── V001__create_inventory_items.sql
│   ├── V002__create_stock_movements.sql
│   ├── V003__create_inventory_balances.sql
│   ├── V004__create_item_projections.sql
│   └── V005__rls_warehouse.sql
├── tests/
│   ├── domain_tests.rs
│   └── integration_tests.rs
└── BC_CONTEXT.md
```

### Пример clorinde SQL (queries/inventory.sql)

```sql
--! upsert_balance : Balance
UPDATE warehouse.inventory_balances
SET balance = balance + :quantity, updated_at = now()
WHERE sku = :sku AND warehouse_id = :warehouse_id AND tenant_id = :tenant_id
RETURNING balance;

--! insert_movement
INSERT INTO warehouse.stock_movements
  (id, sku, warehouse_id, quantity, balance_after, movement_type, tenant_id, created_at)
VALUES (:id, :sku, :warehouse_id, :quantity, :balance_after, :movement_type, :tenant_id, now());

--! get_balance : Balance
SELECT balance FROM warehouse.inventory_balances
WHERE sku = :sku AND warehouse_id = :warehouse_id;
```

### Canonical write path

```
POST /api/warehouse/receive {sku, quantity}
  → auth middleware (JWT → RequestContext)
  → pipeline.execute(ReceiveGoodsHandler, cmd, ctx)
    → RBAC check (warehouse.receive_goods)
    → before_hook (Noop)
    → PgUoW BEGIN + SET LOCAL tenant_id
    → handler:
      → clorinde::get_item_projection(sku) → exists check
      → item.receive(qty) → GoodsReceived
      → clorinde::insert_movement(...)
      → clorinde::upsert_balance(...)
      → uow.add_outbox_entry(GoodsReceived)
    → COMMIT
    → audit + history
  → relay → bus → subscribers
  → HTTP 200
```

### Критерий готовности
```bash
curl -X POST /api/warehouse/receive \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"sku":"BOLT-42","quantity":100}'
# → 200 + balance=100 + outbox row + audit row + history row
```

---

## Фаза 4 — Cross-Context: Catalog -> Warehouse Projection

**Статус:** DONE -- Catalog BC (CreateProduct, GetProduct), cross-BC event projection, SKU validation in warehouse
**Crates:** `catalog` (новый), `warehouse` (handler + projection)

- Catalog: RegisterItem → ItemRegistered event
- Warehouse handler подписан → clorinde `upsert_item_projection`
- ReceiveGoods проверяет SKU через clorinde `get_item_projection`
- **Cargo.toml:** warehouse не зависит от catalog

### Критерий готовности
```
Register → event → projection → receive (known SKU) = OK
Receive (unknown SKU) = 422
No dep warehouse → catalog in Cargo.toml
```

---

## Фаза 5 — Gateway Assembly + Queries

**Статус:** DONE -- Gateway assembly with AppBuilder, query endpoints, auth layer, health check, outbox relay background task, BC-owned RBAC (PermissionRegistry)

Gateway собирает modules, query endpoints, auth layer, health check, outbox relay background task.

```bash
cargo run -p gateway
POST /api/catalog/register → 200
POST /api/warehouse/receive → 200
GET  /api/warehouse/balance?sku=BOLT-42 → {"balance":"100"}
```

---

## Фаза 6 — BC Template Extraction

Warehouse → шаблон с `queries/`, `migrations/`, `BC_CONTEXT.md`.
AI-агент: читает guide → reads warehouse → generates new BC → cargo test = green.

---

## Фаза 7 — Extensions + Thin UI

Extensions: Lua (mlua) + WASM (wasmtime). Thin UI: Askama + HTMX.

---

## 5 действий → 16 инвариантов

| Действие | Покрываемые инварианты |
|----------|----------------------|
| Register item (catalog) | pipeline, audit, event publish, seq_gen |
| Receive goods (warehouse) | pipeline, RLS, outbox, domain history, balance rule |
| Item projection (wh ← catalog) | inter-BC events, local projection, inbox dedup |
| Get balance (query) | query handler, read path, tenant isolation |
| Deny unauthorized receive | RBAC deny, pipeline abort, no UoW begin |

---

## Workspace dependencies

```toml
[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP
axum = { version = "0.8", features = ["macros"] }
axum-extra = { version = "0.10", features = ["typed-header"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }

# Database: Clorinde + tokio-postgres
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
deadpool-postgres = "0.14"
postgres-types = { version = "0.2", features = ["derive", "with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
clorinde = "0.12"
refinery = { version = "0.8", features = ["tokio-postgres"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Auth
jsonwebtoken = "9"

# IDs + Time
uuid = { version = "1", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Errors
thiserror = "2"
anyhow = "1"

# Decimal
bigdecimal = { version = "0.4", features = ["serde"] }

# Async trait
async-trait = "0.1"
```
