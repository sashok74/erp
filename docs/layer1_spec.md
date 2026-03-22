# Layer 1 — Kernel: Platform SDK для Bounded Contexts
> Подробное ТЗ | ERP Pilot on Rust
> Дата: 2026-03-22 | Привязка: ADR v1, BCR, DDD, erp_pilot_plan_rust.md
> Предусловие: Layer 0 выполнен, workspace компилируется

---

## Зачем этот слой

Kernel — **Platform SDK**. Это единственный crate, от которого зависят все Bounded Contexts, включая те, что в будущем напишут сторонние разработчики. Он определяет:

- Как мы идентифицируем сущности (ID)
- Как мы описываем намерения (команды) и факты (события)
- Как мы обрабатываем ошибки
- Как мы строим агрегаты (корни консистентности)

### Почему kernel — не Shared Kernel

В классическом DDD Shared Kernel — это общий код между двумя BC, изменяемый по обоюдному согласию. Это работает внутри одной команды, но ломается, когда BC пишут сторонние организации: они не могут менять ядро, а ядро не должно навязывать им бизнес-типы.

Поэтому kernel в нашей архитектуре — **Published Language** / **Platform SDK**:
- Мы контролируем kernel, сторонние разработчики его используют
- Изменения — только с обратной совместимостью (semver)
- В kernel НЕТ бизнес-примитивов (SKU, Quantity, Money) — это доменные типы каждого BC
- В kernel ЕСТЬ контракты (трейты), идентификаторы, ошибки, формат событий

### Где живут Value Objects

**Не в kernel.** Каждый BC определяет свои Value Objects в `domain/value_objects.rs`:

```
warehouse/domain/value_objects.rs    →  SKU, Quantity, LotNumber, Bin, Zone
finance/domain/value_objects.rs      →  Money, AccountCode, FiscalPeriod
manufacturing/domain/value_objects.rs →  BOMQuantity, RoutingStep, WorkCenterCode
third_party_bc/domain/value_objects.rs → свои типы, свои правила валидации
```

Причина: сторонний разработчик пишет BC для ювелирного производства. У него SKU — составной код с пробой золота и сертификатом, 120 символов. Наш kernel с ограничением «≤50» сломал бы его BC. Value objects — часть ubiquitous language конкретного домена.

### Как BC общаются без общих Value Objects

Через integration events с примитивными типами в payload:

```rust
// Warehouse публикует — примитивы, не доменные типы
pub struct GoodsShippedPayload {
    pub sku: String,           // не SKU — просто строка
    pub quantity: BigDecimal,  // не Quantity — просто число
    pub warehouse_id: Uuid,
}

// Finance получает и создаёт свои доменные объекты
let money = Money::new(payload.amount, "RUB")?;  // своя валидация
```

Это **Anti-Corruption Layer** — каждый BC сам валидирует входящие данные по своим правилам.

### Критическое правило

Kernel не зависит ни от какой инфраструктуры. Нет sqlx (кроме `sqlx::Type` для маппинга ID), нет axum, нет tokio. Только чистые типы и трейты. Если в kernel появляется `use tokio::...` — это ошибка проектирования.

---

## Что мы изучим в этом слое (Rust)

| Концепция | Где применяется | Зачем в Rust |
|-----------|----------------|--------------|
| Newtype pattern | TenantId, UserId, EntityId | Type safety: компилятор не даст перепутать tenant_id с user_id |
| Derive macros | `#[derive(Debug, Clone, Serialize)]` | Автогенерация trait implementations |
| Trait definition | Command, DomainEvent, AggregateRoot | Полиморфизм без наследования (Rust не имеет классов) |
| Associated types | `AggregateRoot::Event` | Связать агрегат с его типом событий на уровне типов |
| Generics | `CloudEvent<T>`, `CommandEnvelope<C>` | Параметрический полиморфизм |
| Trait bounds | `T: Serialize + Send + Sync + 'static` | Ограничения на generic-параметры |
| Error handling | thiserror, `#[from]`, Error trait | Идиоматичная обработка ошибок в Rust |
| `std::mem::take` | AggregateRoot::take_events() | Перемещение данных без копирования |
| Module system | `pub mod types; pub mod errors;` | Организация кода внутри crate'а |

---

## Структура файлов после выполнения

