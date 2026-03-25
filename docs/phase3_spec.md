# Phase 3 — Warehouse Vertical Slice: первый бизнес-код
> Спецификация | ERP Pilot on Rust
> Дата: 2026-03-24 | Привязка: EXECUTION_PLAN Phase 3, engineering_invariants.md
> Предусловие: Phase 0–2 готовы (kernel, event_bus, runtime, auth, db, audit, seq_gen)

---

## Зачем этот шаг

Всё что было до сих пор — **механизмы**: pipeline, event bus, UoW, RLS, outbox, audit, sequences. Теперь мы проверяем, что все механизмы работают **вместе** на реальном бизнес-сценарии.

Warehouse BC — reference implementation. Минимальный домен, но **полный путь** от HTTP до опубликованного события. После Phase 3 этот путь становится шаблоном для генерации новых BC.

### До Phase 3

```
Механизмы есть, но бизнес-кода нет.
Pipeline тестировался с EchoHandler.
Outbox relay публикует тестовые события.
```

### После Phase 3

```
POST /api/warehouse/receive {"sku":"BOLT-42","quantity":100}
  → auth middleware → JWT → RequestContext
  → pipeline.execute(ReceiveGoodsHandler)
    → RBAC: warehouse.receive_goods ✓
    → PgUoW: BEGIN + SET tenant_id
    → handler:
        load/create InventoryItem
        item.receive(qty) → GoodsReceived event
        INSERT stock_movements (balance_after)
        UPSERT inventory_balances (SELECT FOR UPDATE)
        seq_gen: номер документа ПРХ-000001
        domain_history: old/new state
        uow.add_outbox_entry(GoodsReceived)
    → PgUoW: COMMIT
    → PgAuditLog: INSERT audit_log
    → relay → bus → subscribers
  → HTTP 200 {"movement_id":"...","new_balance":"100","doc_number":"ПРХ-000001"}
```

---

## Бизнес-домен (минимальный)

Только то, что нужно для проверки всех механизмов. Ничего лишнего.

| Элемент | Что | Правило |
|---------|-----|---------|
| **InventoryItem** | Агрегат: sku + balance | `balance >= 0` (единственный инвариант) |
| **StockMovement** | Append-only факт: qty, balance_after | Неизменяемый, только INSERT |
| **Sku** | Value object: String, ≤50 chars, непустой | Валидация при создании |
| **Quantity** | Value object: BigDecimal, >= 0 для балансов | Арифметика, Display |
| **GoodsReceived** | Domain event | Опубликовать через outbox |
| **ReceiveGoods** | Command + Handler | Полный canonical write path |
| **GetBalance** | Query + Handler | Read из inventory_balances |

**Чего НЕТ:** партий, сроков годности, резервов, FIFO/FEFO, зон, ячеек, множественных складов. Всё это — бизнес-усложнения, которые добавляются позже без изменения механизмов.

---

## Структура файлов

```
crates/warehouse/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── aggregates.rs        ← InventoryItem (AggregateRoot impl)
│   │   ├── events.rs            ← GoodsReceived (DomainEvent impl)
│   │   ├── value_objects.rs     ← Sku, Quantity (BC-owned, не kernel!)
│   │   └── errors.rs            ← WarehouseDomainError
│   ├── application/
│   │   ├── mod.rs
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   └── receive_goods.rs ← Command + Handler
│   │   └── queries/
│   │       ├── mod.rs
│   │       └── get_balance.rs   ← Query + Handler
│   ├── infrastructure/
│   │   ├── mod.rs
│   │   ├── repos.rs             ← PgInventoryRepo
│   │   └── routes.rs            ← axum HTTP handlers
│   └── module.rs                ← WarehouseModule (BoundedContextModule)
│
├── BC_CONTEXT.md                ← паспорт контекста для AI-агента
│
├── tests/
│   └── integration.rs           ← E2E тесты с реальной БД

migrations/warehouse/
├── 001_create_inventory_items.sql
├── 002_create_stock_movements.sql
├── 003_create_inventory_balances.sql
└── 004_rls_warehouse.sql

queries/warehouse/
├── inventory.sql               ← SQL для Clorinde (или ручной crate)
└── balances.sql
```

---

## Задачи

### 3.1 — SQL-миграции: warehouse schema

Четыре файла в `migrations/warehouse/`:

**001_create_inventory_items.sql** — реестр товаров на складе:
```sql
CREATE TABLE warehouse.inventory_items (
    tenant_id       UUID NOT NULL,
    id              UUID NOT NULL,
    sku             TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, sku)
);
```

**002_create_stock_movements.sql** — append-only журнал движений:
```sql
CREATE TABLE warehouse.stock_movements (
    tenant_id       UUID NOT NULL,
    id              UUID NOT NULL,
    item_id         UUID NOT NULL,
    event_type      TEXT NOT NULL,       -- "goods_received", "goods_shipped"
    quantity        NUMERIC(18,4) NOT NULL,
    balance_after   NUMERIC(18,4) NOT NULL,
    doc_number      TEXT,
    correlation_id  UUID NOT NULL,
    user_id         UUID NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id)
);
```

