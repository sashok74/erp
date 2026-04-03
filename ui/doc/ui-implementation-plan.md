# Этапы реализации: UI Engine для ERP

**Дата**: 2026-04-03
**Backend**: `c:\RustProjects\erp` — Rust modular monolith (axum, CQRS, RBAC, RLS)
**UI**: `c:\RustProjects\erp-ui` — web-рендерер + документация

---

## 1. Что строим

**Движок**, который по метаданным экрана + данным из API + разрешённому поведению рисует UI для любого BC бэкенда. Не набор экранов, а рендерер — добавление нового экрана = добавление метаданных в BC, а не написание UI-кода.

**Результат**: стек + движок, при котором:
- новый экран = конфигурация в Rust (метаданные в BC)
- поведение формы = единый серверный runtime
- web/mobile/desktop рисуют одинаково, потому что потребляют один протокол

---

## 2. Полный стек

### 2.1. Backend (дополнения к существующему)

```
crates/
  erp (existing workspace)
  ├── kernel           ← уже есть
  ├── auth             ← уже есть (PermissionManifest, RBAC)
  ├── runtime          ← уже есть (CommandPipeline, QueryPipeline)
  ├── bc_http          ← уже есть (BcRouter)
  ├── gateway          ← уже есть
  ├── warehouse        ← уже есть (BC)
  ├── catalog          ← уже есть (BC)
  │
  ├── ui_meta          ← НОВЫЙ: типы метаданных (ScreenDef, FieldDef, ActionDef...)
  ├── ui_runtime       ← НОВЫЙ: resolution (meta + permissions + state → BehaviorState)
  └── ui_gateway       ← НОВЫЙ: HTTP endpoints для UI-протокола (/ui/*)
```

| Crate | Зависит от | Назначение |
|---|---|---|
| `ui_meta` | `kernel` | Типы: ScreenDef, SectionDef, FieldDef, ColumnDef, ActionDef, NavDef |
| `ui_runtime` | `ui_meta`, `kernel`, `auth` | Resolution: метаданные + RequestContext → ResolvedScreen |
| `ui_gateway` | `ui_runtime`, `gateway` | HTTP: `/ui/nav`, `/ui/screen/{id}`, `/ui/action` |

### 2.2. Frontend (новый проект в erp-ui)

| Слой | Технология | Назначение |
|---|---|---|
| Язык | TypeScript 5.x | Типизация JSON-протокола |
| Сборка | Vite | Dev server + production build |
| Grid | AG Grid | Collection primitive (inline editing, virtual scroll, keyboard) |
| CSS | Tailwind CSS | Утилитарные классы, design tokens |
| Рендеринг | Vanilla TS | Движок: метаданные → DOM. Framework = сам движок. |
| Push | WebSocket | Уведомления, real-time state updates |
| Иконки | Lucide | Lightweight icon set |

### 2.3. Протокол (JSON)

| Контракт | Направление | Содержимое |
|---|---|---|
| `ScreenDescriptor` | server → client | Структура экрана: секции, поля, колонки, действия, layout |
| `DataPayload` | server → client | Данные: запись, строки, справочники |
| `BehaviorState` | server → client | Состояние: editable/readonly/hidden, enabled/disabled, валидации |
| `UiEvent` | client → server | Действие пользователя: field_changed, action_invoked, row_added |
| `UiEffect` | server → client | Реакция: refresh, toast, navigate, open_dialog, close_form |

---

## 3. Backend: новые crate'ы

### 3.1. ui_meta — описание экранов

Каждый BC описывает свои экраны, так же как описывает свои permissions через `PermissionManifest`. Паттерн тот же — BC владеет метаданными.

```rust
// crates/ui_meta/src/lib.rs — типы

pub struct ScreenDef {
    pub id: ScreenId,                    // "warehouse.receipt_list"
    pub title: String,                   // "Приходные ордера"
    pub kind: ScreenKind,                // List | Card | Tree
    pub sections: Vec<SectionDef>,       // Секции экрана
    pub actions: Vec<ActionDef>,         // Действия (toolbar/ribbon)
    pub data_source: DataSourceDef,      // Какой query/endpoint загружает данные
}

pub enum SectionDef {
    Record(RecordDef),                   // Форма с полями
    Collection(CollectionDef),           // Грид/таблица
    Tabs(Vec<TabDef>),                   // Вкладки внутри формы
}

pub struct FieldDef {
    pub id: String,                      // "date"
    pub label: String,                   // "Дата"
    pub field_type: FieldType,           // Text, Number, Date, Bool, Enum, Lookup
    pub lookup: Option<LookupDef>,       // Для Lookup полей: entity + display field
    pub computed: Option<String>,        // Формула: "quantity * price"
    pub default_value: Option<Value>,
}

pub struct ActionDef {
    pub id: String,                      // "execute"
    pub label: String,                   // "Провести"
    pub command: String,                 // "warehouse.execute_receipt" → CommandPipeline
    pub icon: Option<String>,            // "check"
    pub hotkey: Option<String>,          // "Ctrl+Enter"
    pub confirm: Option<String>,         // "Провести документ?"
    pub position: ActionPosition,        // Toolbar | RowAction | ContextMenu
}
```

