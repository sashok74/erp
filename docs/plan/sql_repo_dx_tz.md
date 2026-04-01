# ТЗ: упрощение SQL/DAL DX для разработчиков Bounded Context

> Цель: сократить ручной код между `queries/*.sql` и handler-кодом BC.
> Статус: proposal
> Дата: 2026-04-01

---

## 1. Проблема

Сейчас путь `SQL -> вызов в handler` для одного запроса выглядит так:

1. Разработчик пишет SQL в `queries/{bc}/*.sql`
2. Разработчик вручную запускает `just clorinde-generate`
3. Разработчик вручную пишет repo-обёртку или `repo_*` macro invocation
4. Разработчик вручную вызывает repo-метод из handler-а

В результате один короткий SQL-запрос порождает:

- ручной bind-список
- ручные конверсии `tid()/uid()/eid()/DecStr()/parse_dec()`
- ручной `client` plumbing
- ручной `.internal("op_name")`
- лишние DTO-слои в read path

Текущая архитектура уже сильна в части `SQL as source of truth`, но "последняя миля" для BC-разработчика остаётся слишком шумной.

---

## 2. Цель

Сделать так, чтобы обычный BC-разработчик для нового запроса писал:

1. SQL
2. минимальную аннотацию, если она нужна
3. одну строку вызова в handler-е

Целевой UX:

```rust
let row = read.warehouse.balances.get_balance(&query.sku).await?;
```

вместо:

```rust
let row = InventoryRepo::get_balance(client, ctx.tenant_id, &query.sku)
    .await
    .internal("get_balance")?;
```

---

## 3. Нецели

Это ТЗ не предполагает:

- генерацию domain logic или handler-ов
- утечку clorinde-generated wire types в domain layer
- отказ от SQL-first подхода
- отказ от явных BC-границ

---

## 4. Основные принципы

1. `queries/*.sql` остаются source of truth для запросов.
2. BC-разработчик не должен руками передавать `client` и `tenant_id` в каждый repo-вызов.
3. Платформа должна скрыть repeatable plumbing, но не размазать границы между domain/application/infrastructure.
4. Generated row types не должны утекать в domain API без явного решения.
5. Инфраструктурные запросы `common/*` не должны быть частью повседневной модели BC-разработчика.
6. Внутри handler-а разработчик должен видеть BC-local API, а не глобальный service locator вида `read.warehouse.*`.
7. Предпочтение отдаётся явным scoped types и extension traits, а не неявной магии build-time/runtime.

---

## 5. Анализ предложений ревьювера

### R1. Автогенерация repo из SQL

Статус: принять как стратегическую цель.

Это даёт наибольший выигрыш по DX. Сейчас repo-слой дублирует информацию, уже содержащуюся в SQL и в clorinde-generated API:

- имя запроса
- список параметров
- cardinality (`exec`, `opt`, `one`, `all`)
- row mapping

Но реализация должна быть не "магической", а декларативной: SQL + минимальные метаданные должны позволять сгенерировать ergonomic wrapper.

### R2. Убрать `client` из API repo-методов

Статус: принять как ближайший архитектурный шаг.

Это самый важный рефакторинг после codegen. Repo должен работать как `&self`-объект, держащий tenant-scoped db context, а не как набор статических функций.

### R3. Автоматический `.internal()` в repo macros

Статус: принять.

Это дешёвый и полезный quick win. Контекст ошибки должен формироваться в repo-слое автоматически по имени метода/запроса.

### R4. Упростить `tid()/uid()/eid()`

Статус: принять частично.

