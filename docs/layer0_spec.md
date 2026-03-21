# Layer 0 — Cargo Workspace + Инфраструктура проекта
> Подробное ТЗ | ERP Pilot on Rust
> Дата: 2026-03-21 | Привязка: ADR v1, erp_pilot_plan_rust.md
> Среда: Debian 12 (LXC на Proxmox), PostgreSQL в локальной сети

---

## Зачем этот слой

Layer 0 — фундамент, на котором стоит всё остальное. Без него невозможно начать писать код ни одного из последующих слоёв. Здесь мы решаем три вещи:

1. **Как организован код** — Cargo workspace с разделением на crate'ы по ответственности
2. **Как мы контролируем качество** — линтеры, проверки лицензий, pinned toolchain
3. **Как запускаем инфраструктуру** — подключение к PostgreSQL, автоматизация рутины

Результат Layer 0: пустой, но полностью рабочий проект. `cargo build --workspace` компилируется, `cargo test --workspace` проходит, `cargo clippy` не ругается, PostgreSQL доступен.

---

## Размещение задач на диске

```
/home/dev/projects/erp/                 ← корень проекта (единый workspace)
│
├── Cargo.toml                          ← [workspace] — определяет все crate'ы
├── Cargo.lock
├── .cargo/config.toml                  ← линкер, оптимизации
├── rust-toolchain.toml                 ← pinned toolchain
├── deny.toml                           ← cargo-deny: лицензии, advisories
├── justfile                            ← автоматизация команд
├── .env                                ← DATABASE_URL и другие переменные
├── .env.example                        ← шаблон .env без секретов
├── .gitignore
├── CLAUDE.md                           ← инструкции для Claude Code / Claude AI
├── README.md
│
├── crates/                             ← все crate'ы проекта
│   ├── kernel/                         ← Layer 1: базовые типы
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  ← пока пустой: pub mod — заготовки
│   │
│   ├── db/                             ← Layer 2: PostgreSQL access
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── event_bus/                      ← Layer 3: in-process event bus
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── auth/                           ← Layer 4: Auth/JWT
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── audit/                          ← Layer 4: Audit Trail
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── seq_gen/                        ← Layer 4: Sequence Generator
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── runtime/                        ← Layer 5: BC Runtime (Command Pipeline)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── extensions/                     ← Layer 8: Lua + WASM
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── warehouse/                      ← Layer 6: MVP Domain
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── gateway/                        ← Layer 7: API Gateway (axum)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                 ← единственный binary crate
│
├── migrations/                         ← SQL миграции
│   ├── common/                         ← общая инфраструктура (tenants, outbox, audit)
│   └── warehouse/                      ← схема warehouse BC
│
├── web/                                ← Layer 9: Thin UI
│   ├── static/
│   │   ├── css/
│   │   └── js/
│   └── templates/
│
├── tests/                              ← integration / e2e тесты
│   ├── integration/
│   └── e2e/
│
└── docs/                               ← документация проекта
    ├── layer0_spec.md                  ← этот документ
    ├── architecture.md                 ← (будущее) архитектурный обзор
    └── rust_patterns.md                ← (будущее) найденные паттерны
```

### Почему единый каталог, а не каталог-на-задачу

Весь ERP — **один Cargo workspace**. Задачи (0.1, 1.1, 2.1...) — это не отдельные проекты, а шаги наполнения одного workspace. Каждая задача добавляет файлы или код в существующую структуру. Так работает реальная разработка: один репозиторий, много crate'ов.

Документация по задачам живёт в `docs/`. Спецификация каждого Layer — отдельный файл.

---

## Задача 0.1 — Инициализация Cargo Workspace

### Зачем

Cargo workspace — стандартный способ организации Rust-монорепо. Все crate'ы:
- Компилируются одной командой (`cargo build --workspace`)
- Разделяют единый `Cargo.lock` (гарантия воспроизводимости)
- Используют общие версии зависимостей через `[workspace.dependencies]`

Для ERP это означает: каждый Bounded Context — отдельный crate с чётким `Cargo.toml`, но все вместе — один проект. Зависимости между crate'ами явные, циклические зависимости невозможны (Cargo это запрещает).

### Что нужно знать из Rust

