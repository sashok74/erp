# Clorinde: интеграция в ERP-проект
> Как SQL-файлы превращаются в типобезопасный Rust-код
> Дата: 2026-03-25 | Обновлено: 2026-03-25 (миграция на Clorinde CLI v1.4.0)
> Паттерн для всех Bounded Contexts

---

## Что делает Clorinde

Clorinde берёт `.sql` файлы с аннотированными запросами, **подключается к реальной PostgreSQL** (с актуальной схемой), валидирует каждый запрос и генерирует **самодостаточный Rust crate** с типобезопасными функциями.

```
Ты пишешь SQL      →  Clorinde проверяет  →  Получаешь Rust crate
(queries/*.sql)        (против реальной БД)    (crates/clorinde-gen/)
```

Если запрос ссылается на несуществующую таблицу или колонку — ошибка **на этапе генерации**, до компиляции.

---

## Текущее состояние

`crates/clorinde-gen/` — **автогенерированный** crate (Clorinde CLI v1.4.0). Все файлы в `src/` помечены `// This file was generated with clorinde. Do not modify.`.

Генерация: `clorinde live -q queries/ -d crates/clorinde-gen "$DATABASE_URL"`

После генерации нужно вручную поправить `name` в `Cargo.toml` на `"clorinde-gen"` (Clorinde генерирует `name = "clorinde"`).

---

## Структура проекта

```
/home/raa/RustProjects/erp/
│
├── queries/                          ← SQL-файлы (SOURCE OF TRUTH)
│   ├── common/
│   │   ├── outbox.sql                   5 запросов к common.outbox
│   │   ├── inbox.sql                    2 запроса к common.inbox
│   │   ├── audit.sql                    1 запрос к common.audit_log
│   │   ├── sequences.sql               3 запроса к common.sequences
│   │   ├── tenants.sql                  2 запроса к common.tenants
│   │   └── domain_history.sql           1 запрос к common.domain_history
│   │
│   └── warehouse/
│       ├── inventory.sql                3 запроса: find, create, insert movement
│       └── balances.sql                 2 запроса: upsert, get balance
│
├── migrations/                       ← DDL (CREATE TABLE)
│   ├── common/*.sql
│   └── warehouse/*.sql
│
└── crates/clorinde-gen/              ← АВТОГЕНЕРИРОВАННЫЙ crate (не редактируем руками!)
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── client/                     ← GenericClient trait + deadpool support
        │   ├── async_.rs
        │   └── async_/
        │       ├── generic_client.rs
        │       └── deadpool.rs
        ├── queries/                    ← зеркалирует структуру queries/
        │   ├── common/
        │   │   ├── outbox.rs
        │   │   ├── inbox.rs
        │   │   └── ...
        │   └── warehouse/
        │       ├── inventory.rs
        │       └── balances.rs
        ├── domain.rs
        ├── type_traits.rs              ← StringSql, JsonSql, ArraySql
        ├── types.rs
        ├── array_iterator.rs
        └── utils.rs
```

**Правило:** один `.sql` файл = один подмодуль. `queries/warehouse/inventory.sql` → `clorinde_gen::queries::warehouse::inventory`.

---

## Как пишутся SQL-файлы

### Синтаксис

```sql
-- queries/warehouse/inventory.sql

--! find_item_by_sku
SELECT i.id, COALESCE(b.balance, 0)::TEXT AS balance
FROM warehouse.inventory_items i
LEFT JOIN warehouse.inventory_balances b
    ON b.tenant_id = i.tenant_id AND b.item_id = i.id
WHERE i.tenant_id = :tenant_id AND i.sku = :sku;

--! create_item
INSERT INTO warehouse.inventory_items (tenant_id, id, sku)
VALUES (:tenant_id, :id, :sku);

--! insert_movement
INSERT INTO warehouse.stock_movements
    (tenant_id, id, item_id, event_type, quantity, balance_after,
     doc_number, correlation_id, user_id)
VALUES
    (:tenant_id, :id, :item_id, :event_type,
     :quantity::TEXT::NUMERIC, :balance_after::TEXT::NUMERIC,
     :doc_number, :correlation_id, :user_id);
```

