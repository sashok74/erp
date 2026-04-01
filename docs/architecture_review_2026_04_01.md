# Сводное архитектурное ревью: консолидация двух рецензий

**Дата:** 2026-04-01
**Ревьюверы:** Claude (R1) и внешний рецензент (R2)

---

## Общий Scorecard (консолидированный)

| # | Область | R1 | R2 | Итого | Критичность |
|---|---------|----|----|-------|-------------|
| 1 | DDD Bounded Contexts | ✅ | ⚠️ | ⚠️ | Высокая |
| 2 | Луковая архитектура | ⚠️ | ❌ | ❌ | Высокая |
| 3 | Event-driven связь | ✅ | ✅ | ✅ | Высокая |
| 4 | Clorinde / zero inline SQL | ✅ | ⚠️ | ⚠️ | Средняя |
| 5 | RLS tenant isolation | ✅ | ❌ | ❌ | Высокая |
| 6 | Outbox/Inbox | ✅ | ✅ | ✅ | Высокая |
| 7 | Canonical pipeline | ✅ | ⚠️ | ⚠️ | Высокая |
| 8 | Value Objects в BC | ✅ | ✅ | ✅ | Средняя |
| 9 | Тесты critical path | ✅ | ✅ | ✅ | Средняя |
| 10 | Ubiquitous Language | ✅ | ✅ | ✅ | Низкая |

**Итого: 5 ✅, 3 ⚠️, 2 ❌**

---

## Где R1 ошибся (самокритика)

### 1. RLS на read path — пропущен критический баг

**Суть:** R1 проверил наличие `SET LOCAL` в `ReadDbContext::acquire()` и отметил ✅. R2 пошёл дальше и **реально проверил в psql** — `SET LOCAL` без явной транзакции (`BEGIN`) **не работает**. PostgreSQL выдаёт предупреждение `SET LOCAL can only be used in transaction blocks`, и tenant context не сохраняется.

**Код проблемы** (`crates/db/src/context.rs:138-143`):
```rust
pub async fn acquire(pool: &PgPool, ctx: &RequestContext) -> Result<Self, AppError> {
    let client = pool.get().await.internal("pool checkout")?;
    set_tenant_context(&**client, ctx.tenant_id).await.internal("set tenant")?;
    // Нет BEGIN! SET LOCAL действует только в transaction block
    Ok(Self { client })
}
```

Для write path (`PgUnitOfWork::begin()`) есть явный `BEGIN` перед `SET LOCAL` — работает корректно. Но **весь query path работает без RLS-фильтрации**.

**Дополнительно:** все миграции используют `ENABLE ROW LEVEL SECURITY` без `FORCE ROW LEVEL SECURITY`. Это значит, что table owner (erp_admin) обходит RLS-политики. Подтверждено: 0 вхождений `FORCE` в миграциях.

**Почему R1 пропустил:** проверял наличие вызова `set_tenant_context()` в коде, но не анализировал семантику `SET LOCAL` без `BEGIN`. Формальная проверка «вызов есть» без понимания PostgreSQL-специфики.

**Урок:** инварианты нужно проверять через реальное выполнение (psql, интеграционный тест), а не только code review.

### 2. Луковая архитектура — недооценена серьёзность

**R1** отметил ⚠️ (improvement, "оставить для MVP"). **R2** отметил ❌ (critical).

**Позиция R2 обоснована:**
- Application -> Infrastructure это **структурное нарушение** onion architecture, а не стилистический вопрос
- Если новый разработчик копирует шаблон, он **унаследует нарушение**
- Без Repository trait невозможны unit-тесты handlers без БД
- Это противоречит документированному инварианту #3

**R1 был прав** в том, что для модульного монолита с одной реализацией trait Repository может быть избыточен. Но R1 должен был отметить это как **нарушение с обоснованным отступлением**, а не как «заметка».

### 3. sqlx в workspace — пропущено

R1 не проверил workspace-level `Cargo.toml` и `justfile`. Подтверждено:
- `Cargo.toml:41` — `sqlx` в workspace dependencies
- `justfile` — 5 вызовов `sqlx` CLI (database create/drop, migrate)

