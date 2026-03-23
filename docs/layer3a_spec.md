# Layer 3a — Event Bus: трейты + InProcessBus
> Подробное ТЗ | ERP Pilot on Rust
> Дата: 2026-03-22 | Привязка: ADR v1, BCR, architecture_diagrams.md
> Предусловие: Layer 1 (Kernel) выполнен

---

## Зачем этот слой

Event Bus — нервная система ERP. Через него Bounded Contexts общаются друг с другом, не зная друг о друге напрямую. Warehouse публикует `GoodsShipped` — Finance подписан и создаёт GL-проводку. Warehouse не знает о существовании Finance. Finance не вызывает Warehouse.

В этом слое мы делаем **только абстракции и in-memory реализацию**. Никакой БД, никакого outbox. Outbox Relay (запись в PostgreSQL, at-least-once delivery) — это Layer 3b, он придёт позже, когда подключим БД.

### Почему сейчас, а не после БД

Потому что **Command Pipeline (Layer 5a) зависит от EventBus trait**, а не от PostgreSQL. Если мы сначала сделаем трейты шины — Pipeline можно собрать и протестировать без БД. Это Onion Architecture в действии: сначала порты (traits), потом адаптеры (реализации).

### Заменяемость: tokio channels → RabbitMQ / NATS / Kafka

`EventBus` — это trait. Сейчас его реализует `InProcessBus` на tokio channels. При переходе к микросервисам:

```
InProcessBus (tokio::sync::broadcast)   ← modular monolith
RabbitMqBus (lapin crate)               ← микросервисы, вариант 1
NatsBus (async-nats crate)              ← микросервисы, вариант 2
KafkaBus (rdkafka crate)                ← микросервисы, вариант 3
```

Domain и Application слои не меняются. Меняется только конфигурация в точке входа (gateway). Это возможно **только** потому что шина — trait, а не конкретная реализация.

---

## Что мы изучим в этом слое (Rust)

| Концепция | Где применяется | Зачем в Rust |
|-----------|----------------|--------------|
| `async trait` | EventBus, EventHandler | Асинхронные методы в trait'ах |
| `Arc<dyn Trait>` | Bus хранит handler'ы как trait objects | Динамический полиморфизм, type erasure |
| `Box<dyn Any>` | Хранение разнотипных handler'ов | Type erasure + downcast |
| `tokio::sync::broadcast` | InProcessBus — канал pub/sub | Async channels, многопоточность |
| `tokio::sync::RwLock` | Регистрация handler'ов | Async read-write lock |
| `TypeId` | Routing событий по типу | Runtime type information в Rust |
| `downcast_ref` | Извлечение конкретного типа из `dyn Any` | Safe dynamic typing |
| `#[async_trait]` или нативный async | Async в trait'ах | edition 2024 vs async_trait crate |
| `Send + Sync + 'static` bounds | Handler'ы в многопоточном runtime | Потокобезопасность через типы |
| `Arc`, `Clone` для shared state | Bus используется из разных частей системы | Shared ownership |

---

## Структура файлов после выполнения

```
crates/event_bus/src/
├── lib.rs              ← pub mod + re-exports
├── traits.rs           ← EventBus trait, EventHandler trait
├── bus.rs              ← InProcessBus (tokio broadcast)
├── registry.rs         ← HandlerRegistry (типизированная регистрация)
└── envelope.rs         ← EventEnvelope (обёртка для передачи через bus)
```

---

## Задача 3a.1 — EventHandler trait

### Зачем в ERP

Каждый BC подписывается на события других BC. Finance подписан на `GoodsShipped` из Warehouse. Planning подписан на `SalesOrderConfirmed` из Sales. Handler — это функция-обработчик, которая вызывается когда приходит событие нужного типа.

### Зачем в Rust (что учим)

**async trait** — Handler должен быть async (внутри может обращаться к БД, вызывать другие сервисы). В Rust 1.75+ поддерживается нативный `async fn` в trait'ах, но с ограничениями для `dyn Trait`. Два подхода:

- `#[async_trait]` crate — добавляет `Box<dyn Future>` под капотом, работает с `dyn`. Проверенное решение, используется в axum, tonic, sqlx.
- Нативный `async fn in trait` — чище, но `dyn EventHandler` пока не работает без workaround'ов (trait не object-safe с async fn).

Для нашего случая `#[async_trait]` — правильный выбор, потому что нам нужны trait objects (`Arc<dyn EventHandler>`). Это стандартный подход в Rust community на 2026 год для случаев, когда нужен dynamic dispatch.

**`Send + Sync + 'static` bounds** — Handler живёт в `Arc`, разделяется между потоками tokio. Rust требует явно указать, что это безопасно.

### Требования к коду

