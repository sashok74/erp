# ТЗ: `repo-gen v1` для генерации BC persistence API поверх Clorinde

> Статус: implementation-ready draft
> Дата: 2026-04-01

---

## 1. Цель

Реализовать детерминированный генератор `repo-gen`, который работает поверх `clorinde` и автоматически создаёт ergonomic persistence API для Bounded Context crates.

Цель для разработчика BC:

1. написать SQL в `queries/{bc}/*.sql`
2. добавить минимальные metadata-аннотации
3. запустить `just generate`
4. использовать готовый API в handler-коде

Цель для платформы:

- убрать ручной repo glue-код
- сохранить `SQL-first` подход
- не допустить утечки `clorinde` wire-типа в application/domain API
- не размыть границы BC

---

## 2. Критерий готовности

Функционал считается реализованным, если выполнены все условия:

1. текущие BC SQL-файлы `warehouse` и `catalog` переведены на metadata v1
2. `repo-gen` генерирует BC persistence API для всех текущих BC-запросов
3. ручной repo glue-код в приложении заменён на generated API
4. приложение компилируется без ручных `bind = [...]`, `tid()`, `DecStr()`, `parse_dec()` и `clorinde_gen::queries::...` в BC handlers
5. полный `just test` проходит без регрессий

Иными словами, приемка идёт не по демо-генерации на toy-примере, а по рефакторингу существующего приложения.

---

## 3. Scope v1

### Входит в `v1`

- explicit CLI tool `repo-gen` в workspace
- inline metadata в `queries/{bc}/*.sql`
- generation только для BC query files:
  - `queries/warehouse/*.sql`
  - `queries/catalog/*.sql`
- поддержка query kinds:
  - `exec`
  - `opt`
  - `one`
  - `all`
- автоматические transport-conversions для текущих паттернов:
  - `TenantId -> Uuid` для скрытого `tenant_id`
  - `BigDecimal -> TEXT::NUMERIC` через `DecStr`
  - `TEXT -> BigDecimal` через `parse_dec`
- generated repo structs + BC facade
- checked-in generated code
- единый workflow `just generate`

### Не входит в `v1`

- generation для `queries/common/*`
- generation handler-ов, aggregate logic, routes, modules
- generation permission manifests
- metadata в отдельной локальной DB
- `build.rs`
- proc-macro implementation
- LLM как источник generated Rust-кода
- поддержка произвольных кастомных конверсий вне текущих паттернов

---

## 4. Базовые принципы

1. `queries/*.sql` остаются source of truth.
2. Metadata хранится рядом с SQL, а не в отдельной базе.
3. `clorinde` остаётся обязательным первым этапом генерации.
4. `repo-gen` отвечает только за ergonomics и BC-facing persistence API.
5. Generated code должен быть детерминированным и пригодным для code review.
6. BC handler должен работать с BC-local API, а не с глобальным service locator.
7. Platform queries (`common/*`) остаются за platform services и не попадают в повседневную модель BC-разработчика.

---

## 5. Целевой workflow

```text
queries/{bc}/*.sql + metadata
  -> just generate
     -> clorinde live
     -> repo-gen --all
     -> cargo fmt
  -> generated BC db API
  -> handler uses generated API
  -> just test / just test-crate {bc}
```

Целевой код в handler:

```rust
let read = db::ReadScope::acquire(&self.pool, ctx.tenant_id).await?;
let wh = crate::db::WarehouseDb::new(read.client(), ctx.tenant_id);

let row = wh.balances.get_balance(&query.sku).await?;
let projection = wh.projections.get_by_sku(&query.sku).await?;

read.finish().await?;
```

И для write path:

```rust
let mut db = PgCommandContext::from_uow(uow)?;
let wh = crate::db::WarehouseDb::new(db.client(), ctx.tenant_id);

let existing = wh.inventory.find_by_sku(sku.as_str()).await?;
wh.inventory.create_item(item_id, sku.as_str()).await?;
wh.inventory.insert_movement(input).await?;

db.record_change(ctx, ...)?;
db.emit_events(&mut aggregate, ctx, "warehouse")?;
```

---

## 6. Metadata v1

Metadata пишется в тех же SQL-файлах, рядом с запросом.

Формат:

```sql
--! query_name
--@ key: value
--@ key: value
SELECT ...
```

### 6.1 Обязательные ключи

