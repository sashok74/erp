# API Testing Rules

Правила API-проверок для Bounded Contexts, публикующих HTTP API через gateway.

## Принцип

BC не считается подключённым, пока:

- manifest авторизации зарегистрирован;
- crate integration tests покрывают command/query авторизацию через pipeline;
- BC имеет свою Postman-коллекцию;
- `postman-full` проходит.

## Integration tests

Следуют [`docs/testing_integration_style.md`](testing_integration_style.md).

Дополнительно:

- command auth — через `CommandPipeline`, query auth — через `QueryPipeline`;
- прямой вызов handler для проверки access control запрещён;
- каждый `command_name()` / `query_name()` покрыт тестом или входит в e2e сценарий.

Минимальный набор сценариев:

- own role → allowed (command + query);
- foreign BC role → denied;
- viewer → allowed на явно выданных grants;
- viewer → denied на write;
- unknown role → denied;
- tenant isolation (если tenant-scoped данные).

## Postman/Newman

### Структура коллекций

Каждый BC — **отдельный файл** коллекции. Environment — общий:

```
tests/postman/
  erp-gateway.postman_environment.json   ← общий: base_url, tenant_id
  catalog.postman_collection.json
  warehouse.postman_collection.json
  smoke.postman_collection.json          ← CI Smoke (cross-BC, health)
```

Запуск:

```bash
just postman-bc catalog        # один BC
just postman-full              # все коллекции
just postman-smoke             # только smoke
```

### Переменные

Общие переменные (`base_url`, `tenant_id`) хранятся **только** в environment файле. Коллекции не дублируют их в своих `variable` — это единая точка конфигурации.

Run-scoped identifiers (`ci_sku`, `catalog_sku_1`, `wh_sku`) создаются в pre-request/test скриптах через `pm.environment.set(...)`.

### Коллекция BC должна быть автономной

- свой `Auth Setup` для нужной роли;
- не зависит от токенов или данных других коллекций;
- не требует ручной подготовки состояния.

### Идемпотентность

Create/write сценарии используют run-scoped identifiers (`Date.now()`, uuid suffix и т.д.). Повторный прогон проходит без ручной очистки.

### Assertions обязательны

Каждый запрос проверяет:

- HTTP status;
- ключевые поля ответа;
- для ошибок: стабильный error payload.

### Минимальный набор запросов BC-коллекции

- `Auth Setup (<bc role>)`;
- happy-path command;
- happy-path query;
- unauthorized without token → 401;
- denied для чужой роли → 401;
- validation error → 400;
- not found / empty result.

Если BC поддерживает viewer:

- `Auth Setup (viewer)`;
- viewer query → allowed;
- viewer write → denied.

## CI Smoke

Отдельная коллекция. Содержит только:

- `GET /health`;
- критичный auth path;
- 1–2 сквозных cross-BC сценария;
- минимум один query с explicit RBAC grants.

Validation и negative cases — только в BC-коллекциях.

## Cross-BC сценарии

Если поведение пользователя зависит от взаимодействия BC, это должно быть видно в API regression suite (smoke или BC-коллекция).

## Definition of Done для нового BC

- manifest roles/permissions добавлен, startup validation проходит;
- crate integration tests покрывают auth сценарии из списка выше;
- Postman-коллекция BC с happy/deny/validation paths;
- cross-BC сценарий, если есть в реальном пользовательском потоке;
- `postman-full` проходит повторно.