Инвариант "Clorinde, не sqlx" формально нарушен на уровне tooling, хотя production query path действительно через Clorinde.

### 4. `doc/dbinfo.md` с секретами — пропущено

R1 не проверил `doc/` директорию (только `docs/`). `doc/dbinfo.md` содержит plain-text пароль БД `vfrfh123` и IP-адреса. Это security issue, не архитектурная проблема, но R2 правильно его поднял.

### 5. Пути в CLAUDE.md — не проверены

R1 отметил "обновить CLAUDE.md", но не проверил конкретные ссылки. `CLAUDE.md:9` ссылается на `docs/EXECUTION_PLAN.md`, реальный путь — `docs/plan/EXECUTION_PLAN.md`.

---

## Где R1 и R2 согласны

| Тема | Консенсус |
|------|-----------|
| BC не импортируют друг друга в production | ✅ Подтверждено обоими |
| Event-driven cross-BC через outbox + projection | ✅ Отлично реализовано |
| Outbox/Inbox/Relay/DLQ | ✅ Добротная реализация |
| Агрегаты rich, не anemic | ✅ Поведение + events |
| Value Objects в BC domain, не в kernel | ✅ Правильное решение |
| UUID v7 повсеместно | ✅ Соблюдается |
| tenant_id в events не консистентен | ⚠️ Catalog vs Warehouse |
| Domain events = integration events (нет разделения) | ⚠️ Допустимо для MVP, но tech debt |
| Документация существенно отстаёт от кода | ⚠️ Нужно обновление |

---

## Консолидированные критические находки

### P0: RLS не работает на read path

**Файлы:** `crates/db/src/context.rs:138`, `crates/db/src/rls.rs:17`, все миграции с RLS
**Проблема:** `SET LOCAL` без `BEGIN` = no-op. Весь query path (`ReadDbContext::acquire()`) не фильтрует по tenant. Плюс нет `FORCE ROW LEVEL SECURITY` — table owner обходит политики.
**Воздействие:** Tenant isolation на чтение **не работает**. Любой authenticated user потенциально видит данные всех tenants через query handlers.
**Серьёзность:** 🔴
**Фикс:**
1. `ReadDbContext::acquire()`: обернуть в `BEGIN` / `COMMIT` (или auto-commit с `SET` вместо `SET LOCAL`)
2. Все миграции: добавить `ALTER TABLE ... FORCE ROW LEVEL SECURITY`
3. Интеграционный тест: проверить что query без tenant context возвращает пустой результат
4. Рассмотреть dedicated DB role (не owner) для application connections

### P1: Application -> Infrastructure нарушение

**Файлы:** `crates/warehouse/src/application/commands/receive_goods.rs:20`, `crates/catalog/src/application/commands/create_product.rs:20` (и query handlers)
**Проблема:** Handlers напрямую импортируют `PgInventoryRepo`/`PgProductRepo` из infrastructure.
**Серьёзность:** 🔴
**Фикс:** Вынести Repository trait в `application/ports.rs`, реализовать в infrastructure. Handlers зависят от trait, не от конкретной реализации.

### P2: Документация вводит в заблуждение

**Файлы:** `CLAUDE.md`, `docs/plan/EXECUTION_PLAN.md`, `docs/auth_overview.md`, `README.md`
**Проблема:** Устаревшие пути, статусы фаз, описание auth-модели, несуществующие crate-имена.
**Серьёзность:** 🟡
**Фикс:** Единовременное обновление документации, пометка archived specs.

### P3: Секреты в репозитории

**Файл:** `doc/dbinfo.md`
**Проблема:** Plain-text пароль БД и IP-адреса в tracked file.
**Серьёзность:** 🟡
**Фикс:** Удалить файл, сменить пароль, добавить `doc/dbinfo.md` в `.gitignore`.

### P4: sqlx в workspace

**Файлы:** `Cargo.toml:41`, `justfile:39-77`
**Проблема:** Инвариант "Clorinde, не sqlx" формально нарушен на tooling-уровне.
**Серьёзность:** 🟡
**Фикс:** Либо уточнить инвариант ("Clorinde for domain queries, sqlx CLI allowed for ops"), либо перевести миграции на refinery/другой runner и убрать sqlx dependency.