| Ключ | Значения | Назначение |
|------|----------|------------|
| `repo` | имя repo-группы | в какой generated repo попадёт метод |
| `kind` | `exec`, `opt`, `one`, `all` | как генератор оформляет вызов |

### 6.2 Опциональные ключи

| Ключ | Значения | Назначение |
|------|----------|------------|
| `dto` | имя Rust struct | имя generated output DTO для read-query |
| `input` | имя Rust struct | имя generated input struct для длинного `exec` |
| `dec` | comma-separated field names | список полей/параметров, где нужен `BigDecimal` conversion |

### 6.3 Правила metadata

1. Для `opt`, `one`, `all` ключ `dto` обязателен.
2. Для `exec` ключ `dto` запрещён.
3. Ключ `input` разрешён только для `exec`.
4. Ключ `dec` разрешён только для полей/параметров, которые реально присутствуют в SQL query.
5. Неизвестные metadata keys считаются ошибкой генерации.

### 6.4 Пример

```sql
--! get_balance
--@ repo: balances
--@ kind: opt
--@ dto: BalanceRow
--@ dec: balance
SELECT item_id, sku, balance::TEXT
FROM warehouse.inventory_balances
WHERE tenant_id = :tenant_id AND sku = :sku;

--! upsert_balance
--@ repo: balances
--@ kind: exec
--@ dec: balance
INSERT INTO warehouse.inventory_balances
    (tenant_id, item_id, sku, balance, last_movement_id, updated_at)
VALUES (:tenant_id, :item_id, :sku, :balance::TEXT::NUMERIC, :last_movement_id, now())
ON CONFLICT (tenant_id, item_id) DO UPDATE SET
    balance = EXCLUDED.balance,
    last_movement_id = EXCLUDED.last_movement_id,
    updated_at = now();
```

---

## 7. Ограничения на SQL для `v1`

Так как SQL и metadata будет подготавливать LLM, `v1` должен опираться на простой и строгий поднабор SQL.

### Обязательные ограничения

1. Один query block начинается с `--! query_name`.
2. Один SQL-файл может содержать несколько query blocks.
3. Все BC query files должны быть сгруппированы по одному узкому domain slice:
   - `inventory.sql`
   - `balances.sql`
   - `products.sql`
4. Во всех BC-запросах `tenant_id` должен быть первым именованным параметром.
5. Имена параметров должны быть стабильными и совпадать с доменным смыслом.
6. Для decimal write path использовать только `:param::TEXT::NUMERIC`.
7. Для decimal read path использовать только `column::TEXT`.

### Нежелательно в `v1`

- SQL, где один и тот же параметр повторяется под разными именами
- необычные alias и касты, не покрытые текущими правилами
- смешивание unrelated операций в одном файле

---

## 8. Архитектура реализации

### 8.1 Формат инструмента

`repo-gen` реализуется как отдельный Rust CLI crate в workspace.

Причины:

- explicit workflow
- хороший debug
- нормальная работа IDE
- проверяемый checked-in output
- отсутствие compile-time магии

### 8.2 Входные данные генератора

`repo-gen` получает:

1. SQL-файлы из `queries/{bc}/`
2. metadata v1 из этих файлов
3. сгенерированные `clorinde` Rust modules из `crates/clorinde-gen/src/queries/{bc}/`

### 8.3 Как именно generator использует Clorinde

`repo-gen` не генерирует SQL API сам.

Он:

1. использует SQL + metadata как source of truth для именования и группировки
2. извлекает из `clorinde-gen` точные Rust-типы параметров и row-структур
3. строит ergonomic wrapper поверх уже существующих `clorinde_gen::queries::{bc}::{file}::{query}()`

### 8.4 Разбор `clorinde-gen`

Для `v1` generator **разрешено** разбирать `clorinde` generated Rust через `syn`.

Это допустимо, потому что:

- `clorinde-gen` уже checked-in и детерминирован
- нам нужны реальные типы params/rows
- это надёжнее, чем дублировать всю типовую модель в metadata

Generator **не должен** использовать regexp-парсинг Rust как основной механизм.

---

## 9. Алгоритм `repo-gen`

### Шаг 1. Discover BCs

Generator находит все BC directories в `queries/`, кроме `common/`.

### Шаг 2. Parse SQL files

Для каждого `queries/{bc}/*.sql`:

- выделить query blocks
- извлечь `query_name`
- извлечь metadata
- сохранить path к исходному SQL
- извлечь список именованных параметров в порядке появления