Ключевые элементы:
- `--! query_name` — аннотация, имя запроса. Станет именем функции в Rust
- `:param_name` — именованный параметр. Clorinde заменит на `$1, $2, ...` при генерации
- `::TEXT::NUMERIC` — двойной cast для NUMERIC-колонок (tokio-postgres не имеет нативной поддержки BigDecimal, передаём как TEXT)
- `::TEXT` в SELECT — читаем NUMERIC как строку
- Один `.sql` файл = несколько запросов, группировка по смыслу

---

## Что генерирует Clorinde

Из `queries/warehouse/inventory.sql` Clorinde создаёт `src/queries/warehouse/inventory.rs`:

```rust
// This file was generated with `clorinde`. Do not modify.

// ─── Params structs ─────────────────────────────────────────────

// Текстовые поля — generic с trait bound StringSql (принимает &str, String, и т.д.)
#[derive(Debug)]
pub struct FindItemBySkuParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub sku: T1,
}

#[derive(Debug)]
pub struct InsertMovementParams<T1: StringSql, T2: StringSql, T3: StringSql, T4: StringSql> {
    pub tenant_id: uuid::Uuid,
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub event_type: T1,
    pub quantity: T2,       // TEXT::NUMERIC — передаём строку
    pub balance_after: T3,  // TEXT::NUMERIC — передаём строку
    pub doc_number: T4,
    pub correlation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}

// ─── Row structs ────────────────────────────────────────────────

// Owned-версия (возвращается из .opt(), .one(), .all())
#[derive(Debug, Clone, PartialEq)]
pub struct FindItemBySku {
    pub id: uuid::Uuid,
    pub balance: String,  // NUMERIC→TEXT cast в SQL → String в Rust
}

// Borrowed-версия (zero-copy, для .map())
pub struct FindItemBySkuBorrowed<'a> {
    pub id: uuid::Uuid,
    pub balance: &'a str,
}

// ─── Query functions ────────────────────────────────────────────

pub fn find_item_by_sku() -> FindItemBySkuStmt { ... }

impl FindItemBySkuStmt {
    // Позиционные параметры
    pub fn bind<C: GenericClient, T1: StringSql>(
        &self, client: &C,
        tenant_id: &uuid::Uuid,
        sku: &T1,
    ) -> FindItemBySkuQuery<..., FindItemBySku, 2> { ... }
}

// SELECT без RETURNING → .opt() / .one() / .all()
// INSERT/UPDATE без RETURNING → .bind().await? → Result<u64>
// INSERT с RETURNING → .bind().one().await? → Result<T>
```

### Маппинг типов PostgreSQL → Rust

| PostgreSQL | Rust (row) | Rust (param) | Пример |
|-----------|-----------|-------------|--------|
| `UUID` | `uuid::Uuid` | `&uuid::Uuid` | tenant_id, item_id |
| `TEXT` / `VARCHAR` | `String` | `&T1: StringSql` | sku, event_type |
| `NUMERIC::TEXT` | `String` | `&T: StringSql` | balance, quantity (через cast) |
| `TIMESTAMPTZ` | `DateTime<FixedOffset>` | `&DateTime<FixedOffset>` | created_at |
| `JSONB` | `serde_json::Value` | `&T: JsonSql` | payload |
| `BOOLEAN` | `bool` | `&bool` | published |
| `BIGINT` / `BIGSERIAL` | `i64` | `&i64` | id, next_value |
| `INTEGER` | `i32` | `&i32` | retry_count |

**Важно:** `TIMESTAMPTZ` маппится в `DateTime<FixedOffset>`, не `DateTime<Utc>`. При передаче из `RequestContext` (который хранит `DateTime<Utc>`) нужна конвертация: `timestamp.fixed_offset()`.

### Transport Adapters (`db::transport`)

Для типов, которые Clorinde передаёт как TEXT (например `NUMERIC` через `::TEXT::NUMERIC` cast),
crate `db` предоставляет **transport adapters** — zero-copy wrappers, реализующие clorinde marker traits.

#### Write path: `DecStr`

```rust
use db::transport::DecStr;

// В bind-списках — без ручной конвертации в строку:
bind = [&tid(tenant_id), &item_id, &DecStr(balance)];
```

`DecStr<'a>` оборачивает `&'a BigDecimal` и реализует `StringSql` (через `ToSql` как TEXT).
Clorinde принимает его везде, где ожидается `StringSql` параметр.

#### Read path: `parse_dec`

```rust
use db::conversions::parse_dec;

// В map-блоках — парсинг TEXT обратно в BigDecimal:
map = |r| { BalanceRow { balance: parse_dec(&r.balance)? } };
```

