# CLAUDE.md — Инструкции для работы с проектом
> Этот файл читается Claude Code и Claude AI при работе с проектом.
> Обновлять по мере развития проекта.

---

## Ключевые документы (порядок чтения)

1. **`docs/plan/EXECUTION_PLAN.md`** — что делать дальше, в каком порядке. **Главный документ.** При конфликте с layer specs — EXECUTION_PLAN главнее.
2. **`docs/plan/engineering_invariants.md`** — правила, которые никогда не ослабляются
3. **`docs/event_bus_architecture.md`** — как работает шина событий
4. **`crates/*/src/lib.rs`** — точка входа в каждый crate, doc-comments объясняют назначение
5. **`docs/testing_integration_style.md`** — как писать integration tests для BC без копипасты setup; новые integration tests делать только по этому шаблону
6. **`docs/api_testing_rules.md`** — как проверять HTTP API новых BC, строить Postman/Newman regression suite и что считается Definition of Done для API

## UI Engine

7. **`doc/ui/guide.md`** — архитектура UI-движка, как добавлять экраны, обработка действий, запрещённые паттерны (смешение метаданных и данных, бизнес-логика на клиенте). **Обязательно к прочтению перед любой работой с `ui/`.**

## При создании нового Bounded Context

1. Прочитать `docs/plan/EXECUTION_PLAN.md` фаза 6 (BC Template)
2. Прочитать `crates/warehouse/BC_CONTEXT.md` (reference implementation)
3. Проверить чеклист из `docs/plan/engineering_invariants.md`

---

## Проект

**ERP нового поколения** — производственная ERP-система для российского рынка (дискретное производство, СМБ). Modular monolith на Rust, PostgreSQL, Event Sourcing для ключевых модулей.

Учебный + исследовательский проект: каждая задача — одновременно изучение Rust и строительство реального слоя ERP.

---

## Среда разработки

- **ОС:** Debian 12 (LXC-контейнер на Proxmox)
- **Корень проекта:** `/home/dev/projects/erp/`
- **PostgreSQL:** отдельный сервер в локальной сети, подключение через `DATABASE_URL` из `.env`
- **Rust:** stable toolchain, edition 2024
- **Task runner:** `just` (justfile в корне проекта)
- **IDE:** VS Code Remote SSH с rust-analyzer
- **MCP:** `erp-dev-pg` — прямой доступ к PostgreSQL (erp_dev) для инспекции схемы, данных, зависимостей. Позволяет выполнять SQL-запросы, смотреть структуру таблиц, сравнивать снимки данных, профилировать колонки и т.д.

---

## Архитектура

### Ключевые решения

- **Modular monolith** — один binary, crate per Bounded Context, split to microservices позже
- **PostgreSQL shared DB** — одна БД, schema per BC, tenant_id + RLS для изоляции
- **Clorinde (SQL-first)** — SQL-запросы в `.sql` файлах → `clorinde generate` → типобезопасный Rust crate. **Не sqlx, не Diesel, не ORM.** Драйвер: `tokio-postgres` + `deadpool-postgres`
- **Hybrid Event Sourcing** — полный ES для Warehouse и Finance, CRUD+events для остальных
- **In-process event bus** — tokio channels; заменяется на RabbitMQ/NATS/Kafka через trait EventBus
- **Single ACID TX** для cross-BC ops, async after-commit для side effects
- **MVP домен:** Warehouse (складской учёт)

### Database stack

```
tokio-postgres        — async PostgreSQL driver (low-level)
deadpool-postgres     — connection pool
postgres-types        — Rust ↔ PG type mapping (derive ToSql/FromSql)
Clorinde CLI          — SQL → типобезопасный Rust crate (codegen)
```

**НЕ используем:** sqlx, Diesel, SeaORM, любые ORM.

### Clorinde: SQL-first подход

```
queries/                        ← SQL-файлы по BC
  ├── common/
  ├── warehouse/
  └── catalog/

crates/clorinde-gen/            ← СГЕНЕРИРОВАННЫЙ crate
```

Цепочка: `.sql` файл → `clorinde generate` → Rust crate → используется в infrastructure.

### Принцип: минимум бизнеса, максимум механизмов

Каждый архитектурный механизм работает с первого дня. Бизнес-логика минимальна.

- «Пока без партий и резервов» — допустимо (упрощение бизнеса)
- «Пока без audit / без events / без RLS» — **недопустимо** (упрощение механизма)

### Структура crate'ов