### Шаг 3. Validate metadata

Generator падает с понятной ошибкой, если:

- отсутствует обязательный ключ
- metadata key неизвестен
- `dto` указан для `exec`
- `input` указан для не-`exec`
- read-query не содержит `dto`
- `tenant_id` отсутствует или не является первым параметром
- `dec` ссылается на несуществующее поле/параметр

### Шаг 4. Parse matching `clorinde-gen` module

Для каждого query generator извлекает:

- Params struct и список его полей
- Row struct и список его полей, если query read-only
- Rust-типы полей
- имя query function

### Шаг 5. Build internal model

Internal model должен содержать:

- `bc`
- `file`
- `query_name`
- `repo_group`
- `kind`
- `sql_path`
- `params` с Rust-типами и флагами конверсии
- `row_fields` с Rust-типами и флагами конверсии
- `dto_name`
- `input_name`

### Шаг 6. Generate Rust

Для каждого BC generator создаёт:

- `crates/{bc}/src/db/generated/mod.rs`
- `crates/{bc}/src/db/generated/types.rs`
- `crates/{bc}/src/db/generated/{repo}.rs`

Также generator создаёт BC facade:

- `WarehouseDb<'a>`
- `CatalogDb<'a>`

### Шаг 7. Format output

После генерации код форматируется через `cargo fmt --all`.

---

## 10. Что именно генерируется

### 10.1 Generated DTO types

Для read queries с `dto` generator создаёт output DTO.

Пример:

```rust
#[derive(Debug)]
pub struct BalanceRow {
    pub item_id: uuid::Uuid,
    pub sku: String,
    pub balance: bigdecimal::BigDecimal,
}
```

### 10.2 Generated input types

Для `exec` с `input` generator создаёт input struct.

Пример:

```rust
pub struct NewStockMovement {
    pub movement_id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub event_type: String,
    pub qty: bigdecimal::BigDecimal,
    pub balance_after: bigdecimal::BigDecimal,
    pub doc_number: String,
    pub correlation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}
```

### 10.3 Generated repo structs

По каждой `repo`-группе generator создаёт `&self`-repo:

```rust
pub struct BalancesRepo<'a> {
    client: &'a deadpool_postgres::Client,
    tenant_id: TenantId,
}
```

### 10.4 Generated methods

Generator должен скрывать:

- `tenant_id`
- `tid()`
- `DecStr()`
- `parse_dec()`
- `.internal("...")`
- прямые вызовы `clorinde_gen::queries::...`

Пример generated метода:

```rust
impl BalancesRepo<'_> {
    /// SQL: queries/warehouse/balances.sql --! get_balance
    pub async fn get_balance(&self, sku: &str) -> Result<Option<BalanceRow>, AppError> {
        use kernel::IntoInternal;

        let row = clorinde_gen::queries::warehouse::balances::get_balance()
            .bind(self.client, &db::conversions::tid(self.tenant_id), &sku)
            .opt()
            .await
            .internal("get_balance")?;

        match row {
            Some(r) => Ok(Some(BalanceRow {
                item_id: r.item_id,
                sku: r.sku,
                balance: db::conversions::parse_dec(&r.balance).internal("get_balance")?,
            })),
            None => Ok(None),
        }
    }
}
```

### 10.5 Generated BC facade

Пример:

```rust
pub struct WarehouseDb<'a> {
    pub inventory: InventoryRepo<'a>,
    pub balances: BalancesRepo<'a>,
    pub projections: ProjectionsRepo<'a>,
}

impl<'a> WarehouseDb<'a> {
    pub fn new(client: &'a deadpool_postgres::Client, tenant_id: TenantId) -> Self {
        Self {
            inventory: InventoryRepo::new(client, tenant_id),
            balances: BalancesRepo::new(client, tenant_id),
            projections: ProjectionsRepo::new(client, tenant_id),
        }
    }
}
```

---

## 11. Что остаётся ручным кодом

Ручным остаётся:

- domain model
- command/query handlers
- orchestration порядка вызовов
- `PgCommandContext`, `ReadScope`, `with_tenant_write`
- platform services (`audit`, `seq_gen`, `relay`, `inbox`)
- маленький stable wrapper `crates/{bc}/src/db/mod.rs`

### Назначение ручного `db/mod.rs`

Этот файл нужен как стабильная точка импорта и место для редких ручных override.

Он:

- реэкспортит generated facade и generated types
- не содержит ручного persistence glue по типовым запросам

