# UI Architecture: Metadata-Driven ERP, Rust-First

**Дата**: 2026-04-02
**Статус**: Working draft
**Источник анализа**: `docs/UI/ui-requirements-analysis.md`

---

## 1. Позиция

`ui-requirements-analysis.md` нужен как источник требований, а не как образец для копирования.
Desktop MERP показывает, какие сценарии и interaction density реально нужны бизнесу, но не диктует будущую web-архитектуру.

Целевая позиция:

- новый UI строится не как набор экранов "по образу и подобию" легаси
- UI описывается метаданными, а не вручную собранными view-модулями
- бизнес-логика и UI-логика доступности/поведения живут в одном серверном слое
- стартовый стек должен оставаться Rust-only настолько долго, насколько это практически возможно
- внешние библиотеки подключаются только под конкретный дефицит базового решения

---

## 2. Что считаем требованиями, а что нет

### 2.1. Реальные требования

Из анализа легаси в целевую систему переходят не контролы, а классы задач:

- плотная работа со списками и табличными данными
- карточки документов с валидацией и вложенными строками
- tree/list навигация по иерархиям
- lookup/select сценарии
- workflow/status transitions
- role-aware and state-aware availability of actions
- print/export и file attachments
- audit/history
- batch operations
- keyboard-heavy сценарии для части пользователей

### 2.2. Легаси-артефакты, которые не являются целью

Это не требования, а историческая форма реализации:

- MDI как MFC-конструкция
- ribbon как обязательный паттерн
- docking panels
- status bar с техметриками БД для всех пользователей
- modal-heavy UX
- специализированные диалоги там, где хватит одного общего паттерна
- client-side вычисление доступности действий

---

## 3. Архитектурные принципы

### 3.1. Metadata First

Каждый экран задается не UI-кодом, а метаданными:

- какая это рабочая область
- какие данные отображаются
- какие секции есть на экране
- какие действия доступны
- какие состояния и ограничения применяются

UI-компоненты не знают бизнес-смысл экрана. Они получают метаданные и рендерят их.

### 3.2. Server Is The Single Source Of Truth

Сервер в Rust определяет:

- видимость секций и полей
- доступность действий
- read-only/editable состояние
- workflow transitions
- side effects после действий
- что и как надо перезагрузить после команды

Клиент не решает "можно или нельзя". Он только рендерит уже разрешенное состояние.

### 3.3. UI Logic In One Place

Нужен единый слой, который вычисляет UI-состояние. Не handler, не шаблон, не grid-адаптер.

Этот слой отвечает за:

- resolution метаданных экрана под пользователя и tenant
- evaluation permissions + entity state + selection state
- выдачу action set
- выдачу field state
- формирование UI effects

Рабочее название: `ui_runtime`.

### 3.4. Rust-First, Staged Adoption

Базовая архитектура должна работать на Rust без обязательного frontend framework.

Порядок принятия решений:

1. Сначала Rust server rendering и metadata runtime.
2. Затем минимальные web-enhancements, если они реально уменьшают сложность.
3. Затем точечное подключение JS-библиотек для тяжелых примитивов.
4. Никаких SPA/state-manager решений "на вырост".

### 3.5. Primitive Reduction

Каталог легаси-компонентов нельзя переносить 1:1. Он должен быть сжат до минимального набора универсальных примитивов.

---

## 4. Минимальный набор UI-примитивов

Все найденные в анализе компоненты сводятся к 6 примитивам.

| Примитив | Назначение | Покрывает |
|---|---|---|
| `Workspace` | shell, навигация, вкладки/маршруты, контекст пользователя | shell, sidebar, breadcrumbs, task badges |
| `Collection` | отображение и работа с наборами данных | grids, tree-grid, history, permission matrix, task inbox, reports |
| `Record` | работа с одной сущностью или документом | cards, forms, tabs, filter forms, settings forms |
| `Selector` | выбор связанной сущности | lookup, picker modal, async search |
| `ActionSurface` | запуск пользовательских действий | toolbar, row actions, workflow buttons, batch actions, create-from |
| `Feedback` | ответы системы пользователю | validation, confirmation, toast, progress, empty/error states |

