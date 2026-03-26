# ERP Convenience Layer — Целевая архитектура
> Версия: 1.0 | Дата: 2026-03-26
> Статус: спецификация для реализации
> Контекст: поверх существующего стека (kernel → runtime → event_bus → db → auth → audit → seq_gen → clorinde-gen → warehouse → catalog → gateway)

---

## 1. Проблема

Текущий код работает и проходит все тесты. Но каждый новый command handler содержит ~40% инфраструктурного boilerplate, идентичного во всех handler'ах:

- Downcast `dyn UnitOfWork` → `PgUnitOfWork` (3 строки)
- Split-borrow dance: scope client → collect envelopes → drop → add outbox (15 строк)
- `pool.get() + set_tenant_context()` в каждом query handler (6 строк)
- `.map_err(|e| AppError::Internal(format!("context: {e}")))` — 8+ раз на handler
- `DomainHistoryWriter::record()` с ручной сериализацией + error mapping (5 строк)
- `take_events() → EventEnvelope::from_domain_event() → uow.add_outbox_entry()` цикл (7 строк)

При 5-10 командах на BC × 6 BC = 30-60 handler'ов — это тысячи строк повторяющегося кода.

---

## 2. Целевое состояние

### 2.1. Handler читается как бизнес-логика

```rust
async fn handle(&self, cmd: &Self::Cmd, ctx: &RequestContext, uow: &mut dyn UnitOfWork)
    -> Result<Self::Result, AppError>
{
    // 1. Value objects
    let sku = Sku::new(&cmd.sku)?;
    let qty = Quantity::new(cmd.quantity.clone())?;

    // 2. DB context (одна строка)
    let mut db = PgCommandContext::from_uow(uow)?;

    // 3. Загрузка / создание агрегата
    let (item_id, old_balance) = WarehouseRepo::find_or_create_item(
        db.client(), ctx.tenant_id, &sku,
    ).await.internal("find_or_create")?;

    let mut item = InventoryItem::from_state(
        EntityId::from_uuid(item_id), sku.clone(),
        Quantity::new(old_balance.clone()).internal("balance")?,
    );

    // 4. Domain logic (одна строка — ВСЯ бизнес-логика)
    let doc = db.next_doc_number(ctx.tenant_id, "warehouse.receipt", "ПРХ-").await?;
    let event = item.receive(&qty, doc.clone())?;

    // 5. Persist (repo struct)
    WarehouseRepo::save_receipt(db.client(), ctx, &item, &event, &doc)
        .await.internal("save")?;

    // 6. Cross-cutting (две строки)
    db.record_change(ctx, "inventory_item", item_id,
        "erp.warehouse.goods_received.v1", &old_balance, &event.new_balance).await?;
    db.emit_events(&mut item, ctx, "warehouse")?;

    Ok(ReceiveGoodsResult { item_id, new_balance: event.new_balance.clone(), doc })
}
```

### 2.2. Query handler — три строки вне бизнес-логики

```rust
async fn handle(&self, query: &Self::Query, ctx: &RequestContext)
    -> Result<Self::Result, AppError>
{
    let db = ReadDbContext::acquire(&self.pool, ctx).await?;

    let balance = WarehouseRepo::get_balance(db.client(), ctx.tenant_id, &query.sku)
        .await.internal("balance")?;

    let product_name = WarehouseRepo::get_product_name(db.client(), ctx.tenant_id, &query.sku)
        .await.internal("projection")?;

    Ok(BalanceResult { sku: query.sku.clone(), balance, product_name })
}
```

### 2.3. Route registration — одна строка на endpoint

```rust
pub fn routes<UF: UnitOfWorkFactory + 'static>(
    pipeline: Arc<CommandPipeline<UF>>,
    pool: Arc<PgPool>,
) -> Router {
    BcRouter::new(pipeline, pool.clone())
        .command::<ReceiveGoodsHandler>(POST, "/receive")
        .command::<ShipGoodsHandler>(POST, "/ship")
        .command::<TransferStockHandler>(POST, "/transfer")
        .command::<AdjustInventoryHandler>(POST, "/adjust")
        .query::<GetBalanceHandler>(GET, "/balance")
        .query::<GetMovementsHandler>(GET, "/movements")
        .build()
}
```

---

## 3. Компоненты convenience layer

### 3.1. `IntoInternal` (crate: kernel)

Файл: `crates/kernel/src/error_ext.rs`

```rust
pub trait IntoInternal<T> {
    fn internal(self, context: &str) -> Result<T, AppError>;
}

impl<T, E: std::fmt::Display> IntoInternal<T> for Result<T, E> {
    fn internal(self, context: &str) -> Result<T, AppError> {
        self.map_err(|e| AppError::Internal(format!("{context}: {e}")))
    }
}
```