---

## 12. Структура файлов после внедрения

Для каждого BC:

```text
crates/{bc}/src/
  db/
    mod.rs              <- hand-written stable entrypoint
    generated/
      mod.rs            <- generated
      types.rs          <- generated DTO/input types
      inventory.rs      <- generated repo group
      balances.rs       <- generated repo group
      projections.rs    <- generated repo group
```

Следствие для текущего приложения:

- ручной код в `application/repos.rs` и `infrastructure/repos.rs` должен быть удалён или сведен к thin re-export / compatibility shim

---

## 13. Workflow разработчика

### Обычный flow

1. изменить migration или схему
2. написать SQL + metadata
3. запустить `just generate`
4. использовать generated API в handler
5. запустить `just test-crate {bc}` или `just test`

### Новый recipe

```just
generate:
    clorinde live -q queries/ -d crates/clorinde-gen/ "$DATABASE_URL"
    cargo run -p repo_gen -- --all
    cargo fmt --all
```

### CI

CI должен:

1. запускать `just generate`
2. падать на drift в:
   - `crates/clorinde-gen/`
   - `crates/*/src/db/generated/`
3. запускать `just test`

---

## 14. Инструкция для LLM

LLM не генерирует Rust wrappers. Она готовит только SQL и metadata.

Источник правил для неё: отдельный companion document
`docs/plan/repo_gen_llm_instruction.md`.

Generator должен быть строгим и рассчитанным на LLM-authored input:

- fail fast
- понятные validation errors
- запрещать неоднозначные конструкции

---

## 15. План внедрения

### Этап 1. CLI skeleton

Сделать:

- crate `repo_gen`
- CLI `--all` и `--bc <name>`
- parser query blocks и metadata
- validator metadata v1

Результат:

- generator умеет читать SQL и падать на ошибках спецификации

### Этап 2. Clorinde introspection

Сделать:

- разбор `crates/clorinde-gen/src/queries/{bc}/*.rs` через `syn`
- извлечение param/row fields
- связка SQL query <-> clorinde types

Результат:

- generator знает точные Rust-типы для текущих BC queries

### Этап 3. Code emission

Сделать:

- генерацию `types.rs`
- генерацию repo-group файлов
- генерацию facade
- generation doc comments со ссылкой на SQL source

Результат:

- generated API компилируется и используется вручную в одном PoC

### Этап 4. Рефакторинг текущих BC

Сделать:

1. добавить metadata во все текущие BC SQL files
2. сгенерировать `warehouse` API
3. заменить ручной repo glue в `warehouse`
4. сгенерировать `catalog` API
5. заменить ручной repo glue в `catalog`
6. удалить/сузить старые `application/repos.rs` и `infrastructure/repos.rs`

Результат:

- существующее приложение работает на generated persistence API

### Этап 5. Финализация workflow

Сделать:

- recipe `just generate`
- CI drift checks
- обновить docs по workflow

Результат:

- новый flow закреплён инструментально

---

## 16. Критерии приёмки

### Функциональные

1. `repo-gen` генерирует API для всех текущих BC queries:
   - `queries/warehouse/inventory.sql`
   - `queries/warehouse/balances.sql`
   - `queries/warehouse/projections.sql`
   - `queries/catalog/products.sql`
2. handlers `warehouse` и `catalog` используют generated API
3. ручной BC repo glue отсутствует как обязательный слой

### Технические

1. generated code checked-in
2. `just generate` детерминирован
3. `cargo check --workspace` проходит после генерации
4. `just test` проходит полностью

### DX

Для нового простого lookup-запроса BC-разработчик делает:

1. пишет SQL block
2. пишет metadata
3. запускает `just generate`
4. получает готовый вызов вида:

```rust
let row = wh.products.find_by_sku(&sku).await?;
```

Без ручного написания:

- `bind = [...]`
- `tid()`
- `DecStr()`
- `parse_dec()`
- `.internal("...")`
- repo macro invocation

---

## 17. Явные решения

В рамках `v1` зафиксированы следующие решения:

- metadata хранится inline в SQL
- `repo-gen` реализуется как отдельный CLI
- `repo-gen` использует `syn` для чтения `clorinde-gen`
- `common/*` не генерируются
- generated code живёт в `crates/{bc}/src/db/generated/`
- стабильный импортный слой остаётся ручным в `crates/{bc}/src/db/mod.rs`
