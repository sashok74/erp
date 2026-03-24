# Layer 2 — Data Access: PostgreSQL + Clorinde + RLS + UoW
> Подробное ТЗ | ERP Pilot on Rust
> Дата: 2026-03-23 | Привязка: ADR v1, Auth Blueprint (L0 conventions), Clorinde deep dive
> Предусловие: Phase 1 полностью выполнена (Kernel, Event Bus, Runtime, Auth)
> Первый шаг Phase 2: впервые подключаемся к реальной БД.

---

## Зачем этот слой

Data Access — мост между нашими абстракциями (traits из Phase 1) и реальным PostgreSQL. Здесь:

1. **Connection Pool** — пул соединений (deadpool-postgres), tenant-aware
2. **RLS** — Row-Level Security: `SET LOCAL app.tenant_id` на каждое соединение → PostgreSQL фильтрует данные автоматически
3. **Unit of Work** — реализация trait из runtime::ports. BEGIN → операции → COMMIT/ROLLBACK с outbox
4. **Миграции** — DDL-скрипты для common schema (tenants, outbox, audit, sequences)
5. **Clorinde** — SQL-запросы в `.sql` файлах, генерация типобезопасного crate

### Конвенции из Auth Blueprint (соблюдаем с первого дня)

Документ `erp_auth_architecture_blueprint.md` определяет правила L0, обязательные **уже сейчас**:

| Конвенция | Что значит | Где проявляется в Layer 2 |
|-----------|-----------|--------------------------|
| `tenant_id UUID NOT NULL` в каждой бизнес-таблице | Безусловная tenant isolation | Все CREATE TABLE в миграциях |
| RLS = defense-in-depth | Даже если приложение забудет WHERE | CREATE POLICY + ENABLE RLS на каждой таблице |
| Команды: `{bc}::{command}` | Namespace с `::`, не с `.` | outbox event_type, audit action |
| Domain layer без проверок ролей | Авторизация только в Pipeline | UoW не проверяет роли |

---

## Что мы изучим в этом слое (Rust)

| Концепция | Где применяется | Зачем в Rust |
|-----------|----------------|--------------|
| `deadpool-postgres` | Connection pool | Async connection pooling, checkout/return |
| `tokio-postgres` | SQL execution | Low-level async PostgreSQL driver |
| `postgres-types` | Rust ↔ PG type mapping | Derive ToSql/FromSql для custom types |
| Lifetime in struct | `Transaction<'a>` | Транзакция живёт не дольше соединения |
| `impl Trait for Struct` | PgUnitOfWork impl UnitOfWork | Реальная реализация порта |
| Clorinde CLI | Code generation | SQL-файлы → типобезопасный Rust crate |
| `SET LOCAL` | RLS tenant context | Переменная сессии, откатывается с TX |
| SQL migrations | Schema management | Идемпотентные DDL-скрипты |
| `Arc<Pool>` | Shared pool across handlers | Один pool для всего приложения |

---

## Структура файлов после выполнения

```
crates/db/src/
├── lib.rs              ← pub mod + re-exports
├── pool.rs             ← PgPool wrapper (deadpool-postgres)
├── rls.rs              ← set_tenant_context(), RLS helpers
├── uow.rs              ← PgUnitOfWork (impl UnitOfWork from runtime)
└── migrate.rs          ← SQL migration runner

migrations/
├── common/
│   ├── V001__schemas.sql           ← CREATE SCHEMA common, warehouse
│   ├── V002__tenants.sql           ← common.tenants table
│   ├── V003__rls_setup.sql         ← current_tenant_id() function
│   ├── V004__outbox.sql            ← common.outbox table
│   ├── V005__audit.sql             ← common.audit_log table
│   └── V006__sequences.sql         ← common.sequences table
└── warehouse/
    └── (пусто — заполнится в Layer 6)

queries/
├── common/
│   ├── outbox.sql                  ← INSERT/SELECT outbox entries
│   └── sequences.sql               ← next_value (SELECT FOR UPDATE)
└── warehouse/
    └── (пусто — заполнится в Layer 6)

crates/clorinde-gen/                ← СГЕНЕРИРОВАННЫЙ crate
├── Cargo.toml
└── src/
    └── (автогенерация Clorinde CLI)
```

---

## Задача 2.1 — SQL-миграции: common schema

### Зачем в ERP

Инфраструктурные таблицы, общие для всех BC: tenant registry, outbox для event delivery, audit log, sequence generator. Живут в schema `common`. Каждый BC получит свою schema позже.

