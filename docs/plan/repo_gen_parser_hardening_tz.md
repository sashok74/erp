# ТЗ: доработка parser-части `repo-gen` через тестирование

> Статус: implementation-ready draft
> Дата: 2026-04-01
> Контекст: `repo-gen v1`

---

## 1. Проблема

Текущий parser в [`crates/repo_gen/src/sql_parser.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/sql_parser.rs#L1) делает две разные работы:

1. полезную и обязательную:
   - ищет `--! query_name`
   - читает `--@ metadata`
   - связывает query block с SQL-файлом

2. лишне хрупкую:
   - сам извлекает SQL parameters
   - сам извлекает `SELECT` columns
   - пытается частично понимать SQL без полноценного SQL parser

Это создаёт риск ложных ошибок и лишней сложности:

- block comments `/* ... */`
- более сложные `SELECT`
- подзапросы и CTE
- SQL, подготовленный LLM

При этом точные параметры и row fields уже можно получить из `clorinde-gen`, то есть дублировать эту работу в SQL parser не требуется.

---

## 2. Цель

Сделать parser-часть `repo-gen`:

- проще
- надёжнее
- лучше приспособленной к LLM-authored SQL
- тестируемой на уровне контрактов

Итоговая цель:

- SQL parser перестаёт быть "самодельным парсером SQL"
- он становится parser-ом query blocks и metadata
- структурные данные о запросе берутся из `clorinde-gen`

---

## 3. Основное решение

### 3.1 Что должен делать SQL parser

SQL parser должен отвечать только за:

1. поиск query blocks по `--!`
2. извлечение metadata `--@`
3. сохранение raw SQL body
4. сохранение file context:
   - `source_file`
   - `file_stem`
   - `query_name`

### 3.2 Что SQL parser делать не должен

SQL parser не должен:

- разбирать `SELECT` columns
- разбирать порядок bind-параметров
- пытаться интерпретировать casts, aliases, subqueries
- быть source of truth для query shape

### 3.3 Откуда брать структурные metadata о запросе

Источник query shape:

- [`crates/repo_gen/src/clorinde_parser.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/clorinde_parser.rs#L1)

Из `clorinde-gen` должны извлекаться:

- полный список bind-параметров
- порядок bind-параметров
- наличие и позиция `tenant_id`
- row fields для read-query
- Rust-типы параметров и row fields

### 3.4 Архитектурное следствие

Validator должен стать двухфазным:

1. `syntax validation` на уровне SQL block + metadata
2. `semantic validation` после объединения SQL metadata с `clorinde` metadata

---

## 4. Scope доработки

### Входит

- рефакторинг [`sql_parser.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/sql_parser.rs#L1)
- доработка [`clorinde_parser.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/clorinde_parser.rs#L1), чтобы он отдавал полную query shape model
- перенос части валидации из [`validator.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/validator.rs#L1) на этап после `clorinde` introspection
- новая тестовая пирамида для parser layer

### Не входит

- замена `clorinde`
- полноценный SQL AST parser
- metadata в отдельной локальной DB
- изменение пользовательского DX `repo-gen v1`

---

## 5. Целевая архитектура parser layer

### Слой A. Block parser

Файл: `sql_parser.rs`

Выход:

```rust
pub struct QueryBlock {
    pub name: String,
    pub metadata: Metadata,
    pub sql_body: String,
    pub source_file: String,
    pub file_stem: String,
}
```

Обязанности:

- делить файл на query blocks
- парсить `--@ key: value`
- не анализировать внутреннюю структуру SQL

### Слой B. Clorinde shape parser

Файл: `clorinde_parser.rs`

Выход:

```rust
pub struct ClorindeQueryInfo {
    pub all_bind_params: Vec<ClorindeParam>,
    pub bind_params_without_tenant: Vec<ClorindeParam>,
    pub row_fields: Vec<ClorindeField>,
}
```

Обязанности:

- извлекать полный bind signature из generated `Stmt::bind()`
- сохранять `tenant_id`, а не выбрасывать его на уровне parser
- отдавать row fields

### Слой C. Semantic validator

Файлы:

- `validator.rs`
- возможно отдельный `semantic_validator.rs`

Обязанности:

- проверять metadata rules
- сверять `dec` с реальными params/row fields
- проверять, что `tenant_id` существует и идёт первым
- проверять согласованность `kind` и query shape

---

## 6. Как именно должно измениться поведение

### Было

`sql_parser.rs` сам извлекает:

- `sql_params`
- `sql_columns`

и validator опирается на них.

### Должно стать

`sql_parser.rs` больше не извлекает:

- `sql_params`
- `sql_columns`

validator получает эти сведения из `clorinde_parser`.

### Следствие

После доработки harmless SQL-детали не должны ломать parser:

- `/* block comments */`
- сложные `SELECT`
- CTE
- подзапросы

Пока SQL block корректно размечен через `--!` и `--@`, parser должен устойчиво отработать.

---

## 7. Требования к тестированию

Доработка должна вестись через test-first подход.

Правило:

1. сначала добавляется падающий тест
2. затем минимальная реализация
3. затем рефакторинг при зелёных тестах

### 7.1 Unit tests для block parser

Файл:

- [`crates/repo_gen/src/sql_parser.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/sql_parser.rs#L1) или отдельный `tests/sql_parser.rs`

Нужно покрыть:

1. один файл с несколькими query blocks
2. `--! query_name : (...)` корректно режется до `query_name`
3. metadata парсится в правильные ключи
4. порядок `--@` строк не влияет на результат
5. пустые строки между metadata и SQL допустимы
6. block comments внутри SQL body не ломают parser
7. строковые литералы с `:` не ломают parser, если parser больше не вытаскивает params

### 7.2 Unit tests для metadata parser

Нужно покрыть:

1. неизвестный key -> ошибка
2. пустой обязательный key -> ошибка
3. `dec` корректно режется по запятым
4. duplicate keys дают понятную ошибку

`duplicate keys` сейчас не валидируются отдельно. Это нужно добавить.

### 7.3 Unit tests для clorinde parser

Файл:

- [`crates/repo_gen/src/clorinde_parser.rs`](/home/raa/RustProjects/erp/crates/repo_gen/src/clorinde_parser.rs#L1)

Нужно покрыть:

1. bind signature извлекается в правильном порядке
2. `tenant_id` сохраняется в `all_bind_params`
3. `bind_params_without_tenant` действительно исключает только `tenant_id`
4. read-query row fields извлекаются корректно
5. exec-query без row struct обрабатывается корректно

### 7.4 Semantic validation tests

Нужно покрыть:

1. `tenant_id` отсутствует в `bind` -> ошибка
2. `tenant_id` не первый -> ошибка
3. `dec` ссылается на несуществующий bind param -> ошибка
4. `dec` ссылается на несуществующее row field -> ошибка
5. `dto` запрещён для `exec`
6. `dto` обязателен для read-query
7. `input` разрешён только для `exec`

### 7.5 Golden / fixture tests

Нужны integration-like tests для реальных fixture-ов.

Предлагаемая структура:

```text
crates/repo_gen/tests/fixtures/
  warehouse/
    queries/
    clorinde/
    expected/
```

Golden tests должны проверять:

1. parse + validate на реальных SQL fixture
2. resolved model на реальном fixture
3. generated code на реальном fixture

Минимум 2 fixture-набора:

- `simple_lookup`
- `decimal_write_and_read`

### 7.6 Regression tests на текущие BC

Нужен тест уровня "реальная схема проекта":

1. взять текущие `queries/warehouse/*.sql`
2. взять текущий `queries/catalog/*.sql`
3. взять текущий `crates/clorinde-gen/src/queries/...`
4. прогнать `repo-gen` в temp dir
5. убедиться, что generation завершается успешно

Это не заменяет `just test`, но даёт быстрый regression guard именно на parser/generator contract.

---

## 8. Последовательность реализации

### Этап 1. Зафиксировать текущие дефекты тестами

Сделать падающие тесты на:

- duplicate metadata keys
- block comments внутри SQL body
- разделение query blocks без анализа SQL содержимого
- наличие `tenant_id` и его порядок на основании `clorinde`, а не SQL parser

### Этап 2. Упростить `sql_parser.rs`

Сделать:

- убрать `extract_params`
- убрать `extract_select_columns`
- добавить `sql_body` в `QueryBlock`
- оставить только block + metadata parsing

### Этап 3. Расширить `clorinde_parser.rs`

Сделать:

- извлекать полный bind list, включая `tenant_id`
- явно отдавать query shape для validator/model

### Этап 4. Перенести semantic validation

Сделать:

- `validator.rs` разделить на syntax и semantic части
- `dec` проверять через реальные поля из `clorinde`
- `tenant_id` порядок проверять через bind signature из `clorinde`

### Этап 5. Добавить fixture tests

Сделать:

- fixtures
- golden tests
- regression test на текущие BC query sets

---

## 9. Требования к качеству ошибок

Ошибки parser/validator должны быть пригодны для LLM и человека.

Формат ошибки должен содержать:

- BC
- файл
- query name
- краткое описание проблемы
- ожидаемое правило

Пример:

```text
warehouse/inventory.sql :: insert_movement
metadata error: duplicate key 'dec'
expected: each metadata key may appear at most once
```

Или:

```text
warehouse/balances.sql :: get_balance
semantic error: dec field 'balance' not found in clorinde row fields
row fields: item_id, sku
```

---

## 10. Критерии приёмки

### Функциональные

1. `sql_parser.rs` больше не анализирует SQL shape
2. query shape берётся из `clorinde_parser`
3. validator корректно работает на объединённой модели

### Тестовые

1. unit tests покрывают block parser, metadata parser, clorinde parser и semantic validator
2. fixture / golden tests присутствуют
3. regression test на текущие BC query sets присутствует
4. `cargo test -p repo_gen` проходит

### Интеграционные

1. `cargo run -p repo_gen -- --all` проходит на текущем проекте
2. `cargo check --workspace` проходит
3. `just test` проходит

---

## 11. Явные решения

В рамках этой доработки фиксируются следующие решения:

- полноценный SQL parser не внедряется
- inline metadata остаётся
- parser упрощается до block parser + metadata parser
- query shape читается из `clorinde-gen`
- доработка выполняется через test-first workflow

