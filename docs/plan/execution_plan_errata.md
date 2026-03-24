# EXECUTION_PLAN errata: sqlx → Clorinde
> Применить к docs/EXECUTION_PLAN.md

---

## Phase 1, задача 1.1

**Было:**
```
**1.1** `PgPool` + конфигурация (.env → DATABASE_URL)
- sqlx PgPool с настраиваемым max_connections
- Health check: `SELECT 1`
```

**Стало:**
```
**1.1** `PgPool` + конфигурация (.env → DATABASE_URL)
- deadpool-postgres Pool с настраиваемым max_size
- Health check: `SELECT 1`
```

---

## Phase 1, критерий готовности

**Было:**
```bash
cargo test -p db                          # PgUoW tests pass
cargo test -p runtime --features pg-tests # pipeline + PgUoW integration
```

**Стало:**
```bash
cargo test -p db                          # PgUoW tests pass
cargo test -p runtime --features pg-tests # pipeline + PgUoW integration
# Примечание: SQL выполняется через tokio-postgres, не sqlx
```

---

## Общее

Все упоминания `sqlx` в EXECUTION_PLAN заменить на `tokio-postgres` / `deadpool-postgres`.
Database stack проекта: `tokio-postgres` + `deadpool-postgres` + `Clorinde CLI`.