### Зачем в Rust (что учим)

**SQL-миграции как код** — DDL-скрипты в репозитории, применяются последовательно, идемпотентны. Это Infrastructure as Code для БД.

### Требования к коду

**Файл: `migrations/common/V001__schemas.sql`**

```sql
-- Создание schemas для modular monolith
-- Каждый BC получит свою schema
CREATE SCHEMA IF NOT EXISTS common;
CREATE SCHEMA IF NOT EXISTS warehouse;
-- Будущие: CREATE SCHEMA IF NOT EXISTS finance;
```

**Файл: `migrations/common/V002__tenants.sql`**

```sql
CREATE TABLE common.tenants (
    id          UUID PRIMARY KEY,
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Файл: `migrations/common/V003__rls_setup.sql`**

```sql
-- Функция для извлечения tenant_id из session variable
-- Используется в RLS-политиках всех таблиц
CREATE OR REPLACE FUNCTION common.current_tenant_id()
RETURNS UUID AS $$
    SELECT NULLIF(current_setting('app.tenant_id', true), '')::UUID;
$$ LANGUAGE sql STABLE;
```

**Файл: `migrations/common/V004__outbox.sql`**

```sql
CREATE TABLE common.outbox (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    event_id        UUID NOT NULL UNIQUE,
    event_type      TEXT NOT NULL,          -- "erp.warehouse.goods_shipped.v1"
    source          TEXT NOT NULL,          -- "warehouse"
    payload         JSONB NOT NULL,
    correlation_id  UUID NOT NULL,
    causation_id    UUID NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    published       BOOLEAN NOT NULL DEFAULT false,
    published_at    TIMESTAMPTZ,
    retry_count     INT NOT NULL DEFAULT 0
);

CREATE INDEX idx_outbox_unpublished
    ON common.outbox (id) WHERE published = false;

CREATE INDEX idx_outbox_tenant_time
    ON common.outbox (tenant_id, created_at DESC);

-- RLS: tenant isolation
ALTER TABLE common.outbox ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON common.outbox
    USING (tenant_id = common.current_tenant_id());