### 4.1. Что не является примитивом

Это композиции или отдельные сервисы:

- master-detail = `Collection + Record`
- tree-grid screen = `Collection` в режиме hierarchy
- multi-tab document = `Record` с несколькими секциями
- workflow = не компонент, а серверная модель состояний + `ActionSurface`
- attachments = `Record + Collection`
- import wizard = `Record + Collection + Feedback`
- print/export = серверный output service
- real-time monitoring = отдельный модуль, не ядро UI
- gantt = отдельный специализированный адаптер, не базовый primitive

### 4.2. Почему именно 6

Это нижняя граница, после которой начинается искусственная компрессия.

Если объединить еще сильнее:

- `Selector` теряется внутри `Record`, хотя у него отдельная нагрузка и reuse
- `ActionSurface` начинает размазываться между toolbar, row actions и workflow
- `Feedback` перестает быть системным контрактом

---

## 5. Центральный слой UI-логики

### 5.1. Новые crate'ы

Предлагаемая структура:

```text
crates/
  ui_meta/
    src/
      screen.rs
      workspace.rs
      collection.rs
      record.rs
      selector.rs
      actions.rs
      feedback.rs
      layout.rs
      lib.rs

  ui_runtime/
    src/
      registry.rs
      resolve.rs
      policies.rs
      actions.rs
      effects.rs
      conditions.rs
      state.rs
      lib.rs
```

### 5.2. Ответственность `ui_meta`

`ui_meta` хранит декларативное описание:

- экранов
- секций
- колонок
- полей
- действий
- navigation items
- layout patterns

Это статическое описание, принадлежащее BC.

### 5.3. Ответственность `ui_runtime`

`ui_runtime` превращает декларативное описание в resolved UI state:

- фильтрует экран по permission
- вычисляет hidden/visible/read-only/enabled
- подставляет state-dependent actions
- формирует UI effects после команд
- приводит все к единому transport format

### 5.4. Контракт для действий

Каждое пользовательское действие должно проходить через один и тот же путь:

1. пользователь инициирует действие
2. `ui_runtime` определяет, доступно ли оно в текущем состоянии
3. backend command/query выполняется
4. `ui_runtime` формирует `UiEffect`
5. клиент применяет эффект без собственной бизнес-логики

Примеры `UiEffect`:

- `RefreshSection("receipts_grid")`
- `RefreshRecord("receipt_card")`
- `NavigateTo("warehouse.receipts/{id}")`
- `OpenDialog("confirm_execute")`
- `ShowToast(success, "...")`
- `Download(url)`

### 5.5. Ключевое правило

Условия вида:

- "кнопка доступна только в draft"
- "поле read-only после execute"
- "строка доступна для batch close только если status in (...)"
- "секция видна только admin"

не должны вычисляться в шаблонах, JS или grid callbacks.

Они вычисляются только в `ui_runtime`.

---

## 6. Модель метаданных

Ниже не финальный API, а обязательная форма модели.

```rust
pub struct ScreenMeta {
    pub id: ScreenId,
    pub title: String,
    pub access: AccessRule,
    pub workspace: WorkspaceMeta,
    pub sections: Vec<SectionMeta>,
    pub actions: Vec<ActionMeta>,
}

pub enum SectionMeta {
    Collection(CollectionMeta),
    Record(RecordMeta),
    SelectorHost(SelectorMeta),
    FeedbackHost(FeedbackMeta),
}

pub struct ActionMeta {
    pub id: String,
    pub label: String,
    pub command: UiCommand,
    pub visibility: VisibilityRule,
    pub availability: AvailabilityRule,
    pub confirm: Option<ConfirmMeta>,
}
```

### 6.1. На клиент нельзя отдавать "сырые" условия

Клиент не должен интерпретировать AST правил.