```
crates/kernel/src/
├── lib.rs              ← pub mod для всех модулей + re-exports
├── types.rs            ← TenantId, UserId, EntityId, RequestContext
├── errors.rs           ← DomainError, AppError
├── commands.rs         ← Command trait, CommandEnvelope<C>
├── events.rs           ← DomainEvent trait, CloudEvent<T>
└── entity.rs           ← AggregateRoot trait, Entity trait
```

---

## Задача 1.1 — Newtype-обёртки для идентификаторов

### Зачем в ERP

В ERP десятки сущностей с UUID-идентификаторами: tenant, user, warehouse, item, order, document... Если все они `Uuid` — компилятор не спасёт от `fn ship_goods(warehouse_id: Uuid, item_id: Uuid)` → `ship_goods(item_id, warehouse_id)`. Аргументы перепутаны, код компилируется, баг в продакшне.

Newtype pattern: `TenantId(Uuid)` — отдельный тип. Компилятор запрещает передать `TenantId` туда, где ожидается `UserId`.

### Зачем в Rust (что учим)

**Newtype pattern** — одна из главных идиом Rust. Обёртка в tuple struct с одним полем. Zero-cost abstraction: в рантайме это тот же `Uuid`, никакого оверхеда.

**Derive macros** — `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`. Rust не генерирует ничего автоматически. Каждый trait надо либо реализовать вручную, либо derive'нуть. Это явность — ты всегда знаешь, что умеет тип.

**`sqlx::Type`** — derive позволяет sqlx прозрачно маппить newtype в/из PostgreSQL UUID. `#[sqlx(transparent)]` говорит: «это просто обёртка, используй внутренний тип».

**UUID v7** — time-ordered UUID (RFC 9562). Первые 48 бит — timestamp. Это даёт:
- Естественную сортировку по времени создания
- Лучшую производительность B-tree индексов (последовательная вставка, нет random I/O)
- Уникальность без координации (нет sequences, нет lock contention)

### Требования к коду

**Файл: `crates/kernel/src/types.rs`**

Создать:

1. **`TenantId`** — newtype над Uuid
   - `new()` → UUID v7
   - `from_uuid(Uuid)` → конструктор из существующего
   - `as_uuid(&self) -> &Uuid` — доступ к внутреннему значению
   - Derive: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type(transparent)
   - `impl Display` — выводит UUID

2. **`UserId`** — аналогично TenantId

3. **`EntityId`** — аналогично, универсальный ID для доменных сущностей

4. **`RequestContext`** — контекст каждого запроса, проходит через весь pipeline:
   ```rust
   pub struct RequestContext {
       pub tenant_id: TenantId,
       pub user_id: UserId,
       pub correlation_id: Uuid,   // сквозная трассировка через BC
       pub causation_id: Uuid,     // ID команды, породившей событие
       pub timestamp: DateTime<Utc>,
   }
   ```
   - Derive: Debug, Clone
   - `new(tenant_id, user_id)` — заполняет correlation_id, causation_id (UUID v7), timestamp (now)

### Тесты

- `TenantId::new()` возвращает валидный UUID v7
- `TenantId` != `UserId` на уровне типов (compile-time safety, можно описать в doc-comment)
- `TenantId` сериализуется в JSON как строка UUID и десериализуется обратно (serde round-trip)
- `RequestContext::new()` заполняет все поля

---

## Задача 1.2 — Иерархия ошибок

### Зачем в ERP

Трёхуровневая иерархия ошибок отражает три слоя Onion Architecture:

- **DomainError** — нарушение бизнес-правил. «Недостаточно остатков», «Баланс не может быть отрицательным». Это НЕ баг, это нормальная ситуация.
- **AppError** — ошибки уровня приложения. «Нет прав», «Ошибка валидации». Включает DomainError через `#[from]`.
- Инфраструктурные ошибки — в kernel не определяем (он не знает про инфраструктуру), но AppError имеет вариант `Internal(String)` для проброса.

### Зачем в Rust (что учим)

**`thiserror`** — процедурный макрос для реализации `std::error::Error`. Генерирует `Display`, `Error`, `From` по атрибутам. Стандарт де-факто для библиотечных crate'ов (в отличие от `anyhow`, который для приложений и тестов).

**`#[error("...")]`** — шаблон для Display. Может интерполировать поля: `#[error("Недостаточно остатков: требуется {required}, доступно {available}")]`.