```rust
// crates/warehouse/src/ui.rs — BC регистрирует свои экраны

pub fn ui_manifest() -> UiManifest {
    UiManifest {
        bc: "warehouse",
        navigation: vec![
            NavGroup {
                id: "warehouse",
                label: "Склад",
                icon: "package",
                items: vec![
                    NavItem::screen("receipt_list", "Приходные ордера"),
                    NavItem::screen("balance_report", "Остатки"),
                ],
            },
        ],
        screens: vec![
            receipt_list_screen(),
            receipt_card_screen(),
            balance_report_screen(),
        ],
    }
}
```

### 3.2. ui_runtime — resolution

Превращает статические метаданные + живые данные + контекст пользователя в разрешённое состояние.

```rust
// crates/ui_runtime/src/resolve.rs

pub struct ScreenResolver {
    meta_registry: Arc<MetaRegistry>,        // Все зарегистрированные экраны
    permission_registry: Arc<PermissionRegistry>,  // Из auth crate
}

impl ScreenResolver {
    /// Отдаёт ScreenDescriptor + BehaviorState для экрана
    pub fn resolve(
        &self,
        screen_id: &ScreenId,
        ctx: &RequestContext,        // tenant, user, roles
        entity_state: Option<&Value>, // текущие данные записи (для card-экранов)
    ) -> Result<ResolvedScreen, AppError> {
        let screen = self.meta_registry.get(screen_id)?;

        // 1. Фильтруем секции и поля по permissions
        // 2. Вычисляем editable/readonly/hidden для каждого поля
        // 3. Вычисляем enabled/disabled для каждого действия
        // 4. Учитываем entity state (status → что доступно)
        // 5. Возвращаем resolved контракт
    }
}

pub struct ResolvedScreen {
    pub descriptor: ScreenDescriptor,   // Структура (JSON контракт #1)
    pub behavior: BehaviorState,        // Состояние (JSON контракт #3)
}
```

**Behavior rules** — декларативные, живут в метаданных:

```rust
pub struct BehaviorRule {
    pub target: String,              // "fields.date" | "actions.execute"
    pub property: Property,          // Editable | Visible | Enabled | Required
    pub condition: Condition,        // EntityState("status", "draft") & HasPermission("warehouse.execute")
}

pub enum Condition {
    Always,
    Never,
    EntityState(String, Vec<String>),       // field = one of values
    HasPermission(String),                  // user has permission
    HasData(String),                        // field is not empty
    Expression(String),                     // "lines.len() > 0"
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}
```

### 3.3. Интеграция с существующим backend

```
Существующий паттерн BC              Дополнение для UI
─────────────────────────            ─────────────────────
registrar.rs → PermissionManifest    ui.rs → UiManifest
              ↓                              ↓
         PermissionRegistry              MetaRegistry
              ↓                              ↓
         JwtPermissionChecker            ScreenResolver
              ↓                              ↓
         CommandPipeline                 /ui/* endpoints
```

Регистрация в `AppBuilder`:

```rust
// crates/gateway/src/app_builder.rs — дополнение

impl AppBuilder {
    pub async fn register<M: BoundedContextModule>(
        &mut self,
        module: &M,
        routes_fn: fn(...) -> Router,
        ui_manifest_fn: fn() -> UiManifest,  // ← НОВОЕ
    ) {
        // ... existing: migrations, handlers, routes ...
        self.meta_registry.register(ui_manifest_fn());  // ← НОВОЕ
    }
}
```

Новые endpoints в gateway:

```
GET  /ui/nav                    → NavigationDescriptor (sidebar)
GET  /ui/screen/{id}            → ResolvedScreen (descriptor + behavior)
GET  /ui/screen/{id}/data       → DataPayload (данные)
POST /ui/screen/{id}/action     → UiEffect[] (результат действия)
POST /ui/screen/{id}/event      → BehaviorState (пересчёт после изменения)
WS   /ui/ws                     → Push: notifications, state updates
```

