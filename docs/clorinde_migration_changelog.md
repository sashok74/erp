# Изменения в существующих ТЗ: sqlx → Clorinde
> Применить при выполнении Layer 0 и Layer 1

---

## Layer 0 (layer0_spec.md) — что изменить

### workspace.dependencies в Cargo.toml

**Убрать:**
```toml
sqlx = { version = "0.8", features = [...] }
```

**Добавить:**
```toml
# --- Database (Clorinde + tokio-postgres) ---
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
deadpool-postgres = { version = "0.14" }
postgres-types = { version = "0.2", features = ["derive"] }
```

### Структура каталогов — добавить

```
/home/dev/projects/erp/
├── queries/                        ← SQL-запросы для Clorinde (DML)
│   ├── common/                     ← outbox, audit, sequences
│   └── warehouse/                  ← inventory queries
│
├── crates/
│   ├── clorinde-gen/               ← СГЕНЕРИРОВАННЫЙ crate (добавить в members)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs              ← (автогенерация Clorinde CLI)
│   ...
```

### workspace members — добавить

```toml
members = [
    ...
    "crates/clorinde-gen",
]
```

### justfile — добавить рецепт

```just
# Перегенерировать Clorinde crate из SQL-запросов
clorinde-generate:
    clorinde generate \
      --queries-path queries/ \
      --destination crates/clorinde-gen/
    @echo "Clorinde crate regenerated"
```

### Инструменты — добавить установку Clorinde CLI

```bash
cargo install clorinde
```

---

## Layer 1 (layer1_spec.md) — что изменить

### Kernel Cargo.toml — убрать sqlx

**Было:**
```toml
[dependencies]
uuid = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
sqlx = { workspace = true }        # для sqlx::Type
```

**Стало:**
```toml
[dependencies]
uuid = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
# НЕТ sqlx — kernel не зависит от БД
```

### types.rs — убрать sqlx::Type

**Было:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TenantId(Uuid);
```

**Стало:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(Uuid);
```

Маппинг ID ↔ PostgreSQL UUID происходит в infrastructure слое (clorinde-gen или repository), не в kernel.

### Промпт (prompt_layer1.md) — обновить

В шаге 1 (types.rs) убрать упоминание sqlx::Type.
В шаге 5 (lib.rs + Cargo.toml) убрать sqlx из зависимостей.

---

## Layer 3a (layer3a_spec.md) — без изменений

Event Bus не зависит от БД. Никаких sqlx/Clorinde.
