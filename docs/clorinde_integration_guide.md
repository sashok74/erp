# Clorinde: интеграция в ERP-проект
> Как SQL-файлы превращаются в типобезопасный Rust-код
> Дата: 2026-03-25 | Паттерн для всех Bounded Contexts

---

## Что делает Clorinde

Clorinde берёт `.sql` файлы с аннотированными запросами, **подключается к реальной PostgreSQL** (с актуальной схемой), валидирует каждый запрос и генерирует **отдельный Rust crate** с типобезопасными функциями.

```
Ты пишешь SQL      →  Clorinde проверяет  →  Получаешь Rust crate
(queries/*.sql)        (против реальной БД)    (crates/clorinde-gen/)
```

Если колонка `balance` в БД — `NUMERIC(18,4)`, сгенерированная функция принимает/возвращает `rust_decimal::Decimal`. Если запрос ссылается на несуществующую таблицу — ошибка **на этапе генерации**, до компиляции.

---

## Структура проекта

```
/home/dev/projects/erp/
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
│   └── warehouse/                    ← Новый BC добавляет свою папку
│       ├── inventory.sql                3 запроса: find, create, insert movement
│       └── balances.sql                 2 запроса: upsert, get balance
│
├── migrations/                       ← DDL (CREATE TABLE)
│   ├── common/*.sql
│   └── warehouse/*.sql
│
└── crates/clorinde-gen/              ← СГЕНЕРИРОВАННЫЙ crate (не редактируем руками!)
    ├── Cargo.toml                       (генерируется Clorinde)
    └── src/
        ├── lib.rs
        └── queries/                     ← зеркалирует структуру queries/
            ├── common/
            │   ├── outbox.rs
            │   ├── inbox.rs
            │   └── ...
            └── warehouse/
                ├── inventory.rs
                └── balances.rs
```

**Правило:** один `.sql` файл = один подмодуль в сгенерированном crate. `queries/warehouse/inventory.sql` → `clorinde_gen::queries::warehouse::inventory`.

---

## Как пишутся SQL-файлы

### Синтаксис

```sql
-- queries/warehouse/inventory.sql

--! find_item_by_sku
SELECT i.id, b.balance
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
    (:tenant_id, :id, :item_id, :event_type, :quantity, :balance_after,
     :doc_number, :correlation_id, :user_id);
```

Ключевые элементы:
- `--! query_name` — аннотация, имя запроса. Станет именем функции в Rust
- `:param_name` — именованный параметр. Clorinde заменит на `$1, $2, ...` при генерации, но в Rust API будут **именованные поля**
- Один `.sql` файл = несколько запросов. Группировка по смыслу (все операции с inventory в одном файле)

### Контроль nullable

```sql
--! find_item_by_sku : (balance?)
SELECT i.id, b.balance
FROM warehouse.inventory_items i
LEFT JOIN warehouse.inventory_balances b ON ...
WHERE i.tenant_id = :tenant_id AND i.sku = :sku;
```

`(balance?)` — поле `balance` может быть NULL (LEFT JOIN). В Rust будет `Option<BigDecimal>`.

---

## Что генерирует Clorinde

Из `queries/warehouse/inventory.sql` Clorinde создаёт `src/queries/warehouse/inventory.rs`:

```rust
// АВТОГЕНЕРИРОВАНО Clorinde — НЕ РЕДАКТИРОВАТЬ!
// Source: queries/warehouse/inventory.sql

use clorinde::GenericClient;

// ─── find_item_by_sku ───────────────────────────────────────────────

/// Row struct — автогенерирован из SELECT колонок
pub struct FindItemBySku {
    pub id: uuid::Uuid,
    pub balance: Option<rust_decimal::Decimal>,  // ? = Option
}

/// Bind parameters — автогенерированы из :named_params
pub struct FindItemBySkuParams<'a> {
    pub tenant_id: &'a uuid::Uuid,
    pub sku: &'a str,
}

/// Выполнить запрос. Возвращает query object с методами .opt(), .one(), .all()
pub fn find_item_by_sku() -> FindItemBySkuStmt {
    FindItemBySkuStmt(clorinde::private::Stmt::new(
        "SELECT i.id, b.balance FROM warehouse.inventory_items i ..."
    ))
}

impl FindItemBySkuStmt {
    /// Bind позиционных параметров
    pub fn bind<'a, C: GenericClient>(
        &'a self, client: &'a C,
        tenant_id: &'a uuid::Uuid,
        sku: &'a str,
    ) -> FindItemBySkuQuery<'a, C> { ... }

    /// Bind через struct
    pub fn params<'a, C: GenericClient>(
        &'a self, client: &'a C,
        params: &'a FindItemBySkuParams<'_>,
    ) -> FindItemBySkuQuery<'a, C> { ... }
}

// ─── create_item ────────────────────────────────────────────────────

/// Нет Row struct — INSERT без RETURNING
pub struct CreateItemParams<'a> {
    pub tenant_id: &'a uuid::Uuid,
    pub id: &'a uuid::Uuid,
    pub sku: &'a str,
}

pub fn create_item() -> CreateItemStmt { ... }

impl CreateItemStmt {
    /// Возвращает количество affected rows (u64)
    pub async fn bind<C: GenericClient>(
        &self, client: &C,
        tenant_id: &uuid::Uuid,
        id: &uuid::Uuid,
        sku: &str,
    ) -> Result<u64, tokio_postgres::Error> { ... }
}
```

### Маппинг типов PostgreSQL → Rust

| PostgreSQL | Rust | Пример |
|-----------|------|--------|
| `UUID` | `uuid::Uuid` | tenant_id, item_id |
| `TEXT` / `VARCHAR` | `&str` (params) / `String` (rows) | sku, event_type |
| `NUMERIC(18,4)` | `rust_decimal::Decimal` | balance, quantity |
| `TIMESTAMPTZ` | `chrono::DateTime<chrono::Utc>` | created_at |
| `JSONB` | `serde_json::Value` | payload |
| `BOOLEAN` | `bool` | published |
| `BIGINT` / `BIGSERIAL` | `i64` | id, next_value |
| `INTEGER` | `i32` | retry_count |

---

## Как используется в коде

### Repository (warehouse)

```rust
// crates/warehouse/src/infrastructure/repos.rs
// НИ ОДНОЙ SQL-СТРОКИ — только вызовы сгенерированного crate

use clorinde_gen::queries::warehouse::{inventory, balances};

pub struct PgInventoryRepo;

impl PgInventoryRepo {
    pub async fn find_by_sku(
        client: &impl clorinde::GenericClient,
        tenant_id: &uuid::Uuid,
        sku: &str,
    ) -> Result<Option<(Uuid, Decimal)>, Error> {
        // Вызов сгенерированной функции — типобезопасно
        let row = inventory::find_item_by_sku()
            .bind(client, tenant_id, sku)
            .opt()
            .await?;

        Ok(row.map(|r| (r.id, r.balance.unwrap_or_default())))
    }

    pub async fn create_item(
        client: &impl clorinde::GenericClient,
        tenant_id: &uuid::Uuid,
        id: &uuid::Uuid,
        sku: &str,
    ) -> Result<(), Error> {
        inventory::create_item()
            .bind(client, tenant_id, id, sku)
            .await?;
        Ok(())
    }

    pub async fn upsert_balance(
        client: &impl clorinde::GenericClient,
        params: &balances::UpsertBalanceParams<'_>,
    ) -> Result<(), Error> {
        balances::upsert_balance()
            .params(client, params)
            .await?;
        Ok(())
    }
}
```

### Или через params struct (для запросов с 5+ параметрами)

```rust
// Много параметров — struct удобнее
let params = inventory::InsertMovementParams {
    tenant_id: ctx.tenant_id.as_uuid(),
    id: &movement_id,
    item_id: &item_id,
    event_type: "goods_received",
    quantity: &qty,
    balance_after: &new_balance,
    doc_number: Some(&doc_number),
    correlation_id: &ctx.correlation_id,
    user_id: ctx.user_id.as_uuid(),
};

inventory::insert_movement().params(client, &params).await?;
```

---

## Когда запускается генерация

### Момент запуска

```
Разработчик:
  1. Пишет/меняет SQL в queries/*.sql
  2. Пишет/меняет миграцию в migrations/*.sql
  3. Применяет миграцию: just db-migrate
  4. Запускает генерацию: just clorinde-generate
  5. cargo build → компилятор проверяет что код соответствует сгенерированным типам
```

