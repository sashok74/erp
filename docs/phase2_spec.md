# Phase 2 — Outbox Relay + Audit + SeqGen: Спецификация
> Подробное описание | ERP Pilot on Rust
> Дата: 2026-03-24 | Привязка: EXECUTION_PLAN Phase 2, engineering_invariants.md
> Предусловие: Phase 0 (76 тестов) + Phase 1 (PgPool, PgUnitOfWork, RLS, миграции) готовы

---

## Цель Phase 2

Замкнуть **полный write-flow** — от HTTP-команды до опубликованного события и audit-записи. После Phase 2 каждая write-операция в системе оставляет полный след: outbox → bus → subscribers, audit log, domain history, нумерация документов.

### До Phase 2 (что есть)

```
POST /api/warehouse/receive
  → Pipeline: auth → hooks → BEGIN TX
    → Handler: бизнес-логика
    → UoW: INSERT outbox entry
  → COMMIT
  → ??? outbox row лежит в БД, никто его не читает
  → ??? audit = Noop (ничего не пишет)
  → ??? нумерация документов отсутствует
```

### После Phase 2 (что должно быть)

```
POST /api/warehouse/receive
  → Pipeline: auth → hooks → BEGIN TX
    → Handler: бизнес-логика
    → Handler: domain history (old/new state)
    → Handler: seq_gen (номер документа)
    → UoW: INSERT outbox entry
  → COMMIT
  → PgAuditLog: INSERT audit_log (кто, что, когда, результат)
  → OutboxRelay (background): poll outbox → deserialize → bus.publish()
    → InProcessBus: доставить подписчикам
    → Subscriber: InboxGuard (dedup) → обработка
  → Relay: UPDATE outbox SET published_at = now()
```

---

## Компоненты Phase 2

### 2.1. Outbox Relay

**Назначение:** фоновый процесс, который забирает неопубликованные события из таблицы `common.outbox` и публикует их через EventBus.

**Механизм:**
- Бесконечный loop в `tokio::spawn`
- Каждая итерация: `SELECT ... WHERE published_at IS NULL ... FOR UPDATE SKIP LOCKED`
- `FOR UPDATE SKIP LOCKED` — если другой процесс уже обрабатывает row, пропустить (для будущего multi-instance)
- Для каждого row: собрать `EventEnvelope` из полей → `bus.publish(envelope)`
- Успех: `UPDATE SET published_at = now()`
- Ошибка: `UPDATE SET retry_count = retry_count + 1`
- retry_count >= 3: row пропускается (dead letter). В будущем — отдельная DLQ таблица.
- После обработки batch'а: `sleep(poll_interval)`

**Конфигурация:**

| Параметр | Default | Описание |
|----------|---------|----------|
| `poll_interval` | 100ms | Пауза между итерациями |
| `batch_size` | 50 | Максимум записей за одну итерацию |
| `max_retries` | 3 | После скольких ошибок — пропускать |

**Гарантии:**
- **At-least-once delivery:** если publish прошёл, но UPDATE published_at упал → при следующем poll event опубликуется повторно. Подписчик должен быть идемпотентным (Inbox).
- **Ordering:** events из одного tenant публикуются в порядке created_at. Между tenant'ами — без гарантий порядка.
- **Concurrency-safe:** `FOR UPDATE SKIP LOCKED` позволяет запускать несколько relay-процессов параллельно.

**Восстановление EventEnvelope из row:**

```
Row                          →  EventEnvelope
─────────────────────────────────────────────
event_id       UUID          →  event_id
event_type     TEXT          →  event_type
source         TEXT          →  source
tenant_id      UUID          →  tenant_id (→ TenantId)
payload        JSONB         →  payload (serde_json::Value)
correlation_id UUID          →  correlation_id
causation_id   UUID          →  causation_id
created_at     TIMESTAMPTZ   →  timestamp
```

`user_id` не хранится в outbox (есть в payload и в audit_log). При восстановлении envelope — использовать `UserId::from_uuid(Uuid::nil())` или добавить колонку `user_id` в outbox. **Рекомендация:** добавить `user_id UUID NOT NULL` в outbox при миграции (или ALTER TABLE, если Phase 1 уже применена).

