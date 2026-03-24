# Engineering Invariants
> Правила, которые действуют с первого коммита и никогда не ослабляются.
> Каждое правило — ответ на конкретный анти-паттерн из LEG (legacy research).
> Дата: 2026-03-24

---

## Правило принятия решений

Для каждой фичи один вопрос:

**Это временное упрощение бизнес-правил или временное упрощение архитектурного механизма?**

- Бизнес-правил → обычно можно: «пока без партий, сроков годности и резервов»
- Механизма → почти всегда нельзя: «пока без прав / без audit / без событий»

---

## Инварианты

### 1. Все write-операции — только через CommandPipeline

Нет «быстрых» INSERT мимо pipeline. Handler — единственное место, где происходит
доменное изменение. Pipeline обеспечивает: auth → hooks → TX → audit.

**Как проверить:** grep по codebase на INSERT/UPDATE вне handler'ов.

### 2. Все write-операции — с PermissionChecker (deny by default)

Команды имеют стабильные permission keys: `warehouse.receive_goods`.
Проверка — только в pipeline. Handler не знает про роли.
Если роль не указана в PermissionMap — доступ запрещён.

**Как проверить:** тест с пустым списком ролей → 403.

### 3. Все write-операции — с audit log + domain history

Audit log: кто, когда, какая команда, какой результат.
Domain history: old_state / new_state JSONB, entity_type, entity_id.
Связь через correlation_id + causation_id + user_id.

**Как проверить:** после каждого write → SELECT audit_log, SELECT domain_history.

### 4. Все межконтекстные связи — только через integration events

Принимающий контекст строит локальную проекцию/зеркало.
Источник не шарит таблицы напрямую.
Формат событий versioned: `erp.{bc}.{event_name}.v{N}`.

**Как проверить:** Cargo.toml: warehouse не зависит от catalog (и наоборот).

### 5. Никаких direct SQL reads across BC boundaries

Каждый BC имеет свои `queries/*.sql` (Clorinde) и свою PostgreSQL-схему.
Чужие данные — через локальные проекции, обновляемые по событиям.

**Как проверить:** SQL-файлы в `crates/warehouse/queries/` содержат только `warehouse.*` таблицы.

### 6. Все events versioned

Формат: `erp.{bc_name}.{event_name}.v{version}`
Пример: `erp.warehouse.goods_received.v1`
При breaking change → v2, v1 продолжает поддерживаться или мигрируется.

### 7. Все tenant-aware таблицы — tenant_id + RLS policy

```sql
ALTER TABLE {table} ENABLE ROW LEVEL SECURITY;
ALTER TABLE {table} FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON {table}
  USING (tenant_id = current_setting('app.tenant_id')::uuid);
```

PgUnitOfWork при begin: `SET LOCAL app.tenant_id = $1`.

**Как проверить:** тест с двумя tenant'ами — данные изолированы.

### 8. Никакой записи мимо UnitOfWork

Все INSERT/UPDATE/DELETE для доменных данных — только внутри PgUoW TX.
Outbox entry — в той же TX (атомарность).
Clorinde-generated functions вызываются через `transaction` reference из UoW.

### 9. RequestContext обязателен в каждом handler

Handler всегда получает `&RequestContext` с tenant_id, user_id,
correlation_id, causation_id, timestamp, roles.

### 10. Handler не знает про роли

Handler не вызывает `check_permission()` и не читает `ctx.roles`.
Авторизация — ответственность pipeline, до вызова handler'а.

---

## Data access: Clorinde workflow

Каждый BC имеет `queries/*.sql` с аннотированными запросами.
Clorinde генерирует type-safe Rust functions.
Handler'ы и repos вызывают сгенерированные функции, не пишут SQL inline.

```
crates/{bc}/queries/
  inventory.sql    ← --! upsert_balance : Balance
  projections.sql  ← --! get_item_projection : ItemProjection
```

SQL — часть deliverable наравне с Rust кодом. AI-агент генерирует оба.

---

## Чеклист для нового BC

При создании нового Bounded Context проверяем:

- [ ] Все commands реализуют `kernel::Command` с `command_name()`
- [ ] Все handlers реализуют `runtime::CommandHandler`
- [ ] Все write пути — через pipeline, нет обходных INSERT
- [ ] SQL-запросы в `queries/*.sql`, Clorinde генерирует Rust
- [ ] Migrations содержат `ENABLE ROW LEVEL SECURITY` + `FORCE` + policy
- [ ] Все таблицы в своей PostgreSQL-схеме (`{bc_name}.*`)
- [ ] Events именованы как `erp.{bc}.{name}.v1`
- [ ] Чужие данные — через event handler + local projection, не через cross-schema SQL
- [ ] Domain tests работают без БД (pure Rust)
- [ ] Integration tests проверяют: outbox, audit, RLS, projection sync
- [ ] BC_CONTEXT.md заполнен (canvas, commands, events, rules, projections needed)