- **Workspace** — корневой `Cargo.toml` с `[workspace]` секцией. Не содержит `[package]`. Перечисляет `members`.
- **Workspace dependencies** — секция `[workspace.dependencies]` позволяет указать версии один раз. Внутри crate'ов пишем `dependency.workspace = true`.
- **Library crate vs binary crate** — все crate'ы, кроме `gateway`, будут library (`src/lib.rs`). `gateway` — binary (`src/main.rs`).
- **Edition** — используем `edition = "2024"` (стабилизирован в Rust 1.85, февраль 2025). Это последняя редакция, даёт новый синтаксис `gen` блоков, улучшенный `use` в trait impls, и другие мелкие улучшения.

### Требования

**Файл: `Cargo.toml` (корень workspace)**

```toml
[workspace]
resolver = "2"
members = [
    "crates/kernel",
    "crates/db",
    "crates/event_bus",
    "crates/auth",
    "crates/audit",
    "crates/seq_gen",
    "crates/runtime",
    "crates/extensions",
    "crates/warehouse",
    "crates/gateway",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "Proprietary"

[workspace.dependencies]
# --- Async runtime ---
tokio = { version = "1", features = ["full"] }

# --- HTTP framework ---
axum = { version = "0.8", features = ["macros"] }
axum-extra = { version = "0.10", features = ["typed-header"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }

# --- Database ---
sqlx = { version = "0.8", features = [
    "runtime-tokio-rustls", "postgres", "uuid", "chrono",
    "json", "migrate", "bigdecimal"
] }

# --- Serialization ---
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# --- Auth ---
jsonwebtoken = "9"

# --- IDs ---
uuid = { version = "1", features = ["v7", "serde"] }

# --- Time ---
chrono = { version = "0.4", features = ["serde"] }

# --- Logging / Tracing ---
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# --- Extension runtime ---
mlua = { version = "0.10", features = ["luajit", "async", "send"] }
wasmtime = "28"

# --- Error handling ---
thiserror = "2"
anyhow = "1"

# --- Validation ---
validator = { version = "0.19", features = ["derive"] }

# --- Decimal ---
bigdecimal = { version = "0.4", features = ["serde"] }

# --- Templates (thin UI) ---
askama = "0.12"

# --- Async trait ---
async-trait = "0.1"

# --- Testing ---
tokio-test = "0.4"
testcontainers = "0.23"

# --- Internal crates ---
kernel = { path = "crates/kernel" }
db = { path = "crates/db" }
event_bus = { path = "crates/event_bus" }
auth = { path = "crates/auth" }
audit = { path = "crates/audit" }
seq_gen = { path = "crates/seq_gen" }
runtime = { path = "crates/runtime" }
extensions = { path = "crates/extensions" }
warehouse = { path = "crates/warehouse" }
```

**Файл: `crates/kernel/Cargo.toml` (пример library crate)**

```toml
[package]
name = "kernel"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
uuid = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
bigdecimal = { workspace = true }
```

**Файл: `crates/kernel/src/lib.rs`**

```rust
//! ERP Kernel — базовые типы, трейты и value objects.
//! Нулевые зависимости от инфраструктуры.
//! Это Shared Kernel из Context Map.

// Модули будут добавлены в Layer 1:
// pub mod types;
// pub mod commands;
// pub mod events;
// pub mod entity;
// pub mod value_objects;
// pub mod errors;
```

**Файл: `crates/gateway/Cargo.toml` (binary crate)**

