# План приемочного тестирования авторизации и раздачи прав

> Статус: Working draft
> Назначение: единый документ приемки целевой архитектуры авторизации
> Связанные документы: `docs/security/BC_OWNED_RBAC.md`, `docs/auth_overview.md`

---

## 1. Цель

Этот документ определяет, по каким сценариям и критериям принимается итоговая архитектура авторизации.

Архитектура считается готовой только если она покрывает полный цикл:

1. BC регистрирует свои роли и действия.
2. Платформа собирает manifests и валидирует их.
3. Пользователю назначаются роли.
4. Роли попадают в runtime-проверку.
5. Команды и запросы разрешаются или запрещаются предсказуемо.
6. Поведение подтверждено unit, integration и end-to-end тестами.

---

## 2. Что должна покрывать итоговая архитектура

Итоговая архитектура должна обеспечивать:

- BC-owned declaration ролей, permissions и grants.
- Platform-enforced проверку прав без knowledge leakage в `auth`.
- Поддержку platform roles (`admin`, `viewer`) без захвата их BC.
- Проверку доступа и для `commands`, и для `queries`.
- Поддержку нескольких ролей у одного пользователя.
- Предсказуемую обработку неизвестных ролей и невалидных manifests.
- Возможность эволюции к DB-backed role assignments без слома runtime semantics.
- Возможность дальнейшего роста в scoped roles и ABAC/SD.

---

## 3. Объект тестирования

Проверяется не только `auth crate`, а вся цепочка:

- `kernel`: contracts и типы registration API.
- BC crates: `PermissionRegistrar` и собственные manifests.
- `auth`: registry, checker, JWT claims handling.
- `gateway`: startup orchestration и fail-fast validation.
- `runtime`: `CommandPipeline` и `QueryPipeline`.
- HTTP слой: реальные protected endpoints.
- Phase B: persistence ролей и назначений в `iam.*`.

---

## 4. Уровни проверки

### 4.1 Unit tests

Проверяют локальную корректность компонентов:

- `PermissionRegistry::from_manifests()`
- `PermissionRegistry::validate()`
- wildcard matching
- platform roles semantics
- JWT claims round-trip
- `JwtPermissionChecker`
- обработку unknown role / invalid role / duplicate manifest entries

### 4.2 Integration tests

Проверяют интеграцию crate'ов внутри workspace:

- `gateway` собирает manifests от нескольких BC
- `CommandPipeline` использует registry
- `QueryPipeline` использует registry
- доменные команды/запросы реально разрешаются и запрещаются
- startup падает на некорректных manifests

### 4.3 End-to-end tests

Проверяют поведение глазами реального пользователя:

- issuance token
- HTTP request с Bearer token
- доступ к `/api/{bc}/...`
- корректные `200/201/401/403`
- отсутствие side effects при отказе
- аудит и трассировка успешных действий

---

## 5. Матрица приемки

### A. Регистрация BC

#### A1. Новый BC подключается без изменений в `auth`

Предусловия:

- в workspace добавлен новый BC, например `procurement`
- BC реализует `PermissionRegistrar`

Проверка:

1. Добавить BC в composition root.
2. Собрать manifests.
3. Убедиться, что `auth` не менялся для поддержки этого BC.
4. Выполнить проверку permission из нового BC.

Ожидаемый результат:

- новый BC работает только за счет своего manifest
- `auth` не содержит hardcoded knowledge о ролях нового BC

#### A2. Gateway валидирует manifests при старте

> **Метод:** `PermissionRegistry::from_manifests_validated()` выполняет все проверки.

Проверка:

1. Дать BC manifest с grant на несуществующую роль.
2. Дать BC manifest с grant на несуществующее action (non-wildcard).
3. Дать BC manifest с duplicate role code.
4. Дать BC manifest с duplicate permission code.
5. Дать BC manifest, конфликтующий с platform role (`admin`, `viewer`).
6. Дать BC manifest с невалидным wildcard (например `warehouse*` вместо `warehouse.*`).

Ожидаемый результат:

- startup падает fail-fast
- ошибка понятная и указывает на конкретный BC и конкретный конфликт

#### A3. BC не может публиковать чужие permissions

Проверка:

1. `catalog` пытается зарегистрировать `warehouse.receive_goods`.

Ожидаемый результат:

- validation error
- `bc_code` и permission prefix обязаны совпадать

### B. Работа с ролями

#### B1. Platform role `admin`

Проверка:

1. Выпустить токен с ролью `admin`.
2. Выполнить команду и запрос из нескольких BC.

Ожидаемый результат:

- разрешены все commands
- разрешены все queries

#### B2. Platform role `viewer`