### Два режима генерации

**`clorinde live`** — подключается к существующей БД (наш основной режим):
```bash
clorinde live \
  -u "$DATABASE_URL" \
  -d crates/clorinde-gen \
  queries/
```
Требует: миграции уже применены к БД.

**`clorinde schema`** — поднимает временный контейнер, применяет схему, генерирует:
```bash
clorinde schema \
  -s migrations/common/*.sql \
  -s migrations/warehouse/*.sql \
  -d crates/clorinde-gen \
  queries/
```
Требует: Docker/Podman. Не нужна постоянная БД. Идеально для CI/CD.

### justfile рецепты

```just
# Генерация из живой БД (разработка)
clorinde-generate:
    clorinde live \
      -u "$(cat .env | grep DATABASE_URL | cut -d= -f2)" \
      -d crates/clorinde-gen \
      queries/
    @echo "clorinde-gen regenerated"

# Генерация через временный контейнер (CI)
clorinde-generate-ci:
    clorinde schema \
      -s migrations/common/*.sql \
      -s migrations/warehouse/*.sql \
      -d crates/clorinde-gen \
      queries/
    @echo "clorinde-gen regenerated (CI mode)"
```

### CI/CD pipeline

```yaml
steps:
  - name: Apply migrations
    run: psql $DATABASE_URL -f migrations/common/*.sql

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
SELECT id, event_type, quantity, balance_after, doc_number, created_at
FROM warehouse.stock_movements
WHERE tenant_id = :tenant_id AND item_id = :item_id
ORDER BY created_at DESC
LIMIT :limit;
```

**Шаг 2.** Перегенерировать:
```bash
just clorinde-generate
```

**Шаг 3.** Использовать в Rust (autocomplete работает!):
```rust
let movements = inventory::get_movements_by_item()
    .bind(client, tenant_id, &item_id, &50i64)
    .all()
    .await?;

for m in movements {
    println!("{}: {} → balance {}", m.event_type, m.quantity, m.balance_after);
}
```

Компилятор проверяет: типы параметров, имена колонок, nullable. Если переименовать колонку в миграции и забыть обновить SQL — Clorinde выдаст ошибку при генерации.

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
# journal::create_journal_entry().bind(&client, ...).one().await?;
```

Clorinde автоматически:
- Создаёт `src/queries/finance/` модуль
- Генерирует `journal.rs` со struct'ами и функциями
- Обновляет `mod.rs` для включения нового модуля

---

## Текущее состояние: ручной crate vs автогенерация

Сейчас `crates/clorinde-gen/` — **ручной crate**, повторяющий то, что Clorinde сгенерировал бы. Каждый файл помечен `// TODO: заменить на автогенерацию Clorinde CLI`.

**Почему ручной:**
1. Clorinde CLI нужно установить и настроить на сервере разработки
2. Нужна живая PostgreSQL со всеми миграциями для `clorinde live`
3. Ручной crate позволяет продвигаться, не блокируясь на tooling

**Переход на автогенерацию:**
1. Установить: `cargo install clorinde`
2. Применить все миграции
3. Запустить: `just clorinde-generate`
4. Удалить ручные файлы, коммитнуть сгенерированные
5. Добавить `just clorinde-generate` в CI pipeline

Ручной crate и автогенерированный — **один и тот же API**. Код, вызывающий `clorinde_gen::queries::warehouse::inventory::find_item_by_sku()`, не изменится.

---

## Зависимости сгенерированного crate

Clorinde генерирует `Cargo.toml` автоматически:

```toml
[package]
name = "clorinde-gen"
version = "0.1.0"
edition = "2021"

[dependencies]
clorinde = "1.3"              # runtime библиотека (GenericClient, Params, etc.)
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
serde_json = "1"
serde = { version = "1", features = ["derive"] }
rust_decimal = { version = "1", features = ["db-tokio-postgres"] }
```

**Ключевое:** сгенерированный crate зависит от `clorinde` (runtime), которая re-exports `tokio_postgres::GenericClient`. Весь наш код работает через `GenericClient` — тот же trait что в `tokio-postgres`.

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