**`#[from]`** — автогенерация `impl From<Source> for Target`. Позволяет `?`-operator: `domain_fn()?` автоматически конвертирует `DomainError` в `AppError`.

**Цепочка ошибок** — `source()` метод trait Error возвращает причину. thiserror подставляет автоматически через `#[source]` или `#[from]`.

**Почему не `anyhow` в kernel:** `anyhow::Error` стирает тип — ты не знаешь, какая именно ошибка произошла. В domain layer нужны конкретные типы: `match error { DomainError::InsufficientStock { .. } => ... }`. `anyhow` — для `main()` и тестов, где тип ошибки неважен.

### Требования к коду

**Файл: `crates/kernel/src/errors.rs`**

1. **`DomainError`** — enum с вариантами:
   - `InsufficientStock { required: String, available: String }`
   - `NegativeBalance`
   - `NotFound(String)`
   - `ConcurrencyConflict { expected: i64, actual: i64 }`
   - `BusinessRule(String)` — catch-all для бизнес-правил

2. **`AppError`** — enum:
   - `Domain(#[from] DomainError)` — автоматическая конвертация
   - `Unauthorized(String)`
   - `Validation(String)`
   - `Internal(String)` — для инфраструктурных ошибок

### Тесты

- `DomainError::InsufficientStock` → Display показывает required и available
- `DomainError` конвертируется в `AppError` через `?` (From trait)
- `AppError::Domain(...)` → `source()` возвращает оригинальную DomainError

---

## Задача 1.3 — Trait Command + CommandEnvelope

### Зачем в ERP

Command (CQRS) — намерение изменить состояние системы. «Принять товар», «Отгрузить», «Перевести между складами». Каждая команда — отдельная структура с полями (что принять, сколько, куда). Trait `Command` — общий контракт для всех команд.

`CommandEnvelope<C>` — обёртка: команда + контекст запроса (кто, когда, какой tenant). Конвейер (Layer 5) работает с envelope'ами, а не с голыми командами.

### Зачем в Rust (что учим)

**Trait как интерфейс** — в Rust нет классов и наследования. Trait определяет поведение. `Command` — маркерный trait с одним методом. Все команды реализуют его.

**Trait bounds** — `pub trait Command: Send + Sync + 'static`. Это требования:
- `Send` — можно безопасно передать между потоками (tokio — многопоточный)
- `Sync` — можно безопасно разделить между потоками через &reference
- `'static` — не содержит заимствованных данных с ограниченным lifetime

Почему `'static`: команда может пережить функцию, которая её создала (например, при dispatch через event bus). Без `'static` Rust запретит это.

**Generic struct** — `CommandEnvelope<C: Command>` — параметрический тип. Один envelope для любой команды, но с type safety.

### Требования к коду

**Файл: `crates/kernel/src/commands.rs`**

1. **`Command` trait:**
   ```rust
   pub trait Command: Send + Sync + 'static {
       /// Имя команды для routing, аудита и логирования.
       /// Формат: "bc_name.command_name", например "warehouse.receive_goods"
       fn command_name(&self) -> &'static str;
   }
   ```

2. **`CommandEnvelope<C: Command>`:**
   ```rust
   pub struct CommandEnvelope<C: Command> {
       pub command: C,
       pub context: RequestContext,
   }
   ```
   - `new(command: C, context: RequestContext) -> Self`

### Тесты

- Создать тестовую структуру `DummyCommand`, реализовать `Command`
- `DummyCommand.command_name()` возвращает "test.do_something"
- `CommandEnvelope::new(cmd, ctx)` — поля доступны

---

## Задача 1.4 — Trait DomainEvent + CloudEvent envelope

### Зачем в ERP

Domain Event (DDD) — факт, который произошёл. «Товар принят», «Товар отгружен», «Остатки скорректированы». Событие неизменяемо (immutable) и необратимо — оно уже случилось.

CloudEvents v1.0 — стандарт CNCF для описания событий. Определяет обязательные поля: id, source, type, time. Мы используем его как envelope для integration events (событий, пересекающих границы BC).

Разница:
- **Domain Event** — внутри BC. `GoodsReceived { sku, quantity, ... }`
- **Integration Event** — между BC. CloudEvent с `data: GoodsShippedPayload`. Payload содержит примитивные типы (String, BigDecimal, Uuid), не доменные value objects.

### Зачем в Rust (что учим)

