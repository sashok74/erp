# Синхронизация документации
> Инструкция: какие изменения внести в существующие файлы.
> Дата: 2026-03-24

---

## 1. Добавить в начало каждого layer spec

### docs/layer0_spec.md

```
> **STATUS: ARCHIVED** — Layer 0 реализован. Код в `crates/` — source of truth.
> **Data access:** spec упоминает sqlx — в проекте используется Clorinde + tokio-postgres.
> Актуальный план: `docs/EXECUTION_PLAN.md`
```

### docs/layer1_spec.md

```
> **STATUS: ARCHIVED** — Layer 1 реализован. Код в `crates/kernel/` — source of truth.
> **Delta:** RequestContext.roles добавлен (для auth). sqlx::Type → postgres-types ToSql/FromSql.
> Актуальный план: `docs/EXECUTION_PLAN.md`
```

### docs/layer3a_spec.md

```
> **STATUS: ARCHIVED** — Layer 3a реализован. Код в `crates/event_bus/` — source of truth.
> **Delta:** InProcessBus — direct dispatch (не broadcast channel). Лучше spec'а.
> Актуальный план: `docs/EXECUTION_PLAN.md`
```

### docs/layer5a_spec.md

```
> **STATUS: ARCHIVED** — Layer 5a реализован. Код в `crates/runtime/` — source of truth.
> Auth (Layer 4a) тоже реализован в `crates/auth/`.
> Актуальный план: `docs/EXECUTION_PLAN.md`
```

---

## 2. Новые файлы в docs/

| Файл | Статус |
|------|--------|
| docs/EXECUTION_PLAN.md | **ACTIVE** — главный execution doc |
| docs/engineering_invariants.md | **ACTIVE** — неснижаемые правила |

---

## 3. Обновить CLAUDE.md

Добавить:

```markdown
## Стек данных

PostgreSQL через **Clorinde** (кодогенерация из .sql файлов) + **tokio-postgres** + **deadpool-postgres**.
НЕ sqlx. SQL-запросы живут в `queries/*.sql` внутри каждого crate.

## Ключевые документы (порядок чтения)

1. `docs/EXECUTION_PLAN.md` — что делать дальше, в каком порядке
2. `docs/engineering_invariants.md` — 10 правил, которые никогда не ослабляются
3. `docs/event_bus_architecture.md` — как работает шина событий
4. `crates/*/src/lib.rs` — точка входа в каждый crate

## При создании нового Bounded Context

1. Прочитать `docs/EXECUTION_PLAN.md` фаза 6 (BC Template)
2. Прочитать `crates/warehouse/BC_CONTEXT.md` (reference implementation)
3. Пройти чеклист из `docs/engineering_invariants.md`
4. SQL-запросы — в `queries/*.sql` (Clorinde), не inline
```

---

## 4. Обновить Cargo.toml (workspace root)

Заменить секцию `[workspace.dependencies]` — Database:

```toml
# Было:
sqlx = { version = "0.8", features = [...] }

# Стало:
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
deadpool-postgres = "0.14"
postgres-types = { version = "0.2", features = ["derive", "with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
clorinde = "0.12"
refinery = { version = "0.8", features = ["tokio-postgres"] }
```

---

## 5. Обновить crates/kernel/Cargo.toml

```toml
# Было:
sqlx = { workspace = true }

# Стало:
postgres-types = { workspace = true }
```

---

## 6. Обновить crates/kernel/src/types.rs

Три ID типа — заменить derive:

```rust
// Было:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TenantId(Uuid);

// Стало:
use postgres_types::{ToSql, FromSql};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSql, FromSql)]
#[postgres(transparent)]
pub struct TenantId(Uuid);
```

Аналогично для `UserId` и `EntityId`.

---

## 7. Обновить project_index.md

Добавить:

```markdown
| 18 | EXP | Execution Plan — главный план реализации | 2026-03-24 |
| 19 | INV | Engineering Invariants — неснижаемые правила | 2026-03-24 |
```

В «Эволюция проекта»:

```markdown
**Фаза 7 — Execution:** EXP + INV (план + инварианты, Clorinde вместо sqlx)
```

---

## 8. Что НЕ меняем

- `erp_architecture_diagrams_mermaid.md` — актуален, без изменений
- `docs/event_bus_architecture.md` — актуален, без изменений
- Исследовательские чаты (SRV–IMP) — архив, без изменений
- `erp_arch_principles_explained.md` — актуален, без изменений
- Код event_bus, runtime, auth — без изменений (не зависят от DB layer)

---

## Итог

```
docs/
├── EXECUTION_PLAN.md            ← ACTIVE: единственный execution doc
├── engineering_invariants.md     ← ACTIVE: 10 неснижаемых правил
├── event_bus_architecture.md     ← Reference
├── layer0_spec.md                ← Archived + Clorinde note
├── layer1_spec.md                ← Archived + delta notes
├── layer3a_spec.md               ← Archived
└── layer5a_spec.md               ← Archived

Cargo.toml                        ← sqlx → tokio-postgres + clorinde
crates/kernel/Cargo.toml          ← sqlx → postgres-types
crates/kernel/src/types.rs        ← sqlx::Type → ToSql/FromSql
CLAUDE.md                         ← + Clorinde note + doc links
```

Два рулящих документа. sqlx нигде. Clorinde — единственный путь к БД.