> **Важно:** `viewer` — платформенная роль, но query-grants для неё регистрируются
> каждым BC в своём manifest (cross-grant rule, см. BC_OWNED_RBAC.md).

Проверка:

1. Выпустить токен с ролью `viewer`.
2. Выполнить read-only query в `catalog` (например `catalog.get_product`).
3. Выполнить read-only query в `warehouse` (например `warehouse.get_balance`).
4. Выполнить write command в `catalog`.
5. Выполнить write command в `warehouse`.

Ожидаемый результат:

- queries разрешены (BC манифесты содержат grants для `viewer` на query-actions)
- commands запрещены

#### B3. BC-owned role

Проверка:

1. Выпустить токен с ролью `warehouse_operator`.
2. Выполнить command `warehouse.receive_goods`.
3. Выполнить query `warehouse.get_balance`.
4. Выполнить command `catalog.create_product`.
5. Выполнить query `catalog.get_product`.

Ожидаемый результат:

- warehouse command и query разрешены (согласно grants в manifest)
- catalog command и query запрещены

#### B4. Несколько ролей у пользователя

Проверка:

1. Выпустить токен с ролями `warehouse_operator` + `catalog_manager`.
2. Выполнить команды и queries обоих BC.

Ожидаемый результат:

- права работают как union

#### B5. Unknown role в token

Проверка:

1. Выпустить токен с несуществующей ролью.
2. Выполнить запрос к защищенному endpoint.

Ожидаемый результат:

- поведение формализовано и стабильно
- рекомендуемое правило: deny
- система не должна silently elevate access

### C. Commands и queries

#### C1. Command authorization

Проверка:

1. Выполнить разрешенную команду.
2. Выполнить запрещенную команду.

Ожидаемый результат:

- разрешенная команда проходит
- запрещенная возвращает auth error
- side effects для запрещенной команды отсутствуют

#### C2. Query authorization

Проверка:

1. Выполнить разрешенный query через `QueryPipeline`.
2. Выполнить запрещенный query через `QueryPipeline`.
3. Выполнить тот же query по HTTP.

Ожидаемый результат:

- query-права проверяются так же строго, как command-права
- тесты не обходят pipeline прямым вызовом handler

#### C3. Wildcard grants

Проверка:

1. Роль имеет `warehouse.*`.
2. Выполнить несколько `warehouse.*` действий.
3. Выполнить `catalog.*`.

Ожидаемый результат:

- wildcard работает только внутри своего BC namespace

### D. JWT и runtime flow

#### D1. Claims format

Проверка:

1. Выпустить JWT.
2. Проверить payload.
3. Преобразовать claims в `RequestContext`.

Ожидаемый результат:

- роли представлены строками
- никаких enum conversion steps не требуется

#### D2. Invalid token

Проверка:

1. Отправить мусорный token.
2. Отправить token с неверной подписью.
3. Отправить expired token.

Ожидаемый результат:

- `401 Unauthorized`
- handler не вызывается

#### D3. Missing auth header

Ожидаемый результат:

- `401 Unauthorized`

### E. Поведение как у реального пользователя

#### E1. Новый пользователь без ролей

Сценарий:

1. Пользователь получает token без ролей.
2. Пытается открыть read endpoint.
3. Пытается выполнить write command.

Ожидаемый результат:

- доступ запрещен либо разрешен только если это явно описано platform policy
- никаких side effects

#### E2. Оператор склада

Сценарий:

1. Пользователь получает роль `warehouse_operator`.
2. Делает `POST /api/warehouse/receive`.
3. Делает `GET /api/warehouse/balance`.
4. Пытается создать товар в catalog.

Ожидаемый результат:

- warehouse write/read разрешены
- catalog write запрещен

#### E3. Менеджер каталога

Сценарий:

1. Пользователь получает роль `catalog_manager`.
2. Создает товар.
3. Читает товар.
4. Пытается выполнить складскую команду.

Ожидаемый результат:

- catalog operations работают
- warehouse command запрещен

#### E4. Администратор tenant'а

Сценарий:

1. Пользователь с `admin` вызывает endpoints нескольких BC.
2. Создает данные.
3. Читает данные.

Ожидаемый результат:

- полный доступ в рамках tenant

#### E5. Отказ не порождает следов бизнес-операции

Проверка:

1. Выполнить запрещенную команду.
2. Проверить БД.
3. Проверить outbox.
4. Проверить audit.

Ожидаемый результат:

- business state не изменился
- outbox не пополнен
- audit не фиксирует успешное выполнение команды

---

## 6. Сценарии Phase B: раздача прав через БД

Эти сценарии обязательны, если архитектура заявляет поддержку полноценной раздачи прав.

### F1. Role definitions persisted from manifests

Проверка:

1. Запустить систему.
2. Синхронизировать manifests в `iam.role_definitions` и `iam.permission_definitions`.

Ожидаемый результат:

- в БД отражены все platform и BC-owned роли/permissions

### F2. User role assignment

Проверка:

1. Назначить пользователю роль в `iam.user_role_assignments`.
2. Выпустить token на основе assignments.
3. Выполнить разрешенное действие.

Ожидаемый результат:

- пользователь получает доступ согласно assignment

### F3. Role revocation

Проверка:

1. Отозвать роль.
2. Повторно выпустить token.
3. Повторить запрос.

Ожидаемый результат:

- доступ отозван

### F4. Tenant isolation

Проверка:

1. Один и тот же `user_id` имеет разные роли в двух tenant.
2. Выпустить token для tenant A и tenant B.

Ожидаемый результат:

- права различаются по tenant

### F5. Scoped role assignment

> **Prerequisite:** `scoped_roles` в JWT (см. BC_OWNED_RBAC.md, Phase B: "Scoped roles в JWT")
> и/или Cedar ABAC (Phase C) для runtime scope enforcement.
> Этот тест неприменим к Phase A.

Проверка:

1. Назначить роль только на контейнер или конкретный scope.
2. Выпустить token с `scoped_roles` содержащим scope assignment.
3. Выполнить действие в допустимом scope.
4. Выполнить то же действие вне scope.

Ожидаемый результат:

- внутри scope доступ есть
- вне scope доступа нет
- global role assignment (без scope) сохраняет доступ ко всем контейнерам

---

## 7. Набор обязательных негативных тестов

Должны существовать тесты на:

- unknown role in grants
- unknown action in grants (non-wildcard)
- duplicate role code across BC
- duplicate permission code across BC
- invalid wildcard pattern (`warehouse*` instead of `warehouse.*`)
- чужой namespace в manifest (`catalog` регистрирует `warehouse.receive_goods`)
- platform role collision (BC defines role named `admin`)
- missing Authorization header
- invalid token signature
- expired token
- unknown role in JWT → deny
- command denied
- query denied
- role revoked (Phase B)
- scope mismatch (Phase B+)

---

## 8. Критерии готовности архитектуры

Архитектура может считаться принятой только если выполнены все пункты:

1. Новый BC подключается без изменения `auth` логики.
2. Все permissions и roles определяются manifests BC или platform policy.
3. Commands и queries проверяются единообразно через pipeline.
4. Неизвестные роли и невалидные manifests приводят к fail-fast или deny.
5. Права нескольких ролей объединяются корректно.
6. `admin` и `viewer` имеют строго определенную platform semantics; viewer grants регистрируются BC.
7. Отказ в авторизации не создает бизнес side effects.
8. End-to-end поток через HTTP подтвержден для основных ролей.
9. Phase B подтверждает назначение, отзыв и tenant isolation ролей.
10. Phase B scoped roles требуют дизайна scope-in-JWT и/или Cedar enforcement (F5 не обязателен для Phase A acceptance).
11. Тесты написаны через реальные pipelines, а не обходят auth прямым вызовом handler.

---

## 9. Рекомендуемый минимальный набор автоматических тестов

Минимум для приемки:

- 8-12 unit tests для registry/checker/JWT
- 6-8 integration tests для startup + command/query auth
- 4-6 HTTP e2e tests на типовые роли
- 3-5 DB-backed tests для Phase B

Ключевые e2e кейсы:

- `viewer` read allowed / write denied
- `warehouse_operator` warehouse allowed / catalog denied
- `catalog_manager` catalog allowed / warehouse denied
- `admin` cross-BC full access
- unknown role denied

---

## 10. Практический порядок выполнения приемки

### Этап 1. Contract acceptance

- проверить `PermissionRegistrar` API
- проверить validation rules
- проверить platform roles contract

### Этап 2. Runtime acceptance

- проверить `PermissionRegistry`
- проверить `JwtPermissionChecker`
- проверить `CommandPipeline`
- проверить `QueryPipeline`

### Этап 3. HTTP acceptance

- проверить middleware
- проверить token verification
- проверить protected endpoints

### Этап 4. IAM acceptance

- проверить persistence manifests
- проверить assignment/revocation
- проверить tenant isolation
- проверить scoped access

---

## 11. Решение о приемке

Решение принимается по трем состояниям:

- `Accepted`
  все обязательные сценарии закрыты, архитектура готова к эксплуатации

- `Accepted with gaps`
  Phase A готова, но есть зафиксированные пробелы в Phase B или scoped access

- `Rejected`
  хотя бы один из базовых инвариантов нарушен:
  нет query authorization, нет fail-fast validation, новый BC требует менять `auth`, или раздача ролей не обеспечивает предсказуемый runtime