### P5: Domain events = integration events

**Файлы:** `crates/catalog/src/domain/events.rs`, `crates/warehouse/src/domain/events.rs`
**Проблема:** Нет разделения internal/public events. Внутренний domain event напрямую сериализуется в outbox как inter-BC контракт.
**Серьёзность:** 🟡
**Фикс (при росте):** Выделить `integration_events.rs` с маппингом domain -> integration event. Пока 1:1, это tech debt, не блокер.

---

## Подробный анализ (R1, проверенный R2)

### Блок 1: DDD-соответствие

#### 1. Bounded Context как единица модульности ⚠️

Каждый BC — отдельный crate с собственной PostgreSQL-схемой:
- `warehouse` -> schema `warehouse.*` (5 миграций)
- `catalog` -> schema `catalog.*` (2 миграции)
- `common` -> schema `common.*` (11 миграций, инфраструктура)

**Cargo.toml зависимости:**
- `warehouse` зависит от: `kernel, runtime, event_bus, bc_http, db, clorinde-gen, audit, seq_gen` ✅
- `catalog` зависит от: `kernel, runtime, event_bus, bc_http, db, clorinde-gen, audit` ✅
- **warehouse НЕ зависит от catalog** ✅
- **catalog НЕ зависит от warehouse в [dependencies]** ✅
- **catalog зависит от warehouse в [dev-dependencies]** ⚠️ — не production breach, но портит чистоту шаблона (R2)

#### 2. Связь только через события ✅

Cross-BC коммуникация реализована исключительно через domain events:
- Catalog публикует `erp.catalog.product_created.v1` -> outbox -> relay -> EventBus
- Warehouse подписывается через `ProductCreatedHandler` -> обновляет `warehouse.product_projections`
- `ProductCreatedHandler` определяет собственную локальную структуру `ProductCreatedEvent` — не импортирует из catalog (Anti-Corruption Layer) ✅

#### 3. Луковая архитектура внутри BC ❌

**Domain слой: ЧИСТЫЙ** ✅
- Нет `tokio-postgres`, `deadpool`, `clorinde` в domain/

**Application слой: НАРУШЕН** ❌
- Handlers импортируют `PgInventoryRepo`/`PgProductRepo` напрямую из infrastructure
- Нет port/trait абстракции для репозиториев

**Infrastructure слой: КОРРЕКТНЫЙ** ✅
- Реализует persistence через clorinde-gen
- HTTP routes через bc_http::BcRouter

#### 4. Агрегаты и Value Objects ✅

**Warehouse:**
- `InventoryItem` — **RICH aggregate**: `receive()` генерирует `GoodsReceived` event, валидирует инварианты
- `Sku` VO: non-empty, <=50 chars, BigDecimal
- `Quantity` VO: BigDecimal, >= 0, Add+Sub arithmetic

**Catalog:**
- `Product` — **RICH aggregate**: `create()` factory генерирует `ProductCreated` event
- `Sku` VO, `ProductName` VO — с валидацией

**В kernel нет бизнес-VO** ✅

#### 5. Ubiquitous Language ✅

Имена отражают язык домена: `ReceiveGoodsCommand`, `GoodsReceived`, `ProductCreated`, `InventoryItem`, `InsufficientStock`, `ПРХ-` (приходная накладная). Нет generic-имён типа `CreateRecord`, `UpdateEntity`.

#### 6. Domain Events как контракты ⚠️

Events используют примитивы в payload (правильно для inter-BC). Конвенция `erp.{bc}.{event}.v{N}` соблюдается. **Но:** нет формального разделения internal vs integration events — domain events напрямую идут в outbox.

#### 7. Антикоррупционный слой ✅

Между BC — ACL присутствует: `ProductCreatedHandler` определяет собственную структуру, десериализует из JSON, не импортирует из catalog. `EventEnvelope` — type-erased transport.

### Блок 2: Архитектурные инварианты

#### 8. Clorinde, не sqlx ⚠️

- Production query path: Clorinde ✅
- Workspace Cargo.toml: sqlx в dependencies ⚠️
- justfile: sqlx CLI для миграций ⚠️
- Inline SQL в db-layer: health check, SET LOCAL, migration runner ⚠️ (допустимо для infra)