Реэкспорт: `pub use error_ext::IntoInternal;` в `kernel/src/lib.rs`.

Заменяет: `.map_err(|e| AppError::Internal(format!("save_movement: {e}")))?`
На: `.internal("save_movement")?`

### 3.2. `PgCommandContext` (crate: db)

Файл: `crates/db/src/context.rs`

```rust
pub struct PgCommandContext<'a> {
    inner: &'a mut PgUnitOfWork,
}
```

Методы:
- `from_uow(uow: &mut dyn UnitOfWork) -> Result<Self, AppError>` — downcast
- `client(&self) -> &deadpool_postgres::Object` — доступ к PostgreSQL внутри TX
- `emit_events<A: AggregateRoot>(&mut self, aggregate, ctx, source) -> Result<(), AppError>` — take_events + outbox
- `record_change<O: Serialize, N: Serialize>(...)  -> Result<(), AppError>` — domain history (делегирует в `DomainHistoryWriter::record_change`)
- `next_doc_number(&self, tenant_id, seq_name, prefix) -> Result<String, AppError>` — обёртка seq_gen

Зависимость: `PgUnitOfWork` получает `pub(crate) fn push_outbox_entry()` для прямого доступа из `PgCommandContext`.

### 3.3. `ReadDbContext` (crate: db)

Файл: `crates/db/src/context.rs` (тот же файл)

```rust
pub struct ReadDbContext {
    client: deadpool_postgres::Object,
}
```

Методы:
- `acquire(pool: &PgPool, ctx: &RequestContext) -> Result<Self, AppError>` — checkout + RLS
- `acquire_arc(pool: &Arc<PgPool>, ctx: &RequestContext) -> Result<Self, AppError>` — convenience
- `client(&self) -> &deadpool_postgres::Object` — доступ к клиенту

### 3.4. `DomainHistoryWriter::record_change` (crate: audit)

Файл: `crates/audit/src/history.rs`

Новый метод рядом с существующим `record()`:

```rust
pub async fn record_change<O: Serialize, N: Serialize>(
    client: &impl GenericClient,
    ctx: &RequestContext,
    entity_type: &str,
    entity_id: Uuid,
    event_type: &str,
    old: Option<&O>,
    new: Option<&N>,
) -> Result<i64, AppError>
```

Автоматическая сериализация в `serde_json::Value` + маппинг ошибок в `AppError`.

### 3.5. `BcRouter` (crate: runtime)

Файл: `crates/runtime/src/router.rs`

Builder pattern для регистрации endpoint'ов BC:

```rust
pub struct BcRouter<UF: UnitOfWorkFactory> {
    pipeline: Arc<CommandPipeline<UF>>,
    pool: Arc<PgPool>,
    router: Router,
}
```

Методы:
- `new(pipeline, pool) -> Self`
- `command<H: CommandHandler>(self, method, path) -> Self` — регистрация command endpoint
- `query<H: QueryHandler>(self, method, path) -> Self` — регистрация query endpoint
- `build(self) -> Router`

Traits для связки HTTP body → command / query params:
```rust
pub trait FromHttpBody: Sized {
    type Body: DeserializeOwned;
    fn from_body(body: Self::Body) -> Self;
}

pub trait FromHttpQuery: Sized {
    type Params: DeserializeOwned;
    fn from_params(params: Self::Params) -> Self;
}
```

