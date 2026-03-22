# CLAUDE.md — Инструкции для работы с проектом
> Этот файл читается Claude Code и Claude AI при работе с проектом.
> Обновлять по мере развития проекта.

---

## Проект

**ERP нового поколения** — производственная ERP-система для российского рынка (дискретное производство, СМБ). Modular monolith на Rust, PostgreSQL, Event Sourcing для ключевых модулей.

Это **учебный + исследовательский проект**: каждая задача — одновременно изучение Rust и строительство реального слоя ERP. Код должен быть production-quality, но с пояснениями «почему так».

---

## Среда разработки

- **ОС:** Debian 12 (LXC-контейнер на Proxmox)
- **Корень проекта:** `/home/dev/projects/erp/`
- **PostgreSQL:** отдельный сервер в локальной сети (не localhost), подключение через `DATABASE_URL` из `.env`
- **Rust:** stable toolchain, edition 2024
- **Task runner:** `just` (justfile в корне проекта)
- **IDE:** VS Code Remote SSH с rust-analyzer

---

## Архитектура

### Ключевые решения (ADR v1)

- **Modular monolith** — один binary, crate per Bounded Context, split to microservices позже
- **PostgreSQL shared DB** — одна БД, schema per BC, tenant_id + RLS для изоляции
- **Hybrid Event Sourcing** — полный ES для Warehouse и Finance, CRUD+events для остальных
- **In-process event bus** — tokio channels сейчас; **легко заменяется на RabbitMQ/NATS/Kafka** при переходе к микросервисам (trait EventBus — единственная точка замены)
- **Single ACID TX** для cross-BC consistency-critical ops, async after-commit для side effects
- **Lua + WASM** extension layer для tenant-кастомизации
- **Thin web UI** — askama + htmx, не SPA
- **MVP домен:** Warehouse (складской учёт)

### Порядок реализации (trait-first, inside-out)

Принцип: **сначала абстракции (traits/порты), потом инфраструктура (адаптеры/БД)**.
Onion Architecture: центр (Domain, Application) → периферия (Infrastructure).

```
Phase 1 — Абстракции (без БД):
  Layer 0   Cargo Workspace + CI/CD scaffold
  Layer 1   Kernel: Platform SDK (трейты, ID, ошибки, CloudEvent)
  Layer 3a  Event Bus: traits + InProcessBus (tokio channels, без БД)
  Layer 5a  BC Runtime: traits + Command Pipeline (stubs, без БД)
  Layer 4a  Auth: JWT + RBAC (без БД)

Phase 2 — Инфраструктура (с БД):
  Layer 2   Data Access: PostgreSQL + миграции + RLS + UoW
  Layer 4b  Audit + SeqGen (реализация поверх БД)
  Layer 3b  Outbox Relay + DLQ (фоновый worker, poll БД)
  Layer 5b  Pipeline: wiring реальных зависимостей

Phase 3 — Бизнес-логика:
  Layer 6   Warehouse BC (MVP, полный Event Sourcing)

Phase 4 — Внешний периметр:
  Layer 7   API Gateway (axum)
  Layer 8   Extension Runtime (Lua + WASM)
  Layer 9   Thin Web UI (askama + htmx)
```

### Структура crate'ов

```
crates/
  kernel/       — Platform SDK: контракты (трейты), идентификаторы, ошибки, CloudEvent.
                  НЕ содержит бизнес-примитивов (Value Objects) — они в каждом BC.
  db/           — PgPool, RLS, UnitOfWork, миграции, outbox
  event_bus/    — EventBus trait + InProcessBus (tokio channels).
                  Trait = точка замены на RabbitMQ/NATS/Kafka.
  auth/         — JWT issue/verify, RBAC, axum middleware
  audit/        — structured audit log writer
  seq_gen/      — gap-free sequence generator per tenant
  runtime/      — CommandPipeline, CommandHandler/QueryHandler traits, BoundedContextModule
  extensions/   — Lua sandbox (mlua), WASM (wasmtime)
  warehouse/    — MVP: domain (aggregates, value objects, events), application (commands, queries), infrastructure (pg repo)
  gateway/      — axum HTTP server, routing, middleware, error handler (единственный binary)
```

### Onion Architecture внутри каждого BC

```
Domain (ядро)       — агрегаты, entities, value objects, domain events. Нулевые зависимости.
Application         — command/query handlers, сервисные интерфейсы (порты)
Infrastructure      — PostgreSQL repo, event publishers, внешние API клиенты
```

Зависимости строго внутрь: Infrastructure → Application → Domain.

---

## Правила написания кода

### Kernel = Platform SDK

- Kernel содержит **только** контракты (трейты), идентификаторы, ошибки и формат событий
- **Value Objects (SKU, Quantity, Money, LotNumber) НЕ в kernel** — они в `domain/value_objects.rs` каждого BC
- Причина: сторонние разработчики смогут писать BC с собственными бизнес-типами, kernel не навязывает им свои ограничения
- BC общаются через integration events с примитивными типами в payload (String, Uuid, числа) — Anti-Corruption Layer

### EventBus = заменяемая абстракция

- `EventBus` trait определён в `event_bus` crate
- `InProcessBus` — реализация на tokio channels для modular monolith
- При переходе к микросервисам: `RabbitMqBus`, `NatsBus`, `KafkaBus` — другая реализация того же trait
- Domain и Application слои **не знают** какой bus используется

### Rust-стиль