Он должен получать уже resolved результат:

```rust
pub struct ResolvedScreen {
    pub id: String,
    pub title: String,
    pub sections: Vec<ResolvedSection>,
    pub actions: Vec<ResolvedAction>,
}

pub struct ResolvedAction {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub hidden: bool,
    pub confirm: Option<ConfirmMeta>,
}
```

### 6.2. Data и UI state разделяются

Нужно разделить:

- domain/application data
- UI metadata
- resolved UI state

Пример:

- `receipt.status = "draft"` это domain data
- `execute action available in draft` это metadata rule
- `button.execute.enabled = true` это resolved UI state

---

## 7. Целевая серверная схема рендеринга

### 7.1. Базовый путь

На старте:

- `axum`
- `askama`
- server-rendered HTML
- обычные Rust handlers

То есть:

- маршрут получает user context
- `ui_runtime` resolve'ит экран
- Askama рендерит HTML на основе resolved model

### 7.2. Что это дает

- один язык
- одна типовая модель
- отсутствие дублирования Rust <-> TS схем
- отсутствие client-side state manager
- отсутствие разъезда между backend permissions и frontend conditions

---

## 8. Рекомендуемый стек по этапам

### 8.1. Этап A: обязательный стартовый стек

Это то, с чего нужно начинать:

| Слой | Технология | Статус |
|---|---|---|
| HTTP / routing | `axum` | уже есть |
| HTML templates | `askama` | уже есть |
| UI metadata | новый crate `ui_meta` | добавить |
| UI logic runtime | новый crate `ui_runtime` | добавить |
| CSS | обычный CSS + design tokens | добавить |
| Client script | отсутствует или минимальный hand-written JS | по необходимости |

Решение:

- без Vue
- без React
- без SPA router
- без state manager
- без обязательного Node toolchain

### 8.2. Этап B: первый допустимый не-Rust инструмент

Если частичные обновления HTML становятся дорогими по количеству шаблонов и обработчиков, первым кандидатом является `HTMX`.

Почему он допустим:

- не меняет архитектурный принцип server-driven UI
- не уводит логику на клиент
- работает поверх Askama
- позволяет внедрять частично, а не делать переписывание

Почему не обязателен на старте:

- сначала нужно стабилизировать metadata model и `ui_runtime`
- premature HTMX adoption не решает проблему архитектуры, только транспорт

### 8.3. Этап C: тяжелый primitive для Collection

Если пилотный `Collection` показывает, что обычные таблицы не тянут:

- inline editing
- virtual scroll
- большие объемы строк
- keyboard-first editing
- tree-grid
- column pinning/configuration

тогда подключается отдельный grid adapter.

Рекомендуемый кандидат:

- `AG Grid Community` как изолированный адаптер только для `Collection`

Правило:

- grid library не становится "новым frontend framework"
- она обслуживает только один primitive
- вся доступность действий и editability по-прежнему приходит с сервера

### 8.4. Этап D: узкоспециализированные библиотеки

Добавляются только при попадании модуля в scope:

| Задача | Когда подключать |
|---|---|
| Gantt | только при реальном проектном/TOIR модуле |
| WebSocket/SSE | когда появятся real-time inbox/monitoring |
| PDF/Excel libs | когда пойдут регламентные формы и экспорт |
| Barcode/mobile scan | только для мобильного/складского контура |

### 8.5. Что НЕ фиксируем сейчас

Пока не фиксируем:

- Vue / React / Svelte
- Alpine.js
- Tailwind
- full SPA routing
- frontend build pipeline на Node
- client-side validation engine как главный источник truth

Все это можно добавить позже, но только если базовая архитектура реально упирается в ограничения.

---

## 9. Отображение требований на примитивы

| Из анализа | Во что сводим |
|---|---|
| DataGrid, TreeGrid, Permission Matrix, Audit History, Task List | `Collection` |
| CRUD Form, Multi-tab Form, Filter Panel, Settings | `Record` |
| Lookup Combo, Selection Dialog | `Selector` |
| Toolbar, Workflow buttons, Batch actions, Create-from | `ActionSurface` |
| Toast, Confirm, Validation summary, Progress | `Feedback` |
| Shell, Sidebar, Tabs, Breadcrumbs, User context | `Workspace` |