---

### 2.2. Inbox — дедупликация событий

**Назначение:** гарантировать exactly-once processing. Relay может опубликовать одно событие дважды (at-least-once). Inbox отсекает повторы.

**Механизм:**
```sql
INSERT INTO common.inbox (event_id, event_type, source)
VALUES ($1, $2, $3)
ON CONFLICT (event_id) DO NOTHING
```
- rows_affected = 1 → новое событие, обрабатывать
- rows_affected = 0 → повтор, пропустить

**Где вызывается:** в event handler'е подписчика, **до** бизнес-логики обработки.

**Ответственность:** каждый subscriber решает сам, нужен ли ему inbox. Для idempotent-by-design handler'ов (UPSERT) inbox не обязателен. Для non-idempotent (INSERT, send email) — обязателен.

**InboxGuard** предоставляет метод `try_process(event_id, event_type, source) → bool`. BC-handler вызывает его и проверяет результат.

---

### 2.3. PgAuditLog — реализация AuditLog trait

**Назначение:** записывать audit trail каждой write-операции. Кто, когда, какую команду выполнил, какой результат.

**Что записывается:**

| Поле | Источник | Пример |
|------|---------|--------|
| tenant_id | RequestContext | `550e8400-...` |
| user_id | RequestContext | `6ba7b810-...` |
| command_name | Command::command_name() | `"warehouse.receive_goods"` |
| result | serde_json::to_value(handler_result) | `{"movement_id":"...","new_balance":"100"}` |
| correlation_id | RequestContext | `7c9e6679-...` |
| causation_id | RequestContext | `a1b2c3d4-...` |
| created_at | Utc::now() | `2026-03-24T10:30:00Z` |

**Когда пишется:** после COMMIT основной транзакции (pipeline step 8). Отдельное соединение из pool, **не** через UoW TX.

**Почему после commit, не внутри TX:**
- Если бизнес-TX откатилась → audit не нужен (команда не выполнена)
- Если audit INSERT упал → команда уже выполнена, данные сохранены. Audit = best-effort. Потеря audit-записи — не катастрофа (retry через tracing/alerting)
- Отдельное соединение = нет дополнительного lock contention на бизнес-TX

**Подстановка в Pipeline:** `Arc<dyn AuditLog>` в `CommandPipeline`. В Phase 1 = `NoopAuditLog`. Теперь = `PgAuditLog`. Pipeline код **не меняется** — меняется только wiring в точке входа (gateway/main.rs).

---

### 2.4. Domain History — что изменилось в данных

**Назначение:** для каждого изменения сущности — зафиксировать состояние до и после. Это не audit (кто/когда), а **дифф данных**.

**Что записывается:**

| Поле | Описание | Пример |
|------|----------|--------|
| tenant_id | Из RequestContext | |
| entity_type | Тип сущности | `"inventory_item"` |
| entity_id | ID конкретной записи | `uuid` |
| event_type | Что произошло | `"goods_received"` |
| old_state | JSON до изменения (null для создания) | `null` |
| new_state | JSON после изменения (null для удаления) | `{"sku":"BOLT-42","balance":"100"}` |
| correlation_id | Из RequestContext | |
| causation_id | Из RequestContext | |
| user_id | Из RequestContext | |

**Когда пишется:** **внутри UoW TX** (рекомендация). Атомарно с domain data. Если TX откатилась — history тоже откатится.

**Два варианта API:**

Вариант A — отдельный writer, handler вызывает явно:
```rust
history.record(ctx, "inventory_item", item_id, "goods_received", old, new).await?;
```

Вариант B — через UoW, handler добавляет, PgUoW пишет при commit:
```rust
uow.record_domain_history("inventory_item", item_id, "goods_received", old, new);
// PgUoW::commit() → INSERT domain_history entries + INSERT outbox entries → COMMIT
```

Вариант B чище — handler не знает про SQL, UoW абстрагирует. **Рекомендуется.**