**Файл: `crates/event_bus/src/traits.rs`**

```rust
/// Обработчик событий. Реализуется подписчиками.
///
/// Пример: Finance реализует EventHandler для GoodsShipped,
/// чтобы создать GL-проводку при отгрузке со склада.
#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    /// Тип события, на которое подписан handler.
    /// Определяет routing: bus доставляет только события нужного типа.
    type Event: DomainEvent;

    /// Обработать событие. Может быть async (обращение к БД, вызов сервисов).
    /// Возвращает Result — ошибка попадёт в retry/DLQ (Layer 3b).
    async fn handle(&self, event: &Self::Event) -> Result<(), anyhow::Error>;
}
```

### Тесты

- Создать тестовый `TestHandler` для тестового `TestEvent`
- `handle()` вызывается, получает событие, возвращает Ok
- `handle()` может вернуть Err

---

## Задача 3a.2 — EventBus trait

### Зачем в ERP

EventBus — центральный компонент: принимает события от издателей (publishers) и доставляет подписчикам (subscribers). Trait определяет контракт, который не зависит от реализации.

Два режима доставки:
- **`publish`** — fire-and-forget, async. Для side effects (нотификации, аналитика, синхронизация с 1С). Издатель не ждёт завершения handler'ов.
- **`publish_and_wait`** — sync в рамках pipeline. Для domain events внутри одной TX (когда handler может reject операцию). Издатель ждёт завершения всех handler'ов.

### Зачем в Rust (что учим)

**Trait as abstraction** — `EventBus` trait позволяет подставить любую реализацию: InProcessBus, RabbitMqBus, MockBus для тестов. Это Dependency Inversion Principle (DIP) — зависимость от абстракции, не от конкретики.

**`Arc<dyn EventBus>`** — Bus используется из многих мест (Pipeline, Gateway, background workers). `Arc` даёт shared ownership, `dyn` — dynamic dispatch. В Rust это единственный способ иметь «один объект, много владельцев» для trait object'а.

### Требования к коду

**Файл: `crates/event_bus/src/traits.rs`** (дополнение)

```rust
/// Шина событий. Центральный компонент межмодульного взаимодействия.
///
/// Реализации:
/// - InProcessBus: tokio channels (modular monolith)
/// - В будущем: RabbitMqBus, NatsBus, KafkaBus (микросервисы)
///
/// Domain и Application слои зависят от этого trait,
/// не от конкретной реализации.
#[async_trait]
pub trait EventBus: Send + Sync + 'static {
    /// Опубликовать событие. Fire-and-forget.
    /// Handler'ы вызываются async, издатель не ждёт.
    async fn publish(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error>;

    /// Опубликовать и дождаться обработки всеми handler'ами.
    /// Используется для domain events внутри TX.
    async fn publish_and_wait(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error>;

    /// Зарегистрировать обработчик. Вызывается при старте приложения.
    async fn subscribe(
        &self,
        event_type: &'static str,
        handler: Arc<dyn ErasedEventHandler>,
    );
}
```

### Тесты

- Trait компилируется с нужными bounds
- Можно создать `Arc<dyn EventBus>` (object-safe)

---

## Задача 3a.3 — EventEnvelope

### Зачем в ERP

Событие путешествует через bus в «конверте»: payload (сериализованные данные) + метаданные (тип, source, tenant_id, correlation_id). Конверт — transport-level обёртка, не привязанная к конкретному типу события. Bus работает с конвертами, handler распаковывает payload в свой тип.

### Зачем в Rust (что учим)

**Type erasure** — bus не знает конкретные типы событий (их десятки). Он работает с `EventEnvelope`, где payload — `serde_json::Value` (стёртый тип). Handler при получении десериализует payload в конкретный тип.

Альтернатива — `Box<dyn Any>`, но serde_json::Value лучше: это формат, совместимый с outbox (JSONB в PostgreSQL), message broker'ами (RabbitMQ payload), и event store.

### Требования к коду

**Файл: `crates/event_bus/src/envelope.rs`**

```rust
/// Конверт события для транспортировки через EventBus.
///
/// Payload сериализован в JSON — это позволяет:
/// - Передавать через in-process bus (текущая реализация)
/// - Записывать в outbox таблицу (Layer 3b)
/// - Отправлять в RabbitMQ/NATS/Kafka (будущее)
/// - Сохранять в event store (Layer 6)
pub struct EventEnvelope {
    pub event_id: Uuid,
    pub event_type: String,          // "erp.warehouse.goods_shipped.v1"
    pub source: String,              // "warehouse"
    pub tenant_id: TenantId,
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub user_id: UserId,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,  // сериализованное событие
}
```