### 9.1. Критические драйверы архитектуры

Из всего анализа на архитектуру по-настоящему давят три вещи:

1. `Collection`
2. `Record`
3. `ActionSurface` + server-side policy resolution

Именно они определяют стек.

### 9.2. Что можно отложить

Не надо выбирать стек под эти вещи в первой итерации:

- gantt
- monitoring
- barcode
- advanced real-time dashboards
- elaborate docking-like layout

---

## 10. Потоки данных

### 10.1. Загрузка экрана

```text
1. Пользователь открывает экран
2. Gateway собирает user context + tenant context
3. ui_runtime.resolve(screen_id, ctx)
4. runtime возвращает ResolvedScreen
5. Askama рендерит экран
6. Клиент показывает уже рассчитанное состояние
```

### 10.2. Выполнение действия

```text
1. Пользователь жмет действие
2. Сервер повторно проверяет availability + permission
3. Выполняется command/query
4. ui_runtime формирует UiEffect[]
5. Клиент применяет эффекты
```

### 10.3. Изменение состояния записи

```text
1. Изменилась запись
2. Сервер обновляет domain state
3. ui_runtime пересчитывает action set и field state
4. UI получает новое resolved состояние
```

---

## 11. Практическая рекомендация по первой итерации

### 11.1. Что делать сейчас

Первая итерация не должна пытаться закрыть весь анализ легаси.

Нужно сделать:

1. `ui_meta`
2. `ui_runtime`
3. один `Workspace`
4. один простой `Collection`
5. один простой `Record`
6. один `Selector`
7. один `ActionSurface`
8. один `Feedback` flow

### 11.2. На каком сценарии проверять

Лучший пилот:

- identity/users
- затем один document-oriented экран склада

Почему:

- уже есть shell
- уже есть auth/roles
- можно проверить metadata resolution
- можно проверить field/button visibility
- можно проверить переход от простой таблицы к плотному collection screen

### 11.3. Что будет критерием успеха

Архитектура годится, если:

- новый экран описывается метаданными BC, а не уникальным UI-кодом
- availability действий не размазана по шаблонам и JS
- добавление поля/колонки/действия не требует переписывать клиентскую логику
- переход на более тяжелый grid-адаптер не ломает модель `Collection`

---

## 12. Roadmap

### Phase 0

- выделить `ui_meta`
- выделить `ui_runtime`
- определить transport format для resolved screen

### Phase 1

- перевести shell/navigation на metadata registry
- описать identity screens через metadata

### Phase 2

- сделать простой `Collection` без тяжелой JS-библиотеки
- проверить фильтры, row actions, role/state resolution

### Phase 3

- если пилот уперся в возможности HTML table, подключить grid adapter
- ограничить его область primitive `Collection`

### Phase 4

- сделать первый document screen
- проверить workflow/status transitions
- проверить attachments/print как внешние сервисы

### Phase 5

- расширять BC coverage
- добавлять специализированные адаптеры только при доказанной необходимости

---

## 13. Итоговое решение

Целевая архитектура:

- metadata-driven UI
- server-resolved state
- единый Rust-слой `ui_runtime` для UI-логики
- минимальный набор из 6 примитивов
- Rust-first stack с поэтапным подключением внешних библиотек

Итоговый базовый стек:

- `Rust`
- `axum`
- `askama`
- `ui_meta`
- `ui_runtime`
- обычный CSS

Первый кандидат на расширение:

- `HTMX`, если нужны server-driven partial updates

Второй кандидат на расширение:

- `AG Grid Community` или аналог, если primitive `Collection` реально требует enterprise-grade grid

Главный архитектурный запрет:

- не переносить UI-логику доступности, workflow и side effects в клиент.