#### 9. RLS для tenant isolation ❌

- Write path (`PgUnitOfWork::begin()`): `BEGIN` + `SET LOCAL` ✅
- **Read path (`ReadDbContext::acquire()`): `SET LOCAL` без `BEGIN` = не работает** ❌
- **Нет `FORCE ROW LEVEL SECURITY`** — table owner обходит политики ❌
- Политики единообразны: `USING (tenant_id = common.current_tenant_id())`
- 11 из 13 таблиц с RLS (2 исключения обоснованы: tenants, inbox)

#### 10. Outbox/Inbox ✅

- Outbox atomicity: events в той же TX что бизнес-данные (`flush_history` -> `flush_outbox` -> `COMMIT`) ✅
- Relay: `SELECT FOR UPDATE SKIP LOCKED`, retry 3x, DLQ ✅
- Inbox dedup: `(event_id, handler_name)`, `ON CONFLICT DO NOTHING` ✅
- InboxBusDecorator: оборачивает все subscribe ✅

#### 11. Canonical write path ⚠️

Pipeline реализован полностью:
```
HTTP -> auth middleware -> RBAC -> BEGIN+RLS -> handler -> commit -> audit
```

**Но:** read path не проходит через транзакцию, RLS не работает (см. P0). Canonical **write** path корректен, canonical **read** path — нет.

#### 12-15. UUID v7, tenant_id, core contracts

- UUID v7 повсеместно (0 использований v4) ✅
- tenant_id в EventEnvelope всегда ✅, в payload не консистентен ⚠️
- Core contracts стабильны, trait signatures не менялись ✅

### Блок 3: Качество кода

#### 16. Инкапсуляция ⚠️

- BC экспортируют `pub mod application`, `pub mod domain`, `pub mod infrastructure` — слишком широко (R2)
- `pub(crate)` используется для внутренних типов в db crate ✅

#### 17. Ошибки ✅

- `thiserror` для типизированных ошибок
- `anyhow` — в infrastructure и tests (шире заявленного "только main/tests", но допустимо для adapter code)
- `IntoInternal` trait — ergonomic `.internal("context")?`

#### 18. Тесты ✅

Integration tests покрывают critical path:
- Warehouse: 7 тестов (happy path, cumulative, auth, query, RLS, relay, seq_gen)
- Catalog: 11 тестов (happy path, duplicate, auth, cross-BC projection, tenant isolation)
- Domain unit-тесты в каждом BC

#### 19. Convenience layer ✅

- `PgCommandContext` — downcast UoW + emit_events + record_change
- `ReadDbContext` — pool checkout + RLS (но RLS не работает, см. P0)
- `BcRouter` — builder для HTTP routes BC
- `from_body!` / `from_query_params!` макросы
- `IntoInternal` trait

### Блок 4: Cross-BC взаимодействие

#### 20. Product projections ✅

- `warehouse.product_projections` таблица существует (migration V005)
- `ProductCreatedHandler` подписывается на `erp.catalog.product_created.v1`
- UPSERT (идемпотентно)
- `GetBalanceQuery` обогащает результат product_name из projection

#### 21. Event handling идемпотентность ✅

- InboxAwareHandler dedup по `(event_id, handler_name)`
- ProjectCreatedHandler использует UPSERT — safe on replay
- Relay sequential -> concurrent delivery невозможна

#### 22. Shared Kernel ✅

- kernel содержит только контракты: traits, ID newtypes, errors, CloudEvent
- 0 бизнес-VO в kernel
- Sku дублируется в warehouse и catalog — осознанное решение по DDD

---

## Аудит документации (консолидированный)