#### Как добавить новый transport adapter

1. Определить wrapper в `db::transport`, например `MoneyStr<'a>(&'a Money)`
2. Реализовать `postgres_types::ToSql` (сериализация в TEXT через `Display` или кастомную логику)
3. Реализовать `clorinde_gen::StringSql` (marker trait, без методов)
4. Использовать `MoneyStr(&amount)` в bind-списках

---

## Как используется в коде

### GenericClient

Clorinde генерирует свой trait `GenericClient` (в `crate::client::async_::GenericClient`), который реализован для:
- `tokio_postgres::Client`
- `tokio_postgres::Transaction`
- `deadpool_postgres::Client` (через feature `deadpool`)

Callers импортируют: `use clorinde_gen::client::GenericClient;`

### Repository (warehouse) — реальный код

```rust
use clorinde_gen::client::GenericClient;

pub struct PgInventoryRepo;

impl PgInventoryRepo {
    pub async fn find_by_sku(
        client: &impl GenericClient,
        tenant_id: TenantId,
        sku: &str,
    ) -> Result<Option<(Uuid, BigDecimal)>> {
        let tid = *tenant_id.as_uuid();
        let row = clorinde_gen::queries::warehouse::inventory::find_item_by_sku()
            .bind(client, &tid, &sku)
            .opt()
            .await?;

        Ok(row.map(|r| {
            let balance = BigDecimal::from_str(&r.balance)?;
            Ok((r.id, balance))
        }).transpose()?)
    }

    pub async fn create_item(
        client: &impl GenericClient,
        tenant_id: TenantId,
        item_id: Uuid,
        sku: &str,
    ) -> Result<()> {
        let tid = *tenant_id.as_uuid();
        clorinde_gen::queries::warehouse::inventory::create_item()
            .bind(client, &tid, &item_id, &sku)
            .await?;
        Ok(())
    }

    pub async fn save_movement(
        client: &impl GenericClient,
        tenant_id: TenantId,
        movement_id: Uuid,
        qty: &BigDecimal,
        balance_after: &BigDecimal,
        /* ... */
    ) -> Result<()> {
        let tid = *tenant_id.as_uuid();
        // DecStr — transport adapter, реализует StringSql напрямую
        clorinde_gen::queries::warehouse::inventory::insert_movement()
            .bind(client, &tid, &movement_id, &item_id,
                  &event_type, &DecStr(qty), &DecStr(balance_after), &doc_number,
                  &correlation_id, &user_id)
            .await?;
        Ok(())
    }
}
```

### Паттерн вызова

| Тип запроса | Метод | Пример |
|------------|-------|--------|
| SELECT 0..1 строка | `.bind(...).opt().await?` | `find_item_by_sku()` |
| SELECT ровно 1 | `.bind(...).one().await?` | `insert_audit_log()` (RETURNING id) |
| SELECT N строк | `.bind(...).all().await?` | `get_unpublished_events()` |
| INSERT/UPDATE | `.bind(...).await?` | `create_item()`, `mark_published()` |

---

## Генерация

### CLI синтаксис (Clorinde v1.4.0)

**`clorinde live`** — подключается к существующей БД (основной режим разработки):
```bash
clorinde live \
  -q queries/ \
  -d crates/clorinde-gen \
  "$DATABASE_URL"
```
Требует: миграции уже применены к БД. URL — **позиционный аргумент** (не `-u`).

**`clorinde schema`** — поднимает временный контейнер, применяет схему, генерирует:
```bash
clorinde schema \
  -q queries/ \
  -d crates/clorinde-gen \
  migrations/common/*.sql migrations/warehouse/*.sql
```
Требует: Docker/Podman. Не нужна постоянная БД. Для CI/CD.

### justfile рецепты

```just
# Генерация из живой БД (разработка)
clorinde-generate:
    clorinde live \
      -q queries/ \
      -d crates/clorinde-gen \
      "$(grep DATABASE_URL .env | cut -d= -f2)"
    @echo "clorinde-gen regenerated from live DB"

# Генерация через временный контейнер (CI)
clorinde-generate-ci:
    clorinde schema \
      -q queries/ \
      -d crates/clorinde-gen \
      migrations/common/*.sql migrations/warehouse/*.sql
    @echo "clorinde-gen regenerated (CI mode)"
```