**Trait с Serialize bound** — `pub trait DomainEvent: Serialize + Send + Sync + 'static`. Событие должно сериализоваться (для event store, outbox, шины). Bound `Serialize` гарантирует это на уровне типов.

**Generics с bounds** — `CloudEvent<T: Serialize>` — envelope для любого payload'а, но только если payload сериализуем.

**`serde` round-trip** — serialize → JSON string → deserialize → исходная структура. Это критично для event store: записали событие в PostgreSQL как JSONB, прочитали обратно — должно совпасть.

### Требования к коду

**Файл: `crates/kernel/src/events.rs`**

1. **`DomainEvent` trait:**
   ```rust
   pub trait DomainEvent: Serialize + Send + Sync + 'static {
       /// Тип события для routing и event store.
       /// Формат: "erp.bc_name.event_name.v1"
       fn event_type(&self) -> &'static str;

       /// ID агрегата, породившего событие
       fn aggregate_id(&self) -> Uuid;
   }
   ```

2. **`CloudEvent<T: Serialize>`** — integration event envelope:
   ```rust
   pub struct CloudEvent<T: Serialize> {
       pub specversion: String,        // всегда "1.0"
       pub id: Uuid,                   // уникальный event_id (UUID v7)
       pub source: String,             // "warehouse", "finance", ...
       pub r#type: String,             // "erp.warehouse.goods_received.v1"
       pub time: DateTime<Utc>,
       pub datacontenttype: String,    // "application/json"
       pub subject: Option<String>,    // aggregate_id как строка

       // ERP extensions
       pub tenant_id: TenantId,
       pub correlation_id: Uuid,
       pub causation_id: Uuid,
       pub user_id: UserId,

       // payload — примитивные типы, не доменные value objects
       pub data: T,
   }
   ```
   - `new(source, event_type, subject, data, context: &RequestContext)` — заполняет specversion, id, time, datacontenttype автоматически
   - Derive: Debug, Clone, Serialize, Deserialize (Deserialize требует bound `T: DeserializeOwned`)

### Тесты

- Создать тестовый `TestEvent`, реализовать `DomainEvent` + Serialize
- serde round-trip: TestEvent → JSON → TestEvent
- CloudEvent::new() заполняет specversion = "1.0", id = UUID v7, time ≈ now
- CloudEvent → JSON → CloudEvent: все поля сохраняются

---

## Задача 1.5 — Trait AggregateRoot

### Зачем в ERP

Aggregate Root (DDD) — главная единица консистентности. Внутри агрегата все инварианты выполняются. Извне агрегат — атомарная единица: загружается целиком, сохраняется целиком.

В нашей архитектуре гибридный ES:
- **Warehouse, Finance** — полный Event Sourcing: состояние восстанавливается из цепочки событий. `apply(event)` обновляет состояние. `take_events()` забирает накопленные события для сохранения.
- **Остальные BC** — CRUD + events: состояние хранится в обычных таблицах, но при изменении генерируются события для шины.

Trait `AggregateRoot` покрывает оба случая.

### Зачем в Rust (что учим)

**Associated types** — `type Event: DomainEvent`. Каждый агрегат связан с конкретным типом событий на уровне системы типов. `InventoryItem::Event = WarehouseEvent`. Это строже, чем generic: один агрегат — один тип событий.

**`std::mem::take`** — в `take_events()` забираем вектор событий, оставляя пустой. Без копирования, без clone. Move semantics — одна из суперспособностей Rust.

**`&self` vs `&mut self`** — `apply(&mut self, event)` мутирует состояние. `take_events(&mut self)` забирает данные. Borrow checker гарантирует, что никто другой не читает агрегат во время мутации.

### Требования к коду

**Файл: `crates/kernel/src/entity.rs`**

1. **`Entity` trait** — базовый контракт для всех сущностей:
   ```rust
   pub trait Entity {
       fn id(&self) -> EntityId;
   }
   ```

2. **`AggregateRoot` trait:**
   ```rust
   pub trait AggregateRoot: Send + Sync {
       /// Тип событий этого агрегата
       type Event: DomainEvent;

       /// Применить событие к состоянию (для ES-based агрегатов).
       /// Для CRUD+events — обновляет внутренние поля по событию.
       fn apply(&mut self, event: &Self::Event);

       /// Забрать накопленные неопубликованные события.
       /// После вызова внутренний буфер пуст.
       fn take_events(&mut self) -> Vec<Self::Event>;
   }
   ```

