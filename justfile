# ERP Pilot — команды разработки
# Использование: just <recipe>

set dotenv-load := true
set dotenv-override := true

# === Сборка и тесты ===

build:
    cargo build --workspace

build-release:
    cargo build --workspace --release

test:
    cargo test --workspace -j 1 -- --test-threads=1

test-crate crate:
    cargo test -p {{crate}} -j 1 -- --test-threads=1

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
    @psql "$DATABASE_URL" -c "SELECT 1" > /dev/null && echo "OK: PostgreSQL доступен"

db-migrate:
    @cargo run -p gateway --quiet 2>&1 &  pid=$$!; sleep 3; kill $$pid 2>/dev/null; echo "Миграции применены (gateway startup)"

db-reset:
    @echo "Пересоздание БД..."
    @psql "$DATABASE_URL" -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid()" > /dev/null 2>&1 || true
    @dropdb -f --if-exists "$( echo $DATABASE_URL | sed 's|.*/||' )" 2>/dev/null; \
     createdb "$( echo $DATABASE_URL | sed 's|.*/||' )" 2>/dev/null; \
     echo "БД пересоздана. Запустите 'just run' для применения миграций."

clorinde-generate:
    clorinde generate \
      --queries-path queries/ \
      --destination crates/clorinde-gen/
    @echo "Clorinde crate regenerated"

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
    cargo install just
    cargo install cargo-watch
    @echo "Все инструменты установлены"

deps:
    cargo tree --workspace --depth 1

clean:
    cargo clean

postman-bc bc:
	tests/postman/run_newman.sh run tests/postman/{{bc}}.postman_collection.json \
	  -e tests/postman/erp-gateway.postman_environment.json

postman-smoke:
	tests/postman/run_newman.sh run tests/postman/smoke.postman_collection.json \
	  -e tests/postman/erp-gateway.postman_environment.json

postman-full: (postman-bc "catalog") (postman-bc "warehouse") postman-smoke
