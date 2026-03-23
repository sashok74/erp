# Event Bus — архитектура шины событий

## Идея

Bounded Contexts (модули ERP) не вызывают друг друга напрямую. Вместо этого один модуль **публикует событие**, а другие **подписываются** на него. Это развязывает модули: Warehouse ничего не знает о Finance, но Finance реагирует на отгрузки.

---

## Схема архитектуры

```mermaid
graph TB
    subgraph "Bounded Context: Warehouse"
        WH_AGG["Aggregate<br/>(GoodsShipped)"]
    end

    subgraph "Bounded Context: Finance"
        FIN_H["FinanceHandler<br/>impl EventHandler"]
    end

    subgraph "Bounded Context: Audit"
        AUD_H["AuditHandler<br/>impl EventHandler"]
    end

    subgraph "Event Bus crate"
        direction TB

        subgraph "Type Erasure"
            ADAPTER_F["EventHandlerAdapter&lt;FinanceHandler&gt;<br/>impl ErasedEventHandler"]
            ADAPTER_A["EventHandlerAdapter&lt;AuditHandler&gt;<br/>impl ErasedEventHandler"]
        end

        ENVELOPE["EventEnvelope<br/>event_type + JSON payload<br/>+ tenant_id, correlation_id, ..."]

        REGISTRY["HandlerRegistry<br/>RwLock&lt;HashMap&lt;String, Vec&lt;Arc&lt;dyn ErasedEventHandler&gt;&gt;&gt;&gt;"]

        BUS["InProcessBus<br/>impl EventBus"]
    end

    WH_AGG -- "1. from_domain_event()" --> ENVELOPE
    ENVELOPE -- "2. publish(envelope)" --> BUS
    BUS -- "3. get_handlers(event_type)" --> REGISTRY
    REGISTRY -- "4. dispatch" --> ADAPTER_F
    REGISTRY -- "4. dispatch" --> ADAPTER_A
    ADAPTER_F -- "5. deserialize_payload()" --> FIN_H
    ADAPTER_A -- "5. deserialize_payload()" --> AUD_H

    style WH_AGG fill:#4a9eff,color:#fff
    style FIN_H fill:#2ecc71,color:#fff
    style AUD_H fill:#2ecc71,color:#fff
    style ENVELOPE fill:#f39c12,color:#fff
    style BUS fill:#e74c3c,color:#fff
    style REGISTRY fill:#9b59b6,color:#fff
    style ADAPTER_F fill:#8e44ad,color:#fff
    style ADAPTER_A fill:#8e44ad,color:#fff
```

## Схема потока данных

```mermaid
sequenceDiagram
    participant WH as Warehouse BC
    participant ENV as EventEnvelope
    participant BUS as InProcessBus
    participant REG as HandlerRegistry
    participant ADP as EventHandlerAdapter<H>
    participant FIN as FinanceHandler

    WH->>ENV: from_domain_event(&event, &ctx, "warehouse")
    Note over ENV: serde_json::to_value(event)<br/>→ payload: Value

    WH->>BUS: publish(envelope)
    BUS->>REG: get_handlers("erp.warehouse.goods_shipped.v1")
    REG-->>BUS: Vec<Arc<dyn ErasedEventHandler>>

    alt publish (fire-and-forget)
        BUS->>ADP: tokio::spawn → handle_envelope(&env)
        Note over BUS: Не ждёт завершения
    else publish_and_wait (синхронный)
        BUS->>ADP: handle_envelope(&envelope).await?
        Note over BUS: Ждёт, ошибка → return Err
    end

    ADP->>ADP: envelope.deserialize_payload::<Event>()
    Note over ADP: JSON → конкретный тип
    ADP->>FIN: handler.handle(&event).await
```

## Схема type erasure

```mermaid
graph LR
    subgraph "Типизированный мир (Domain)"
        EH["EventHandler<br/>type Event = GoodsShipped<br/>fn handle(&self, event: &GoodsShipped)"]
    end

    subgraph "Нетипизированный мир (Transport)"
        EEH["ErasedEventHandler<br/>fn handle_envelope(&self, env: &EventEnvelope)"]
    end

    ADAPT["EventHandlerAdapter&lt;H&gt;<br/>deserialize_payload() → H::Event"]

    EH -- "оборачивается в" --> ADAPT
    ADAPT -- "реализует" --> EEH

    style EH fill:#2ecc71,color:#fff
    style EEH fill:#e74c3c,color:#fff
    style ADAPT fill:#9b59b6,color:#fff
```

---

## Слои (4 уровня абстракции)