**Расширение UnitOfWork trait:** добавить метод:
```rust
fn record_domain_history(
    &mut self,
    entity_type: &str,
    entity_id: Uuid,
    event_type: &str,
    old_state: Option<serde_json::Value>,
    new_state: Option<serde_json::Value>,
);
```

PgUnitOfWork накапливает записи и INSERT'ит при commit() вместе с outbox entries.
InMemoryUnitOfWork (stub) — тоже накапливает для проверки в тестах.

---

### 2.5. PgSequenceGenerator — нумерация документов

**Назначение:** gap-free per-tenant номера документов. Каждый тип документа — своя последовательность.

**Примеры:**

| Тип | seq_name | Prefix | Результат |
|-----|----------|--------|-----------|
| Приёмка | `warehouse.receipt` | `ПРХ-` | `ПРХ-000001`, `ПРХ-000002` |
| Отгрузка | `warehouse.shipment` | `ОТГ-` | `ОТГ-000001` |
| GL-проводка | `finance.journal` | `ЖО-` | `ЖО-000001` |

**Механизм:**
1. `INSERT ... ON CONFLICT DO NOTHING` — создать sequence если нет (idempotent)
2. `SELECT prefix, next_value ... FOR UPDATE` — lock строку
3. `UPDATE SET next_value = next_value + 1` — инкрементировать
4. Return `format!("{prefix}{next_value:06}")` → `"ПРХ-000001"`

**Gap-free гарантия:** `FOR UPDATE` блокирует строку. Если TX откатилась — UPDATE откатился, номер не потрачен. Concurrent TX ждёт release lock → получает следующий номер. Нет пропусков.

**Trade-off:** `FOR UPDATE` = точка сериализации. Высокий concurrent throughput на одной sequence → bottleneck. Для ERP (десятки-сотни документов в минуту) — не проблема. Для тысяч/сек → рассмотреть advisory locks или pre-allocation.

**Где вызывается:** handler, внутри UoW TX (та же транзакция, что и domain data).

---

## Wiring: как компоненты соединяются

### До Phase 2 (Phase 1)

```rust
// gateway/main.rs (или тест)
let pipeline = CommandPipeline::new(
    Arc::new(pg_uow_factory),
    Arc::new(bus),
    Arc::new(jwt_checker),
    Arc::new(NoopExtensionHooks),
    Arc::new(NoopAuditLog),           // ← Noop!
);
// Outbox relay не запущен
```

### После Phase 2

```rust
// gateway/main.rs
let pipeline = CommandPipeline::new(
    Arc::new(pg_uow_factory),
    Arc::new(bus),
    Arc::new(jwt_checker),
    Arc::new(NoopExtensionHooks),
    Arc::new(PgAuditLog::new(pool.clone())),  // ← Real!
);

// Запуск outbox relay
let relay = OutboxRelay::new(pool.clone(), bus.clone(), Duration::from_millis(100), 50);
tokio::spawn(async move { relay.run().await });
```

**Pipeline code не изменён.** Изменился только набор `Arc<dyn Trait>` при сборке.

---

## Acceptance Criteria: что проверяем

### Тесты компонентов

| Компонент | Тест | Ожидание |
|-----------|------|----------|
| OutboxRelay | INSERT outbox → poll_and_publish | bus.publish вызван |
| OutboxRelay | publish error | retry_count++ |
| OutboxRelay | 3 errors | row пропускается |
| OutboxRelay | two outbox rows | оба опубликованы по порядку |
| InboxGuard | try_process(new event_id) | true |
| InboxGuard | try_process(same event_id) | false |
| PgAuditLog | log() | SELECT audit_log → row |
| DomainHistory | record() | SELECT domain_history → row |
| DomainHistory | через UoW rollback | domain_history пуста |
| PgSeqGen | next_value × 2 | "ПРХ-000001", "ПРХ-000002" |
| PgSeqGen | разные tenants | независимые последовательности |
| PgSeqGen | 10 concurrent | 10 последовательных номеров |

### E2E Integration Test

**Самый важный тест Phase 2** — полный write-flow через все компоненты:

```
Шаг 1: Собрать pipeline с реальными зависимостями
        PgUoWFactory, InProcessBus, JwtPermissionChecker, PgAuditLog

Шаг 2: Подписать тестовый handler на событие "test.executed"

Шаг 3: Запустить OutboxRelay (tokio::spawn)

Шаг 4: pipeline.execute(test_handler, test_cmd, ctx)

Шаг 5: Подождать relay (200ms)

Шаг 6: Проверить:
        ✓ Handler result = Ok
        ✓ common.outbox: row exists, published_at IS NOT NULL
        ✓ common.audit_log: row exists (command_name, user_id, result)
        ✓ common.domain_history: row exists (entity_type, old/new state)
        ✓ InProcessBus subscriber: был вызван (AtomicBool = true)
```

### Инварианты (из engineering_invariants.md)

После Phase 2 **все** перечисленные инварианты зацементированы:

| # | Инвариант | Чем подтверждён |
|---|-----------|----------------|
| 1 | Все write через Pipeline | Код: нет INSERT вне handler'ов |
| 2 | Все write с PermissionChecker | Тест: пустые roles → 403 |
| 3 | Все write с audit + history | E2E: audit_log + domain_history rows |
| 4 | Межконтекстные связи через events | Outbox → relay → bus → subscriber |
| 5 | Нет cross-BC SQL reads | Cargo.toml: нет cross-BC deps |
| 6 | Events versioned | EventEnvelope.event_type = "erp.*.v1" |
| 7 | Все таблицы: tenant_id + RLS | RLS test: tenant A ≠ tenant B |
| 8 | Никакой записи мимо UoW | PgUoW = единственный путь INSERT |
| 9 | RequestContext в каждом handler | Trait signature |
| 10 | Handler не знает про роли | Код: handler не читает ctx.roles |

---

## Структура файлов после Phase 2

```
crates/db/src/
├── lib.rs
├── pool.rs           ← Phase 1
├── rls.rs            ← Phase 1
├── uow.rs            ← Phase 1 (расширен: record_domain_history)
├── migrate.rs        ← Phase 1
├── relay.rs          ← Phase 2: OutboxRelay
└── inbox.rs          ← Phase 2: InboxGuard

crates/audit/src/
├── lib.rs
├── logger.rs         ← Phase 2: PgAuditLog
└── history.rs        ← Phase 2: DomainHistoryWriter (если не в UoW)

crates/seq_gen/src/
├── lib.rs
└── generator.rs      ← Phase 2: PgSequenceGenerator
```

### Расширение существующих trait'ов

**UnitOfWork trait (runtime/ports.rs)** — добавить:
```rust
fn record_domain_history(
    &mut self,
    entity_type: &str,
    entity_id: Uuid,
    event_type: &str,
    old_state: Option<serde_json::Value>,
    new_state: Option<serde_json::Value>,
);
```

**InMemoryUnitOfWork (runtime/stubs.rs)** — расширить:
```rust
pub history_entries: Vec<DomainHistoryEntry>,  // для проверки в тестах
```

---

## Чему научимся (Rust)

| Концепция | Где | Зачем |
|-----------|-----|-------|
| `tokio::spawn` + infinite loop | OutboxRelay::run() | Background workers в async Rust |
| `FOR UPDATE SKIP LOCKED` | Relay poll | Concurrent-safe batch processing |
| `ON CONFLICT DO NOTHING` | InboxGuard | Idempotent INSERT pattern |
| `SELECT FOR UPDATE` | SeqGen | Pessimistic locking для gap-free |
| Trait extension | UnitOfWork + record_domain_history | Добавление методов без breaking change |
| `Duration` + `sleep` | Relay poll interval | Async timing в tokio |
| `AtomicBool` / `AtomicUsize` | E2E test subscriber verification | Thread-safe counters в async |
| `Arc<dyn Trait>` wiring | Pipeline с реальными компонентами | Dependency injection в production |

---

## Следующий шаг

Phase 2 готова → **Phase 3: Warehouse Vertical Slice** — первый настоящий бизнес-код. `ReceiveGoods` command от HTTP до event publish. Минимальный домен: InventoryItem, Sku, Quantity, balance >= 0.