- **Edition 2024** (`rust-version = "1.85"`)
- **Clippy pedantic** включён: `#![warn(clippy::pedantic)]`
- `#![allow(clippy::module_name_repetitions)]` — допускается в DDD-контексте
- **rustfmt** — max_width = 100, tab_spaces = 4
- **thiserror** для типизированных ошибок, **anyhow** только в main/tests
- **Newtype pattern** для всех идентификаторов: `TenantId(Uuid)`, `UserId(Uuid)`, `EntityId(Uuid)`
- **UUID v7** (time-ordered) для всех новых ID
- **BigDecimal** для денег и количеств (не f64) — в value objects каждого BC
- Все публичные типы — `#[derive(Debug, Clone, Serialize, Deserialize)]` где уместно
- Комментарии в коде — на русском для бизнес-логики, на английском для технических деталей

### Row structs = заменяемый слой

- Row structs (`#[derive(sqlx::FromRow)]`) живут в infrastructure, не в domain
- Маппинг `Row → Domain` — в repository
- В будущем Row structs будут генерироваться Metadata Engine из метаданных
- Domain structs остаются ручными (бизнес-логика не генерируется)

### Зависимости между crate'ами

```
kernel          ← ни от кого (чистые типы и трейты)
event_bus       ← kernel
db              ← kernel
auth            ← kernel
audit           ← kernel, db
seq_gen         ← kernel, db
runtime         ← kernel, db, event_bus, auth, audit, extensions
extensions      ← kernel
warehouse       ← kernel, db, event_bus, runtime, seq_gen
gateway         ← kernel, auth, runtime, warehouse (+ будущие BC)
```

Циклические зависимости запрещены (Cargo это гарантирует).

### SQL и миграции

- **Миграции** в `migrations/common/` (общая инфра) и `migrations/<bc_name>/` (per BC)
- Формат имени: `V001__description.sql`
- Каждый BC — своя PostgreSQL schema: `warehouse.*`, `finance.*`, etc.
- Общая инфраструктура — schema `common.*`
- **RLS** на всех таблицах с данными tenant'ов
- Запуск: `just db-migrate`

### Тестирование

- Unit-тесты — `#[cfg(test)] mod tests` внутри файлов
- Integration-тесты — `tests/integration/`, используют testcontainers (Docker + PostgreSQL)
- **Trait-first тесты** — Pipeline и Bus тестируются с mock/stub реализациями, без БД, мгновенно
- Запуск: `just test` (всё) или `just test-crate kernel` (один crate)

---

## Команды

```bash
just build          # собрать workspace
just test           # все тесты
just test-crate X   # тесты crate X
just lint           # clippy --workspace -D warnings
just fmt            # rustfmt
just fmt-check      # проверка форматирования
just deny           # cargo-deny: лицензии + advisories
just check          # fmt-check + lint + deny + test (полная проверка)
just run            # запустить gateway
just watch          # авто-перезапуск при изменениях
just db-ping        # проверить подключение к PostgreSQL
just db-migrate     # применить миграции
just db-reset       # пересоздать БД
```

---

## Контекст для задач

Каждая задача в проекте — это одновременно:
1. **Изучение конкретной концепции Rust** (newtype, trait, async, lifetime, etc.)
2. **Создание конкретного слоя ERP** (kernel, event bus, runtime, etc.)

При генерации кода для задачи:
- Объяснять выбор подхода (почему так принято в Rust community)
- Показывать связь с архитектурой ERP (зачем этот код в контексте системы)
- Писать тесты для каждого публичного API
- Следовать onion architecture: Domain не зависит от Infrastructure
- Value objects — в BC, не в kernel
- EventBus — через trait, заменяемый на RabbitMQ/NATS

---

## Документация проекта

- `docs/layer0_spec.md` — ТЗ Layer 0 (workspace, toolchain, justfile)
- `docs/layer1_spec.md` — ТЗ Layer 1 (kernel: Platform SDK)
- `docs/layer3a_spec.md` — ТЗ Layer 3a (event bus: traits + InProcessBus)
- `docs/architecture_diagrams.md` — архитектурные схемы (Mermaid)
- `docs/` — ТЗ для остальных Layer'ов (добавляются по мере работы)
- `CLAUDE.md` — этот файл

### Исследовательская база (Project Knowledge в Claude)

- `project_index.md` — индекс всех 17 исследовательских артефактов
- `erp_architecture_diagrams_mermaid.md` — 6 архитектурных схем (Mermaid)

---

## Текущий статус

- [x] Исследование и архитектурные решения (17 артефактов)
- [x] ADR v1 зафиксирован
- [x] Архитектурные диаграммы (6 типов)
- [x] План реализации (Layers 0–9, trait-first порядок)
- [x] LXC-контейнер подготовлен (Debian 12, Rust, Docker, PostgreSQL client)
- [ ] **Layer 0** — Cargo workspace + toolchain + justfile
- [ ] **Layer 1** — Kernel: Platform SDK
- [ ] **Layer 3a** — Event Bus: traits + InProcessBus ← СЛЕДУЮЩИЙ ПОСЛЕ Layer 1
- [ ] Layer 5a — BC Runtime: traits + Pipeline (stubs)
- [ ] Layer 4a — Auth: JWT + RBAC
- [ ] Layer 2 — Data Access (PostgreSQL + RLS)
- [ ] Layer 4b — Audit, SeqGen (с БД)
- [ ] Layer 3b — Outbox Relay + DLQ (с БД)
- [ ] Layer 5b — Pipeline wiring
- [ ] Layer 6 — Warehouse BC (MVP)
- [ ] Layer 7 — API Gateway (axum)
- [ ] Layer 8 — Extension Runtime (Lua + WASM)
- [ ] Layer 9 — Thin Web UI