```

**Файл: `migrations/common/V005__audit.sql`**

```sql
CREATE TABLE common.audit_log (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    user_id         UUID NOT NULL,
    correlation_id  UUID NOT NULL,
    action          TEXT NOT NULL,          -- "warehouse::receive_goods" (:: convention!)
    entity_type     TEXT,
    entity_id       UUID,
    old_state       JSONB,
    new_state       JSONB,
    metadata        JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_tenant_time
    ON common.audit_log (tenant_id, created_at DESC);

-- RLS
ALTER TABLE common.audit_log ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON common.audit_log
    USING (tenant_id = common.current_tenant_id());
```

**Файл: `migrations/common/V006__sequences.sql`**

```sql
CREATE TABLE common.sequences (
    tenant_id   UUID NOT NULL,
    seq_name    TEXT NOT NULL,          -- "warehouse::receipt"
    prefix      TEXT NOT NULL DEFAULT '',
    next_value  BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY (tenant_id, seq_name)
);

-- RLS
ALTER TABLE common.sequences ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON common.sequences
    USING (tenant_id = common.current_tenant_id());
```

### Критерий готовности

Миграции применяются без ошибок: `psql -f migrations/common/V001__schemas.sql` и далее по порядку. Или через migration runner (задача 2.5).

---

## Задача 2.2 — PgPool: connection pool wrapper

### Зачем в ERP

Один pool соединений для всего приложения. Каждый запрос берёт соединение из pool'а, устанавливает tenant context (RLS), выполняет работу, возвращает соединение.

### Зачем в Rust (что учим)

**`deadpool-postgres`** — async connection pool поверх `tokio-postgres`. Checkout → использование → automatic return. Конфигурируется из `DATABASE_URL`.

**`Arc<PgPool>`** — pool разделяется между всеми handler'ами через `Arc`. Один pool — десятки concurrent запросов.

### Требования к коду

**Файл: `crates/db/src/pool.rs`**

```rust
/// Обёртка над deadpool-postgres Pool.
/// Единый pool для всего приложения, shared через Arc.
pub struct PgPool {
    pool: deadpool_postgres::Pool,
}
```

Методы:
- `new(database_url: &str) -> Result<Self, anyhow::Error>` — создать pool из DATABASE_URL
- `get(&self) -> Result<deadpool_postgres::Client, anyhow::Error>` — взять соединение из pool'а
- `inner(&self) -> &deadpool_postgres::Pool` — доступ к нижележащему pool'у
- `health_check(&self) -> Result<(), anyhow::Error>` — `SELECT 1` для проверки

### Тесты

- `health_check()` → Ok (требует реальную БД — integration test)
- Pool configuration: max_size, timeouts

---

## Задача 2.3 — RLS: tenant context на соединении

### Зачем в ERP

Multi-tenancy: данные всех tenant'ов в одних таблицах, но каждый tenant видит только свои. PostgreSQL RLS (Row-Level Security) фильтрует строки автоматически, если установлен `app.tenant_id` на соединении.

`SET LOCAL app.tenant_id = '...'` — устанавливает переменную **в рамках транзакции**. При COMMIT/ROLLBACK переменная сбрасывается. Defense-in-depth: даже если application code забудет WHERE tenant_id = $1, RLS не пропустит чужие данные.

### Зачем в Rust (что учим)

**`SET LOCAL`** — SQL-команда, влияющая только на текущую транзакцию. Идеально для tenant context.

**Потокобезопасность** — каждое соединение из pool'а = отдельная PG session. SET LOCAL на одном соединении не влияет на другие. Tokio может обрабатывать тысячи concurrent запросов, каждый со своим tenant_id.

### Требования к коду

**Файл: `crates/db/src/rls.rs`**

```rust
/// Устанавливает tenant context на PostgreSQL-соединении (внутри TX).
/// После этого RLS-политики фильтруют данные автоматически.
///
/// ВАЖНО: вызывать ПОСЛЕ BEGIN, ПЕРЕД любыми запросами к данным.
/// SET LOCAL откатывается вместе с транзакцией.
pub async fn set_tenant_context(
    tx: &deadpool_postgres::Transaction<'_>,
    tenant_id: TenantId,
) -> Result<(), anyhow::Error> {
    tx.execute(
        &format!("SET LOCAL app.tenant_id = '{}'", tenant_id.as_uuid()),
        &[],
    ).await?;
    Ok(())
}
```

### Тесты (integration, с реальной БД)

- INSERT запись с tenant_id = A → SET tenant_id = A → SELECT → видна
- SET tenant_id = B → SELECT → НЕ видна (RLS фильтрует)
- Без SET tenant_id → SELECT → НИЧЕГО не видно (current_tenant_id() = NULL → policy не совпадает)

---

## Задача 2.4 — PgUnitOfWork: реализация UnitOfWork trait

### Зачем в ERP

UnitOfWork — одна ACID-транзакция для одной команды. Pipeline вызывает `begin()` → handler работает → `commit()`. Внутри TX: бизнес-данные + outbox entries записываются атомарно.

Это **реализация порта** из runtime::ports::UnitOfWork. В Phase 1 мы тестировали Pipeline с InMemoryUnitOfWork. Теперь подставляем реальную PG-транзакцию.

### Зачем в Rust (что учим)

**Lifetime `'a`** — транзакция заимствует соединение. `Transaction<'a>` живёт не дольше, чем `Client` (соединение из pool). Rust гарантирует это на уровне типов.

**`impl UnitOfWork for PgUnitOfWork`** — конкретная реализация абстрактного порта. Pipeline не изменился — только подставлена реальная реализация.

**Outbox в TX** — `add_outbox_entry()` накапливает events. При `commit()` они INSERT'ятся в common.outbox **в той же транзакции**, что и бизнес-данные. Атомарность: либо оба записаны, либо оба откачены.

### Требования к коду

**Файл: `crates/db/src/uow.rs`**

```rust
/// Реализация UnitOfWork на PostgreSQL-транзакции.
/// BEGIN → SET tenant_id (RLS) → операции → INSERT outbox → COMMIT.
pub struct PgUnitOfWork<'a> {
    tx: deadpool_postgres::Transaction<'a>,
    outbox_entries: Vec<EventEnvelope>,
    tenant_id: TenantId,
}
```

Реализация `UnitOfWork` trait:
- `add_outbox_entry(&mut self, envelope: EventEnvelope)` — push в Vec
- `commit(self)` — INSERT всех outbox entries в common.outbox → tx.commit()
- `rollback(self)` — tx.rollback(), outbox entries отброшены

**PgUnitOfWorkFactory:**

```rust
pub struct PgUnitOfWorkFactory {
    pool: Arc<PgPool>,
}
```

Реализация `UnitOfWorkFactory` trait:
- `begin(&self, ctx: &RequestContext)` → взять Client из pool → BEGIN → SET tenant_id → вернуть PgUnitOfWork

**Доступ к TX для handler'ов:**

Handler'ам нужен доступ к транзакции для SQL-запросов. Добавить метод:
- `transaction(&mut self) -> &deadpool_postgres::Transaction<'_>` — для выполнения SQL внутри TX