- Derive: Debug, Clone, Serialize, Deserialize
- `from_domain_event<E: DomainEvent>(event: &E, ctx: &RequestContext, source: &str) -> Result<Self, serde_json::Error>` — сериализует событие в payload
- `deserialize_payload<E: DeserializeOwned>(&self) -> Result<E, serde_json::Error>` — десериализует payload в конкретный тип

### Тесты

- `from_domain_event` → `deserialize_payload` round-trip
- event_type, source, tenant_id заполняются корректно
- Сериализация EventEnvelope в JSON и обратно

---

## Задача 3a.4 — ErasedEventHandler + HandlerRegistry

### Зачем в ERP

Bus хранит handler'ы для разных типов событий. Проблема: `EventHandler<Event = GoodsShipped>` и `EventHandler<Event = OrderConfirmed>` — это разные типы. Нельзя положить их в один `Vec` напрямую. Нужен type erasure: обернуть конкретный handler в «стёрку», которая принимает `EventEnvelope`, десериализует payload и вызывает типизированный `handle()`.

### Зачем в Rust (что учим)

**Type erasure pattern** — один из фундаментальных паттернов Rust. Конкретный тип `T: EventHandler<Event = E>` оборачивается в `dyn ErasedEventHandler`, который принимает `EventEnvelope` (нетипизированный конверт). Внутри обёртка знает тип `E` и десериализует.

**`HashMap<String, Vec<Arc<dyn ErasedEventHandler>>>`** — registry handler'ов, ключ = event_type строка. При публикации bus ищет handler'ы по event_type конверта.

**`Arc<dyn Trait>`** — handler'ы разделяются между потоками tokio. `Arc` + `dyn` = shared ownership + dynamic dispatch.

### Требования к коду

**Файл: `crates/event_bus/src/registry.rs`**

1. **`ErasedEventHandler` trait** — type-erased версия EventHandler:
   ```rust
   /// Type-erased handler. Bus работает с этим trait,
   /// не зная конкретный тип события.
   #[async_trait]
   pub trait ErasedEventHandler: Send + Sync + 'static {
       /// Обработать конверт. Реализация десериализует payload
       /// в конкретный тип и вызывает типизированный handle().
       async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<(), anyhow::Error>;

       /// Тип события, на которое подписан handler (для routing).
       fn event_type(&self) -> &'static str;
   }
   ```

2. **`EventHandlerAdapter<H>`** — обёртка, реализующая ErasedEventHandler для конкретного H: EventHandler:
   ```rust
   pub struct EventHandlerAdapter<H: EventHandler> {
       handler: H,
   }
   ```
   - `handle_envelope()`: десериализует `envelope.payload` → `H::Event`, вызывает `handler.handle(&event)`
   - `event_type()`: возвращает `H::Event::event_type()` (нужен экземпляр или associated const)

3. **`HandlerRegistry`** — реестр handler'ов:
   ```rust
   pub struct HandlerRegistry {
       handlers: RwLock<HashMap<String, Vec<Arc<dyn ErasedEventHandler>>>>,
   }
   ```
   - `register<H: EventHandler>(&self, handler: H)` — оборачивает в Adapter, кладёт в map по event_type
   - `get_handlers(&self, event_type: &str) -> Vec<Arc<dyn ErasedEventHandler>>` — возвращает handler'ы для типа события

### Тесты

- Зарегистрировать два handler'а на разные event_type
- `get_handlers("type_a")` → один handler
- `get_handlers("type_b")` → другой handler
- `get_handlers("unknown")` → пустой Vec

---

## Задача 3a.5 — InProcessBus: реализация на tokio channels

### Зачем в ERP

Рабочая реализация EventBus для modular monolith. Все BC живут в одном процессе — события передаются через tokio channels, без сети, без сериализации на уровне транспорта (сериализация в envelope уже сделана — она нужна для единообразия с будущим RabbitMQ).

### Зачем в Rust (что учим)

**`tokio::sync::broadcast`** — multi-producer, multi-consumer канал. Каждый subscriber получает копию каждого сообщения. Идеально для pub/sub паттерна.

Альтернатива — `tokio::sync::mpsc` (multi-producer, single-consumer). Но для event bus нужен broadcast: одно событие → несколько handler'ов.

**`tokio::spawn`** — для fire-and-forget dispatch. Handler'ы выполняются в отдельных задачах tokio, не блокируя издателя.

**Error handling в async** — handler может вернуть ошибку. В `publish` (fire-and-forget) ошибка логируется. В `publish_and_wait` — собираются все ошибки. В будущем (Layer 3b) ошибка → retry → DLQ.

### Требования к коду

**Файл: `crates/event_bus/src/bus.rs`**