Handler авторегистрации (`BcRouter` создаёт handler'ы через `H::new(pool.clone())`):
```rust
pub trait CommandHandlerFactory: CommandHandler {
    fn new(pool: Arc<PgPool>) -> Self;
}
```

Автоматическая обработка внутри:
- Извлечение `RequestContext` из request extensions
- Десериализация body (command) или query params (query)
- Вызов `pipeline.execute()` или `handler.handle()`
- Маппинг `AppError` → HTTP response (через `AppErrorResponse`)

---

## 4. Структура файлов (что меняется)

```
crates/
├── kernel/src/
│   ├── error_ext.rs          ← NEW: IntoInternal trait
│   └── lib.rs                ← MODIFIED: pub mod error_ext + re-export
│
├── db/src/
│   ├── context.rs            ← NEW: PgCommandContext + ReadDbContext
│   ├── uow.rs                ← MODIFIED: pub(crate) push_outbox_entry
│   └── lib.rs                ← MODIFIED: pub mod context + re-exports
│
├── audit/src/
│   └── history.rs            ← MODIFIED: + record_change method
│
├── runtime/src/
│   ├── router.rs             ← NEW: BcRouter + FromHttpBody/FromHttpQuery traits
│   └── lib.rs                ← MODIFIED: pub mod router + re-exports
│
├── warehouse/src/
│   ├── application/
│   │   ├── commands/
│   │   │   └── receive_goods.rs  ← REFACTORED: uses all helpers
│   │   └── queries/
│   │       └── get_balance.rs    ← REFACTORED: uses ReadDbContext
│   ├── infrastructure/
│   │   ├── repos.rs              ← REFACTORED: domain-friendly methods
│   │   └── routes.rs             ← REFACTORED: uses BcRouter
│   └── module.rs                 ← SIMPLIFIED
│
├── catalog/src/
│   ├── application/
│   │   ├── commands/
│   │   │   └── create_product.rs ← REFACTORED: uses all helpers
│   │   └── queries/
│   │       └── get_product.rs    ← REFACTORED: uses ReadDbContext
│   ├── infrastructure/
│   │   ├── repos.rs              ← REFACTORED
│   │   └── routes.rs             ← REFACTORED: uses BcRouter
│   └── module.rs                 ← SIMPLIFIED
│
└── gateway/src/
    └── main.rs                   ← SIMPLIFIED: less boilerplate per BC
```

---

## 5. Инварианты (не меняются)

- `CommandPipeline` — без изменений. Auth → hooks → TX → handler → commit → audit.
- `UnitOfWork` trait — без изменений. `add_outbox_entry` остаётся для тестов и pipeline.
- `EventBus`, `EventEnvelope`, `InProcessBus` — без изменений.
- `PgAuditLog`, `JwtPermissionChecker`, `JwtService` — без изменений.
- Все миграции, SQL-запросы, clorinde-gen — без изменений.
- Integration-тесты — без изменений API handler'ов.
- `InMemoryUnitOfWork` + stubs — без изменений (для unit-тестов).

---

## 6. Диаграмма зависимостей (convenience layer)

```
┌──────────────────────────────────────────────────────────────────┐
│  Command Handler (BC crate)                                      │
│                                                                  │
│  uses: PgCommandContext, IntoInternal                            │
│  calls: db.client(), db.emit_events(), db.record_change()       │
│  calls: Repo methods, seq_gen                                    │
│  returns: Result<HandlerResult, AppError>                        │
└────────────┬─────────────────────────────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────────────────────────────┐
│  PgCommandContext (db crate)                                     │
│                                                                  │
│  wraps: &mut PgUnitOfWork                                        │
│  provides: client(), emit_events(), record_change(),             │
│            next_doc_number()                                     │
│  delegates: DomainHistoryWriter::record_change (audit crate)     │
│  delegates: PgSequenceGenerator::next_value (seq_gen crate)      │
│  uses: push_outbox_entry() (internal to db crate)                │
└────────────┬─────────────────────────────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────────────────────────────┐
│  PgUnitOfWork (db crate)                                         │
│                                                                  │
│  owns: deadpool_postgres::Object (active TX)                     │
│  has: outbox_entries: Vec<EventEnvelope>                          │
│  public: add_outbox_entry() — trait method for pipeline/tests    │
│  pub(crate): push_outbox_entry() — for PgCommandContext          │
│  on commit: flush_outbox → COMMIT                                │
│  on error: ROLLBACK                                              │
└──────────────────────────────────────────────────────────────────┘
```

---

## 7. Метрики успеха

| Метрика | До | После |
|---------|-----|-------|
| Строк в receive_goods handler body | 90 | ~40 |
| `.map_err(...)` в одном handler | 8 | 0 |
| Split-borrow dance | 15 строк | 0 |
| Pool + RLS boilerplate в query handler | 6 строк | 1 (`ReadDbContext::acquire`) |
| Route registration на endpoint | ~20 строк | 1 строка |
| Новый command handler (skeleton) | ~100 строк | ~50 строк |
| Новый query handler (skeleton) | ~60 строк | ~25 строк |
| compile — должен проходить | ✓ | ✓ |
| Все integration-тесты | ✓ | ✓ |

---

## 8. Что намеренно НЕ входит в scope

- ORM / query builder — clorinde остаётся SQL-first
- Генерация SQL из таблиц (`erp-sql scaffold`) — отдельный будущий CLI
- Генерация BC (`erp-scaffold crud`) — отдельный будущий CLI
- Repo input structs (замена 10-параметровых методов) — отдельный рефакторинг
- Dynamic RBAC из БД — текущий статический PermissionMap достаточен для MVP
- Extensions runtime (Lua/WASM) — NoopExtensionHooks пока остаётся