Или расширить trait UnitOfWork в runtime:
- `fn as_pg_transaction(&mut self) -> Option<&deadpool_postgres::Transaction<'_>>` — downcasting

**Прагматичное решение:** handler получает `&mut dyn UnitOfWork`. Для PG-запросов — downcast через `Any`. Или добавить generic параметр в CommandHandler. Обсудить при реализации — оба подхода рабочие.

### Тесты (integration)

- begin → commit → outbox entries записаны в common.outbox
- begin → rollback → outbox entries НЕ записаны
- begin → SET tenant_id → SELECT → данные фильтруются по tenant
- Два concurrent UoW с разными tenant_id → изоляция

---

## Задача 2.5 — Migration runner

### Зачем в ERP

Автоматическое применение миграций при старте или по команде `just db-migrate`. Миграции выполняются последовательно: V001, V002, ..., записывают свой статус в служебную таблицу.

### Зачем в Rust (что учим)

Простой runner: читает `.sql` файлы из каталога, сортирует по имени, выполняет те, что ещё не применены. Можно использовать `refinery` crate или написать свой минимальный runner.

### Требования к коду

**Файл: `crates/db/src/migrate.rs`**

- `run_migrations(pool: &PgPool, migrations_dir: &str) -> Result<(), anyhow::Error>`
- Создаёт служебную таблицу `common._migrations (name TEXT PK, applied_at TIMESTAMPTZ)`
- Читает `.sql` файлы, сортирует, выполняет новые
- Логирует: `tracing::info!("Applied migration: V001__schemas.sql")`

### Тесты (integration)

- Применение миграций → все таблицы созданы
- Повторный запуск → ничего не делает (идемпотентность)

---

## Задача 2.6 — Clorinde: SQL-запросы + генерация crate

### Зачем в ERP

SQL-запросы живут в `.sql` файлах, организованных по BC. Clorinde CLI генерирует типобезопасный Rust crate из этих файлов, проверяя каждый запрос против реальной схемы БД.

На данном этапе — только common-запросы (outbox, sequences). Warehouse queries появятся в Layer 6.

### Зачем в Rust (что учим)

**Code generation** — аналог protoc для SQL. Пишем `.sql`, запускаем `clorinde generate`, получаем `.rs` с типизированными функциями и структурами. Без макро-магии, код читаемый.

**Separation of concerns** — SQL пишет тот, кто знает SQL (или DBA). Rust-код использует сгенерированные функции. Если схема изменилась — перегенерация ловит несоответствия.

### Требования к коду

**Файл: `queries/common/outbox.sql`**

```sql
--! insert_outbox_entry
INSERT INTO common.outbox
    (tenant_id, event_id, event_type, source, payload,
     correlation_id, causation_id, created_at)
VALUES
    (:tenant_id, :event_id, :event_type, :source, :payload::jsonb,
     :correlation_id, :causation_id, :created_at)
RETURNING id;

--! get_unpublished_events
SELECT id, tenant_id, event_id, event_type, source, payload,
       correlation_id, causation_id, created_at, retry_count
FROM common.outbox
WHERE published = false
ORDER BY id
LIMIT :batch_size
FOR UPDATE SKIP LOCKED;

--! mark_published
UPDATE common.outbox
SET published = true, published_at = NOW()
WHERE id = :id;

--! increment_retry
UPDATE common.outbox
SET retry_count = retry_count + 1
WHERE id = :id;
```

**Файл: `queries/common/sequences.sql`**

```sql
--! next_value
SELECT prefix, next_value
FROM common.sequences
WHERE tenant_id = :tenant_id AND seq_name = :seq_name
FOR UPDATE;

--! increment_sequence
UPDATE common.sequences
SET next_value = next_value + 1
WHERE tenant_id = :tenant_id AND seq_name = :seq_name;

--! ensure_sequence
INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value)
VALUES (:tenant_id, :seq_name, :prefix, 1)
ON CONFLICT (tenant_id, seq_name) DO NOTHING;
```

**Генерация:**

```bash
just clorinde-generate
# → crates/clorinde-gen/src/ обновлён с типизированными функциями
```

**crates/clorinde-gen/Cargo.toml** — создаётся Clorinde CLI автоматически.

### justfile рецепт