Важно: `TenantId`, `UserId`, `EntityId` уже реализуют `postgres_types::ToSql` transparently в [types.rs](/home/raa/RustProjects/erp/crates/kernel/src/types.rs#L15). Но текущий clorinde-generated API принимает `&uuid::Uuid`, а не `&impl ToSql`, поэтому простой `impl ToSql` не уберёт `tid()` сам по себе.

Следствие:

- quick win: скрыть `tid()/uid()/eid()` внутри repo/wrapper generation
- не обещать, что один только `ToSql` решит проблему

### R5. Уменьшить количество DTO-слоёв

Статус: принять частично.

Полный отказ от промежуточных DTO нежелателен:

- generated row type — это wire/infrastructure type
- он может содержать transport-детали вроде `String` вместо `BigDecimal`

Но можно убрать один лишний слой там, где BC read-model совпадает с SQL shape, или генерировать BC-owned read DTO из SQL-аннотаций.

---

## 6. Единое решение

Предлагается трёхслойная схема.

### Слой A. SQL + metadata

`queries/*.sql` остаются source of truth.

SQL расширяется минимальными метаданными:

```sql
--! get_balance
--@ kind: opt
--@ dto: BalanceRow
SELECT item_id, sku, balance::TEXT AS "balance: Dec"
FROM warehouse.inventory_balances
WHERE tenant_id = :tenant_id AND sku = :sku;
```

Минимальный набор metadata:

- `kind`: `exec | opt | one | all | scalar`
- `dto`: имя целевого BC-owned DTO, если нужен mapping
- optional field hints для transport-конверсий

### Слой B. Generated wrapper layer

Поверх `clorinde-gen` появляется второй generation step: ergonomic wrapper generation.

Он генерирует:

- tenant-scoped repo objects
- bind plumbing
- mapping из generated row -> BC DTO
- automatic error context
- doc link на исходный SQL

Пример target API:

```rust
pub struct WarehouseReadScope<'a> {
    client: &'a dyn GenericClient,
    tenant_id: TenantId,
}

impl WarehouseReadScope<'_> {
    /// SQL: queries/warehouse/balances.sql --! get_balance
    pub async fn get_balance(&self, sku: &str) -> Result<Option<BalanceRow>, AppError>;
}
```

Важно: это BC-local API. Иерархия вида `read.warehouse.*` допустима только на composition-root уровне, но не как повседневный handler API.

### Слой C. Handler-facing DB context

Handler получает не raw client, а typed context или BC-local scope, построенный поверх базового db context.

Пример:

```rust
let read = self.read.scoped(ctx.tenant_id);
let row = read.balances.get_balance(&query.sku).await?;
```

и для write path:

```rust
let mut db = PgCommandContext::from_uow(uow)?;
let repo = warehouse::db::WriteScope::new(db.client(), ctx.tenant_id);
```

При этом `PgCommandContext` сохраняет platform responsibilities:

- `record_change(...)`
- `emit_events(...)`
- доступ к raw client в edge-cases

BC-specific repo views должны жить в crate самого BC через extension trait или local wrapper, а не в crate `db`.

---

## 7. Целевой API

### 7.1 Query handler

```rust
let read = self.read.scoped(ctx.tenant_id);

let row = read.balances.get_balance(&query.sku).await?;
let projection = read.projections.get_by_sku(&query.sku).await?;
```

### 7.2 Command handler

```rust
let mut db = PgCommandContext::from_uow(uow)?;
{
    let repo = warehouse::db::WriteScope::new(db.client(), ctx.tenant_id);

    let existing = repo.inventory.find_by_sku(sku.as_str()).await?;
    repo.inventory.create_item(item_id, sku.as_str()).await?;
    repo.inventory.insert_movement(NewStockMovement { ... }).await?;
    repo.balances.upsert_balance(UpsertInventoryBalance { ... }).await?;
}

db.record_change(ctx, ...)?;
db.emit_events(&mut aggregate, ctx, "warehouse")?;
```

### 7.3 Правило передачи параметров

Param structs не должны навязываться всем методам.

Правило:

- короткие и очевидные сигнатуры: отдельные аргументы
- длинные write-операции или несколько однотипных полей: input struct

Примеры:

```rust
repo.inventory.create_item(item_id, sku.as_str()).await?;
repo.balances.get_balance(&query.sku).await?;
repo.inventory.insert_movement(NewStockMovement { ... }).await?;
```

Generator должен поддерживать оба режима. Предпочтительно сделать это явным через metadata, а не жёсткой эвристикой.

### 7.4 Sequence API

BC не должен видеть три SQL-вызова `ensure + next + increment`.

Остаётся единый platform service:

```rust
let doc_number = seq.next_number("warehouse.receipt", "ПРХ-").await?;
```

### 7.5 Event handler path

Event handler не должен работать с raw `clorinde` вызовами напрямую.

Целевой стиль:

```rust
db::with_tenant_write_ctx(&self.pool, tenant_id, |tx| {
    Box::pin(async move {
        let repo = warehouse::db::WriteScope::new(tx.client(), tx.tenant_id());
        repo.projections
            .upsert_product_projection(ProductProjectionUpsert { ... })
            .await?;
        Ok(())
    })
}).await
```

Важно: `with_tenant_write_ctx` здесь обозначает typed base write context. Конкретное имя helper-а может отличаться, но event handler path должен получить такой же ergonomic layer, как command handler.

### 7.6 Platform-only APIs

Следующие группы запросов не должны использоваться BC-разработчиком напрямую:

- `common.outbox`
- `common.inbox`
- `common.audit`
- `common.domain_history`
- `common.tenants`

Они оборачиваются в platform services:

- `sys.outbox.poll_unpublished(...)`
- `sys.outbox.mark_published(...)`
- `sys.inbox.is_processed(...)`
- `sys.audit.append(...)`
- `sys.history.append(...)`
- `sys.tenants.get(...)`

---

## 8. Что должно быть сгенерировано автоматически

Для каждого SQL query generation layer должен уметь создавать:

1. ergonomic method name
2. BC-local repo ownership (`inventory`, `balances`, `products`, etc.)
3. bind-список
4. transport conversions
5. row mapping
6. error context
7. doc comment с указанием исходного SQL

Разработчик не должен вручную писать:

- `bind = [...]`
- `tid()/uid()/eid()`
- `DecStr(...)`
- `.internal("...")`
- `clorinde_gen::queries::{bc}::{file}::{query}()`

---

## 9. Что не должно генерироваться автоматически

Не генерируются:

- domain validation
- handler orchestration
- event emission
- domain history semantics
- API response assembly, если он объединяет несколько read models

То есть generation покрывает persistence plumbing, но не бизнес-логику.

---

## 10. План внедрения

### Фаза 1. Quick wins

Срок: короткий.

Сделать:

1. `repo_*` macros автоматически добавляют error context
2. покрыть все типовые patterns реальным использованием `repo_exec!`, `repo_opt!`, `repo_one!`, `repo_all!`
3. унифицировать guide/tooling вокруг единственного workflow `just clorinde-generate`
4. у каждого repo-метода должен быть doc link на SQL source

Ожидаемый эффект:

- меньше `.internal(...)` в handler-ах
- меньше ручного шума без смены архитектурной модели

### Фаза 2. Tenant-scoped repo objects

Срок: средний.

Сделать:

1. ввести typed read/write db contexts или BC-local scopes поверх них
2. перевести repo API со статических функций на `&self`
3. убрать `client` и `tenant_id` из BC handler call sites
4. оставить `PgCommandContext` platform-level базой и строить BC-specific views вне crate `db`

Ожидаемый эффект:

- handler-ы становятся заметно короче
- repo получает нормальную форму для mocking и тестов

### Фаза 3. SQL -> repo wrapper generation

Срок: средний/высокий.

Сделать:

1. определить metadata syntax в SQL
2. реализовать отдельный CLI generator ergonomic wrappers поверх `clorinde-gen`
3. встроить его в единый workflow `just generate` / `just clorinde-generate`
4. генерировать DTO mapping и transport conversions

Предпочтительный mechanism:

- отдельный CLI tool поверх текущего явного generation workflow

Причины:

- не ухудшает compile times
- проще отлаживать, чем `build.rs` и proc-macro
- согласуется с уже существующим explicit codegen pattern в repo

Нежелательно по умолчанию:

- парсить Rust output `clorinde-gen` как текст, если можно опереться на SQL metadata и naming conventions
- уводить решение в proc-macro/build.rs без явной необходимости

Ожидаемый эффект:

- разработчик пишет SQL и почти не пишет repo glue-код

---

## 11. Критерии приёмки

Решение считается успешным, если для нового простого lookup-запроса BC-разработчик:

1. пишет SQL в `queries/{bc}/*.sql`
2. запускает generation
3. получает готовый вызов в стиле:

```rust
let row = read.products.find_by_sku(&sku).await?;
```

и при этом не пишет вручную:

- bind array
- `tid()/uid()/eid()`
- `.internal("...")`
- repo wrapper function

Дополнительные критерии:

- source SQL легко находится из Rust API
- generated API не протекает transport-деталями в application layer
- infrastructure queries скрыты от BC-разработчика

---

## 12. Итоговые приоритеты

### P1. Обязательные

1. `&self`-repo с tenant-scoped context
2. auto error context inside repo layer
3. единый generation workflow

### P2. Следующие

1. wrapper generation поверх SQL
2. скрытие `tid()/uid()/eid()/DecStr()/parse_dec()` внутри generated layer

### P3. Осторожно

1. сокращение DTO-слоёв только без утечки wire types в domain/application
2. не полагаться на один только `ToSql` для решения проблемы `tid()`

---

## 13. Краткая формула решения

Целевое состояние:

`SQL -> clorinde-gen -> generated repo wrappers -> typed db context -> 1-line handler call`

Нежелательное состояние:

`SQL -> clorinde-gen -> ручной repo macro -> ручной bind -> ручной mapping -> handler`