**После генерации:** поправить `name = "clorinde"` → `name = "clorinde-gen"` в `crates/clorinde-gen/Cargo.toml`.

### CI/CD pipeline

```yaml
steps:
  - name: Generate Clorinde
    run: just clorinde-generate

  - name: Check for drift
    run: git diff --exit-code crates/clorinde-gen/
    # Если разработчик изменил SQL но забыл перегенерировать — CI упадёт

  - name: Build & test
    run: cargo test --workspace
```

---

## Workflow разработчика: добавление нового запроса

Пример: добавить запрос `get_movements_by_item` в warehouse.

**Шаг 1.** Добавить запрос в SQL-файл:
```sql
-- queries/warehouse/inventory.sql (добавляем в конец)

--! get_movements_by_item
SELECT id, event_type, quantity::TEXT, balance_after::TEXT, doc_number, created_at
FROM warehouse.stock_movements
WHERE tenant_id = :tenant_id AND item_id = :item_id
ORDER BY created_at DESC
LIMIT :limit;
```

**Шаг 2.** Перегенерировать:
```bash
just clorinde-generate
# Поправить name в Cargo.toml если перезаписался
```

**Шаг 3.** Использовать в Rust (autocomplete работает!):
```rust
let movements = clorinde_gen::queries::warehouse::inventory::get_movements_by_item()
    .bind(client, &tenant_id, &item_id, &50i64)
    .all()
    .await?;

for m in movements {
    println!("{}: {} → balance {}", m.event_type, m.quantity, m.balance_after);
}
```

---

## Workflow: добавление нового Bounded Context

Пример: добавить Finance BC.

```bash
# 1. Создать директорию для SQL
mkdir -p queries/finance

# 2. Написать запросы
cat > queries/finance/journal.sql << 'SQL'
--! create_journal_entry
INSERT INTO finance.journal_entries
    (tenant_id, id, entry_number, description, posted_at)
VALUES (:tenant_id, :id, :entry_number, :description, :posted_at)
RETURNING id;

--! get_journal_by_id
SELECT id, entry_number, description, posted_at
FROM finance.journal_entries
WHERE tenant_id = :tenant_id AND id = :id;
SQL

# 3. Применить миграции finance
psql $DATABASE_URL -f migrations/finance/001_create_journal.sql

# 4. Перегенерировать
just clorinde-generate
# → crates/clorinde-gen/src/queries/finance/journal.rs создан автоматически

# 5. Использовать
# use clorinde_gen::queries::finance::journal;
# journal::create_journal_entry().bind(client, ...).one().await?;
```

---

## Зависимости сгенерированного crate

Clorinde генерирует **самодостаточный crate** (без внешней runtime-библиотеки):

```toml
[package]
name = "clorinde-gen"   # ← вручную меняем с "clorinde" на "clorinde-gen"
version = "0.1.0"
edition = "2021"

[dependencies]
chrono = { version = "0.4", features = ["serde"] }
deadpool-postgres = { version = "0.14", optional = true }
futures = "0.3"
postgres = { version = "0.19", optional = true }
postgres-protocol = "0.6"
postgres-types = { version = "0.2", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["raw_value"] }
tokio-postgres = { version = "0.7", features = ["with-chrono-0_4", "with-uuid-1", "with-serde_json-1"] }
uuid = { version = "1", features = ["serde"] }

[features]
default = ["dep:postgres", "deadpool"]
deadpool = ["dep:deadpool-postgres", "tokio-postgres/default"]
```

**Ключевое:** crate самодостаточен — содержит собственный `GenericClient` trait, type traits (`StringSql`, `JsonSql`), и клиентскую обвязку. Нет зависимости на внешний `clorinde` runtime.

---

## Сводка: паттерн для каждого BC

| Шаг | Что делает разработчик | Что делает Clorinde |
|-----|----------------------|---------------------|
| 1. Миграция | `migrations/{bc}/NNN_*.sql` | — |
| 2. SQL-запросы | `queries/{bc}/*.sql` с `--!` аннотациями | — |
| 3. Генерация | `just clorinde-generate` | Проверяет SQL → генерирует Rust |
| 4. Repository | Вызывает `clorinde_gen::queries::{bc}::*` | — |
| 5. CI | `git diff --exit-code crates/clorinde-gen/` | Ловит drift |

**Ноль SQL в бизнес-коде.** SQL живёт в `.sql` файлах. Rust-код вызывает сгенерированные типобезопасные функции.