| Слой | Файл | Что делает |
|------|------|------------|
| `DomainEvent` | `kernel/src/events.rs` | Trait — что произошло (определяется в каждом BC) |
| `EventEnvelope` | `event_bus/src/envelope.rs` | Транспортная обёртка: JSON payload + метаданные |
| `EventBus` trait | `event_bus/src/traits.rs` | Контракт publish/subscribe (заменяемый) |
| `InProcessBus` | `event_bus/src/bus.rs` | Реализация: tokio, in-memory |

---

## 1. Доменное событие

Каждый BC определяет свои события, реализуя трейт `DomainEvent` из kernel:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoodsShipped {
    id: Uuid,
    sku: String,
    quantity: i32,
}

impl DomainEvent for GoodsShipped {
    fn event_type(&self) -> &'static str { "erp.warehouse.goods_shipped.v1" }
    fn aggregate_id(&self) -> Uuid { self.id }
}
```

## 2. Конверт — type erasure

Bus не может работать с дженериками (нельзя хранить `Vec<разных типов>`).
Событие **сериализуется в JSON** и оборачивается в `EventEnvelope`:

```rust
// envelope.rs:50-66
let envelope = EventEnvelope::from_domain_event(&event, &ctx, "warehouse")?;
// Внутри: payload = serde_json::to_value(event) → serde_json::Value
```

Конверт несёт метаданные: `event_type`, `tenant_id`, `correlation_id`, `timestamp` —
всё что нужно для routing и трассировки.

## 3. Подписчик — EventHandler → ErasedEventHandler

Подписчик реализует типизированный `EventHandler`:

```rust
// traits.rs:22-37
#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    type Event: DomainEvent;                                    // конкретный тип
    async fn handle(&self, event: &Self::Event) -> Result<()>;  // типизированный вызов
    fn handled_event_type(&self) -> &'static str;               // routing key
}
```

Bus хранит обработчики в `HashMap<String, Vec<Arc<dyn ???>>>` — ему нужен
**один trait без дженериков**. Это `ErasedEventHandler`:

```rust
// registry.rs:23-31
#[async_trait]
pub trait ErasedEventHandler: Send + Sync + 'static {
    async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<()>;
    fn event_type(&self) -> &'static str;
}
```

Мост между ними — `EventHandlerAdapter<H>`:

```rust
// registry.rs:48-62
impl<H: EventHandler> ErasedEventHandler for EventHandlerAdapter<H>
where H::Event: DeserializeOwned
{
    async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<()> {
        let event: H::Event = envelope.deserialize_payload()?;  // JSON → тип
        self.handler.handle(&event).await                        // типизированный вызов
    }
}
```

## 4. Реестр и dispatch

`HandlerRegistry` хранит обработчиков, сгруппированных по типу события:

```
"erp.warehouse.goods_shipped.v1"  →  [FinanceHandler, AuditHandler]
"erp.finance.invoice_created.v1"  →  [NotificationHandler]
```

При публикации Bus ищет обработчиков по строке `event_type` и вызывает каждого.

## 5. Два режима публикации

```rust
// bus.rs — InProcessBus

// Fire-and-forget: handler'ы в отдельных tokio tasks, ошибки логируются
async fn publish(&self, envelope: EventEnvelope) {
    for handler in handlers {
        let env = envelope.clone();
        tokio::spawn(async move {           // ← не ждём
            handler.handle_envelope(&env).await;
        });
    }
}

// Синхронный: ждём каждого, первая ошибка → return Err
async fn publish_and_wait(&self, envelope: EventEnvelope) {
    for handler in handlers {
        handler.handle_envelope(&envelope).await?;  // ← ждём
    }
}
```

- `publish` — для side effects после коммита TX (отправить email, обновить кэш)
- `publish_and_wait` — для domain events внутри транзакции (consistency)

## 6. Заменяемость

`EventBus` — trait. Сейчас `InProcessBus` (память, tokio).
При переходе к микросервисам — `RabbitMqBus` / `NatsBus` / `KafkaBus`,
тот же trait, domain-код не меняется:

```mermaid
graph LR
    DOMAIN["Domain / Application<br/>(зависит от trait EventBus)"]

    subgraph "Реализации (взаимозаменяемы)"
        IN["InProcessBus<br/>tokio channels"]
        RMQ["RabbitMqBus"]
        NATS["NatsBus"]
        KAFKA["KafkaBus"]
    end

    DOMAIN --> IN
    DOMAIN -.-> RMQ
    DOMAIN -.-> NATS
    DOMAIN -.-> KAFKA

    style DOMAIN fill:#4a9eff,color:#fff
    style IN fill:#2ecc71,color:#fff
    style RMQ fill:#95a5a6,color:#fff
    style NATS fill:#95a5a6,color:#fff
    style KAFKA fill:#95a5a6,color:#fff
```