### Тесты

- Создать тестовый агрегат `Counter` с событием `Incremented { value: i32 }`
- `Counter::apply(Incremented { value: 5 })` → внутреннее состояние = 5
- `Counter::take_events()` → возвращает список событий, после чего список пуст
- Повторный `take_events()` → пустой Vec

---

## Задача 1.6 — Финальная сборка: lib.rs + полная проверка

### Зачем

Собрать все модули в единый публичный API crate'а `kernel`. Убедиться что:
- Все модули компилируются вместе
- Нет циклических зависимостей между модулями
- Re-export ключевых типов для удобства: `use kernel::TenantId` вместо `use kernel::types::TenantId`

### Требования к коду

**Файл: `crates/kernel/src/lib.rs`**

```rust
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! ERP Kernel — Platform SDK для Bounded Contexts.
//!
//! Определяет контракты (трейты), идентификаторы, ошибки и формат событий.
//! Не содержит бизнес-примитивов (Value Objects) — они живут в каждом BC.
//!
//! Нулевые зависимости от инфраструктуры.

pub mod commands;
pub mod entity;
pub mod errors;
pub mod events;
pub mod types;

// Re-exports для удобства
pub use commands::{Command, CommandEnvelope};
pub use entity::{AggregateRoot, Entity};
pub use errors::{AppError, DomainError};
pub use events::{CloudEvent, DomainEvent};
pub use types::{EntityId, RequestContext, TenantId, UserId};
```

### Обновить Cargo.toml kernel

Убедиться что зависимости минимальны — только то, что нужно для типов и трейтов:

```toml
[dependencies]
uuid = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
sqlx = { workspace = true }        # только для sqlx::Type на ID
```

Нет bigdecimal — он не нужен в kernel (value objects с BigDecimal живут в BC).

### Финальная проверка

```bash
cargo build --workspace
cargo test -p kernel
cargo clippy -p kernel -- -D warnings
cargo fmt --all -- --check
just check
```

Все тесты зелёные, clippy без warnings, fmt без diff.

---

## Сводка: что получаем после Layer 1

| Файл | Содержание | Тесты |
|------|------------|-------|
| `types.rs` | TenantId, UserId, EntityId, RequestContext | serde round-trip, construction |
| `errors.rs` | DomainError, AppError | Display, From conversion, source() |
| `commands.rs` | Command trait, CommandEnvelope | trait implementation, envelope creation |
| `events.rs` | DomainEvent trait, CloudEvent | serde round-trip, field population |
| `entity.rs` | AggregateRoot trait, Entity trait | apply/take_events lifecycle |
| `lib.rs` | Modules + re-exports | integration compile |

### Чему научились (Rust)

- **Newtype pattern** — type safety через zero-cost обёртки
- **Derive macros** — автогенерация Debug, Clone, Serialize, Deserialize, Hash, Eq
- **Traits** — полиморфизм без наследования, associated types, trait bounds
- **Generics** — параметрические типы с bounds (Serialize, Send, Sync, 'static)
- **Error handling** — thiserror, #[from], цепочки ошибок, Display
- **Move semantics** — std::mem::take, ownership, &mut self
- **Module system** — pub mod, re-exports, видимость (pub vs pub(crate))

### Связь с архитектурой ERP

| Архитектурный элемент | Где реализовано |
|----------------------|-----------------|
| Kernel = Platform SDK | Весь crate: контракты, не бизнес-типы |
| Value objects — в каждом BC | НЕТ value_objects.rs в kernel |
| UUID v7 для всех ID | types.rs |
| CloudEvents v1.0 стандарт | events.rs |
| Onion: Domain без зависимостей | kernel не зависит от infra |
| Hybrid ES (AggregateRoot) | entity.rs — apply/take_events |
| Multi-tenancy (TenantId) | types.rs, RequestContext |
| Сквозная трассировка | RequestContext.correlation_id |
| Anti-Corruption Layer | CloudEvent.data — примитивы, не доменные типы |

---

## Следующий шаг

Layer 1 готов → **Layer 2 (Data Access)**: PgPool, RLS, Unit of Work, SQL-миграции, outbox. Первая связь с PostgreSQL. Crate `db` будет зависеть от `kernel`.