```rust
/// In-process EventBus на tokio channels.
///
/// Реализация для modular monolith. Все BC в одном процессе,
/// события передаются через память.
///
/// При переходе к микросервисам заменяется на RabbitMqBus/NatsBus,
/// реализующий тот же trait EventBus.
pub struct InProcessBus {
    registry: HandlerRegistry,
}
```

Реализация `EventBus` для `InProcessBus`:

- **`publish`**: получает envelope → находит handler'ы по event_type → для каждого `tokio::spawn(handler.handle_envelope(envelope.clone()))`. Fire-and-forget: ошибки логируются через `tracing::warn!`.
- **`publish_and_wait`**: то же, но `join_all` вместо spawn. Ждёт завершения всех handler'ов. Собирает ошибки.
- **`subscribe`**: делегирует в `registry.register()`.

Конструктор:
- `InProcessBus::new() -> Self`

### Тесты

- Подписать handler на "test.event" → publish envelope с типом "test.event" → handler вызван
- Подписать два handler'а на один тип → оба вызваны
- Publish envelope с неизвестным типом → никто не вызван, без ошибки
- `publish_and_wait` — handler с ошибкой → Result::Err возвращён
- `publish` — handler с ошибкой → publish возвращает Ok (fire-and-forget), ошибка залогирована

---

## Задача 3a.6 — Финальная сборка: lib.rs + полная проверка

### Требования к коду

**Файл: `crates/event_bus/src/lib.rs`**

```rust
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Event Bus — межмодульное взаимодействие через события.
//!
//! Trait EventBus определяет контракт. InProcessBus — реализация
//! для modular monolith на tokio channels.
//!
//! При переходе к микросервисам: заменить InProcessBus на RabbitMqBus,
//! NatsBus или KafkaBus — domain и application код не меняется.

pub mod bus;
pub mod envelope;
pub mod registry;
pub mod traits;

pub use bus::InProcessBus;
pub use envelope::EventEnvelope;
pub use registry::{ErasedEventHandler, EventHandlerAdapter, HandlerRegistry};
pub use traits::{EventBus, EventHandler};
```

**Обновить `crates/event_bus/Cargo.toml`:**

```toml
[package]
name = "event_bus"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
kernel = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
anyhow = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }   # с features = ["macros", "rt-multi-thread"] для тестов
```

### Финальная проверка

```bash
cargo build --workspace
cargo test -p event_bus
cargo clippy -p event_bus -- -D warnings
cargo fmt --all -- --check
just check
```

---

## Сводка: что получаем после Layer 3a

| Файл | Содержание | Тесты |
|------|------------|-------|
| `traits.rs` | EventHandler trait, EventBus trait | Compile, object safety |
| `envelope.rs` | EventEnvelope (transport wrapper) | serde round-trip, from_domain_event |
| `registry.rs` | ErasedEventHandler, Adapter, HandlerRegistry | register + get_handlers routing |
| `bus.rs` | InProcessBus (tokio channels) | publish, publish_and_wait, subscribe |
| `lib.rs` | Modules + re-exports | Integration compile |

### Чему научились (Rust)

- **async trait** — `#[async_trait]` для trait objects с async методами
- **Type erasure** — `dyn ErasedEventHandler`, `EventHandlerAdapter<H>`
- **Trait objects** — `Arc<dyn Trait>`, dynamic dispatch, object safety
- **tokio channels** — `broadcast` для pub/sub, `spawn` для fire-and-forget
- **`RwLock`** — async read-write lock для concurrent handler registration
- **`Send + Sync + 'static`** — потокобезопасность через систему типов
- **Error handling в async** — join_all, error collection, logging

### Связь с архитектурой ERP

| Архитектурный элемент | Где реализовано |
|----------------------|-----------------|
| In-process event bus (ADR v1) | InProcessBus на tokio broadcast |
| Заменяемость на RabbitMQ/NATS/Kafka | EventBus trait — единственная точка замены |
| Domain events внутри TX | publish_and_wait (синхронный dispatch) |
| Async side effects after commit | publish (fire-and-forget) |
| CloudEvents-совместимый формат | EventEnvelope с теми же полями |
| Outbox готовность | EventEnvelope сериализуется в JSON → JSONB в outbox (Layer 3b) |

### Что НЕ сделано (Layer 3b, после подключения БД)

- Outbox Relay (фоновый worker: poll PostgreSQL → publish через bus)
- Dead Letter Queue (3 retry → DLQ таблица)
- Inbox (дедупликация по event_id)
- Гарантия at-least-once delivery

---

## Следующий шаг

Layer 3a готов → **Layer 5a (BC Runtime)**: CommandHandler trait, QueryHandler trait, Command Pipeline со stub-зависимостями (MockBus, MockAuth, MockAudit). Тестируется без БД.