| Документ | R1 | R2 | Итого | Рекомендация |
|----------|----|----|-------|-------------|
| `CLAUDE.md` | ⚠️ Частично устарел | ❌ Вводит в заблуждение | ❌ | Обновить целиком: пути, статусы, crate list |
| `docs/plan/EXECUTION_PLAN.md` | ⚠️ Частично | ⚠️ Частично | ⚠️ | Обновить статусы фаз |
| `docs/plan/engineering_invariants.md` | ✅ | ⚠️ | ⚠️ | Исправить разделы про RLS и auth |
| `docs/auth_overview.md` | ⚠️ | ❌ | ❌ | Переписать под PermissionRegistry |
| `docs/event_bus_architecture.md` | ✅ | — | ✅ | Оставить |
| `docs/testing_integration_style.md` | ✅ | ✅ | ✅ | Оставить |
| `docs/api_testing_rules.md` | ✅ | ✅ | ✅ | Оставить |
| `docs/target_architecture.md` | ✅ | ⚠️ | ⚠️ | Обновить имена/пути |
| `docs/phase2_spec.md` | ⚠️ Безвреден | — | ⚠️ | Пометить IMPLEMENTED |
| `docs/phase3_spec.md` | ⚠️ Безвреден | — | ⚠️ | Пометить IMPLEMENTED |
| `README.md` | — | ❌ | ❌ | Обновить структуру workspace |
| `doc/dbinfo.md` | — | ❌ | ❌ | **Удалить (секреты!)** |

---

## Финальный вердикт

### Что действительно хорошо (обе рецензии согласны)

1. **BC isolation** — warehouse и catalog правильно изолированы, связь только через events
2. **Outbox/Inbox/Relay** — надёжная реализация с retry, DLQ, dedup
3. **Rich domain model** — агрегаты с поведением, VO с валидацией, domain events
4. **CommandPipeline** — полный canonical path (auth -> hooks -> tx -> handler -> commit -> audit)
5. **Type safety** — newtype ID, BigDecimal для количеств, UUID v7
6. **Convenience layer** — PgCommandContext, BcRouter, DTO macros убирают boilerplate
7. **Тестовое покрытие** — integration tests покрывают critical path, auth, RLS (write), events

### Что нужно исправить до масштабирования

| Приоритет | Задача | Оценка трудоёмкости |
|-----------|--------|-------------------|
| 🔴 P0 | RLS на read path + FORCE RLS | 2-4 часа |
| 🔴 P1 | Repository trait (onion fix) | 4-6 часов |
| 🟡 P2 | Обновление документации | 2-3 часа |
| 🟡 P3 | Удалить секреты из repo | 15 минут |
| 🟡 P4 | Уточнить sqlx инвариант | 30 минут |
| 🟡 P5 | Integration events (по мере роста) | Не блокер сейчас |

### Ответ на главный вопрос

> **Может ли новый разработчик за 1-2 дня создать новый BC по образцу Warehouse?**

**R1 ответил:** Да.
**R2 ответил:** Скорее нет, не безопасно без сопровождения.

**Консолидированный ответ: Нет — пока. Да — после двух исправлений.**

Сейчас новый разработчик:
- **Сможет** собрать структуру crate по образцу (domain -> application -> infrastructure -> module -> registrar)
- **Сможет** подключить pipeline, outbox, audit, events
- **Унаследует** сломанный RLS на read path (не осознавая, что tenant isolation для queries не работает)
- **Унаследует** application->infrastructure зависимость (закрепляя нарушение onion)
- **Будет путаться** в устаревшей документации

**После исправления P0 (RLS) и P1 (onion)** — платформа достаточно зрелая для масштабирования. Оба фикса суммарно занимают ~1 рабочий день, после чего ответ становится уверенным "да".

---

## Рефлексия: ошибки первого ревью

R1 допустил системную ошибку: **проверял наличие кода, а не его корректность**. `set_tenant_context()` вызывается в `ReadDbContext::acquire()` — формально "RLS есть". Но без `BEGIN` этот вызов не имеет эффекта в PostgreSQL. Второй рецензент пошёл на уровень глубже — проверил в psql — и обнаружил реальную проблему.

Урок для будущих ревью: **инварианты уровня инфраструктуры нельзя проверять только code review. Нужен runtime verification** (тест, psql, MCP-запрос к БД).

R1 также проявил склонность к "benefit of the doubt" — нашёл нарушение onion архитектуры, но понизил серьёзность до "improvement" с аргументом "прагматичное решение для MVP". Это reasonable в контексте одного проекта, но не годится для architectural review, задача которого — выявить все нарушения заявленных принципов, независимо от прагматических оправданий.