---

## 4. Frontend: web-рендерер

### 4.1. Структура проекта

```
erp-ui/
  doc/                          ← документация (уже есть)
  src/
    engine/
      renderer.ts               ← ScreenDescriptor → DOM
      behavior.ts               ← BehaviorState → enable/disable/hide
      data-binder.ts            ← DataPayload → заполнение полей
      event-emitter.ts          ← UiEvent → server
      effect-handler.ts         ← UiEffect → toast/navigate/refresh
    shell/
      app.ts                    ← Точка входа
      outlook-bar.ts            ← Навигация (из NavigationDescriptor)
      ribbon.ts                 ← Toolbar (из actions активной формы)
      mdi.ts                    ← Tab manager
      status-bar.ts             ← Уведомления, контекст
    primitives/
      record.ts                 ← Рендерер Record секции (форма)
      collection.ts             ← Рендерер Collection секции (AG Grid)
      selector.ts               ← Lookup/picker компонент
      feedback.ts               ← Toast, confirm, validation
    controls/
      text-field.ts             ← Стандартные ERP-контролы
      number-field.ts
      date-field.ts
      bool-field.ts
      enum-field.ts
      lookup-field.ts
    protocol/
      types.ts                  ← TypeScript-типы контрактов (из Rust через codegen)
      api.ts                    ← HTTP client
      ws.ts                     ← WebSocket client
    styles/
      tokens.css                ← Design tokens
      shell.css
      controls.css
  index.html
  vite.config.ts
  tsconfig.json
  tailwind.config.ts
```

### 4.2. Движок рендеринга (core loop)

```
┌──────────────────────────────────────────────────────┐
│                     Engine Core                       │
│                                                       │
│  1. Получить ResolvedScreen (descriptor + behavior)  │
│  2. Renderer: descriptor → DOM elements              │
│  3. DataBinder: data → заполнить поля/грид           │
│  4. BehaviorApplier: behavior → disable/hide/show    │
│  5. EventEmitter: user action → POST /ui/event       │
│  6. EffectHandler: UiEffect[] → toast/navigate/...   │
│  7. goto 4 (при изменении behavior)                  │
│                                                       │
│  Шаг 2 выполняется один раз при открытии экрана.     │
│  Шаги 4-7 — цикл жизни формы.                       │
└──────────────────────────────────────────────────────┘
```

### 4.3. Ribbon ↔ MDI

```typescript
// Когда активная вкладка меняется:
mdi.onTabChange((tab) => {
  const screen = tab.resolvedScreen;
  ribbon.render(screen.descriptor.actions, screen.behavior.actions);
});

// Когда пользователь нажимает кнопку в Ribbon:
ribbon.onAction((actionId) => {
  const tab = mdi.activeTab();
  tab.emitEvent({ type: 'action_invoked', action: actionId });
});
```

---

## 5. Этапы реализации

### Phase 0 — Протокол и типы

**Цель**: зафиксировать JSON-контракт между server и client.

**Backend** (`erp` repo):
- [ ] Crate `ui_meta`: типы `ScreenDef`, `FieldDef`, `ColumnDef`, `ActionDef`, `NavDef`
- [ ] Crate `ui_meta`: сериализация в JSON (`serde`)
- [ ] Определить `BehaviorRule` и `Condition` enum
- [ ] Определить `UiEffect` enum (Refresh, Toast, Navigate, CloseForm, OpenDialog)

**Frontend** (`erp-ui` repo):
- [ ] `protocol/types.ts` — TypeScript-зеркало Rust-типов
- [ ] Валидация: один и тот же пример JSON парсится и в Rust, и в TS

**Результат**: два crate'а понимают одинаковый JSON. Ни одной строки UI пока нет.

---

### Phase 1 — UI Runtime (server)

**Цель**: сервер умеет отдавать resolved экраны.

**Backend**:
- [ ] Crate `ui_runtime`: `MetaRegistry` — хранит все зарегистрированные экраны
- [ ] Crate `ui_runtime`: `ScreenResolver` — resolve(screen_id, ctx, entity_state) → ResolvedScreen
- [ ] Интеграция с `PermissionRegistry` из `auth`
- [ ] Evaluation `Condition` дерева (entity state + permissions → bool)
- [ ] Gateway endpoints: `GET /ui/nav`, `GET /ui/screen/{id}`
- [ ] Warehouse BC: `ui.rs` — описать 2-3 экрана в метаданных (receipt list, receipt card)
- [ ] Catalog BC: `ui.rs` — описать product list, product card