```
crates/
  kernel/         — Platform SDK: трейты, ID, ошибки, CloudEvent, security. Без зависимости от БД.
  event_bus/      — EventBus trait + InProcessBus. Заменяем на RabbitMQ/NATS.
  runtime/        — CommandPipeline, QueryPipeline, CommandHandler/QueryHandler, ports, stubs.
  auth/           — JWT, PermissionRegistry (BC-owned RBAC), PermissionChecker, axum middleware.
  db/             — PgPool (deadpool), RLS, PgUnitOfWork, ScopedPgConnection, migrations, outbox relay.
  clorinde-gen/   — СГЕНЕРИРОВАННЫЙ: типобезопасные SQL-функции.
  audit/          — PgAuditLog (impl AuditLog trait).
  seq_gen/        — PgSequenceGenerator (gap-free per-tenant).
  bc_http/        — Общие HTTP-утилиты для BC: DTO-макросы, repo-макросы, axum helpers.
  warehouse/      — Reference BC: domain, application, infrastructure.
  catalog/        — Второй BC: CreateProduct, GetProduct, cross-BC events.
  gateway/        — axum HTTP server (единственный binary), AppBuilder.
  test_support/   — Общий setup для integration tests.
  extensions/     — Lua (mlua) + WASM (wasmtime) (planned).
```

---

## Engineering Invariants (сокращённо)

Полный список: `docs/plan/engineering_invariants.md`

1. Все write — только через CommandPipeline
2. Все write — с PermissionChecker (deny by default)
3. Все write — с audit log + domain history
4. Межконтекстные связи — только через integration events
5. Нет direct SQL reads across BC boundaries
6. Events versioned: `erp.{bc}.{event}.v{N}`
7. Все tenant-таблицы: tenant_id + RLS policy
8. Никакой записи мимо UnitOfWork
9. RequestContext обязателен в каждом handler
10. Handler не знает про роли

---

## Правила написания кода

### Kernel = Platform SDK

- Только контракты, ID, ошибки, CloudEvent
- **НЕТ** tokio-postgres, deadpool, Clorinde в kernel
- Value Objects (SKU, Quantity, Money) — в domain/ каждого BC

### Rust-стиль

- Edition 2024, clippy pedantic
- `thiserror` для ошибок, `anyhow` только в main/tests
- Newtype для ID: `TenantId(Uuid)`, `UserId(Uuid)`, `EntityId(Uuid)`
- UUID v7 (time-ordered)
- BigDecimal для денег/количеств (в BC, не в kernel)
- `pub(crate)` для инкапсуляции

### Naming conventions

- Commands: `warehouse.receive_goods` (permission keys)
- Events: `erp.warehouse.goods_received.v1` (CloudEvents convention)
- Schemas: `common.*`, `warehouse.*`, `finance.*`
- Sequences: `warehouse.receipt` (prefix + counter)

### SQL + Clorinde

- DDL-миграции: `migrations/{bc}/NNN_description.sql`
- DML-запросы: `queries/{bc}/entity.sql`
- Генерация: `just clorinde-generate`
- Row structs — в clorinde-gen (автогенерация), не ручные

### Тестирование

- Domain unit-тесты: `#[cfg(test)] mod tests`, без БД
- Integration: с реальной PostgreSQL через DATABASE_URL
- Integration tests для BC писать по `docs/testing_integration_style.md`; общий setup брать из `crates/test_support`
- API regression и Postman/Newman сценарии для новых BC строить по `docs/api_testing_rules.md`
- Pipeline тесты: со stubs, мгновенные

---

## Команды

```bash
just build              # собрать workspace
just test               # все тесты
just test-crate X       # тесты одного crate
just lint               # clippy -D warnings
just fmt / fmt-check    # форматирование
just deny               # лицензии + advisories
just check              # полная проверка
just run                # запустить gateway
just db-migrate         # применить миграции (запускает gateway)
just db-reset           # пересоздать БД (затем just run)
just clorinde-generate  # перегенерировать SQL crate
```

---

## Текущий статус

Фазы 0-5 реализованы. Все механизмы работают end-to-end.

| Crate | Роль |
|-------|------|
| kernel | Platform SDK: TenantId, UserId, Command, DomainEvent, AggregateRoot, AppError, CloudEvent, RequestContext, security (PermissionRegistrar, PermissionManifest) |
| event_bus | EventBus trait, InProcessBus, EventEnvelope, HandlerRegistry |
| runtime | CommandPipeline, QueryPipeline, CommandHandler, QueryHandler, ports, stubs |
| auth | JwtService, PermissionRegistry (BC-owned RBAC), JwtPermissionChecker, axum middleware |
| db | PgPool (deadpool), RLS, PgUnitOfWork, ScopedPgConnection, миграции (refinery), outbox relay |
| clorinde-gen | СГЕНЕРИРОВАННЫЙ: типобезопасные SQL-функции для common/warehouse/catalog |
| audit | PgAuditLog (impl AuditLog trait) |
| seq_gen | PgSequenceGenerator (gap-free per-tenant) |
| bc_http | Общие HTTP-утилиты для BC: DTO-макросы, repo-макросы, axum helpers |
| warehouse | Reference BC: domain, application, infrastructure, integration tests |
| catalog | Второй BC: CreateProduct, GetProduct, cross-BC events |
| gateway | axum HTTP server, AppBuilder, модульная сборка BC |
| test_support | Общий setup для integration tests (PgPool, миграции, tenant) |

### Следующий: Phase 6 -- BC Template Extraction

См. `docs/plan/EXECUTION_PLAN.md` Phase 6.