**003_create_inventory_balances.sql** — текущие остатки (проекция):
```sql
CREATE TABLE warehouse.inventory_balances (
    tenant_id       UUID NOT NULL,
    item_id         UUID NOT NULL,
    sku             TEXT NOT NULL,
    balance         NUMERIC(18,4) NOT NULL DEFAULT 0,
    last_movement_id UUID,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, item_id)
);
```

**004_rls_warehouse.sql** — RLS на все таблицы:
```sql
ALTER TABLE warehouse.inventory_items ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON warehouse.inventory_items
    USING (tenant_id = common.current_tenant_id());

ALTER TABLE warehouse.stock_movements ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON warehouse.stock_movements
    USING (tenant_id = common.current_tenant_id());

ALTER TABLE warehouse.inventory_balances ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON warehouse.inventory_balances
    USING (tenant_id = common.current_tenant_id());
```

### 3.2 — Domain: Value Objects (Sku, Quantity)

**Файл:** `warehouse/src/domain/value_objects.rs`

Value objects живут **в BC, не в kernel** (решение Platform SDK).

- **Sku(String)** — приватное поле, `new()` → Result: непустой, ≤50 chars
- **Quantity(BigDecimal)** — `new()`, `zero()`, `is_negative()`, `is_zero()`, Add, Sub, Display, PartialEq

Тесты: валидация, арифметика, serde round-trip.

### 3.3 — Domain: WarehouseDomainError

**Файл:** `warehouse/src/domain/errors.rs`

```rust
#[derive(Debug, Clone, Error)]
pub enum WarehouseDomainError {
    #[error("SKU невалиден: {0}")]
    InvalidSku(String),
    #[error("Количество не может быть отрицательным")]
    NegativeQuantity,
    #[error("Недостаточно остатков: требуется {required}, доступно {available}")]
    InsufficientStock { required: String, available: String },
}
```

Конвертация в `kernel::DomainError` через `From` или mapping в handler.

### 3.4 — Domain: GoodsReceived event

**Файл:** `warehouse/src/domain/events.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodsReceived {
    pub item_id: Uuid,
    pub sku: String,          // примитив, не Sku — для сериализации
    pub quantity: BigDecimal,  // примитив, не Quantity
    pub new_balance: BigDecimal,
    pub doc_number: String,
    pub warehouse_id: Uuid,    // placeholder, пока один склад
}
```

Реализует `kernel::DomainEvent`:
- `event_type()` → `"erp.warehouse.goods_received.v1"`
- `aggregate_id()` → `item_id`

### 3.5 — Domain: InventoryItem aggregate

**Файл:** `warehouse/src/domain/aggregates.rs`

```rust
pub struct InventoryItem {
    id: EntityId,
    sku: Sku,
    balance: Quantity,
    pending_events: Vec<GoodsReceived>,
}
```

Методы:
- `new(id, sku)` — создать с balance = 0
- `receive(quantity: Quantity, doc_number: String, warehouse_id: Uuid) → Result<(), WarehouseDomainError>` — увеличить balance, создать GoodsReceived event
- `balance()` → &Quantity
- `sku()` → &Sku

Impl `AggregateRoot`:
- `apply(event)` — обновить balance из event
- `take_events()` — забрать pending events

**Инвариант:** `balance >= 0`. Для receive — всегда true (добавляем). Для будущего ship — проверка.

### 3.6 — Infrastructure: PgInventoryRepo

**Файл:** `warehouse/src/infrastructure/repos.rs`

Repository загружает/сохраняет агрегат через SQL:
- `find_by_sku(client, tenant_id, sku) → Option<InventoryItem>` — SELECT из inventory_items + inventory_balances
- `save_item(client, item) → Result` — INSERT/UPDATE inventory_items
- `save_movement(client, movement_data) → Result` — INSERT stock_movements
- `upsert_balance(client, item_id, sku, balance) → Result` — INSERT ON CONFLICT UPDATE inventory_balances
- `get_balance(client, tenant_id, sku) → Option<BalanceRow>` — для query handler

SQL через `tokio_postgres::GenericClient` (тот же client что в UoW).

### 3.7 — Application: ReceiveGoodsCommand + Handler

**Файл:** `warehouse/src/application/commands/receive_goods.rs`

Command:
```rust
#[derive(Debug, Deserialize)]
pub struct ReceiveGoodsCommand {
    pub sku: String,
    pub quantity: BigDecimal,
}

impl Command for ReceiveGoodsCommand {
    fn command_name(&self) -> &'static str { "warehouse.receive_goods" }
}
```

