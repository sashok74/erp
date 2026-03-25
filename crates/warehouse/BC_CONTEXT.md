# Warehouse BC — Bounded Context Passport

> Паспорт контекста для AI-агента и разработчиков.

## Назначение

Складской учёт: приёмка товара, учёт остатков, движения.
Reference implementation для всех будущих BC.

## Aggregates

| Агрегат | Файл | Инвариант |
|---------|------|-----------|
| `InventoryItem` | `domain/aggregates.rs` | `balance >= 0` |

## Commands

| Команда | Permission key | Handler |
|---------|---------------|---------|
| `ReceiveGoodsCommand` | `warehouse.receive_goods` | `ReceiveGoodsHandler` |

## Queries

| Запрос | Handler |
|--------|---------|
| `GetBalanceQuery` | `GetBalanceHandler` |

## Domain Events

| Событие | Event type | Payload |
|---------|-----------|---------|
| `GoodsReceived` | `erp.warehouse.goods_received.v1` | `item_id, sku, quantity, new_balance, doc_number` |

## Value Objects

| VO | Файл | Правила |
|----|------|---------|
| `Sku` | `domain/value_objects.rs` | Непустой, ≤50 символов |
| `Quantity` | `domain/value_objects.rs` | `>= 0`, BigDecimal |

## Tables (schema: warehouse)

| Таблица | Назначение | RLS |
|---------|-----------|-----|
| `inventory_items` | Реестр товаров | ✓ |
| `stock_movements` | Append-only журнал движений | ✓ |
| `inventory_balances` | Текущие остатки (проекция) | ✓ |

## Sequence

| Seq name | Prefix | Формат |
|----------|--------|--------|
| `warehouse.receipt` | `ПРХ-` | `ПРХ-000001` |

## Roles

| Роль | Доступ |
|------|--------|
| `admin` | Все команды |
| `warehouse_manager` | Все warehouse.* |
| `warehouse_operator` | `warehouse.receive_goods` |
| `viewer` | Нет доступа к командам |

## Write Flow

```
POST /receive → pipeline.execute(ReceiveGoodsHandler)
  → auth: check warehouse.receive_goods
  → UoW: BEGIN + SET tenant_id
  → handler:
      find_by_sku / create_item
      seq_gen: ПРХ-NNNNNN
      item.receive(qty, doc_number)
      save_movement + upsert_balance
      domain_history: old/new state
      outbox: GoodsReceived event
  → UoW: COMMIT
  → audit: log command
  → relay: outbox → bus → subscribers
```