```just
clorinde-generate:
    clorinde generate \
      --queries-path queries/ \
      --destination crates/clorinde-gen/
    @echo "Clorinde crate regenerated"
```

### Критерий готовности

- `just clorinde-generate` проходит без ошибок
- `cargo build -p clorinde-gen` компилируется
- Сгенерированные struct'ы содержат правильные типы (UUID, JSONB → serde_json::Value, TIMESTAMPTZ → DateTime)

---

## Задача 2.7 — Финальная сборка: lib.rs + integration тесты

### Требования к коду

**Файл: `crates/db/src/lib.rs`**

```rust
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Data Access — PostgreSQL connection pool, RLS, Unit of Work, migrations.
//!
//! Реализует UnitOfWork из runtime::ports поверх PostgreSQL-транзакций.
//! RLS обеспечивает tenant isolation на уровне БД.
//! Clorinde-gen crate содержит типобезопасные SQL-запросы.

pub mod migrate;
pub mod pool;
pub mod rls;
pub mod uow;

pub use pool::PgPool;
pub use rls::set_tenant_context;
pub use uow::{PgUnitOfWork, PgUnitOfWorkFactory};
```

**Обновить `crates/db/Cargo.toml`:**

```toml
[dependencies]
kernel = { workspace = true }
runtime = { workspace = true }
event_bus = { workspace = true }
tokio-postgres = { workspace = true }
deadpool-postgres = { workspace = true }
postgres-types = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
tokio = { workspace = true }
```

### Integration тесты

Файл `tests/integration/db_tests.rs` (или внутри crate):

1. **Подключение** — pool.health_check() → Ok
2. **Миграции** — run_migrations() → таблицы созданы
3. **RLS изоляция** — tenant A не видит данные tenant B
4. **UoW commit** — outbox entries записаны
5. **UoW rollback** — outbox entries НЕ записаны
6. **Clorinde queries** — insert_outbox_entry + get_unpublished_events round-trip

### Финальная проверка

```bash
cargo build --workspace
cargo test -p db
cargo test -p clorinde-gen
cargo clippy --workspace -- -D warnings
just check
```

---

## Сводка: что получаем после Layer 2

| Файл/Артефакт | Содержание |
|---------------|-----------|
| `migrations/common/*.sql` | 6 DDL-скриптов: schemas, tenants, RLS, outbox, audit, sequences |
| `queries/common/*.sql` | SQL-запросы для outbox и sequences |
| `crates/clorinde-gen/` | Сгенерированный crate с типобезопасными функциями |
| `db/pool.rs` | PgPool wrapper (deadpool-postgres) |
| `db/rls.rs` | set_tenant_context() — RLS helper |
| `db/uow.rs` | PgUnitOfWork — реализация UnitOfWork trait на PG-транзакции |
| `db/migrate.rs` | Migration runner |

### Чему научились (Rust)

- **deadpool-postgres** — async connection pooling
- **tokio-postgres** — low-level async PG driver
- **Lifetime `'a` в struct** — Transaction живёт не дольше Client
- **`impl Trait for Struct`** — реальная реализация порта из Phase 1
- **Clorinde** — SQL-first codegen, type-safe queries
- **SET LOCAL** — session variables для RLS
- **Integration testing** — тесты с реальной БД

### Связь с архитектурой ERP

| Архитектурный элемент | Где реализовано |
|----------------------|-----------------|
| PostgreSQL shared DB (ADR) | pool.rs, migrations |
| tenant_id + RLS (ADR, Auth Blueprint L0) | rls.rs, RLS policies в каждой миграции |
| Single ACID TX (ADR) | uow.rs — BEGIN/COMMIT + outbox атомарно |
| Outbox для at-least-once delivery | common.outbox таблица + insert_outbox_entry |
| `::` naming convention (Auth Blueprint) | audit_log.action, outbox.event_type |
| Clorinde SQL-first (ADR update) | queries/*.sql → clorinde-gen crate |

### Auth Blueprint L0 checklist — что заложено

- [x] tenant_id UUID NOT NULL в каждой таблице
- [x] RLS ENABLE + CREATE POLICY на каждой таблице
- [x] `::` convention в audit.action и outbox.event_type
- [x] Domain layer не проверяет роли (UoW не знает о ролях)

---

## Следующий шаг

Layer 2 готов → **Layer 4b (Audit + SeqGen)**: реализации AuditLog и SequenceGenerator поверх PostgreSQL + Clorinde. Используют PgPool и clorinde-gen запросы.