Handler:
```rust
impl CommandHandler for ReceiveGoodsHandler {
    type Cmd = ReceiveGoodsCommand;
    type Result = ReceiveGoodsResult;

    async fn handle(&self, cmd, ctx, uow) -> Result<ReceiveGoodsResult, AppError> {
        // 1. Validate: Sku::new, Quantity::new
        // 2. Downcast uow → PgUnitOfWork → client
        // 3. Repo: find_by_sku или create new InventoryItem
        // 4. item.receive(qty, doc_number, warehouse_id)
        // 5. Repo: save_movement + upsert_balance
        // 6. DomainHistoryWriter::record(client, ctx, ...)
        // 7. SeqGen: next_value(client, tenant, "warehouse.receipt", "ПРХ-")
        // 8. EventEnvelope::from_domain_event → uow.add_outbox_entry
        // 9. Return ReceiveGoodsResult { movement_id, new_balance, doc_number }
    }
}
```

### 3.8 — Application: GetBalanceQuery + Handler

**Файл:** `warehouse/src/application/queries/get_balance.rs`

```rust
pub struct GetBalanceQuery { pub sku: String }
pub struct BalanceResult { pub sku: String, pub balance: BigDecimal }

impl QueryHandler for GetBalanceHandler {
    // SELECT из inventory_balances WHERE sku = $1
}
```

### 3.9 — Infrastructure: axum routes

**Файл:** `warehouse/src/infrastructure/routes.rs`

```
POST /receive  → deserialize ReceiveGoodsCommand → pipeline.execute
GET  /balance  → deserialize query params → GetBalanceHandler
```

### 3.10 — WarehouseModule: BoundedContextModule impl

**Файл:** `warehouse/src/module.rs`

```rust
impl BoundedContextModule for WarehouseModule {
    fn name(&self) -> &'static str { "warehouse" }
    fn routes(&self) → Router { ... }
    async fn register_handlers(&self, bus) { ... }
}
```

### 3.11 — BC_CONTEXT.md: паспорт контекста

Для будущего AI-агента — описание BC: commands, events, rules, aggregates.

### 3.12 — Integration тесты: E2E canonical write path

**Самый важный тест:** полный путь от command до event publish.

```
1. Apply warehouse migrations
2. Create pipeline с реальными зависимостями
3. Start outbox relay
4. pipeline.execute(ReceiveGoodsCommand{sku:"BOLT-42", quantity:100}, ctx)
5. Assert:
   ✓ Result: new_balance = 100, doc_number = "ПРХ-000001"
   ✓ warehouse.inventory_balances: balance = 100
   ✓ warehouse.stock_movements: 1 row, balance_after = 100
   ✓ common.outbox: 1 row, event_type = "erp.warehouse.goods_received.v1"
   ✓ common.audit_log: 1 row
   ✓ common.domain_history: 1 row
6. Wait relay → bus subscriber called
```

---

## Acceptance Criteria

| Проверка | Ожидание |
|----------|----------|
| `cargo test -p warehouse` | Domain unit tests pass (без БД) |
| `cargo test -p warehouse --test integration` | E2E с реальной PostgreSQL |
| InventoryItem: receive 100 → balance = 100 | Инвариант balance >= 0 |
| Second receive 50 → balance = 150 | Cumulative |
| stock_movements: 2 rows, balance_after correct | Append-only |
| outbox: event_type = "erp.warehouse.goods_received.v1" | Event published |
| audit_log: command = "warehouse.receive_goods" | Audit trail |
| domain_history: old_state → new_state | Change tracking |
| doc_number: "ПРХ-000001", "ПРХ-000002" | Gap-free sequencing |
| RLS: tenant A не видит данные tenant B | Tenant isolation |
| GetBalance query: returns correct balance | Read path |
| Unauthorized user → 403, no side effects | RBAC enforcement |

## Engineering Invariants — все зацементированы

После Phase 3 каждый инвариант проверен на реальном бизнес-сценарии, а не на тестовых stubs:

1. ✓ Write через Pipeline (ReceiveGoodsHandler)
2. ✓ PermissionChecker (warehouse.receive_goods)
3. ✓ Audit + domain history
4. ✓ Events через outbox → bus
5. ✓ SQL только в warehouse.* (нет cross-BC reads)
6. ✓ Event versioned: erp.warehouse.goods_received.v1
7. ✓ tenant_id + RLS на всех таблицах
8. ✓ Запись через UoW
9. ✓ RequestContext в handler
10. ✓ Handler не знает про роли

---

## Что изучим (Rust)

| Концепция | Где | Зачем |
|-----------|-----|-------|
| BigDecimal в domain | Quantity value object | Точная арифметика для денег/количеств |
| Onion в действии | domain/ → application/ → infrastructure/ | Реальное разделение слоёв |
| `downcast_mut::<PgUnitOfWork>()` | handler → SQL | Доступ к PG client через trait object |
| axum extractors | routes.rs | JSON body, query params, Extension\<RequestContext\> |
| `INSERT ON CONFLICT` | upsert_balance | Идемпотентный upsert |
| `SELECT FOR UPDATE` | upsert_balance | Pessimistic lock для balance |
| Module composition | WarehouseModule → gateway | Plug-in BC registration |

---

## Следующий шаг

Phase 3 готова → **Phase 4: Catalog BC + cross-context projection** — второй BC, inter-BC events, local projection. Warehouse проверяет SKU через свою проекцию, не читая catalog.* таблицы.
