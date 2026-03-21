# ERP нового поколения

Производственная ERP-система для российского рынка (дискретное производство, СМБ).
Modular monolith на Rust, PostgreSQL, Event Sourcing для ключевых модулей.

## Быстрый старт

```bash
# 1. Установить инструменты
just setup

# 2. Скопировать и настроить переменные окружения
cp .env.example .env
# Отредактировать .env — указать реальный адрес PostgreSQL

# 3. Собрать проект
just build

# 4. Запустить проверки
just check

# 5. Запустить gateway
just run
```

## Структура проекта

```
crates/
  kernel/       — базовые типы, трейты, value objects
  db/           — PostgreSQL access layer
  event_bus/    — in-process event bus
  auth/         — JWT, RBAC, middleware
  audit/        — structured audit log
  seq_gen/      — gap-free sequence generator
  runtime/      — command pipeline, BC runtime
  extensions/   — Lua + WASM sandbox
  warehouse/    — MVP domain: складской учёт
  gateway/      — HTTP server (axum)
```

## Команды

```bash
just build          # собрать workspace
just test           # все тесты
just lint           # clippy
just fmt            # форматирование
just check          # полная проверка (fmt + lint + deny + test)
just run            # запустить gateway
just db-ping        # проверить PostgreSQL
just db-migrate     # применить миграции
```