```toml
[package]
name = "gateway"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[[bin]]
name = "erp-gateway"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
axum = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

**Файл: `crates/gateway/src/main.rs`**

```rust
fn main() {
    println!("ERP Gateway — placeholder");
}
```

Остальные 8 library crate'ов (`db`, `event_bus`, `auth`, `audit`, `seq_gen`, `runtime`, `extensions`, `warehouse`) — по аналогии с `kernel`. Минимальный `Cargo.toml` + пустой `lib.rs`. Зависимости на другие внутренние crate'ы пока не прописываем — это произойдёт в соответствующих Layer'ах.

### Критерий готовности

```bash
cargo build --workspace          # компилируется без ошибок
cargo test --workspace           # 0 тестов, 0 ошибок
cargo run -p gateway             # выводит "ERP Gateway — placeholder"
```

---

## Задача 0.2 — Toolchain, линтеры, cargo-deny

### Зачем

В Rust-сообществе качество кода контролируется на уровне инструментов ещё до code review:

- **rust-toolchain.toml** — фиксирует версию компилятора. Все участники проекта и CI используют одну и ту же версию. Без этого «у меня работает, у тебя нет».
- **cargo clippy** — статический анализатор. Ловит баги, антипаттерны, неидиоматический код. В серьёзных проектах запускается с `-D warnings` (warnings = ошибки компиляции).
- **cargo fmt** — единый стиль форматирования. Без споров про табы vs пробелы.
- **cargo-deny** — проверяет дерево зависимостей: лицензии (нет GPL в проприетарном коде?), известные уязвимости (RustSec advisory DB), дубликаты, запрещённые crate'ы.

Для ERP это критично: проприетарная лицензия означает, что мы **не можем** случайно затянуть GPL-зависимость. cargo-deny это ловит автоматически.

### Что нужно знать из Rust

- **Toolchain** — `stable`, `beta`, `nightly`. Мы используем `stable`. Файл `rust-toolchain.toml` автоматически скачивает нужную версию при первом `cargo build`.
- **Clippy lints** — сотни правил, сгруппированных по категориям: `correctness`, `suspicious`, `style`, `complexity`, `perf`, `pedantic`. Мы включаем `pedantic` на уровне warn, а `correctness` и `suspicious` — на deny.
- **rustfmt** — настраивается через `rustfmt.toml`. Стандартные настройки + несколько важных для читаемости.

### Требования

**Файл: `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

> Примечание: не привязываемся к конкретной версии (типа "1.85.0"), пока не столкнёмся с breaking change. `stable` достаточно для начала. При необходимости воспроизводимости — заменить на конкретную версию.

**Файл: `rustfmt.toml`**

```toml
edition = "2024"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
```

**Файл: `.cargo/config.toml`**

```toml
# Ускорение линковки на Linux (mold — самый быстрый линкер)
# Установка: sudo apt install mold
# Раскомментировать после установки:
# [target.x86_64-unknown-linux-gnu]
# linker = "clang"
# rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[build]
incremental = true

[term]
color = "always"
```

**Файл: `deny.toml`**

```toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
yanked = "warn"

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Zlib",
    "BSL-1.0",
    "OpenSSL",
    "CC0-1.0",
]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

**Clippy: в каждом `lib.rs` / `main.rs`:**

```rust
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
```

### Инструкция по установке

```bash
cargo install cargo-deny
cargo deny check

# Опционально: mold (быстрый линкер, ощутимо ускоряет incremental builds)
sudo apt install mold
```

### Критерий готовности

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo deny check
```

---

## Задача 0.3 — Подключение к PostgreSQL + justfile

### Зачем

PostgreSQL — единственная СУБД в проекте (решение из ADR v1). Нужно:

1. Подключиться к существующему серверу в домашней сети
2. Создать базу данных для проекта
3. Автоматизировать повседневные команды через justfile

**justfile** (утилита `just`) — современная замена Makefile в Rust-сообществе. Синтаксис проще, нет проблем с табами. Используется в большинстве серьёзных Rust-проектов (ripgrep, nushell, Zed, и т.д.).

### Что нужно знать

- **just** — устанавливается через `cargo install just`. Файл `justfile` описывает рецепты (recipes), аналогично Make-таргетам.
- **sqlx** — async Rust драйвер для PostgreSQL. Поддерживает compile-time проверку SQL, миграции, connection pooling.
- **dotenv / .env** — файл с переменными окружения. `sqlx` автоматически читает `DATABASE_URL` из `.env`.

### Подготовка PostgreSQL

> Предполагается: в домашней сети есть сервер с PostgreSQL 16+.
> IP-адрес сервера, порт, логин/пароль — подставить свои.

На сервере PostgreSQL нужно выполнить один раз:

```sql
-- 1. Создать роль для ERP
CREATE ROLE erp_admin WITH LOGIN PASSWORD 'your_secure_password_here';

-- 2. Создать базу данных
CREATE DATABASE erp_dev OWNER erp_admin;

-- 3. Подключиться к erp_dev и создать расширения
\c erp_dev
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- 4. Разрешить erp_admin создавать схемы
GRANT CREATE ON DATABASE erp_dev TO erp_admin;
```

> Убедиться, что `pg_hba.conf` разрешает подключение с LXC-контейнера по IP.
> Строка типа: `host erp_dev erp_admin 192.168.x.0/24 scram-sha-256`

### Требования

