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
