# Postman / Newman

## Файлы

| Файл | Назначение |
|------|------------|
| `erp-gateway.postman_environment.json` | Общее окружение: `base_url`, `tenant_id` |
| `smoke.postman_collection.json` | CI Smoke: health, cross-BC flow, viewer grants |
| `catalog.postman_collection.json` | Catalog BC: happy path, auth, validation, viewer |
| `warehouse.postman_collection.json` | Warehouse BC: happy path, auth, validation, viewer |

Каждая BC-коллекция автономна: свой auth setup, не зависит от других коллекций.

## Предусловия

1. Поднять gateway: `DEV_MODE=1 just run`
2. Убедиться, что `base_url` в environment указывает на нужный адрес.

## Запуск

```bash
just postman-smoke            # smoke (health + cross-BC + viewer)
just postman-bc catalog       # один BC
just postman-bc warehouse     # один BC
just postman-full             # всё: catalog → warehouse → smoke
```

## Что проверяет smoke

- `GET /health`
- catalog: create product + get product (catalog_manager)
- warehouse: receive goods + get balance (warehouse_operator)
- viewer: explicit query grants для обоих BC