**Результат**: `curl /ui/screen/warehouse.receipt_card` возвращает JSON с полной структурой экрана + разрешённым поведением.

---

### Phase 2 — Web Shell

**Цель**: пустой shell с навигацией и вкладками.

**Frontend**:
- [ ] Проект: Vite + TypeScript + Tailwind
- [ ] Auth: login форма → JWT → хранение в memory
- [ ] API client: `fetch` wrapper с JWT + tenant headers
- [ ] Shell layout: OutlookBar (left) + Ribbon (top) + MDI area (center) + StatusBar (bottom)
- [ ] OutlookBar: загрузка `GET /ui/nav` → рендер навигации
- [ ] MDI: tab manager — открыть/закрыть/переключить вкладку
- [ ] Ribbon: пустой, но рендерится из данных (подготовка к Phase 5)
- [ ] Routing: URL ↔ open tabs (`/screen/warehouse.receipt_list`)

**Результат**: работающий shell, по клику в sidebar открывается пустая вкладка.

---

### Phase 3 — Record (формы)

**Цель**: рендерер Record-секций — динамические формы из метаданных.

**Frontend**:
- [ ] `engine/renderer.ts` — принимает `ScreenDescriptor`, создаёт DOM
- [ ] Controls: `TextField`, `NumberField`, `DateField`, `BoolField`, `EnumField`
- [ ] Control factory: `FieldDef.field_type` → создать нужный контрол
- [ ] Layout: секции формы (grid layout, 1/2/3 колонки)
- [ ] `engine/data-binder.ts` — `DataPayload` → заполнить значения полей
- [ ] `engine/behavior.ts` — `BehaviorState` → disable/readonly/hide поля
- [ ] Dirty tracking: отслеживание изменений
- [ ] Submit: собрать данные → `POST /ui/screen/{id}/action` (save)

**Backend**:
- [ ] Endpoint `GET /ui/screen/{id}/data` — загрузка данных записи
- [ ] Endpoint `POST /ui/screen/{id}/action` — выполнение действия (маппинг на CommandPipeline)
- [ ] Возврат `UiEffect[]` после команды

**Результат**: открытие карточки продукта из каталога. Поля заполнены, можно редактировать и сохранить.

---

### Phase 4 — Collection (гриды)

**Цель**: рендерер Collection-секций — AG Grid из метаданных.

**Frontend**:
- [ ] AG Grid интеграция: `CollectionDef` → AG Grid column defs
- [ ] Маппинг типов: `FieldType` → AG Grid cell renderers/editors
- [ ] Загрузка данных: `DataPayload.rows` → AG Grid rowData
- [ ] Серверная пагинация: Server-Side Row Model (для больших списков)
- [ ] Row selection (single / multi)
- [ ] Row actions (context menu)
- [ ] Навигация: double-click на строке → открыть Card экран в новой вкладке

**Backend**:
- [ ] Endpoint `GET /ui/screen/{id}/data` с пагинацией, сортировкой, фильтрами
- [ ] Query параметры: `?page=1&size=50&sort=date:desc&filter=status:draft`

**Результат**: список приходных ордеров. Пагинация, сортировка, double-click → открытие карточки.

---

### Phase 5 — ActionBar + Feedback + Behavior

**Цель**: замкнуть цикл поведения — действия, обратная связь, пересчёт состояния.

**Frontend**:
- [ ] Ribbon рендерит `actions[]` из активной вкладки
- [ ] Ribbon применяет `behavior.actions` — enabled/disabled/hidden
- [ ] Hotkeys: привязка `ActionDef.hotkey` → keyboard listener
- [ ] Toast система (Feedback primitive): success/error/warning
- [ ] Confirmation dialog: `ActionDef.confirm` → модалка перед выполнением
- [ ] Validation display: ошибки из `BehaviorState.validations` → подсветка полей
- [ ] `engine/effect-handler.ts` — обработка `UiEffect[]`:
  - `Refresh` → перезагрузить данные
  - `Toast(msg)` → показать уведомление
  - `Navigate(screen_id)` → открыть новую вкладку
  - `CloseForm` → закрыть текущую вкладку
  - `UpdateBehavior(state)` → применить новое состояние

**Backend**:
- [ ] Endpoint `POST /ui/screen/{id}/event` — пересчёт BehaviorState при изменении поля
- [ ] Формирование `UiEffect[]` после выполнения команды

**Результат**: полный цикл — открыть карточку → редактировать → нажать "Провести" в Ribbon → получить toast → форма стала readonly.

---

### Phase 6 — Selector (lookup)

