# Integration Test Style Guide

Этот документ задаёт единый стиль integration tests для Bounded Context crates в этом репозитории.

## Цель

Не копировать в каждом BC один и тот же test setup:

- создание `PgPool`
- применение миграций
- создание `RequestContext`
- сборка `CommandPipeline`
- cleanup данных по `tenant_id`

Общий код должен жить в [`crates/test_support/src/lib.rs`](/home/raa/RustProjects/erp/crates/test_support/src/lib.rs).

## Где размещать тесты BC

Для каждого BC integration tests хранятся рядом с crate:

- `crates/<bc>/tests/integration.rs` для небольшого набора тестов
- если тестов становится много, раскладывать в:
  - `crates/<bc>/tests/common/mod.rs`
  - `crates/<bc>/tests/commands/*.rs`
  - `crates/<bc>/tests/queries/*.rs`
  - `crates/<bc>/tests/e2e/*.rs`

Правило: новый BC не должен изобретать свой setup, если его можно собрать из `test_support`.

## Обязательный подход

Использовать helpers из `test_support`:

- `shared_pool(&[...migrations])`
- `request_context(&[...roles])`
- `command_pipeline(pool, bus)`
- `cleanup_tenant(pool, tenant_id, tables)`

В самом тестовом файле BC должны оставаться только:

- BC-специфичный список таблиц для cleanup
- маленькие role-specific helpers вроде `operator_ctx()`
- сами сценарии тестов

## Рекомендуемый шаблон

```rust
use std::sync::Arc;

use event_bus::InProcessBus;
use kernel::types::{RequestContext, TenantId};
use test_support::{
    cleanup_tenant as cleanup_tenant_rows, command_pipeline, request_context, shared_pool,
};

const BC_TABLES: &[&str] = &[
    "warehouse.inventory_balances",
    "warehouse.stock_movements",
    "common.outbox",
    "common.audit_log",
    "common.domain_history",
];

fn operator_ctx() -> RequestContext {
    request_context(&["warehouse_operator"])
}

async fn setup_pool() -> Arc<db::PgPool> {
    shared_pool(&["../../migrations/common", "../../migrations/warehouse"]).await
}

async fn cleanup_tenant(pool: &db::PgPool, tenant_id: TenantId) {
    cleanup_tenant_rows(pool, tenant_id, BC_TABLES).await;
}

#[tokio::test]
async fn happy_path_receive_goods() {
    let pool = setup_pool().await;
    let pipeline = command_pipeline(pool.clone(), Arc::new(InProcessBus::new()));

    // test scenario

    cleanup_tenant(&pool, operator_ctx().tenant_id).await;
}
```

## Что не делать

- не дублировать `static POOL` или `OnceCell` в каждом BC
- не дублировать один и тот же `database_url()` helper по файлам
- не собирать `CommandPipeline` вручную без причины
- не делать cleanup через копипаст SQL в каждом тестовом файле
- не смешивать несколько стилей test harness в одном crate

## Когда можно отступить

Отступление допустимо только если тесту нужен нестандартный wiring, например:

- подписка кастомных event handlers на bus
- дополнительный набор миграций другого BC для cross-context scenario
- особый cleanup, который нельзя выразить списком таблиц

Даже в этом случае нужно переиспользовать `shared_pool`, `request_context` и `cleanup_tenant` настолько, насколько это возможно.

## Текущее состояние

Сейчас этому шаблону уже следуют:

- [`crates/catalog/tests/integration.rs`](/home/raa/RustProjects/erp/crates/catalog/tests/integration.rs)
- [`crates/warehouse/tests/integration.rs`](/home/raa/RustProjects/erp/crates/warehouse/tests/integration.rs)
