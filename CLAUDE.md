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
- **In-process event bus** — tokio channels, outbox/inbox для будущего split
- **Single ACID TX** для cross-BC consistency-critical ops, async after-commit для side effects
- **Lua + WASM** extension layer для tenant-кастомизации
- **Thin web UI** — askama + htmx, не SPA
- **MVP домен:** Warehouse (складской учёт)

### Слои (Layers)

```
Layer 0  Cargo Workspace + CI/CD scaffold
Layer 1  Kernel: базовые типы, трейты, ошибки
Layer 2  Data Access: PostgreSQL + миграции + RLS
Layer 3  Event Infrastructure: in-process bus + outbox
Layer 4  Generic Contexts: Auth/JWT, Audit, SeqGen
Layer 5  BC Runtime: Command Pipeline
Layer 6  MVP Domain: Warehouse BC (полный Event Sourcing)
Layer 7  API Gateway: HTTP-слой (axum)
Layer 8  Extension Runtime: Lua (+ WASM заготовка)
Layer 9  Thin Web UI: HTML/CSS/JS (не SPA)
```

### Структура crate'ов

```
crates/
  kernel/       — базовые типы, трейты, value objects (нулевые зависимости от infra)
  db/           — PgPool, RLS, UnitOfWork, миграции, outbox
  event_bus/    — InProcessBus, EventHandler trait, relay, DLQ
  auth/         — JWT issue/verify, RBAC, axum middleware
  audit/        — structured audit log writer
  seq_gen/      — gap-free sequence generator per tenant
  runtime/      — CommandPipeline, CommandHandler/QueryHandler traits, BoundedContextModule
  extensions/   — Lua sandbox (mlua), WASM (wasmtime)
  warehouse/    — MVP: domain (aggregates, events), application (commands, queries), infrastructure (pg repo)
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

### Rust-стиль

- **Edition 2024** (`rust-version = "1.85"`)
- **Clippy pedantic** включён: `#![warn(clippy::pedantic)]`
- `#![allow(clippy::module_name_repetitions)]` — допускается в DDD-контексте
- **rustfmt** — max_width = 100, tab_spaces = 4
- **thiserror** для типизированных ошибок, **anyhow** только в main/tests
- **Newtype pattern** для всех идентификаторов: `TenantId(Uuid)`, `UserId(Uuid)`, `EntityId(Uuid)`
- **UUID v7** (time-ordered) для всех новых ID
- **BigDecimal** для денег и количеств (не f64)
- Все публичные типы — `#[derive(Debug, Clone, Serialize, Deserialize)]` где уместно
- Комментарии в коде — на русском для бизнес-логики, на английском для технических деталей

### Зависимости между crate'ами

```
kernel          ← ни от кого (чистые типы)
db              ← kernel
event_bus       ← kernel
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
2. **Создание конкретного слоя ERP** (kernel, data access, event bus, etc.)

При генерации кода для задачи:
- Объяснять выбор подхода (почему так принято в Rust community)
- Показывать связь с архитектурой ERP (зачем этот код в контексте системы)
- Писать тесты для каждого публичного API
- Следовать onion architecture: Domain не зависит от Infrastructure

---

## Документация проекта

- `docs/layer0_spec.md` — ТЗ Layer 0 (workspace, toolchain, justfile)
- `docs/` — ТЗ для остальных Layer'ов (добавляются по мере работы)
- `CLAUDE.md` — этот файл

### Исследовательская база (Project Knowledge в Claude)

- `project_index.md` — индекс всех 17 исследовательских артефактов
- `erp_architecture_diagrams_mermaid.md` — 6 архитектурных схем (Mermaid)

Полные исследовательские документы (SRV, CLF, UNI, CLD, DBP, CUS, LEG, VS1, DMS, KRN, ARC, DDD, ADR, SHR, VIZ, BCR, IMP) доступны по ссылкам в project_index.md. При необходимости — запросить у пользователя конкретный документ.

---

## Текущий статус

- [x] Исследование и архитектурные решения (17 артефактов)
- [x] ADR v1 зафиксирован
- [x] Архитектурные диаграммы (6 типов)
- [x] План реализации (60 задач, Layers 0–9)
- [x] LXC-контейнер подготовлен (Debian 12, Rust, Docker, PostgreSQL client)
- [ ] **Layer 0** — Cargo workspace + toolchain + justfile ← ТЕКУЩИЙ ЭТАП
- [ ] Layer 1 — Kernel types and traits
- [ ] Layer 2 — Data Access (PostgreSQL + RLS)
- [ ] Layer 3 — Event Infrastructure
- [ ] Layer 4 — Generic Contexts (Auth, Audit, SeqGen)
- [ ] Layer 5 — BC Runtime (Command Pipeline)
- [ ] Layer 6 — Warehouse BC (MVP)
- [ ] Layer 7 — API Gateway (axum)
- [ ] Layer 8 — Extension Runtime (Lua + WASM)
- [ ] Layer 9 — Thin Web UI