**Цель**: компонент выбора связанной сущности.

**Frontend**:
- [ ] `LookupField`: combo с поиском (debounced async)
- [ ] Popup selector: модальный список для сложного выбора
- [ ] `LookupDef` → какой endpoint вызывать, какое поле отображать
- [ ] Кэширование справочников (часто используемые: склады, единицы)

**Backend**:
- [ ] Generic query endpoint для справочников: `GET /ui/lookup/{entity}?q=...`
- [ ] Или: каждый BC объявляет lookup-источники в `UiManifest`

**Результат**: в карточке приходного ордера поле "Склад" — lookup с поиском по справочнику складов.

---

### Phase 7 — Inline Editing (Collection)

**Цель**: редактирование строк документа прямо в гриде.

**Frontend**:
- [ ] AG Grid: `editable` из `BehaviorState` per column
- [ ] Cell editors: text, number, date, lookup (custom AG Grid editors)
- [ ] Keyboard navigation: Tab → next cell, Enter → next row, Escape → revert
- [ ] Computed columns: формулы из `FieldDef.computed` вычисляются клиентом
- [ ] Add/remove rows
- [ ] Dirty tracking для строк грида
- [ ] Batch save: собрать изменённые строки → отправить одним запросом

**Результат**: карточка приходного ордера со строками (номенклатура, кол-во, цена, итого). Кладовщик вводит 50 строк с клавиатуры.

---

### Phase 8 — Пилот: полный цикл двух BC

**Цель**: доказать, что движок работает end-to-end.

**Catalog BC**:
- [ ] Список продуктов (Collection) — фильтрация, сортировка, пагинация
- [ ] Карточка продукта (Record) — создание, редактирование
- [ ] Действия: создать, сохранить, удалить

**Warehouse BC**:
- [ ] Список приходных ордеров (Collection)
- [ ] Карточка приходного ордера (Record + Collection строки)
- [ ] Действия: создать, сохранить, провести, отменить проведение
- [ ] Workflow: draft → posted (поля readonly после проведения)
- [ ] Lookup: выбор склада, номенклатуры

**Сквозные сценарии**:
- [ ] Login → sidebar → открыть список → double-click → карточка → редактировать → сохранить
- [ ] Создать приходный ордер → добавить строки в гриде → провести → попытаться редактировать (blocked)
- [ ] Два пользователя с разными ролями видят разный набор кнопок

**Результат**: рабочий ERP для двух модулей. Новый модуль = метаданные в BC, без нового UI-кода.

---

## 6. Критерии успеха

| Критерий | Проверка |
|---|---|
| Новый экран без UI-кода | Добавить `ScreenDef` в BC → экран появился |
| Одинаковое поведение | Один BehaviorState → web/mobile рендерят одинаково |
| Permissions работают | `warehouse_operator` не видит кнопку "Удалить", `warehouse_manager` — видит |
| Workflow работает | После "Провести" поля формы стали readonly, кнопка "Провести" исчезла |
| Grid production-ready | 1000 строк, inline editing, Tab-навигация, virtual scroll |
| Добавление BC | Новый BC регистрирует `UiManifest` → навигация + экраны автоматически |

---

## 7. Порядок зависимостей

```
Phase 0 ─── Protocol + Types ───────────────────────────────────────┐
   │                                                                │
Phase 1 ─── UI Runtime (server) ───┐                                │
   │                               │ (server готов)                 │
Phase 2 ─── Web Shell ─────────────┤                                │
   │                               │                                │
Phase 3 ─── Record (forms) ────────┤                                │
   │                               │                                │
Phase 4 ─── Collection (grids) ────┤                                │
   │                               │                                │
Phase 5 ─── ActionBar + Behavior ──┤  ← MVP: формы + гриды + кнопки│
   │                               │                                │
Phase 6 ─── Selector (lookup) ─────┤                                │
   │                               │                                │
Phase 7 ─── Inline Editing ────────┘                                │
   │                                                                │
Phase 8 ─── Pilot (Catalog + Warehouse) ────────────────────────────┘
```

Phases 2-4 можно частично параллелить (shell + формы + гриды разрабатываются одновременно после готовности Phase 1).

---

## 8. За рамками этого плана (следующие итерации)

- Tree-grid (иерархические справочники: номенклатура, подразделения)
- Concord (согласование документов)
- File attachments
- Print/Export (серверный PDF)
- Dashboard / виджеты
- WebSocket push notifications
- Audit history viewer
- Import/Export wizards
- Gantt chart
- Мобильный рендерер
- Tenant-specific UI customization (переопределение метаданных)