**Файл: `.env.example`**

```bash
# PostgreSQL — подставить IP сервера в домашней сети
DATABASE_URL=postgres://erp_admin:your_password@192.168.1.100:5432/erp_dev

# Режим логирования
RUST_LOG=info,sqlx=warn
```

**Файл: `.env`** (копия `.env.example` с реальными значениями, в `.gitignore`)

**Файл: `.gitignore`**

```gitignore
# Rust
/target
**/*.rs.bk

# Environment
.env
!.env.example

# IDE
.idea/
.vscode/settings.json
*.swp
*~

# OS
.DS_Store
```

**Файл: `justfile`**

```just
# ERP Pilot — команды разработки
# Использование: just <recipe>

set dotenv-load := true

# === Сборка и тесты ===

build:
    cargo build --workspace

build-release:
    cargo build --workspace --release

test:
    cargo test --workspace

test-crate crate:
    cargo test -p {{crate}}

# === Качество кода ===

fmt-check:
    cargo fmt --all -- --check

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace -- -D warnings

deny:
    cargo deny check

check: fmt-check lint deny test

# === База данных ===

db-ping:
    @echo "Проверка подключения к PostgreSQL..."
    sqlx database create || echo "БД уже существует"
    @echo "OK: PostgreSQL доступен"

db-migrate:
    sqlx migrate run --source migrations/common
    @echo "Common migrations: OK"

db-revert:
    sqlx migrate revert --source migrations/common

db-reset:
    sqlx database drop -y
    sqlx database create
    just db-migrate

# === Запуск ===

run:
    cargo run -p gateway

run-debug:
    RUST_LOG=debug cargo run -p gateway

watch:
    cargo watch -x 'run -p gateway'

# === Утилиты ===

setup:
    cargo install cargo-deny
    cargo install sqlx-cli --no-default-features --features postgres
    cargo install just
    cargo install cargo-watch
    @echo "Все инструменты установлены"

deps:
    cargo tree --workspace --depth 1

clean:
    cargo clean
```

### Проверка подключения: простой Rust-тест

```rust
// tests/integration/db_connection.rs
// Запуск: cargo test --test db_connection

#[tokio::test]
async fn test_postgres_connection() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("Failed to connect to PostgreSQL");
    let row: (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to execute query");
    assert_eq!(row.0, 1);
}
```

### Критерий готовности

```bash
just build                      # workspace компилируется
just check                      # fmt + lint + deny + test — всё зелёное
just db-ping                    # PostgreSQL отвечает
just run                        # "ERP Gateway — placeholder"
```

---

## Сводка: что получаем после Layer 0

| Артефакт | Статус | Для чего |
|----------|--------|----------|
| Cargo workspace с 10 crate'ами | Компилируется | Основа всех последующих слоёв |
| rust-toolchain.toml | Pinned stable | Воспроизводимость сборки |
| cargo fmt + clippy + deny | Проходят | Quality gate |
| justfile с 15+ рецептами | Работает | Автоматизация рутины |
| .env + подключение к PostgreSQL | Установлено | Готовность к Layer 2 |
| CLAUDE.md | В корне проекта | Контекст для AI-ассистента |
| .gitignore + .env.example | В репозитории | Безопасность секретов |

### Чему научились (Rust)

- **Cargo workspace** — организация монорепо, workspace dependencies, library vs binary crate
- **Toolchain management** — rust-toolchain.toml, edition 2024, pinned versions
- **Ecosystem tools** — clippy (linting), rustfmt (formatting), cargo-deny (supply chain security)
- **just** — task runner, dotenv integration, рецепты с параметрами
- **sqlx-cli** — управление PostgreSQL из командной строки

### Связь с архитектурой ERP

| Архитектурное решение (ADR v1) | Где заложено |
|-------------------------------|-------------|
| Modular monolith → split later | Один workspace, crate per BC |
| PostgreSQL shared DB | .env + justfile рецепты |
| Proprietary license | cargo-deny: allow-list без GPL |
| MVP: Warehouse | crate `warehouse` уже в workspace |

---

## Следующий шаг

После завершения Layer 0 → переход к **Layer 1 (Kernel)**: базовые типы `TenantId`, `UserId`, трейты `Command`, `DomainEvent`, `AggregateRoot`, value objects `SKU`, `Quantity`, `Money`. Чистый Rust без инфраструктурных зависимостей.
